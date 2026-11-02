//! The files of a kept source, one by one.

use std::io::{Cursor, Read};

use flare_document::{
    MAX_ORIGIN_BYTES, MAX_ORIGIN_FILES, Origin, OriginData, OriginFile, is_safe_entry_name,
};

/// The files of a source, and what could not be listed.
pub(crate) struct Entries {
    pub(crate) files: Vec<OriginFile>,
    /// Entries that were left out, and why. Each one is a reason the export is not a copy.
    pub(crate) skipped: Vec<String>,
}

/// The entries of the archive or folder `origin` holds, in their original order. An archive is
/// untrusted: entry names must stay inside the document, and what is read is bounded by the
/// same limit as the source itself, whatever sizes the archive declares.
pub(crate) fn entries_of(origin: &Origin) -> Entries {
    match origin.data() {
        OriginData::Files(files) => Entries {
            files: files.clone(),
            skipped: Vec::new(),
        },
        OriginData::Bytes(bytes) => zip_entries(bytes),
    }
}

fn zip_entries(bytes: &[u8]) -> Entries {
    let mut out = Entries {
        files: Vec::new(),
        skipped: Vec::new(),
    };
    let mut archive = match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(archive) => archive,
        Err(error) => {
            out.skipped
                .push(format!("the archive cannot be read again: {error}"));
            return out;
        }
    };
    let mut budget = MAX_ORIGIN_BYTES as u64;
    let mut seen = std::collections::HashSet::new();
    for index in 0..archive.len() {
        if out.files.len() >= MAX_ORIGIN_FILES {
            out.skipped
                .push(format!("more than {MAX_ORIGIN_FILES} entries"));
            break;
        }
        let mut entry = match archive.by_index(index) {
            Ok(entry) => entry,
            Err(error) => {
                out.skipped
                    .push(format!("entry {index} cannot be read: {error}"));
                continue;
            }
        };
        let name = match entry.name() {
            Ok(name) => name.into_owned(),
            Err(error) => {
                out.skipped.push(format!(
                    "entry {index} has a name that is not text: {error}"
                ));
                continue;
            }
        };
        if !is_safe_entry_name(&name) {
            out.skipped.push(format!("{name:?} is not a safe name"));
            continue;
        }
        if !seen.insert(name.clone()) {
            out.skipped.push(format!("{name:?} appears twice"));
            continue;
        }
        let mut data = Vec::new();
        if !entry.is_dir() {
            let read = (&mut entry).take(budget + 1).read_to_end(&mut data);
            if let Err(error) = read {
                out.skipped
                    .push(format!("{name:?} cannot be read: {error}"));
                continue;
            }
            if data.len() as u64 > budget {
                out.skipped
                    .push(format!("{name:?} and the rest are beyond the size limit"));
                break;
            }
            budget -= data.len() as u64;
        }
        out.files.push(OriginFile { name, bytes: data });
    }
    out
}
