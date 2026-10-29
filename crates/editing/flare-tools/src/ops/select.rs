//! Which spans a loop or a frame selects.

use emath::Pos2;
use flare_core::SceneId;
use flare_document::{Content, Document};

use super::*;

/// Whether `point` is inside the closed polygon `polygon` (even-odd rule).
pub fn polygon_contains(polygon: &[Pos2], point: Pos2) -> bool {
    let mut inside = false;
    let mut previous = match polygon.last() {
        Some(p) => *p,
        None => return false,
    };
    for &current in polygon {
        if (current.y > point.y) != (previous.y > point.y)
            && point.x
                < (previous.x - current.x) * (point.y - current.y) / (previous.y - current.y)
                    + current.x
        {
            inside = !inside;
        }
        previous = current;
    }
    inside
}

/// The spans on unlocked layers at `frame` whose bounds are centered inside `polygon`: what a
/// lasso drawn around them selects, topmost layer first.
pub fn spans_in_polygon(
    document: &Document,
    scene: SceneId,
    frame: u32,
    polygon: &[Pos2],
) -> Vec<SpanRef> {
    if polygon.len() < 3 {
        return Vec::new();
    }
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|layer| !layer.is_locked())
        .filter_map(|layer| {
            let span = layer.span_at(frame)?;
            let center = content_bounds(document, span.content())?.center();
            polygon_contains(polygon, center).then_some(SpanRef {
                layer: layer.id(),
                start: span.start(),
            })
        })
        .collect()
}

/// Every span with something to show on unlocked layers at `frame`: what Select All selects.
pub fn spans_at_frame(document: &Document, scene: SceneId, frame: u32) -> Vec<SpanRef> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|layer| !layer.is_locked())
        .filter_map(|layer| {
            let span = layer.span_at(frame)?;
            (!matches!(span.content(), Content::Empty)).then_some(SpanRef {
                layer: layer.id(),
                start: span.start(),
            })
        })
        .collect()
}
