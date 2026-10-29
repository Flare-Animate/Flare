//! Timeline frame operations (insert/remove frames, keyframes, blank keyframes).
//!
//! Each function turns the current layer plus a frame number into a command batch, or
//! `None` when the operation does not apply there (so callers can leave the document
//! alone). Operations rebuild the layer's span list, which keeps them easy to reason
//! about: spans stay sorted, non-overlapping, and later content shifts as a unit.

use std::num::NonZeroU32;

use flare_core::{InstanceId, SceneId, ShapeId};
use flare_document::{Content, FrameSpan, Instance, Layer, Shape, SoundCue};

use flare_commands::Command;

/// Content for a new keyframe copied from `content`: same artwork, fresh identities so
/// IDs stay unique across the document.
fn duplicate(content: &Content) -> Option<Content> {
    Some(match content {
        Content::Empty => Content::Empty,
        Content::Instance(i) => {
            Content::Instance(Instance::new(InstanceId::new(), i.symbol(), i.transform()))
        }
        Content::Shape(s) => {
            let copy = Shape::new(ShapeId::new(), s.path().to_vec(), s.fill()).ok()?;
            Content::Shape(copy.painted_like(s))
        }
    })
}

fn span(start: u32, duration: u32, content: Content, like: &FrameSpan) -> Option<FrameSpan> {
    let duration = NonZeroU32::new(duration)?;
    Some(FrameSpan::new(start, duration, content).with_tween(like.tween()))
}

/// Replaces a layer's whole span list. Removals come first, so every step is valid.
fn replace_spans(scene: SceneId, layer: &Layer, new: Vec<FrameSpan>) -> Vec<Command> {
    let removals = layer.spans().iter().map(|old| Command::RemoveSpan {
        scene,
        layer: layer.id(),
        start: old.start(),
    });
    let inserts = new.into_iter().map(|span| Command::InsertSpan {
        scene,
        layer: layer.id(),
        span,
    });
    removals.chain(inserts).collect()
}

fn shifted(span: &FrameSpan, start: u32) -> Option<FrameSpan> {
    Some(FrameSpan::new(start, span.duration(), span.content().clone()).with_tween(span.tween()))
}

/// A run of frames lifted from a layer: its spans, relative to the first frame copied.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameClip {
    length: NonZeroU32,
    spans: Vec<FrameSpan>,
}

impl FrameClip {
    /// How many frames the clip covers, gaps included.
    pub fn length(&self) -> u32 {
        self.length.get()
    }

    pub fn spans(&self) -> &[FrameSpan] {
        &self.spans
    }
}

/// Copies frames `start..=end` of `layer`. Spans the range cuts through are cut to it, and a
/// tween that loses its target by the cut is dropped. `None` when the range is empty or backwards,
/// or holds no span at all.
pub fn copy_frames(layer: &Layer, start: u32, end: u32) -> Option<FrameClip> {
    if start > end {
        return None;
    }
    let stop = u64::from(end) + 1;
    let mut spans = Vec::new();
    for s in layer.spans() {
        let (from, to) = (
            u64::from(s.start()).max(u64::from(start)),
            s.end().min(stop),
        );
        if from >= to {
            continue;
        }
        let cut_end = s.end() > stop;
        let piece = FrameSpan::new(
            u32::try_from(from - u64::from(start)).ok()?,
            NonZeroU32::new(u32::try_from(to - from).ok()?)?,
            s.content().clone(),
        );
        spans.push(if cut_end {
            piece
        } else {
            piece.with_tween(s.tween())
        });
    }
    if spans.is_empty() {
        return None;
    }
    let length = NonZeroU32::new(end.checked_sub(start)?.checked_add(1)?)?;
    Some(FrameClip { length, spans })
}

/// Pastes `clip` over the frames starting at `at`, replacing what is there and leaving the rest of
/// the layer where it is. A span the pasted range cuts through keeps its two ends, the far one
/// with fresh identities, and every pasted span gets its own so IDs stay unique. `None` when the
/// range would run past the largest frame number.
pub fn paste_frames(
    scene: SceneId,
    layer: &Layer,
    at: u32,
    clip: &FrameClip,
) -> Option<Vec<Command>> {
    let stop = u64::from(at) + u64::from(clip.length.get());
    if stop > u64::from(u32::MAX) {
        return None;
    }
    let mut out = Vec::with_capacity(layer.spans().len() + clip.spans.len() + 1);
    for s in layer.spans() {
        if s.end() <= u64::from(at) || u64::from(s.start()) >= stop {
            out.push(s.clone());
            continue;
        }
        if s.start() < at {
            out.push(span(s.start(), at - s.start(), s.content().clone(), s)?);
        }
        if s.end() > stop {
            let from = u32::try_from(stop).ok()?;
            let rest = u32::try_from(s.end() - stop).ok()?;
            out.push(span(from, rest, duplicate(s.content())?, s)?);
        }
    }
    for pasted in &clip.spans {
        out.push(
            FrameSpan::new(
                at.checked_add(pasted.start())?,
                pasted.duration(),
                duplicate(pasted.content())?,
            )
            .with_tween(pasted.tween()),
        );
    }
    out.sort_by_key(FrameSpan::start);
    Some(replace_spans(scene, layer, out))
}

