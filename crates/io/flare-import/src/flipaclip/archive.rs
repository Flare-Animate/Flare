//! The zip container of a FlipaClip project. Entries have no extensions: `meta`, `bg` (the
//! background, optional) and `<layer>/<frame>` for each drawn frame. Names are checked as paths
//! before anything else, so a name that would leave the project is refused rather than skipped.

use std::io::{Cursor, Read};

use super::error::FlipaError;
use super::limits::{MAX_ENTRIES, MAX_ENTRY_BYTES, MAX_META_BYTES, MAX_TOTAL_ENTRY_BYTES};

/// One drawn frame of one layer, as stored.
pub(super) struct Picture {
    pub name: String,
    pub layer: u32,
    pub frame: u32,
    pub data: Vec<u8>,
}

/// Everything the importer reads from the container; `unsupported` names what is not read.
pub(super) struct Archive {
    pub meta: Vec<u8>,
    pub background: Option<Vec<u8>>,
    pub pictures: Vec<Picture>,
    pub unsupported: Vec<String>,
}

enum Kind {
    Meta,
    Background,
    Picture(u32, u32),
    Other,
}

pub(super) fn read(bytes: &[u8]) -> Result<Archive, FlipaError> {
    let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|_| FlipaError::NotZip)?;
    if zip.len() > MAX_ENTRIES {
        return Err(FlipaError::TooLarge("the project's entry count"));
    }
    // The zip crate keeps one entry per name, so a repeated name is seen here and nowhere else.
    if zip.has_duplicate_names() {
        return Err(FlipaError::DuplicateEntry(
            "(a name listed twice)".to_owned(),
        ));
    }
    let mut meta = None;
    let mut out = Archive {
        meta: Vec::new(),
        background: None,
        pictures: Vec::new(),
        unsupported: Vec::new(),
    };
    let mut total = 0_u64;
    for index in 0..zip.len() {
        let (name, size, is_dir) = {
            let entry = zip.by_index(index).map_err(|_| FlipaError::NotZip)?;
            let listed = entry.name().map(|n| n.into_owned());
            let size = entry.size();
            let is_dir = entry.is_dir();
            let name = listed.map_err(|_| FlipaError::UnsafeName("(not UTF-8)".to_owned()))?;
            (name, size, is_dir)
        };
        check_name(&name)?;
        if is_dir {
            continue;
        }
        if size > MAX_ENTRY_BYTES {
            return Err(FlipaError::TooLarge("an entry"));
        }
        let mut file = zip.by_index(index).map_err(|_| FlipaError::NotZip)?;
        let limit = match classify(&name) {
            Kind::Meta => MAX_META_BYTES,
            _ => MAX_ENTRY_BYTES,
        };
        let data = read_capped(&mut file, &name, limit)?;
        total += data.len() as u64;
        if total > MAX_TOTAL_ENTRY_BYTES {
            return Err(FlipaError::TooLarge("the project's entries together"));
        }
        match classify(&name) {
            Kind::Meta => meta = Some(data),
            Kind::Background => out.background = Some(data),
            Kind::Picture(layer, frame) => out.pictures.push(Picture {
                name: shorten(&name),
                layer,
                frame,
                data,
            }),
            Kind::Other => out.unsupported.push(shorten(&name)),
        }
    }
    out.meta = meta.ok_or(FlipaError::NoMeta)?;
    Ok(out)
}

/// Refuses a name that is absolute, has a drive or backslash, is empty, or has a `.`, `..` or
/// empty part. Directory entries may end with `/`; nothing else may.
fn check_name(name: &str) -> Result<(), FlipaError> {
    let parts_ok = name
        .trim_end_matches('/')
        .split('/')
        .all(|part| !part.is_empty() && part != "." && part != "..");
    let unsafe_char = name
        .chars()
        .any(|c| c == '\\' || c == ':' || c.is_control());
    if !parts_ok || unsafe_char || name.starts_with('/') || name.len() > 256 {
        return Err(FlipaError::UnsafeName(shorten(name)));
    }
    Ok(())
}

fn classify(name: &str) -> Kind {
    match name {
        "meta" => Kind::Meta,
        "bg" => Kind::Background,
        _ => match name.split_once('/') {
            Some((layer, frame)) if digits(layer) && digits(frame) => {
                match (layer.parse::<u32>(), frame.parse::<u32>()) {
                    (Ok(layer), Ok(frame)) => Kind::Picture(layer, frame),
                    _ => Kind::Other,
                }
            }
            _ => Kind::Other,
        },
    }
}

/// Decimal digits, at most six, so a number always fits and a name stays short.
fn digits(text: &str) -> bool {
    !text.is_empty() && text.len() <= 6 && text.bytes().all(|b| b.is_ascii_digit())
}

fn read_capped(file: &mut impl Read, name: &str, limit: u64) -> Result<Vec<u8>, FlipaError> {
    let mut data = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut data)
        .map_err(|_| FlipaError::CorruptEntry(shorten(name)))?;
    if data.len() as u64 > limit {
        return Err(FlipaError::TooLarge("an entry"));
    }
    Ok(data)
}

/// A name for a message: at most 64 characters, with control characters left out.
pub(super) fn shorten(name: &str) -> String {
    name.chars().filter(|c| !c.is_control()).take(64).collect()
}
