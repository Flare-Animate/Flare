//! Timeline frame operations (insert/remove frames, keyframes, blank keyframes).
//!
//! Each function turns the current layer plus a frame number into a command batch, or
//! `None` when the operation does not apply there (so callers can leave the document
//! alone). Operations rebuild the layer's span list, which keeps them easy to reason
//! about: spans stay sorted, non-overlapping, and later content shifts as a unit.

use std::num::NonZeroU32;

use flare_core::{InstanceId, SceneId, ShapeId};
use flare_document::{Content, FrameSpan, Instance, Layer, Shape};

use crate::Command;

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
    Some(replace_spans(scene, layer, out))
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
    Some(replace_spans(scene, layer, out))
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
#[allow(clippy::expect_used)]
mod tests {
    use flare_core::{DocumentId, FrameRate, LayerId, Rgba8};
    use flare_document::{Document, Scene, Timeline, Tween};

    use super::*;
    use crate::EditorSession;

    struct Fixture {
        session: EditorSession,
        scene: SceneId,
    }

    fn shape_span(start: u32, duration: u32) -> FrameSpan {
        let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE).expect("rect");
        FrameSpan::new(
            start,
            NonZeroU32::new(duration).expect("nonzero"),
            Content::Shape(shape),
        )
    }

    fn fixture(spans: Vec<FrameSpan>) -> Fixture {
        let (scene, layer) = (SceneId::new(), LayerId::new());
        let layer_value = Layer::new(layer, "L", spans).expect("layer");
        let scene_value = Scene::new(scene, "S", Timeline::new(vec![layer_value]));
        let rate = FrameRate::new(24, 1).expect("rate");
        let doc = Document::new(DocumentId::new(), rate, vec![scene_value], vec![]).expect("doc");
        Fixture {
            session: EditorSession::new(doc),
            scene,
        }
    }

    fn layer(f: &Fixture) -> &Layer {
        &f.session.document().scenes()[0].timeline().layers()[0]
    }

    /// (start, duration, kind) where kind is 'S' shape, 'E' empty.
    fn shape_of(f: &Fixture) -> Vec<(u32, u32, char)> {
        layer(f)
            .spans()
            .iter()
            .map(|s| {
                let kind = match s.content() {
                    Content::Empty => 'E',
                    Content::Shape(_) => 'S',
                    Content::Instance(_) => 'I',
                };
                (s.start(), s.duration().get(), kind)
            })
            .collect()
    }

    fn run(f: &mut Fixture, commands: Option<Vec<Command>>) {
        f.session
            .apply(commands.expect("operation applies"))
            .expect("valid result");
    }

    #[test]
    fn insert_frame_lengthens_the_span_and_shifts_later_spans() {
        let mut f = fixture(vec![shape_span(0, 5), shape_span(5, 3)]);
        let cmds = insert_frame(f.scene, layer(&f), 2);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 3, 'S')]);
        f.session.undo();
        assert_eq!(
            shape_of(&f),
            vec![(0, 5, 'S'), (5, 3, 'S')],
            "one undo step"
        );
    }

    #[test]
    fn insert_frame_past_the_end_extends_and_gaps_are_left_alone() {
        let mut f = fixture(vec![shape_span(0, 3), shape_span(6, 2)]);
        assert!(
            insert_frame(f.scene, layer(&f), 4).is_none(),
            "inside a gap"
        );
        let cmds = insert_frame(f.scene, layer(&f), 9);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 3, 'S'), (6, 4, 'S')]);
        let empty = fixture(vec![]);
        assert!(insert_frame(empty.scene, layer(&empty), 0).is_none());
    }

    #[test]
    fn remove_frame_shortens_shifts_and_drops_single_frame_spans() {
        let mut f = fixture(vec![shape_span(0, 5), shape_span(5, 1), shape_span(6, 4)]);
        let cmds = remove_frame(f.scene, layer(&f), 1);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 4, 'S'), (4, 1, 'S'), (5, 4, 'S')]);
        let cmds = remove_frame(f.scene, layer(&f), 4);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 4, 'S'), (4, 4, 'S')]);
        assert!(remove_frame(f.scene, layer(&f), 50).is_none());
    }

    #[test]
    fn insert_keyframe_splits_with_a_copy_that_has_fresh_ids() {
        let mut f = fixture(vec![shape_span(0, 10)]);
        let original = match layer(&f).spans()[0].content() {
            Content::Shape(s) => s.id(),
            _ => panic!("shape"),
        };
        assert!(
            insert_keyframe(f.scene, layer(&f), 0).is_none(),
            "already a keyframe"
        );
        let cmds = insert_keyframe(f.scene, layer(&f), 4);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 4, 'S'), (4, 6, 'S')]);
        let ids: Vec<ShapeId> = layer(&f)
            .spans()
            .iter()
            .map(|s| match s.content() {
                Content::Shape(shape) => shape.id(),
                _ => panic!("shape"),
            })
            .collect();
        assert_eq!(ids[0], original);
        assert_ne!(ids[0], ids[1], "the copy has its own identity");
    }

    #[test]
    fn keyframes_past_the_end_hold_then_add_a_one_frame_keyframe() {
        let mut f = fixture(vec![shape_span(0, 3)]);
        let cmds = insert_keyframe(f.scene, layer(&f), 6);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 1, 'S')]);
        let cmds = insert_blank_keyframe(f.scene, layer(&f), 9);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 3, 'S'), (9, 1, 'E')]);
        let gap = fixture(vec![shape_span(0, 2), shape_span(5, 2)]);
        assert!(insert_keyframe(gap.scene, layer(&gap), 3).is_none());
    }

    #[test]
    fn blank_keyframes_empty_the_new_part_only() {
        let mut f = fixture(vec![shape_span(0, 10)]);
        let cmds = insert_blank_keyframe(f.scene, layer(&f), 3);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 3, 'S'), (3, 7, 'E')]);
    }

    #[test]
    fn clear_keyframe_merges_into_the_previous_span_or_empties_the_first() {
        let mut f = fixture(vec![shape_span(0, 3), shape_span(3, 4)]);
        let cmds = clear_keyframe(f.scene, layer(&f), 3);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 7, 'S')]);
        let cmds = clear_keyframe(f.scene, layer(&f), 0);
        run(&mut f, cmds);
        assert_eq!(
            shape_of(&f),
            vec![(0, 7, 'E')],
            "no previous span: it becomes empty"
        );
        assert!(
            clear_keyframe(f.scene, layer(&f), 2).is_none(),
            "not a keyframe start"
        );
        // A gap before the keyframe means there is nothing to absorb it.
        let mut gap = fixture(vec![shape_span(0, 2), shape_span(5, 2)]);
        let cmds = clear_keyframe(gap.scene, layer(&gap), 5);
        run(&mut gap, cmds);
        assert_eq!(shape_of(&gap), vec![(0, 2, 'S'), (5, 2, 'E')]);
    }

    #[test]
    fn tweens_survive_splits_and_shifts() {
        let first = shape_span(0, 6).with_tween(Tween::Linear);
        let mut f = fixture(vec![first, shape_span(6, 2)]);
        let cmds = insert_keyframe(f.scene, layer(&f), 3);
        run(&mut f, cmds);
        let tweens: Vec<Tween> = layer(&f).spans().iter().map(FrameSpan::tween).collect();
        assert_eq!(tweens, vec![Tween::Linear, Tween::Linear, Tween::None]);
        let cmds = insert_frame(f.scene, layer(&f), 0);
        run(&mut f, cmds);
        assert_eq!(layer(&f).spans()[0].tween(), Tween::Linear);
    }

    #[test]
    fn overflowing_frame_numbers_do_not_panic() {
        let f = fixture(vec![shape_span(0, 3)]);
        assert!(insert_frame(f.scene, layer(&f), u32::MAX).is_none());
        // A keyframe at the far end is representable; the layer simply becomes very long.
        assert!(insert_keyframe(f.scene, layer(&f), u32::MAX).is_some());
    }

    #[test]
    fn move_span_shifts_one_span_and_refuses_overlaps() {
        let mut f = fixture(vec![shape_span(0, 3), shape_span(6, 2)]);
        let cmds = move_span(f.scene, layer(&f), 0, 2);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(2, 3, 'S'), (6, 2, 'S')]);
        assert!(
            move_span(f.scene, layer(&f), 2, 4).is_none(),
            "4..7 meets 6..8"
        );
        assert!(
            move_span(f.scene, layer(&f), 2, 3).is_some(),
            "3..6 just touches"
        );
        assert!(move_span(f.scene, layer(&f), 2, 2).is_none(), "no movement");
        assert!(
            move_span(f.scene, layer(&f), 1, 9).is_none(),
            "no span starts at 1"
        );
        assert!(
            move_span(f.scene, layer(&f), 6, u32::MAX).is_none(),
            "past the last frame"
        );
        // Moving past a neighbour is allowed when the destination is free, and the list
        // stays sorted.
        let cmds = move_span(f.scene, layer(&f), 2, 10);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(6, 2, 'S'), (10, 3, 'S')]);
        f.session.undo();
        assert_eq!(
            shape_of(&f),
            vec![(2, 3, 'S'), (6, 2, 'S')],
            "one undo step"
        );
    }

    #[test]
    fn resize_span_changes_only_the_length_and_stops_at_the_next_span() {
        let mut f = fixture(vec![shape_span(0, 3), shape_span(6, 2)]);
        let cmds = resize_span(f.scene, layer(&f), 0, 6);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 6, 'S'), (6, 2, 'S')]);
        assert!(
            resize_span(f.scene, layer(&f), 0, 7).is_none(),
            "would overlap"
        );
        assert!(
            resize_span(f.scene, layer(&f), 0, 0).is_none(),
            "zero length"
        );
        assert!(resize_span(f.scene, layer(&f), 0, 6).is_none(), "unchanged");
        let cmds = resize_span(f.scene, layer(&f), 6, 1);
        run(&mut f, cmds);
        assert_eq!(
            shape_of(&f),
            vec![(0, 6, 'S'), (6, 1, 'S')],
            "shrinking is fine"
        );
    }

    #[test]
    fn a_new_keyframe_copies_the_stroke_and_gradient_of_the_artwork() {
        use flare_core::DocumentPoint;
        use flare_document::{Gradient, Stroke};
        let point = |x| DocumentPoint::new(x, 0.0).expect("finite");
        let gradient = Gradient::linear(point(0.0), point(5.0), Rgba8::WHITE, Rgba8::TRANSPARENT)
            .expect("gradient");
        let stroke = Stroke::new(2.0, Rgba8::WHITE).expect("stroke");
        let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 5.0, 5.0, Rgba8::WHITE)
            .expect("rect")
            .with_stroke(stroke)
            .with_gradient(gradient);
        let span = FrameSpan::new(
            0,
            NonZeroU32::new(6).expect("nonzero"),
            Content::Shape(shape),
        );
        let mut f = fixture(vec![span]);
        let cmds = insert_keyframe(f.scene, layer(&f), 3);
        run(&mut f, cmds);
        let spans = layer(&f).spans();
        assert_eq!(spans.len(), 2);
        let Content::Shape(copy) = spans[1].content() else {
            panic!("a shape expected")
        };
        assert_eq!(copy.gradient(), Some(gradient));
        assert_eq!(copy.stroke(), Some(stroke));
    }

    fn first_ids(layer: &Layer) -> Vec<flare_core::ShapeId> {
        layer
            .spans()
            .iter()
            .filter_map(|s| match s.content() {
                Content::Shape(shape) => Some(shape.id()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn copying_cuts_spans_to_the_range_and_makes_the_start_frame_zero() {
        let f = fixture(vec![shape_span(0, 5), shape_span(5, 3), shape_span(10, 2)]);
        let clip = copy_frames(layer(&f), 3, 6).expect("clip");
        assert_eq!(clip.length(), 4);
        let shape: Vec<(u32, u32)> = clip
            .spans()
            .iter()
            .map(|s| (s.start(), s.duration().get()))
            .collect();
        assert_eq!(
            shape,
            vec![(0, 2), (2, 2)],
            "frames 3..5 of the first, 5..7 of the second"
        );
        // Only a gap: nothing to copy. Backwards: nothing either.
        assert!(copy_frames(layer(&f), 8, 9).is_none());
        assert!(copy_frames(layer(&f), 6, 3).is_none());
        // A range running past the end still copies what is there.
        let tail = copy_frames(layer(&f), 10, 40).expect("clip");
        assert_eq!(tail.length(), 31);
        assert_eq!(tail.spans().len(), 1);
    }

    #[test]
    fn pasting_replaces_the_covered_frames_and_keeps_the_rest_in_place() {
        let mut f = fixture(vec![shape_span(0, 4), shape_span(10, 4)]);
        let clip = copy_frames(layer(&f), 10, 13).expect("clip"); // one 4-frame span
        let cmds = paste_frames(f.scene, layer(&f), 2, &clip);
        run(&mut f, cmds);
        // 0..2 stays, the paste covers 2..6 (cutting the first span's tail), 10..14 stays.
        assert_eq!(shape_of(&f), vec![(0, 2, 'S'), (2, 4, 'S'), (10, 4, 'S')]);
        f.session.undo();
        assert_eq!(
            shape_of(&f),
            vec![(0, 4, 'S'), (10, 4, 'S')],
            "one undo step"
        );
    }

    #[test]
    fn a_paste_inside_a_span_splits_it_and_every_piece_has_its_own_identity() {
        let mut f = fixture(vec![shape_span(0, 12), shape_span(20, 2)]);
        let clip = copy_frames(layer(&f), 20, 21).expect("clip");
        let cmds = paste_frames(f.scene, layer(&f), 4, &clip);
        run(&mut f, cmds);
        assert_eq!(
            shape_of(&f),
            vec![(0, 4, 'S'), (4, 2, 'S'), (6, 6, 'S'), (20, 2, 'S')]
        );
        let ids = first_ids(layer(&f));
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "no two pieces share a shape id");
    }

    #[test]
    fn pasting_past_the_end_or_over_a_gap_works_and_overflow_is_refused() {
        let mut f = fixture(vec![shape_span(0, 2)]);
        let clip = copy_frames(layer(&f), 0, 1).expect("clip");
        let cmds = paste_frames(f.scene, layer(&f), 8, &clip);
        run(&mut f, cmds);
        assert_eq!(shape_of(&f), vec![(0, 2, 'S'), (8, 2, 'S')]);
        assert!(paste_frames(f.scene, layer(&f), u32::MAX, &clip).is_none());
        assert!(paste_frames(f.scene, layer(&f), u32::MAX - 1, &clip).is_none());
        assert!(paste_frames(f.scene, layer(&f), u32::MAX - 2, &clip).is_some());
    }

    #[test]
    fn a_tween_cut_off_from_its_target_is_not_copied() {
        use flare_document::Tween;
        let mut first = shape_span(0, 4);
        first = first.with_tween(Tween::Linear);
        let f = fixture(vec![first, shape_span(4, 4)]);
        let whole = copy_frames(layer(&f), 0, 3).expect("clip");
        assert_eq!(
            whole.spans()[0].tween(),
            Tween::Linear,
            "the span is copied whole"
        );
        let cut = copy_frames(layer(&f), 0, 1).expect("clip");
        assert_eq!(
            cut.spans()[0].tween(),
            Tween::None,
            "cut short, its target is gone"
        );
    }
}