/// Moves the sound cues after `frame` along with the spans they sit beside, by `delta` frames.
fn with_cue_shift(
    layer: &Layer,
    frame: u32,
    delta: i64,
    scene: SceneId,
    mut commands: Vec<Command>,
) -> Option<Vec<Command>> {
    if layer.cues().iter().any(|c| c.start > frame) {
        let cues = layer
            .cues()
            .iter()
            .map(|c| {
                let start = if c.start > frame {
                    u32::try_from(i64::from(c.start) + delta).ok()?
                } else {
                    c.start
                };
                Some(SoundCue { start, ..*c })
            })
            .collect::<Option<Vec<_>>>()?;
        commands.push(Command::SetCues {
            scene,
            layer: layer.id(),
            cues,
        });
    }
    Some(commands)
}

/// Insert Frame (F5): lengthens the span at `frame` by one and shifts later spans right.
/// Past the end of the layer it extends the last span to cover `frame`.
pub fn insert_frame(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    let spans = layer.spans();
    let mut out = Vec::with_capacity(spans.len());
    if let Some(at) = spans
        .iter()
        .position(|s| s.start() <= frame && u64::from(frame) < s.end())
    {
        for (i, s) in spans.iter().enumerate() {
            out.push(match i.cmp(&at) {
                std::cmp::Ordering::Less => s.clone(),
                std::cmp::Ordering::Equal => span(
                    s.start(),
                    s.duration().get().checked_add(1)?,
                    s.content().clone(),
                    s,
                )?,
                std::cmp::Ordering::Greater => shifted(s, s.start().checked_add(1)?)?,
            });
        }
    } else {
        let last = spans.last()?;
        if u64::from(frame) < last.end() {
            return None; // inside a gap between spans
        }
        let grown = frame.checked_add(1)?.checked_sub(last.start())?;
        out.extend(spans[..spans.len() - 1].iter().cloned());
        out.push(span(last.start(), grown, last.content().clone(), last)?);
    }
    with_cue_shift(layer, frame, 1, scene, replace_spans(scene, layer, out))
}

/// Remove Frame (Shift+F5): shortens the span at `frame` by one and shifts later spans
/// left. A one-frame span disappears.
pub fn remove_frame(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    let spans = layer.spans();
    let at = spans
        .iter()
        .position(|s| s.start() <= frame && u64::from(frame) < s.end())?;
    let mut out = Vec::with_capacity(spans.len());
    for (i, s) in spans.iter().enumerate() {
        match i.cmp(&at) {
            std::cmp::Ordering::Less => out.push(s.clone()),
            std::cmp::Ordering::Equal => {
                if s.duration().get() > 1 {
                    out.push(span(
                        s.start(),
                        s.duration().get() - 1,
                        s.content().clone(),
                        s,
                    )?);
                }
            }
            std::cmp::Ordering::Greater => out.push(shifted(s, s.start() - 1)?),
        }
    }
    with_cue_shift(layer, frame, -1, scene, replace_spans(scene, layer, out))
}

/// Splits the span at `frame` so a new keyframe starts there with `content` for the new
/// part. Past the end of the layer, the last span is held up to `frame` first.
fn split_at(scene: SceneId, layer: &Layer, frame: u32, blank: bool) -> Option<Vec<Command>> {
    let spans = layer.spans();
    let new_content = |from: &Content| {
        if blank {
            Some(Content::Empty)
        } else {
            duplicate(from)
        }
    };
    let mut out = Vec::with_capacity(spans.len() + 1);
    if let Some(at) = spans
        .iter()
        .position(|s| s.start() <= frame && u64::from(frame) < s.end())
    {
        let target = &spans[at];
        if target.start() == frame {
            return None; // already a keyframe
        }
        out.extend(spans[..at].iter().cloned());
        out.push(span(
            target.start(),
            frame - target.start(),
            target.content().clone(),
            target,
        )?);
        let rest = u32::try_from(target.end() - u64::from(frame)).ok()?;
        out.push(span(frame, rest, new_content(target.content())?, target)?);
        out.extend(spans[at + 1..].iter().cloned());
    } else {
        let last = spans.last()?;
        if u64::from(frame) < last.end() {
            return None; // inside a gap
        }
        out.extend(spans[..spans.len() - 1].iter().cloned());
        out.push(span(
            last.start(),
            frame.checked_sub(last.start())?,
            last.content().clone(),
            last,
        )?);
        out.push(span(frame, 1, new_content(last.content())?, last)?);
    }
    Some(replace_spans(scene, layer, out))
}

