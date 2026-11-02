//! Keeping the file an import was read from, so that it can be written back unchanged.
//!
//! Reading a file never touches it: it is opened for reading only, and nothing is created beside
//! it. What was read is kept with the document ([`flare_document::Origin`]) together with a
//! fingerprint of the document the import made. When the source cannot be kept (it is too big,
//! a folder holds a symbolic link, something cannot be read) the import still succeeds and says
//! so in a diagnostic, because an export of it could then not be a copy of the original.

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use flare_core::SymbolId;
use flare_document::{
    Document, MAX_ORIGIN_BYTES, MAX_ORIGIN_FILES, Origin, OriginData, OriginFile, OriginKind,
    is_safe_entry_name,
};

use crate::xfl::{Diagnostic, Severity};

/// Folders deeper than this are not kept (and are not a real project).
const MAX_DEPTH: usize = 32;

/// Attaches the source to `document`, or explains in `diagnostics` why it could not be.
pub(crate) fn attach(
    document: Document,
    kind: OriginKind,
    name: &str,
    data: OriginData,
    symbol_files: &[(SymbolId, String)],
    diagnostics: &mut Vec<Diagnostic>,
) -> Document {
    let baseline = flare_storage::baseline(&document, |id| {
        symbol_files
            .iter()
            .find(|(s, _)| *s == id)
            .map(|(_, f)| f.clone())
    });
    let origin = baseline
        .map_err(|error| error.to_string())
        .and_then(|baseline| {
            Origin::new(kind, name, data, baseline).map_err(|error| error.to_string())
        });
    match origin {
        Ok(origin) => document.with_origin(Some(std::sync::Arc::new(origin))),
        Err(reason) => {
            not_kept(diagnostics, &reason);
            document
        }
    }
}

/// Records that the source was not kept, and why.
pub(crate) fn not_kept(diagnostics: &mut Vec<Diagnostic>, reason: &str) {
    diagnostics.push(Diagnostic {
        severity: Severity::Warning,
        location: "source".into(),
        message: format!(
            "the original file was not kept ({reason}); exporting this project again will rebuild it from what Flare understood, not copy the original"
        ),
    });
}

/// `name` without its extension, for the name of a kept source.
pub(crate) fn stem(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| s.len() <= flare_document::MAX_ORIGIN_NAME_BYTES && !s.contains(['/', '\\']))
        .unwrap_or("Document")
        .to_owned()
}

/// Every file and empty folder under `root`, by name relative to it, sorted. Nothing is followed
/// through a symbolic link: a folder that has one is refused, because what it points at is not
/// part of the folder.
pub(crate) fn read_folder(root: &Path) -> Result<Vec<OriginFile>, String> {
    let mut files = Vec::new();
    let mut total = 0_usize;
    walk(root, "", 0, &mut files, &mut total)?;
    files.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(files)
}

fn walk(
    dir: &Path,
    prefix: &str,
    depth: usize,
    files: &mut Vec<OriginFile>,
    total: &mut usize,
) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err("the folder is nested too deeply".into());
    }
    let listing = fs::read_dir(dir).map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
    let mut any = false;
    for entry in listing {
        let entry = entry.map_err(|e| format!("cannot list {}: {e}", dir.display()))?;
        any = true;
        let file_name = entry.file_name();
        let Some(file_name) = file_name.to_str() else {
            return Err("a file name is not valid text".into());
        };
        let name = format!("{prefix}{file_name}");
        let kind = entry
            .file_type()
            .map_err(|e| format!("cannot inspect {name}: {e}"))?;
        if kind.is_symlink() {
            return Err(format!("{name} is a symbolic link"));
        }
        if kind.is_dir() {
            walk(&entry.path(), &format!("{name}/"), depth + 1, files, total)?;
        } else if kind.is_file() {
            if !is_safe_entry_name(&name) {
                return Err(format!("{name:?} is not a safe file name"));
            }
            if files.len() >= MAX_ORIGIN_FILES {
                return Err(format!("it has more than {MAX_ORIGIN_FILES} files"));
            }
            let mut bytes = Vec::new();
            let room = MAX_ORIGIN_BYTES.saturating_sub(*total) as u64;
            fs::File::open(entry.path())
                .and_then(|f| f.take(room + 1).read_to_end(&mut bytes))
                .map_err(|e| format!("cannot read {name}: {e}"))?;
            *total += bytes.len();
            if *total > MAX_ORIGIN_BYTES {
                return Err(format!("it holds more than {MAX_ORIGIN_BYTES} bytes"));
            }
            files.push(OriginFile { name, bytes });
        } else {
            return Err(format!("{name} is not a plain file or folder"));
        }
    }
    if !any && !prefix.is_empty() {
        files.push(OriginFile {
            name: prefix.to_owned(),
            bytes: Vec::new(),
        });
    }
    Ok(())
}

/// Reads at most `MAX_ORIGIN_BYTES + 1` bytes of `reader`.
pub(crate) fn read_all_capped(reader: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(MAX_ORIGIN_BYTES as u64 + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}
