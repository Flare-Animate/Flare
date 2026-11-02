//! Writing a document to the file its name says.

use std::path::{Path, PathBuf};

use flare_document::Document;
use flare_export::{ExportError, Output, Severity};
use thiserror::Error;

use super::{FileFormat, xfl_folder};

/// Most file names an [`ExportSummary`] lists; the count is always whole.
pub const MAX_NAMED_FILES: usize = 50;

/// Why a document could not be saved. The old file is untouched in every case.
#[derive(Debug, Error)]
pub enum SaveError {
    #[error("cannot write {path}: {source}")]
    Project {
        path: PathBuf,
        source: flare_storage::StorageError,
    },
    #[error("cannot write {path}: {source}")]
    Export { path: PathBuf, source: ExportError },
    /// The file there is an Animate document from before CS5, which Flare reads but cannot write;
    /// a rebuilt copy would replace everything it holds.
    #[error("{0} is a pre-CS5 binary .fla, which Flare cannot write; save under another name")]
    LegacyFla(PathBuf),
    /// A movie is published, not saved.
    #[error("{0} is a movie; save as .fla, .xfl or .flare, or publish it")]
    Movie(PathBuf),
}

/// What a save to an Animate format wrote, from the export's report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportSummary {
    /// The file is the one the document was imported from, byte for byte.
    pub exact: bool,
    /// The export's own sentence for why (`unchanged since import`, `not imported from a file`).
    pub reason: String,
    /// Files in the document written, folders included.
    pub files: usize,
    /// Files Flare rebuilt from the document, by name (at most [`MAX_NAMED_FILES`]).
    pub rebuilt: Vec<String>,
    /// How many files Flare rebuilt, which is `rebuilt.len()` unless the list was cut short.
    pub rebuilt_count: usize,
    /// Files of the original written back as they were.
    pub carried: usize,
    /// Everything the export said, one line each: what the format cannot hold and what it changed.
    pub notes: Vec<String>,
    /// How many of the notes are warnings (a change in meaning), the rest being information.
    pub warnings: usize,
}

/// A finished save.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    /// The file the document now lives at. For `.xfl` this is the stub inside the folder that was
    /// written, which is not always the name that was asked for.
    pub path: PathBuf,
    pub format: FileFormat,
    /// The export's report, for `.fla` and `.xfl`.
    pub summary: Option<ExportSummary>,
}

/// Turns an export's report into what a person is told.
pub fn summarize(output: &Output) -> ExportSummary {
    let rebuilt_count = output.files.len();
    ExportSummary {
        exact: output.lossless.exact,
        reason: output.lossless.reason.clone(),
        files: output.file_count(),
        rebuilt: output
            .files
            .iter()
            .take(MAX_NAMED_FILES)
            .map(|(name, _)| name.clone())
            .collect(),
        rebuilt_count,
        carried: output.kept.len(),
        notes: output
            .diagnostics
            .iter()
            .map(|d| {
                let kind = match d.severity {
                    Severity::Info => "Info",
                    Severity::Warning => "Warning",
                };
                format!("{kind} {}: {}", d.location, d.message)
            })
            .collect(),
        warnings: output
            .diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Warning)
            .count(),
    }
}

/// Writes `document` to `path` in the format its name says (`.fla`, `.xfl` or `.flare`; anything
/// else is a `.flare` project, as a path that was opened is saved back to). The file is replaced
/// only by a complete new one, so a failed save leaves the old one whole.
///
/// A `.fla` or `.xfl` made from an import is the original file when nothing changed and the
/// original with only the edited parts rebuilt otherwise (see `flare-export`).
pub fn save(document: &Document, path: &Path) -> Result<Saved, SaveError> {
    let format = if path.is_dir() {
        FileFormat::Xfl
    } else {
        FileFormat::of_path(path).unwrap_or(FileFormat::Flare)
    };
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("swf"))
    {
        return Err(SaveError::Movie(path.to_owned()));
    }
    let export = |source| SaveError::Export {
        path: path.to_owned(),
        source,
    };
    match format {
        FileFormat::Flare => {
            flare_storage::save(document, path).map_err(|source| SaveError::Project {
                path: path.to_owned(),
                source,
            })?;
            Ok(Saved {
                path: path.to_owned(),
                format,
                summary: None,
            })
        }
        FileFormat::Fla => {
            if is_legacy_fla(path) {
                return Err(SaveError::LegacyFla(path.to_owned()));
            }
            let output = flare_export::export_xfl(document).map_err(export)?;
            flare_export::replace_fla(&output, path).map_err(export)?;
            Ok(Saved {
                path: path.to_owned(),
                format,
                summary: Some(summarize(&output)),
            })
        }
        FileFormat::Xfl => {
            let mut output = flare_export::export_xfl(document).map_err(export)?;
            let folder = xfl_folder(path);
            // The stub is named for the document, and for a new one that is the name typed.
            let stem = if path.is_dir() || FileFormat::of_path(path).is_none() {
                folder.file_name()
            } else {
                path.file_stem()
            };
            if let Some(stem) = stem {
                output.name = stem.to_string_lossy().into_owned();
            }
            flare_export::replace_directory(&output, &folder).map_err(export)?;
            Ok(Saved {
                path: stub_of(&folder, &output.name),
                format,
                summary: Some(summarize(&output)),
            })
        }
    }
}

/// The `.xfl` file inside `folder` that stands for it: the one named `name` if there, else the
/// first at its root (a folder that came with another name keeps it), else its document file.
fn stub_of(folder: &Path, name: &str) -> PathBuf {
    let own = folder.join(format!("{name}.xfl"));
    if own.is_file() {
        return own;
    }
    let mut stubs: Vec<PathBuf> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|p| p.is_file())
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("xfl")))
        .collect();
    stubs.sort();
    stubs
        .into_iter()
        .next()
        .unwrap_or_else(|| folder.join(flare_import::XFL_DOCUMENT_FILE))
}

/// Whether the file at `path` is a binary `.fla` from before CS5.
fn is_legacy_fla(path: &Path) -> bool {
    use std::io::Read;
    let mut head = [0_u8; 8];
    std::fs::File::open(path)
        .and_then(|mut file| file.read_exact(&mut head))
        .is_ok()
        && head == flare_import::legacy::CFB_MAGIC
}
