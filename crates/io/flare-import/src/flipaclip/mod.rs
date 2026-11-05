//! FlipaClip projects (`.fc`) as Flare projects. A `.fc` file is a zip archive whose entries have
//! no extensions: `meta` (JSON), `bg` (the background, optional) and `<layer>/<frame>` (one
//! zstd-compressed RGBA picture per drawn frame). What the layout is, and what is assumed rather
//! than seen, is in docs/FLIPACLIP.md. Sound and fonts are not read; they are named as unsupported.

mod archive;
mod build;
mod error;
mod frame;
mod limits;
mod meta;
#[cfg(test)]
mod tests;

use crate::xfl::{ImportError, Imported};

/// Imports a FlipaClip project from its bytes. `file` names it in the messages.
pub(crate) fn import(file: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let wrap = |e: error::FlipaError| ImportError::Foreign(format!("{file}: {e}"));
    let archive = archive::read(bytes).map_err(wrap)?;
    let meta = meta::parse(&archive.meta).map_err(wrap)?;
    let (document, diagnostics) = build::build(&meta, archive)?;
    Ok(Imported {
        document,
        diagnostics,
    })
}
