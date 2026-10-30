//! Moving spans through a matrix.

use flare_commands::Command;
use flare_core::{DocumentPoint, InstanceId, SceneId, ShapeId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Gradient, Instance, PathSegment, Shape, Tween};
use glamx::{DAffine2, DVec2};

use super::*;

/// `shape` with every point mapped through `matrix`; the stroke width is kept.
pub(super) fn transform_shape(shape: &Shape, matrix: DAffine2) -> Option<Shape> {
    transform_shape_as(shape, shape.id(), matrix)
}

/// Like `transform_shape`, but the result has the identity `id` (for copies).
pub(super) fn transform_shape_as(shape: &Shape, id: ShapeId, matrix: DAffine2) -> Option<Shape> {
    let moved = |p: &DocumentPoint| {
        let q = matrix.transform_point2(p.as_vec());
        DocumentPoint::new(q.x, q.y).ok()
    };
    let path = shape
        .path()
        .iter()
        .map(|segment| {
            Some(match segment {
                PathSegment::MoveTo(p) => PathSegment::MoveTo(moved(p)?),
                PathSegment::LineTo(p) => PathSegment::LineTo(moved(p)?),
                PathSegment::CubicTo(a, b, c) => {
                    PathSegment::CubicTo(moved(a)?, moved(b)?, moved(c)?)
                }
                PathSegment::Close => PathSegment::Close,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let moved = Shape::new(id, path, shape.fill()).ok()?;
    // Rebuilding must not drop the stroke or the gradient, and the gradient moves with the
    // artwork.
    let moved = moved.painted_like(shape);
    Some(match shape.gradient() {
        Some(g) => {
            let map = |p: DocumentPoint| {
                let q = matrix.transform_point2(p.as_vec());
                DocumentPoint::new(q.x, q.y).ok()
            };
            let mapped = Gradient::linear(map(g.from())?, map(g.to())?, g.start(), g.end()).ok()?;
            moved.with_gradient(mapped)
        }
        None => moved,
    })
}

/// Moves a span's content by `delta`, keeping identity, duration and tween.
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

/// Maps a span's content through `matrix` (in stage coordinates), keeping identity, duration
/// and tween. `None` when the span is missing or empty, the matrix is the identity or cannot
/// be inverted (a zero scale would destroy the content), or the result is not finite.
pub fn transform(
    document: &Document,
    scene: SceneId,
    at: SpanRef,
    matrix: DAffine2,
) -> Option<Vec<Command>> {
    if matrix == DAffine2::IDENTITY
        || !matrix.is_finite()
        || matrix.matrix2.determinant().abs() < 1e-9
    {
        return None;
    }
    let span = find_span(document, scene, at)?;
    let content = match span.content() {
        Content::Empty => return None,
        Content::Shape(shape) => Content::Shape(transform_shape(shape, matrix)?),
        Content::Instance(instance) => {
            let moved = matrix * instance.transform().as_affine();
            if !tween_allows(document, scene, at, span, moved) {
                return None;
            }
            let transform = Transform2D::new(moved).ok()?;
            Content::Instance(Instance::new(instance.id(), instance.symbol(), transform))
        }
    };
    let replacement = FrameSpan::new(span.start(), span.duration(), content).with_motion_of(span);
    Some(vec![
        Command::RemoveSpan {
            scene,
            layer: at.layer,
            start: at.start,
        },
        Command::InsertSpan {
            scene,
            layer: at.layer,
            span: replacement,
        },
    ])
}

/// `content` moved through `matrix` with fresh shape and instance identities, so the result
/// is independent of the original. `None` for empty content or a non-finite result.
pub(super) fn fresh_content(content: &Content, matrix: DAffine2) -> Option<Content> {
    Some(match content {
        Content::Empty => return None,
        Content::Shape(shape) => Content::Shape(transform_shape_as(shape, ShapeId::new(), matrix)?),
        Content::Instance(instance) => {
            let moved = matrix * instance.transform().as_affine();
            let transform = Transform2D::new(moved).ok()?;
            Content::Instance(Instance::new(
                InstanceId::new(),
                instance.symbol(),
                transform,
            ))
        }
    })
}
