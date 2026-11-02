//! Source attachments: data from a foreign file that Flare cannot edit but must not lose.
//!
//! An attachment is an opaque piece of source text (for an XFL import, one XML element),
//! tied to an entity by a stable ID and checked by a content digest. It travels with the
//! document through edits, saves and exports, and is dropped only when the entity it hangs
//! off is deleted. It is *not* semantically valid after arbitrary edits: a frame-level
//! attachment remembers the keyframe it came from, and an exporter reports one it can no
//! longer place instead of guessing.

use flare_core::{LayerId, SceneId, SymbolId};
use thiserror::Error;

/// Largest single attachment, so imports and files cannot make a document unbounded.
pub const MAX_ATTACHMENT_BYTES: usize = 1024 * 1024;
/// Most attachments one document may hold.
pub const MAX_ATTACHMENTS: usize = 10_000;

/// What an attachment hangs off. The ID is the entity's stable identity, so the attachment
/// survives edits to everything around it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Anchor {
    Document,
    Scene(SceneId),
    Symbol(SymbolId),
    Layer(LayerId),
}

/// Where in the anchored entity the source text belongs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Placement {
    /// A child of the anchored document or symbol, after the parts Flare understands.
    Child,
    /// A whole layer of a scene or symbol, at its original position among the layers.
    LayerAt { index: u32 },
    /// An element inside the keyframe that began at `frame` on the anchored layer.
    FrameElement { frame: u32 },
    /// Something else attached to that keyframe (scripts, sounds, labels).
    FrameExtra { frame: u32 },
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AttachmentError {
    #[error("an attachment may be at most {MAX_ATTACHMENT_BYTES} bytes, this one is {0}")]
    TooLarge(usize),
    #[error("{placement:?} cannot be attached to {anchor:?}")]
    BadCombination {
        anchor: Anchor,
        placement: Placement,
    },
    #[error("the content digest does not match the content")]
    DigestMismatch,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attachment {
    format: String,
    anchor: Anchor,
    placement: Placement,
    content: String,
    digest: [u8; 32],
}

fn combination_ok(anchor: Anchor, placement: Placement) -> bool {
    matches!(
        (anchor, placement),
        (Anchor::Document | Anchor::Symbol(_), Placement::Child)
            | (
                Anchor::Scene(_) | Anchor::Symbol(_),
                Placement::LayerAt { .. }
            )
            | (
                Anchor::Layer(_),
                Placement::FrameElement { .. } | Placement::FrameExtra { .. }
            )
    )
}

impl Attachment {
    /// `format` names the source format ("xfl"); `content` is the source text.
    pub fn new(
        format: impl Into<String>,
        anchor: Anchor,
        placement: Placement,
        content: impl Into<String>,
    ) -> Result<Self, AttachmentError> {
        let content = content.into();
        let digest = *blake3::hash(content.as_bytes()).as_bytes();
        Self::from_parts(format.into(), anchor, placement, content, digest)
    }

    /// Rebuilds an attachment read from storage, refusing one whose digest does not match.
    pub fn from_parts(
        format: String,
        anchor: Anchor,
        placement: Placement,
        content: String,
        digest: [u8; 32],
    ) -> Result<Self, AttachmentError> {
        if content.len() > MAX_ATTACHMENT_BYTES {
            return Err(AttachmentError::TooLarge(content.len()));
        }
        if !combination_ok(anchor, placement) {
            return Err(AttachmentError::BadCombination { anchor, placement });
        }
        if *blake3::hash(content.as_bytes()).as_bytes() != digest {
            return Err(AttachmentError::DigestMismatch);
        }
        Ok(Self {
            format,
            anchor,
            placement,
            content,
            digest,
        })
    }

    pub fn format(&self) -> &str {
        &self.format
    }

    pub fn anchor(&self) -> Anchor {
        self.anchor
    }

    pub fn placement(&self) -> Placement {
        self.placement
    }

    pub fn content(&self) -> &str {
        &self.content
    }

    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn digests_identify_content_and_mismatches_are_refused() {
        let a = Attachment::new("xfl", Anchor::Document, Placement::Child, "<a/>").expect("a");
        let b = Attachment::new("xfl", Anchor::Document, Placement::Child, "<a/>").expect("b");
        let other =
            Attachment::new("xfl", Anchor::Document, Placement::Child, "<b/>").expect("other");
        assert_eq!(a.digest(), b.digest());
        assert_ne!(a.digest(), other.digest());
        let rebuilt = Attachment::from_parts(
            "xfl".into(),
            a.anchor(),
            a.placement(),
            a.content().into(),
            *a.digest(),
        );
        assert_eq!(rebuilt, Ok(a.clone()));
        let tampered = Attachment::from_parts(
            "xfl".into(),
            a.anchor(),
            a.placement(),
            "<evil/>".into(),
            *a.digest(),
        );
        assert_eq!(tampered, Err(AttachmentError::DigestMismatch));
    }

    #[test]
    fn only_sensible_anchor_and_placement_pairs_are_allowed() {
        let (scene, symbol, layer) = (SceneId::new(), SymbolId::new(), LayerId::new());
        let ok = |anchor, placement| Attachment::new("xfl", anchor, placement, "<x/>").is_ok();
        assert!(ok(Anchor::Document, Placement::Child));
        assert!(ok(Anchor::Symbol(symbol), Placement::Child));
        assert!(ok(Anchor::Scene(scene), Placement::LayerAt { index: 2 }));
        assert!(ok(Anchor::Symbol(symbol), Placement::LayerAt { index: 0 }));
        assert!(ok(
            Anchor::Layer(layer),
            Placement::FrameElement { frame: 4 }
        ));
        assert!(ok(Anchor::Layer(layer), Placement::FrameExtra { frame: 4 }));
        assert!(
            !ok(Anchor::Scene(scene), Placement::Child),
            "scenes have layers, not loose children"
        );
        assert!(!ok(Anchor::Document, Placement::LayerAt { index: 0 }));
        assert!(!ok(Anchor::Layer(layer), Placement::Child));
        assert!(!ok(Anchor::Document, Placement::FrameElement { frame: 0 }));
    }

    #[test]
    fn size_is_bounded() {
        let big = "x".repeat(MAX_ATTACHMENT_BYTES + 1);
        assert_eq!(
            Attachment::new("xfl", Anchor::Document, Placement::Child, big),
            Err(AttachmentError::TooLarge(MAX_ATTACHMENT_BYTES + 1))
        );
        assert!(
            Attachment::new(
                "xfl",
                Anchor::Document,
                Placement::Child,
                "x".repeat(MAX_ATTACHMENT_BYTES)
            )
            .is_ok()
        );
    }
}
