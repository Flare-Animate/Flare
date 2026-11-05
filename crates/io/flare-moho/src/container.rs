//! Finds the project document: a ZIP holding `Project.mohoproj` (or `Project.animeproj`), or a
//! bare JSON document. The container is decided from the bytes, never from the extension.

use std::io::{Cursor, Read};
use std::path::Path;

use zip::result::ZipError;

use crate::error::MohoError;
use crate::limits::{MAX_DOCUMENT_BYTES, MAX_FILE_BYTES};
use crate::model::Project;
use crate::parse::parse_document;

/// What the bytes are.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// A ZIP archive with the document inside.
    Zip,
    /// The document itself (`.mohoproj`, `.animeproj`, Moho 13 and later).
    RawJson,
}

const DOCUMENT_NAMES: [&str; 2] = ["Project.mohoproj", "Project.animeproj"];
/// Pre-11 `.anme` files are plain text that starts with this phrase.
const LEGACY_HEADER: &[u8] = b"Anime Studio Project";
pub(crate) const UTF8_BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
const EXTENSIONS: [&str; 5] = ["moho", "anime", "mohoproj", "animeproj", "anme"];

/// Whether the file name has a Moho extension. Only a hint for the caller; [`detect`] decides
/// what the bytes are.
pub fn is_moho_path(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// Identifies the container from the leading bytes. A binary file (a SWF, a PNG) is refused as
/// not Moho; only the legacy phrase, at the very start, makes a file legacy.
pub fn detect(bytes: &[u8]) -> Result<Container, MohoError> {
    if bytes.len() >= 4 && bytes[0] == b'P' && bytes[1] == b'K' && matches!(bytes[2], 3 | 5 | 7) {
        return Ok(Container::Zip);
    }
    let head = &bytes[..bytes.len().min(64)];
    if find(head, LEGACY_HEADER)
        .is_some_and(|at| at < 16 && head[..at].iter().all(u8::is_ascii_whitespace))
    {
        return Err(MohoError::LegacyFormat);
    }
    let body = head.strip_prefix(&UTF8_BOM).unwrap_or(head);
    match body.iter().find(|b| !b.is_ascii_whitespace()).copied() {
        Some(b'{') => Ok(Container::RawJson),
        _ => Err(MohoError::NotMoho),
    }
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Reads a project file, refusing anything over [`MAX_FILE_BYTES`] before it is parsed.
pub fn read_file(path: &Path) -> Result<Project, MohoError> {
    let file = std::fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    let entry = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    parse_bytes(&bytes, entry)
}

/// Parses a project from memory. `entry` names the document when it is not inside a ZIP.
pub fn parse_bytes(bytes: &[u8], entry: &str) -> Result<Project, MohoError> {
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > MAX_FILE_BYTES {
        return Err(MohoError::TooLarge {
            what: "the project file",
            size,
            limit: MAX_FILE_BYTES,
        });
    }
    match detect(bytes)? {
        Container::Zip => {
            let (name, document) = unzip(bytes)?;
            parse_document(&document, name)
        }
        Container::RawJson => parse_document(bytes, entry.to_owned()),
    }
}

/// Returns the document member's name and its bytes, read no further than the document limit.
fn unzip(bytes: &[u8]) -> Result<(String, Vec<u8>), MohoError> {
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes))?;
    for name in DOCUMENT_NAMES {
        let mut entry = match archive.by_name(name) {
            Ok(entry) => entry,
            Err(ZipError::FileNotFound) => continue,
            Err(e) => return Err(e.into()),
        };
        if entry.size() > MAX_DOCUMENT_BYTES {
            return Err(MohoError::TooLarge {
                what: "the project document",
                size: entry.size(),
                limit: MAX_DOCUMENT_BYTES,
            });
        }
        let mut document = Vec::new();
        entry
            .by_ref()
            .take(MAX_DOCUMENT_BYTES + 1)
            .read_to_end(&mut document)?;
        return Ok((name.to_owned(), document));
    }
    Err(MohoError::MissingDocument)
}
