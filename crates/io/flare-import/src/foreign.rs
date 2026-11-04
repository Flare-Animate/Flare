//! Import of other animation tools' files: Moho projects, BVH motion and Toonz scenes and levels.
//! The readers and converters live in `flare-moho` and `flare-toonz`; this only picks one by the
//! file's name (a Moho container and a `.fla` are both zip files, so the bytes cannot tell) and
//! turns their notes into the importer's diagnostics.

use std::io::Read;
use std::path::Path;

use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// Which foreign reader handles `path`, going by its extension.
pub fn foreign_kind(path: &Path) -> Option<&'static str> {
    if flare_moho::is_moho_path(path) {
        return Some("moho");
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "bvh" => Some("bvh"),
        "tnz" | "tpl" | "plt" | "pli" | "tlv" => Some("toonz"),
        _ => None,
    }
}

const MAX_BYTES: u64 = 64 * 1024 * 1024;

pub fn import_foreign_path(path: &Path) -> Result<Imported, ImportError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    import_foreign_bytes(path, bytes)
}

/// Imports a Moho, BVH or Toonz file held in memory; `name` says which.
pub fn import_foreign_bytes(name: &Path, bytes: Vec<u8>) -> Result<Imported, ImportError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ImportError::TooLarge("the file".into()));
    }
    let file = name
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let foreign = |e: &dyn std::fmt::Display| ImportError::Foreign(format!("{file}: {e}"));
    match foreign_kind(name) {
        Some("moho") => {
            let project = flare_moho::parse_bytes(&bytes, file).map_err(|e| foreign(&e))?;
            let out = flare_moho::to_flare_document(&project).map_err(|e| foreign(&e))?;
            Ok(from_moho(out))
        }
        Some("bvh") => {
            let text = String::from_utf8(bytes).map_err(|_| foreign(&"not UTF-8 text"))?;
            let parsed = flare_moho::bvh::parse(&text).map_err(|e| foreign(&e))?;
            let out = flare_moho::bvh::to_document(&parsed).map_err(|e| foreign(&e))?;
            Ok(from_moho(out))
        }
        Some("toonz") => crate::toonz::import(name, &bytes).map_err(|e| foreign(&e)),
        _ => Err(ImportError::NotXfl("unknown file type".into())),
    }
}

fn from_moho(out: flare_moho::Converted) -> Imported {
    Imported {
        document: out.document,
        diagnostics: out
            .notes
            .into_iter()
            .map(|n| Diagnostic {
                severity: match n.severity {
                    flare_moho::Severity::Info => Severity::Info,
                    flare_moho::Severity::Warning => Severity::Warning,
                    flare_moho::Severity::Unsupported => Severity::Unsupported,
                },
                location: n.location,
                message: n.message,
            })
            .collect(),
    }
}
