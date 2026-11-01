//! Finding a span of a scene's timeline.

use flare_core::{LayerId, SceneId};
use flare_document::{Document, FrameSpan, Timeline};

/// A selected span, by its layer and start frame (spans on a layer never overlap, so this is unique).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpanRef {
    pub layer: LayerId,
    pub start: u32,
}

pub(crate) fn scene_timeline(document: &Document, scene: SceneId) -> Option<&Timeline> {
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
