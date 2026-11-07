//! The zip container of a Wick project: `project.json` and `assets/<uuid>.<ext>` for each image,
//! sound and font. Names are checked as paths before anything else, so a name that would leave the
//! project is refused rather than skipped.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use super::error::WickError;
use super::limits::{MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_PROJECT_BYTES, MAX_TOTAL_ENTRY_BYTES};

/// What the importer reads from the container: the project text and the asset files by name
/// (`<uuid>.<ext>`, the path under `assets/` without the folder).
pub(super) struct Archive {
    pub project: Vec<u8>,
    pub assets: HashMap<String, Vec<u8>>,
}

pub(super) fn read(bytes: &[u8]) -> Result<Archive, WickError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| WickError::NotZip)?;
    if zip.len() > MAX_ENTRIES {
        return Err(WickError::TooLarge("the project's entry count"));
    }
    // The zip crate keeps one entry per name, so a repeated name is seen here and nowhere else.
    if zip.has_duplicate_names() {
        return Err(WickError::DuplicateEntry(
            "(a name listed twice)".to_owned(),
        ));
    }
    let mut project = None;
    let mut assets = HashMap::new();
    let mut total = 0_u64;
    for index in 0..zip.len() {
        let (name, size, is_dir) = {
            let entry = zip.by_index(index).map_err(|_| WickError::NotZip)?;
            let listed = entry.name().map(|n| n.into_owned());
            let size = entry.size();
            let is_dir = entry.is_dir();
            let name = listed.map_err(|_| WickError::UnsafeName("(not UTF-8)".to_owned()))?;
            (name, size, is_dir)
        };
        check_name(&name)?;
        if is_dir {
            continue;
        }
        let limit = if name == "project.json" {
            MAX_PROJECT_BYTES
        } else {
            MAX_ENTRY_BYTES
        };
        if size > limit {
            return Err(WickError::TooLarge("an entry"));
        }
        let wanted = name == "project.json" || name.starts_with("assets/");
        if !wanted {
            continue;
        }
        let mut file = zip.by_index(index).map_err(|_| WickError::NotZip)?;
        let data = read_capped(&mut file, &name, limit)?;
        total += data.len() as u64;
        if total > MAX_TOTAL_ENTRY_BYTES {
            return Err(WickError::TooLarge("the project's entries together"));
        }
        if name == "project.json" {
            project = Some(data);
        } else if let Some(asset) = name.strip_prefix("assets/").filter(|a| !a.contains('/')) {
            assets.insert(asset.to_owned(), data);
        }
    }
    Ok(Archive {
        project: project.ok_or(WickError::NoProject)?,
        assets,
    })
}

/// Refuses a name that is absolute, has a drive or backslash, is empty, or has a `.`, `..` or
/// empty part. Directory entries may end with `/`; nothing else may.
fn check_name(name: &str) -> Result<(), WickError> {
    let parts_ok = name
        .trim_end_matches('/')
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..");
    let unsafe_char = name
        .chars()
        .any(|c| c == '\\' || c == ':' || c.is_control());
    if !parts_ok || unsafe_char || name.starts_with('/') || name.len() > 256 {
        return Err(WickError::UnsafeName(shorten(name)));
    }
    Ok(())
}

fn read_capped(file: &mut impl Read, name: &str, limit: u64) -> Result<Vec<u8>, WickError> {
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|_| WickError::CorruptEntry(shorten(name)))?;
    if data.len() as u64 > limit {
        return Err(WickError::TooLarge("an entry"));
    }
    Ok(data)
}

/// A name for a message: at most 64 characters, with control characters left out.
fn shorten(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(64).collect()
}
