//! Source attachment records.

use crate::StorageError;
use flare_core::{LayerId, SceneId, SymbolId};
use flare_document::{Anchor, Attachment, Placement};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::*;

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "id")]
pub(super) enum AnchorWire {
    Document,
    Scene(String),
    Symbol(String),
    Layer(String),
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind")]
pub(super) enum PlacementWire {
    Child,
    LayerAt { index: u32 },
    FrameElement { frame: u32 },
    FrameExtra { frame: u32 },
}

#[derive(Serialize, Deserialize)]
pub(crate) struct AttachmentWire {
    pub(super) format: String,
    pub(super) anchor: AnchorWire,
    pub(super) placement: PlacementWire,
    pub(super) content: String,
    /// Lowercase hex of the content's BLAKE3 digest.
    pub(super) digest: String,
}

pub(super) fn hex(bytes: &[u8; 32]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(super) fn unhex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut out = [0_u8; 32];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(text.get(i * 2..i * 2 + 2)?, 16).ok()?;
    }
    Some(out)
}

impl AttachmentWire {
    pub(crate) fn of(attachment: &Attachment) -> Self {
        let id = |u: &Uuid| u.to_string();
        Self {
            format: attachment.format().to_owned(),
            anchor: match attachment.anchor() {
                Anchor::Document => AnchorWire::Document,
                Anchor::Scene(s) => AnchorWire::Scene(id(s.as_uuid())),
                Anchor::Symbol(s) => AnchorWire::Symbol(id(s.as_uuid())),
                Anchor::Layer(l) => AnchorWire::Layer(id(l.as_uuid())),
            },
            placement: match attachment.placement() {
                Placement::Child => PlacementWire::Child,
                Placement::LayerAt { index } => PlacementWire::LayerAt { index },
                Placement::FrameElement { frame } => PlacementWire::FrameElement { frame },
                Placement::FrameExtra { frame } => PlacementWire::FrameExtra { frame },
            },
            content: attachment.content().to_owned(),
            digest: hex(attachment.digest()),
        }
    }

    /// Rebuilds the attachment, refusing one whose digest, size or anchor kind is wrong.
    pub(crate) fn build(self) -> Result<Attachment, StorageError> {
        let anchor = match self.anchor {
            AnchorWire::Document => Anchor::Document,
            AnchorWire::Scene(id) => Anchor::Scene(SceneId::from_uuid(uuid(&id)?)),
            AnchorWire::Symbol(id) => Anchor::Symbol(SymbolId::from_uuid(uuid(&id)?)),
            AnchorWire::Layer(id) => Anchor::Layer(LayerId::from_uuid(uuid(&id)?)),
        };
        let placement = match self.placement {
            PlacementWire::Child => Placement::Child,
            PlacementWire::LayerAt { index } => Placement::LayerAt { index },
            PlacementWire::FrameElement { frame } => Placement::FrameElement { frame },
            PlacementWire::FrameExtra { frame } => Placement::FrameExtra { frame },
        };
        let digest = unhex(&self.digest).ok_or_else(|| {
            StorageError::Corrupt("attachment digest is not 64 hex digits".into())
        })?;
        Attachment::from_parts(self.format, anchor, placement, self.content, digest)
            .map_err(|error| StorageError::Corrupt(error.to_string()))
    }
}
