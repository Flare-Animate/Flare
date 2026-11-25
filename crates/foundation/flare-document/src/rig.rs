//! A rig kept in the project: a skeleton for one symbol. The document stores the rig as the
//! versioned JSON `flare-rig` reads and writes and checks only what it can without reading it
//! (size, one rig for a symbol, the symbol exists); `flare_editor::rig_library` parses and
//! validates the bones, which is where a rig is made, so a rig in a document is always one that
//! read back.

use flare_core::SymbolId;

/// The most rigs a project holds (one per symbol).
pub const MAX_RIG_ASSETS: usize = 256;
/// The longest rig, as JSON text, in bytes.
pub const MAX_RIG_PAYLOAD_BYTES: usize = 256 * 1024;

/// Why a rig could not be stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum RigAssetError {
    #[error("a rig is at most {MAX_RIG_PAYLOAD_BYTES} bytes")]
    TooLarge,
    #[error("a rig cannot be empty")]
    Empty,
}

/// A rig and the symbol it moves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RigAsset {
    symbol: SymbolId,
    payload: String,
}

impl RigAsset {
    pub fn new(symbol: SymbolId, payload: impl Into<String>) -> Result<Self, RigAssetError> {
        let payload = payload.into();
        if payload.trim().is_empty() {
            return Err(RigAssetError::Empty);
        }
        if payload.len() > MAX_RIG_PAYLOAD_BYTES {
            return Err(RigAssetError::TooLarge);
        }
        Ok(Self { symbol, payload })
    }

    pub fn symbol(&self) -> SymbolId {
        self.symbol
    }

    /// The rig, as the JSON `flare_rig::Rig` reads.
    pub fn payload(&self) -> &str {
        &self.payload
    }
}
