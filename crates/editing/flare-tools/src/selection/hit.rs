//! Which span a hit on the stage belongs to.

use flare_core::SceneId;
use flare_document::{Content, Document};
use flare_eval::DrawnShape;

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
