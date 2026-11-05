//! Nintendo Flipnote Studio files as Flare projects. DSi `.ppm` files are read; 3DS `.kwz` files
//! are recognised by their magic and refused by name until their reader lands. The RSA signature
//! of either is not checked: a file that was edited outside Flipnote Studio imports as readily as
//! one made in it. See docs/FLIPNOTE.md for the layouts and what is verified.

mod convert;
pub mod error;
pub mod kwz;
pub mod ppm;
pub(crate) mod reader;
#[cfg(test)]
mod tests;

use crate::xfl::{ImportError, Imported};

/// Whether `bytes` start with the PPM magic, `PARA`.
pub fn looks_like_ppm(bytes: &[u8]) -> bool {
    bytes.starts_with(ppm::header::MAGIC)
}

/// Whether `bytes` start with the KWZ section magic, `KFH`. The fourth byte varies between files
/// (real samples carry 0x14 there), so only the three letters are compared.
pub fn looks_like_kwz(bytes: &[u8]) -> bool {
    bytes.starts_with(b"KFH")
}

/// Imports a Flipnote file held in memory. `name` is used for the document and the messages.
pub(crate) fn import(name: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let fail = |e: &dyn std::fmt::Display| ImportError::Foreign(format!("{name}: {e}"));
    if looks_like_kwz(bytes) {
        let parsed = kwz::Kwz::parse(bytes).map_err(|e| fail(&e))?;
        return kwz::convert::convert(&parsed, name);
    }
    let parsed = ppm::Ppm::parse(bytes).map_err(|e| fail(&e))?;
    convert::convert(&parsed, name)
}
