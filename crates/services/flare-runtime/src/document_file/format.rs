//! Which format a name means.

use std::path::{Path, PathBuf};

use flare_import::XFL_DOCUMENT_FILE;

/// A format a document can be saved in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileFormat {
    /// A zipped Animate document, `.fla`. The default of Save As, as in Animate.
    Fla,
    /// An uncompressed Animate document: a folder holding `DOMDocument.xml`, the library and a
    /// `.xfl` stub file that names it.
    Xfl,
    /// Flare's own project file.
    Flare,
}

impl FileFormat {
    /// What Save As writes when the name says nothing.
    pub const DEFAULT: Self = Self::Fla;

    /// Every format, in the order a Save As list offers them.
    pub const ALL: [Self; 3] = [Self::Fla, Self::Xfl, Self::Flare];

    /// The file name ending, without the dot.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Fla => "fla",
            Self::Xfl => "xfl",
            Self::Flare => "flare",
        }
    }

    /// The format `path` names, by its last extension in any case, or `None`. The document file of
    /// an XFL folder names its format too. Only the last extension counts: `a.flare.fla` is a
    /// `.fla`.
    pub fn of_path(path: &Path) -> Option<Self> {
        if is_document_file(path) {
            return Some(Self::Xfl);
        }
        let extension = path.extension()?.to_str()?;
        Self::ALL
            .into_iter()
            .find(|format| extension.eq_ignore_ascii_case(format.extension()))
    }
}

fn is_document_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(XFL_DOCUMENT_FILE))
}

/// The name Save As writes for `typed`, and its format. A name that says a format keeps it; a name
/// that says none (no extension, or one that is not a document's such as `drawing.v2` or `.swf`)
/// gets `.fla` added rather than being guessed at, so nothing is ever written under a name that
/// would be opened as something else.
pub fn resolve_save_path(typed: &Path) -> (PathBuf, FileFormat) {
    match FileFormat::of_path(typed) {
        Some(format) => (typed.to_owned(), format),
        None => {
            let mut name = typed.as_os_str().to_owned();
            name.push(".");
            name.push(FileFormat::DEFAULT.extension());
            (PathBuf::from(name), FileFormat::DEFAULT)
        }
    }
}

/// The folder an `.xfl` document lives in, for `path` as a document is opened or saved:
///
/// - a folder is itself;
/// - a `DOMDocument.xml` belongs to the folder it is in;
/// - an `.xfl` stub that exists, in a folder that holds a `DOMDocument.xml`, belongs to that folder
///   (it is the document being saved again);
/// - any other `.xfl` is a new document, which is a folder named after the file, beside it, with
///   the stub inside (`dir/a.xfl` is saved as `dir/a/a.xfl`). Animate does the same: the stub
///   never stands alone beside its folder. This also means saving a new document next to another
///   XFL document cannot overwrite that one.
pub fn xfl_folder(path: &Path) -> PathBuf {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if path.is_dir() {
        return path.to_owned();
    }
    if is_document_file(path) || (path.is_file() && parent.join(XFL_DOCUMENT_FILE).is_file()) {
        return parent.to_owned();
    }
    let stem = path.file_stem().unwrap_or(path.as_os_str());
    parent.join(stem)
}
