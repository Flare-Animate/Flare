//! Selection-based editing operations as pure functions from a document to commands.
//!
//! Nothing here touches any UI or the session, so every operation is unit-testable and the
//! UI only decides *which* operation to run. Selections are frame spans, identified by
//! their layer and start frame (spans on a layer never overlap, so this is unique).

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, FrameSpan, Timeline};
use flare_eval::DrawnShape;

mod anchors;
mod arrange;
mod bounds;
mod clipboard;
mod grouping;
mod paint;
mod select;
mod transform;

#[allow(unused_imports)]
use anchors::*;
pub use anchors::{anchors, move_anchor};
#[allow(unused_imports)]
use arrange::*;
pub use arrange::{Align, align_deltas, distribute_deltas};
#[allow(unused_imports)]
use bounds::*;
pub use bounds::{shape_bounds, span_bounds, spans_bounds, symbol_bounds};
#[allow(unused_imports)]
use clipboard::*;
pub use clipboard::{
    ClipItem, clip_bounds, convert_to_symbol, convert_to_symbol_as, copy_span, duplicate,
    duplicate_through, paste, paste_offset, place_instance,
};
#[allow(unused_imports)]
use grouping::*;
pub use grouping::{break_apart, group};
#[allow(unused_imports)]
use paint::*;
pub use paint::{restyle, set_ease, set_tween};
#[allow(unused_imports)]
use select::*;
pub use select::{polygon_contains, spans_at_frame, spans_in_polygon};
#[allow(unused_imports)]
use transform::*;
pub use transform::{transform, translate};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanRef {
    pub layer: LayerId,
    pub start: u32,
}

fn scene_timeline(document: &Document, scene: SceneId) -> Option<&Timeline> {
    document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .map(|s| s.timeline())
}

pub fn find_span(document: &Document, scene: SceneId, at: SpanRef) -> Option<&FrameSpan> {
    scene_timeline(document, scene)?
        .layers()
        .iter()
        .find(|layer| layer.id() == at.layer)?
        .spans()
        .iter()
        .find(|span| span.start() == at.start)
}

/// The scene-level span that produced a hit shape at `frame`: the shape's own span, or
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
        let owns = match (span.content(), drawn.path.first()) {
            (Content::Shape(shape), None) => shape.id() == drawn.shape.id(),
            (Content::Instance(instance), Some(top)) => instance.id() == *top,
            _ => false,
        };
        if owns {
            return Some(SpanRef {
                layer: layer.id(),
                start: span.start(),
            });
        }
    }
    None
}

pub fn delete(scene: SceneId, at: SpanRef) -> Vec<Command> {
    vec![Command::RemoveSpan {
        scene,
        layer: at.layer,
        start: at.start,
    }]
}

#[cfg(test)]
mod tests;
