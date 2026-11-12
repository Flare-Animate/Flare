//! Reading a document from the file its name says.

use std::path::{Path, PathBuf};

use flare_document::{Document, OriginKind};

use flare_import::{ImportError, PathKind, XFL_DOCUMENT_FILE};
use flare_storage::StorageError;
use thiserror::Error;

use super::FileFormat;

/// Why a file could not be read.
#[derive(Debug, Error)]
pub enum LoadError {
    #[error("{0}")]
    Project(#[from] StorageError),
    #[error("{0}")]
    Import(#[from] ImportError),
}

/// A document read from a file.
#[derive(Debug)]
pub struct Loaded {
    pub document: Document,
    /// The file the document is at, which is the one asked for except for an XFL folder, which is
    /// its `.xfl` stub (or its `DOMDocument.xml` if it has none).
    pub path: PathBuf,
    /// Whether the document lives in this file and Save writes it back there: a `.flare`, and a
    /// `.fla` or XFL document whose original Flare kept. A movie, a pre-CS5 binary `.fla` (which
    /// Flare cannot write) and an Animate file too big to keep are only imported, and saving
    /// asks for a name rather than replacing them with a rebuilt copy.
    pub backed: bool,
    /// What the import could not carry over, one line each; empty for a project.
    pub notes: Vec<String>,
}

/// Reads `path`: a `.flare` project, or an Animate `.fla`, `.xfl` or XFL folder, or anything else
/// the importer reads (a movie, a picture, a video, another program's project), which is
/// imported. The importer's own list decides, so File > Open takes every kind it offers. A name
/// that says nothing is a project, as it always was.
pub fn load(path: &Path) -> Result<Loaded, LoadError> {
    let format = if path.is_dir() {
        Some(FileFormat::Xfl)
    } else {
        FileFormat::of_path(path)
    };
    if format.is_none() && !only_imported(path) || format == Some(FileFormat::Flare) {
        return Ok(Loaded {
            document: flare_storage::load(path)?,
            path: path.to_owned(),
            backed: true,
            notes: Vec::new(),
        });
    }
    let imported = flare_import::import_path(path)?;
    let backed = saves_in_place(&imported.document, path);
    let notes = imported
        .diagnostics
        .iter()
        .map(|d| format!("{:?} {}: {}", d.severity, d.location, d.message))
        .collect();
    let path = if backed && format == Some(FileFormat::Xfl) {
        xfl_document_path(path)
    } else {
        path.to_owned()
    };
    Ok(Loaded {
        document: imported.document,
        path,
        backed,
        notes,
    })
}

/// The file that stands for the XFL document `path` was opened through: its `.xfl` stub if the
/// folder has one, so a folder, its stub and its `DOMDocument.xml` all open as the same name.
fn xfl_document_path(path: &Path) -> PathBuf {
    let folder = if path.is_dir() {
        path
    } else {
        path.parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
    };
    let mut stubs: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xfl")))
        .collect();
    stubs.sort();
    if path.is_file() && stubs.contains(&path.to_owned()) {
        return path.to_owned();
    }
    stubs
        .into_iter()
        .next()
        .unwrap_or_else(|| folder.join(XFL_DOCUMENT_FILE))
}

/// Whether `path` names a file Flare reads only by importing it and never writes: a movie, a
/// picture, a video or another program's project (anything the importer takes that is not an
/// Animate document).
pub(crate) fn only_imported(path: &Path) -> bool {
    FileFormat::of_path(path).is_none()
        && !path.is_dir()
        && flare_import::classify(path, false) == Some(PathKind::Import)
}

/// Whether saving `document` to `path` writes back the file it was read from rather than
/// replacing something Flare did not keep: a project always, an Animate document when the
/// import kept the original of that kind (so unknown files survive), never a movie, a picture or
/// anything else only imported, or a file Flare could not keep whole (a pre-CS5 `.fla`, one
/// beyond the size bound).
pub fn saves_in_place(document: &Document, path: &Path) -> bool {
    let format = if path.is_dir() {
        Some(FileFormat::Xfl)
    } else {
        FileFormat::of_path(path)
    };
    let kind = document.origin().map(|origin| origin.kind());
    match format {
        Some(FileFormat::Flare) => true,
        Some(FileFormat::Fla) => kind == Some(OriginKind::Fla),
        Some(FileFormat::Xfl) => kind == Some(OriginKind::Xfl),
        None => !only_imported(path),
    }
}
