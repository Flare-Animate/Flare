//! Making copies of what a span holds: through a matrix, with fresh identities, on a layer of their own.

use flare_commands::Command;
use flare_core::{DocumentPoint, InstanceId, LayerId, SceneId, ShapeId, Transform2D};
use flare_document::{
    BitmapInstance, Content, FrameSpan, Gradient, Instance, Layer, PathSegment, Shape,
};
use glamx::DAffine2;

use super::span::SpanRef;

/// `shape` with every point mapped through `matrix`; the stroke width is kept.
pub(crate) fn transform_shape(shape: &Shape, matrix: DAffine2) -> Option<Shape> {
    transform_shape_as(shape, shape.id(), matrix)
}

/// Like `transform_shape`, but the result has the identity `id` (for copies).
pub(crate) fn transform_shape_as(shape: &Shape, id: ShapeId, matrix: DAffine2) -> Option<Shape> {
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
    // Text stays text through a move and an even scale, where the source can follow; a turn, a
    // shear or a stretch makes the letters something a font cannot set, so they become outlines.
    let moved = match text_after(shape, matrix) {
        Some(source) => moved.with_text(source),
        None => moved,
    };
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

/// The text source of `shape` after the shape is mapped through `matrix`, if it is still text.
fn text_after(shape: &Shape, matrix: DAffine2) -> Option<flare_document::TextSource> {
    let source = shape.text()?;
    let (x, y) = (matrix.matrix2.x_axis, matrix.matrix2.y_axis);
    let scale = x.x;
    let even = x.y.abs() <= 1e-9
        && y.x.abs() <= 1e-9
        && scale > 0.0
        && (scale - y.y).abs() <= 1e-9 * scale.max(1.0);
    if !even {
        return None;
    }
    let at = matrix.transform_point2(source.origin().as_vec());
    source.moved_and_scaled(DocumentPoint::new(at.x, at.y).ok()?, scale)
}

/// `content` moved through `matrix` with fresh shape and instance identities, so the result
/// is independent of the original. `None` for empty content or a non-finite result.
pub(crate) fn fresh_content(content: &Content, matrix: DAffine2) -> Option<Content> {
    Some(match content {
        Content::Empty | Content::Camera(_) | Content::Armature(_) => return None,
        Content::Shape(shape) => Content::Shape(transform_shape_as(shape, ShapeId::new(), matrix)?),
        Content::Bitmap(placed) => {
            let transform = Transform2D::new(matrix * placed.transform().as_affine()).ok()?;
            Content::Bitmap(
                BitmapInstance::new(InstanceId::new(), placed.bitmap(), transform)
                    .with_opacity(placed.opacity()),
            )
        }
        Content::Instance(instance) => {
            let moved = matrix * instance.transform().as_affine();
            let transform = Transform2D::new(moved).ok()?;
            Content::Instance(
                Instance::new(InstanceId::new(), instance.symbol(), transform)
                    .with_playback(instance.playback()),
            )
        }
    })
}

/// A new layer holding one span, as the commands that add it.
pub(crate) fn new_layer_with(
    scene: SceneId,
    name: &str,
    span: FrameSpan,
) -> Option<(Vec<Command>, SpanRef)> {
    let layer_id = LayerId::new();
    let layer = Layer::new(layer_id, name, vec![]).ok()?;
    let at = SpanRef {
        layer: layer_id,
        start: span.start(),
        element: 0,
    };
    Some((
        vec![
            Command::AddLayer { scene, layer },
            Command::InsertSpan {
                scene,
                layer: layer_id,
                span,
            },
        ],
        at,
    ))
}
