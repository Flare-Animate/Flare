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
}
