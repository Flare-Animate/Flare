//! Which span a hit on the stage belongs to.

use flare_core::SceneId;
use flare_document::{Content, Document};
use flare_eval::DrawnShape;
use glamx::DVec2;

use crate::common::{SpanRef, scene_timeline};

/// The scene-level element that produced a hit shape at `frame`: the shape itself, or
/// the top-level instance that contains it.
pub fn locate_hit(
    document: &Document,
    scene: SceneId,
    frame: u32,
    drawn: &DrawnShape<'_>,
) -> Option<SpanRef> {
    for layer in scene_timeline(document, scene)?.layers() {
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        let owned = span
            .elements()
            .position(|element| match (element, drawn.path.first()) {
                (Content::Shape(shape), None) => shape.id() == drawn.shape.id(),
                (Content::Instance(instance), Some(top)) => instance.id() == *top,
                _ => false,
            });
        if let Some(element) = owned {
            return Some(SpanRef {
                layer: layer.id(),
                start: span.start(),
                element,
            });
        }
    }
    None
}

/// The instance on a layer of `scene` that shows a picture under `point` at `frame`, topmost
/// first: a picture inside a symbol (a video brought in, a picture made a symbol) is picked by
/// its rectangle, as a picture on a layer is, and the pick is the instance holding it. Pictures
/// on a layer itself are `magic_wand::bitmap_at`'s.
pub fn picture_at(
    document: &Document,
    scene: SceneId,
    frame: u32,
    point: DVec2,
) -> Option<SpanRef> {
    let evaluated = flare_eval::evaluate_frame_without_camera(document, scene, frame).ok()?;
    let top = evaluated.bitmaps.iter().rev().find_map(|drawn| {
        let top = *drawn.path.first()?;
        let matrix = drawn.transform;
        if matrix.matrix2.determinant().abs() < 1e-12 {
            return None;
        }
        let local = matrix.inverse().transform_point2(point);
        let (w, h) = (
            f64::from(drawn.bitmap.width()),
            f64::from(drawn.bitmap.height()),
        );
        (local.x >= 0.0 && local.y >= 0.0 && local.x < w && local.y < h).then_some(top)
    })?;
    for layer in scene_timeline(document, scene)?.layers() {
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        let owned = span
            .elements()
            .position(|element| matches!(element, Content::Instance(i) if i.id() == top));
        if let Some(element) = owned {
            return Some(SpanRef {
                layer: layer.id(),
                start: span.start(),
                element,
            });
        }
    }
    None
}
