//! The document's rigs, edited: the one place that decides how a rig is made, changed or taken
//! out of a project, so the Rigs panel, `flare rig` and the MCP server do the same thing and every
//! change is one undo step. A rig is stored as the JSON `flare_rig` reads; nothing is stored that
//! does not read back as a valid rig for its symbol.

use flare_commands::Command;
use flare_core::SymbolId;
use flare_document::{Document, RigAsset, RigAssetError};
use flare_rig::{Rig, RigCommand, RigError};

/// Why a rig could not be stored, changed or removed.
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RigLibraryError {
    #[error("there is no such symbol in this project")]
    NoSuchSymbol,
    #[error("the symbol already has a rig")]
    AlreadyRigged,
    #[error("the symbol has no rig")]
    NoRig,
    #[error("the rig is for another symbol")]
    WrongSymbol,
    #[error("not a rig: {0}")]
    NotARig(String),
    #[error(transparent)]
    Rig(#[from] RigError),
    #[error(transparent)]
    Asset(#[from] RigAssetError),
}

/// The rig of `symbol`, read back from the project; `None` when it has none.
pub fn find(document: &Document, symbol: SymbolId) -> Result<Option<Rig>, RigLibraryError> {
    document
        .rig_assets()
        .iter()
        .find(|asset| asset.symbol() == symbol)
        .map(|asset| parse(asset.payload()))
        .transpose()
}

/// A rig from JSON text, checked.
pub fn parse(text: &str) -> Result<Rig, RigLibraryError> {
    let rig: Rig =
        serde_json::from_str(text).map_err(|error| RigLibraryError::NotARig(error.to_string()))?;
    rig.validate()?;
    Ok(rig)
}

fn with(document: &Document, rig: &Rig) -> Result<Command, RigLibraryError> {
    rig.validate()?;
    let text =
        serde_json::to_string(rig).map_err(|error| RigLibraryError::NotARig(error.to_string()))?;
    let asset = RigAsset::new(rig.symbol, text)?;
    let mut assets = document.rig_assets().to_vec();
    assets.retain(|existing| existing.symbol() != rig.symbol);
    assets.push(asset);
    Ok(Command::SetRigAssets(assets))
}

fn kind_of(
    document: &Document,
    symbol: SymbolId,
) -> Result<flare_document::SymbolKind, RigLibraryError> {
    document
        .symbols()
        .iter()
        .find(|s| s.id() == symbol)
        .map(flare_document::Symbol::kind)
        .ok_or(RigLibraryError::NoSuchSymbol)
}

/// The command that gives `symbol` an empty rig, on the clock its kind gives it.
pub fn create(document: &Document, symbol: SymbolId) -> Result<Command, RigLibraryError> {
    let kind = kind_of(document, symbol)?;
    if find(document, symbol)?.is_some() {
        return Err(RigLibraryError::AlreadyRigged);
    }
    with(document, &Rig::new(symbol, kind))
}

/// The command that stores `rig` for its symbol, replacing the one there.
pub fn set(document: &Document, rig: &Rig) -> Result<Command, RigLibraryError> {
    kind_of(document, rig.symbol)?;
    with(document, rig)
}

/// The command that applies `edits` to the rig of `symbol`, all or none.
pub fn edit(
    document: &Document,
    symbol: SymbolId,
    edits: &[RigCommand],
) -> Result<Command, RigLibraryError> {
    let rig = find(document, symbol)?.ok_or(RigLibraryError::NoRig)?;
    with(document, &rig.apply(edits)?)
}

/// The command that takes the rig of `symbol` out of the project.
pub fn remove(document: &Document, symbol: SymbolId) -> Result<Command, RigLibraryError> {
    find(document, symbol)?.ok_or(RigLibraryError::NoRig)?;
    let mut assets = document.rig_assets().to_vec();
    assets.retain(|existing| existing.symbol() != symbol);
    Ok(Command::SetRigAssets(assets))
}

#[cfg(test)]
#[path = "rig_library_tests.rs"]
mod tests;
