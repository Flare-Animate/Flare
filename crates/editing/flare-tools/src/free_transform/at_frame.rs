//! Moving or transforming what a frame shows when that frame only shows a held drawing.
//!
//! A layer that holds its drawing shows its last keyframe on every frame after it. Changing the
//! drawing there must not reach back into the frames before, so the change starts a keyframe at
//! the frame (a copy of everything held, then the change applied to the copy) and the keyframe
//! it was held from is kept as long as it was, up to the frame. Anywhere else the edit is the
//! plain one on the span that shows.

use std::num::NonZeroU32;

use flare_commands::{Command, apply_all};
use flare_core::SceneId;
use flare_document::{Content, Document, FrameSpan, Tween};
use glamx::{DAffine2, DVec2};

use super::{transform_many, translate_many};
use crate::common::{SpanRef, find_span, fresh_content, scene_timeline};

/// The keyframes that start at `frame` for every ref whose span is only held there, with the refs
/// as they are once those keyframes exist. Refs of spans that really cover `frame` are returned
/// unchanged. `None` when there is nothing to start (no ref needs it).
pub fn own_held(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    frame: u32,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    let timeline = scene_timeline(document, scene)?;
    let mut commands = Vec::new();
    let mut mapped = Vec::with_capacity(refs.len());
    let mut started: Vec<(flare_core::LayerId, u32)> = Vec::new();
    for at in refs {
        let held = timeline
            .layers()
            .iter()
            .find(|layer| layer.id() == at.layer)
            .filter(|layer| layer.span_covering(frame).is_none())
            .and_then(|layer| layer.span_at(frame))
            .filter(|span| span.start() == at.start && span.tween() == Tween::None);
        let Some(held) = held else {
            mapped.push(*at);
            continue;
        };
        mapped.push(SpanRef {
            start: frame,
            ..*at
        });
        if started.contains(&(at.layer, at.start)) {
            continue;
        }
        started.push((at.layer, at.start));
        let elements: Vec<Content> = held
            .elements()
            .filter(|element| !matches!(element, Content::Empty))
            .map(|element| fresh_content(element, DAffine2::IDENTITY))
            .collect::<Option<_>>()?;
        let before = NonZeroU32::new(frame.checked_sub(held.start())?)?;
        let to_end = u32::try_from(timeline.frame_count())
            .unwrap_or(u32::MAX)
            .saturating_sub(frame);
        let length = NonZeroU32::new(to_end).unwrap_or(NonZeroU32::MIN);
        let copy = FrameSpan::new(frame, length, Content::Empty)
            .with_motion_of(held)
            .with_elements(elements)
            .ok()
            .flatten()?;
        commands.push(Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: held.start(),
        });
        commands.push(Command::InsertSpan {
            scene,
            layer: at.layer,
            span: held.retimed(held.start(), before),
        });
        commands.push(Command::InsertSpan {
            scene,
            layer: at.layer,
            span: copy,
        });
    }
    (!commands.is_empty()).then_some((commands, mapped))
}

/// What `edit` makes of `refs` seen from `frame`: with held spans made keyframes of their own
/// first. The refs come back as they are after the commands, so a selection can follow.
fn at_frame(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    frame: u32,
    edit: impl FnOnce(&Document, &[SpanRef]) -> Option<Vec<Command>>,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    let Some((mut commands, mapped)) = own_held(document, scene, refs, frame) else {
        return edit(document, refs).map(|c| (c, refs.to_vec()));
    };
    let working = apply_all(document, commands.iter().cloned())
        .ok()
        .flatten()?;
    // The copy is checked to exist before it is edited: a ref must name something.
    mapped
        .iter()
        .all(|at| find_span(&working, scene, *at).is_some())
        .then_some(())?;
    commands.extend(edit(&working, &mapped)?);
    Some((commands, mapped))
}

/// [`translate_many`] for the frame `frame`: a held drawing is copied to a keyframe there first.
pub fn translate_many_at(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    frame: u32,
    delta: DVec2,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    at_frame(document, scene, refs, frame, |doc, refs| {
        translate_many(doc, scene, refs, delta)
    })
}

/// [`transform_many`] for the frame `frame`: a held drawing is copied to a keyframe there first.
pub fn transform_many_at(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    frame: u32,
    matrix: DAffine2,
) -> Option<(Vec<Command>, Vec<SpanRef>)> {
    at_frame(document, scene, refs, frame, |doc, refs| {
        transform_many(doc, scene, refs, matrix)
    })
}
