//! Which spans a frame selects.

use flare_core::SceneId;
use flare_document::{Content, Document};

use crate::common::*;

/// Every element with something to show on unlocked layers at `frame`: what Select All selects.
pub fn spans_at_frame(document: &Document, scene: SceneId, frame: u32) -> Vec<SpanRef> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|layer| !layer.is_locked())
        .filter_map(|layer| Some((layer, layer.span_at(frame)?)))
        .flat_map(|(layer, span)| {
            span.elements()
                .enumerate()
                .filter(|(_, content)| !matches!(content, Content::Empty))
                .map(|(element, _)| SpanRef {
                    layer: layer.id(),
                    start: span.start(),
                    element,
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
