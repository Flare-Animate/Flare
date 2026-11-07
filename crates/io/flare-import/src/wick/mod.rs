//! Wick Editor projects (`.wick`) as Flare projects. A `.wick` file is a zip archive: `project.json`
//! holds the project and a table of objects (clips, buttons, layers, frames, paths, tweens, and
//! their scripts), and `assets/<uuid>.<ext>` holds each image, sound and font. What the layout is,
//! and what is assumed rather than seen, is in docs/WICK.md. Sounds, fonts and text are reported as
//! not imported; scripts are kept as text and not run.

mod archive;
mod build;
mod color;
mod ctx;
mod easing;
mod error;
mod json;
mod library;
mod limits;
mod paths;
#[cfg(test)]
mod refusals;
#[cfg(test)]
mod tests;
mod timeline;

use crate::xfl::{ImportError, Imported};
use error::WickError;

/// Imports a Wick project from its bytes. `file` names it in the messages.
pub(crate) fn import(file: &str, bytes: &[u8]) -> Result<Imported, ImportError> {
    let wrap = |e: WickError| ImportError::Foreign(format!("{file}: {e}"));
    refuse_other_layouts(bytes).map_err(wrap)?;
    let archive = archive::read(bytes).map_err(wrap)?;
    let parsed = json::parse(&archive.project).map_err(wrap)?;
    let (document, diagnostics) = build::build(&parsed, &archive.assets).map_err(wrap)?;
    Ok(Imported {
        document,
        diagnostics,
    })
}

/// Whether `bytes` is a zip that holds a `project.json`, which no Animate file does. An XFL folder
/// or a `.fla` has no such entry, so this tells a Wick project apart without its name.
pub(crate) fn looks_like_wick(bytes: &[u8]) -> bool {
    zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .is_ok_and(|zip| zip.index_for_name("project.json").is_some())
}

/// Gzip and the plain JSON of Wick's pre-1.0.9 projects are named as what they are, rather than
/// read as a zip and reported as corrupt.
fn refuse_other_layouts(bytes: &[u8]) -> Result<(), WickError> {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return Err(WickError::Gzip);
    }
    if bytes.iter().find(|b| !b.is_ascii_whitespace()) == Some(&b'{') {
        return Err(WickError::LegacyJson);
    }
    Ok(())
}