/// Insert Keyframe (F6): a new keyframe at `frame` holding a copy of the current content.
pub fn insert_keyframe(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    split_at(scene, layer, frame, false)
}

/// Convert to Keyframes: the span holding `frame` becomes one keyframe per frame, each a copy
/// with its own identities. `None` when no span holds `frame`, it is only one frame long, or it
/// is tweened (the in-between positions are not baked in).
pub fn convert_to_keyframes(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    let target = layer
        .spans()
        .iter()
        .find(|s| s.start() <= frame && u64::from(frame) < s.end())?;
    if target.duration().get() < 2 || target.tween() != flare_document::Tween::None {
        return None;
    }
    let mut out: Vec<FrameSpan> = layer
        .spans()
        .iter()
        .filter(|s| s.start() != target.start())
        .cloned()
        .collect();
    out.push(span(target.start(), 1, target.content().clone(), target)?);
    for offset in 1..target.duration().get() {
        out.push(span(
            target.start().checked_add(offset)?,
            1,
            duplicate(target.content())?,
            target,
        )?);
    }
    out.sort_by_key(FrameSpan::start);
    Some(replace_spans(scene, layer, out))
}

/// Insert Blank Keyframe (F7): a new keyframe at `frame` with nothing in it.
pub fn insert_blank_keyframe(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    split_at(scene, layer, frame, true)
}

/// Clear Keyframe (Shift+F6): the keyframe starting at `frame` is absorbed by the span
/// before it when adjacent, otherwise it becomes an empty span.
pub fn clear_keyframe(scene: SceneId, layer: &Layer, frame: u32) -> Option<Vec<Command>> {
    let spans = layer.spans();
    let at = spans.iter().position(|s| s.start() == frame)?;
    let mut out: Vec<FrameSpan> = spans.to_vec();
    let target = out.remove(at);
    match at.checked_sub(1).map(|p| &out[p]) {
        Some(prev) if prev.end() == u64::from(target.start()) => {
            let merged = span(
                prev.start(),
                prev.duration().get().checked_add(target.duration().get())?,
                prev.content().clone(),
                prev,
            )?;
            out[at - 1] = merged;
        }
        _ => out.insert(
            at,
            span(
                target.start(),
                target.duration().get(),
                Content::Empty,
                &target,
            )?,
        ),
    }
    Some(replace_spans(scene, layer, out))
}

/// Whether `candidate` (start, end) would overlap any span of `layer` other than the one
/// starting at `skip`.
fn collides(layer: &Layer, skip: u32, start: u32, end: u64) -> bool {
    layer
        .spans()
        .iter()
        .filter(|other| other.start() != skip)
        .any(|other| u64::from(start) < other.end() && u64::from(other.start()) < end)
}

/// Drags the span starting at `from` so it starts at `to`, keeping its length. `None` when
/// there is no such span, nothing changes, the move would overlap a neighbour or run past
/// the largest frame number.
pub fn move_span(scene: SceneId, layer: &Layer, from: u32, to: u32) -> Option<Vec<Command>> {
    if from == to {
        return None;
    }
    let old = layer.spans().iter().find(|s| s.start() == from)?;
    let end = u64::from(to) + u64::from(old.duration().get());
    if end > u64::from(u32::MAX) || collides(layer, from, to, end) {
        return None;
    }
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: layer.id(),
            start: from,
        },
        Command::InsertSpan {
            scene,
            layer: layer.id(),
            span: shifted(old, to)?,
        },
    ])
}

/// Drags the end of the span starting at `from` so it lasts `duration` frames. `None` when
/// there is no such span, the length is zero or unchanged, or it would overlap the next span.
pub fn resize_span(
    scene: SceneId,
    layer: &Layer,
    from: u32,
    duration: u32,
) -> Option<Vec<Command>> {
    let old = layer.spans().iter().find(|s| s.start() == from)?;
    if duration == 0 || duration == old.duration().get() {
        return None;
    }
    let end = u64::from(from) + u64::from(duration);
    if end > u64::from(u32::MAX) || collides(layer, from, from, end) {
        return None;
    }
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: layer.id(),
            start: from,
        },
        Command::InsertSpan {
            scene,
            layer: layer.id(),
            span: span(from, duration, old.content().clone(), old)?,
        },
    ])
}

#[cfg(test)]
mod tests;
