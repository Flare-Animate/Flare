//! Moving spans through a matrix.

use flare_commands::Command;
use flare_core::{SceneId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, Tween};
use glamx::{DAffine2, DVec2};

use crate::common::*;

/// Moves the element `at` names by `delta`, keeping identity, duration and tween.
/// `None` when the span is missing, the delta is zero or the result is not finite.
pub fn translate(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    delta: DVec2,
) -> Option<Vec<Command>> {
    if delta == DVec2::ZERO {
        return None;
    }
    transform(document, scene, at, DAffine2::from_translation(delta))
}

/// [`translate`] for every element of `refs`, as one list of commands.
pub fn translate_many(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    delta: DVec2,
) -> Option<Vec<Command>> {
    if delta == DVec2::ZERO {
        return None;
    }
    transform_many(document, scene, refs, DAffine2::from_translation(delta))
}

/// Whether an instance span may take the transform `moved`. A motion tween interpolates between
/// two exposures, which only works for a scale, rotation and translation, so shearing or
/// mirroring an exposure that starts or ends a tween would make the frame impossible to draw.
pub(super) fn tween_allows(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    span: &FrameSpan,
    moved: DAffine2,
) -> bool {
    if flare_eval::can_tween(moved) {
        return true;
    }
    if span.tween() == Tween::Linear {
        return false;
    }
    // An earlier exposure that tweens into this one needs it to be tweenable too.
    let tweened_into = scene_timeline(document, scene)
        .and_then(|t| t.layers().iter().find(|l| l.id() == at.layer))
        .is_some_and(|layer| {
            layer
                .spans()
                .iter()
                .any(|p| p.end() == u64::from(span.start()) && p.tween() == Tween::Linear)
        });
    !tweened_into
}

/// Maps the element `at` names through `matrix` (in stage coordinates), keeping identity, duration
/// and tween. `None` when the element is missing or empty, the matrix is the identity or cannot
/// be inverted (a zero scale would destroy the content), or the result is not finite.
pub fn transform(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    matrix: DAffine2,
) -> Option<Vec<Command>> {
    transform_many(document, scene, &[at], matrix)
}

/// [`transform`] for every element of `refs`, as one list of commands. Elements that cannot take
/// the matrix are left as they are; `None` when none can.
pub fn transform_many(
    document: &Document,
    scene: SceneId,
    refs: &[SpanRef],
    matrix: DAffine2,
) -> Option<Vec<Command>> {
    if matrix == DAffine2::IDENTITY
        || !matrix.is_finite()
        || matrix.matrix2.determinant().abs() < 1e-9
    {
        return None;
    }
    let mut out = Vec::new();
    for group in by_span(refs) {
        let Some(first) = group.first() else {
            continue;
        };
        let Some(span) = find_span(document, scene, *first) else {
            continue;
        };
        let mut elements: Vec<Content> = span.elements().cloned().collect();
        let mut changed = false;
        for at in &group {
            let Some(content) = span.element(at.element) else {
                continue;
            };
            if let Some(moved) = moved_content(document, scene, *at, span, content, matrix) {
                elements[at.element] = moved;
                changed = true;
            }
        }
        if !changed {
            continue;
        }
        if let Ok(Some(replacement)) = span.with_elements(elements) {
            out.extend(replace_span(scene, *first, replacement));
        }
    }
    (!out.is_empty()).then_some(out)
}

fn moved_content(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    span: &FrameSpan,
    content: &Content,
    matrix: DAffine2,
) -> Option<Content> {
    Some(match content {
        Content::Empty | Content::Camera(_) => return None,
        Content::Shape(shape) => Content::Shape(transform_shape(shape, matrix)?),
        Content::Instance(instance) => {
            let moved = matrix * instance.transform().as_affine();
            if !tween_allows(document, scene, at, span, moved) {
                return None;
            }
            let transform = Transform2D::new(moved).ok()?;
            Content::Instance(
                Instance::new(instance.id(), instance.symbol(), transform)
                    .with_playback(instance.playback()),
            )
        }
    })
}
