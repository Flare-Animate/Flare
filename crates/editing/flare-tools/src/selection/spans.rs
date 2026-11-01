//! Which spans a frame selects.

use flare_core::SceneId;
use flare_document::{Content, Document};

use crate::common::*;

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
