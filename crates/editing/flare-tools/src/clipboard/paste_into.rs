//! Pasting into the layer: what Animate does, as opposed to a new layer for each pasted thing.

use std::fmt;

use flare_core::SceneId;
use flare_document::{Content, Document};
use glamx::{DAffine2, DVec2};

use super::ClipItem;
use crate::common::fresh_content;
use crate::drawing::{DrawPlan, DrawRefusal, Target, place_contents};

/// Why a paste was not made.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PasteRefusal {
    /// A pasted instance's symbol is not in this document.
    MissingSymbol,
    /// A pasted shape moved to somewhere that is not a finite place.
    Unplaceable,
    /// The layer, or the keyframe on it, cannot take it (see [`DrawRefusal`]).
    Layer(DrawRefusal),
}

impl fmt::Display for PasteRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSymbol => f.write_str("a symbol it uses is not in this document"),
            Self::Unplaceable => f.write_str("it would land somewhere that is not a place"),
            Self::Layer(refusal) => refusal.fmt(f),
        }
    }
}

impl std::error::Error for PasteRefusal {}

/// The commands that paste `items`, in order and above what is there, into the keyframe of `target`
/// at `frame` (a keyframe is made there if the layer has none; see [`place_contents`]), moved by
/// `delta` stage units and with fresh identities, as one batch. Which layer, and when it is
/// refused, is the drawing's rule: a pasted thing goes where a drawn one would.
pub fn paste_into(
    document: &Document,
    scene: SceneId,
    target: Target,
    items: &[ClipItem],
    frame: u32,
    delta: DVec2,
    new_layer: &str,
) -> Result<DrawPlan, PasteRefusal> {
    let mut contents = Vec::with_capacity(items.len());
    for item in items {
        if let Content::Instance(instance) = &item.content
            && document.symbol(instance.symbol()).is_none()
        {
            return Err(PasteRefusal::MissingSymbol);
        }
        let content = fresh_content(&item.content, DAffine2::from_translation(delta))
            .ok_or(PasteRefusal::Unplaceable)?;
        contents.push(content);
    }
    place_contents(document, scene, target, frame, None, contents, new_layer)
        .map_err(PasteRefusal::Layer)
}

#[cfg(test)]
#[path = "paste_into_tests.rs"]
mod tests;
