//! What can be read from a legacy `.fla`'s `Contents` stream: the document properties and the
//! library table.
//!
//! The stream is a serialized object graph (Microsoft MFC style: length-prefixed strings, class
//! declarations as `FF FF`, back-references as `xx 80`). Nothing here walks that graph. The
//! properties are found by the fixed-size record they sit in and the library items by the stream
//! names they carry, and each is checked on every file it was tried on (see the tests). Anything
//! that does not fit is left out and reported, not guessed.

use std::collections::HashMap;

use super::layout::{StreamKind, classify};

/// Bytes from the start of the stage rectangle to the marker that ends the properties.
const MARKER_OFFSET: usize = 67;
/// The marker: a `3` and the number 180, which sits right after the properties in every file seen.
const MARKER: [u8; 5] = [3, 0xB4, 0, 0, 0];
/// Twips in a pixel.
const TWIPS: u32 = 20;
const MAX_ITEM_NAME_UNITS: usize = 255;

/// The document-wide settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentProps {
    pub width: u32,
    pub height: u32,
    pub frame_rate: f64,
    /// Red, green, blue. Only white and black have been checked against a movie.
    pub background: [u8; 3],
}

/// Why the properties could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropsError {
    /// The properties record was not found where it is in the files checked.
    NotFound,
    /// It was found, but a value is out of range.
    OutOfRange(&'static str),
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_le_bytes(
        data.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}

/// The most of `Contents` searched for the properties; they are near its start.
const SEARCH_LIMIT: usize = 1024 * 1024;

pub fn properties(contents: &[u8], max_side: u32) -> Result<DocumentProps, PropsError> {
    let window = contents.get(..SEARCH_LIMIT).unwrap_or(contents);
    let mut error = PropsError::NotFound;
    for (offset, candidate) in window.windows(MARKER.len()).enumerate() {
        if candidate != MARKER || offset < MARKER_OFFSET {
            continue;
        }
        let start = offset - MARKER_OFFSET;
        // The stage rectangle: left, right, top, bottom in twips.
        let (Some(x0), Some(x1), Some(y0), Some(y1)) = (
            u32_at(contents, start),
            u32_at(contents, start + 4),
            u32_at(contents, start + 8),
            u32_at(contents, start + 12),
        ) else {
            continue;
        };
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let side = |low: u32, high: u32| (high - low + TWIPS / 2) / TWIPS;
        let (width, height) = (side(x0, x1), side(y0, y1));
        if width == 0 || height == 0 || width > max_side || height > max_side {
            error = PropsError::OutOfRange("the stage size");
            continue;
        }
        // The frame rate is a 16.16 fixed-point number.
        let Some(rate) = u32_at(contents, start + 61) else {
            continue;
        };
        let frame_rate = f64::from(rate) / 65536.0;
        if !(frame_rate > 0.0 && frame_rate <= 1000.0) {
            error = PropsError::OutOfRange("the frame rate");
            continue;
        }
        let colour = contents
            .get(start + 53..start + 56)
            .unwrap_or(&[255, 255, 255]);
        return Ok(DocumentProps {
            width,
            height,
            frame_rate,
            background: [colour[0], colour[1], colour[2]],
        });
    }
    Err(error)
}

/// What a library item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    Graphic,
    MovieClip,
    Button,
    Bitmap,
    Sound,
    Video,
    /// A media item whose class could not be named.
    OtherMedia,
    /// A symbol whose kind was not recognised, or an item not found in `Contents`.
    Unknown,
}

impl ItemKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Graphic => "graphic",
            Self::MovieClip => "movie clip",
            Self::Button => "button",
            Self::Bitmap => "bitmap",
            Self::Sound => "sound",
            Self::Video => "video",
            Self::OtherMedia => "media",
            Self::Unknown => "unknown",
        }
    }
}

/// One entry of the library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryItem {
    /// The stream that holds the item's content, such as `Symbol 1`.
    pub stream: String,
    /// The name shown in the library. Folders are not read, so this is the bare name.
    pub name: String,
    pub kind: ItemKind,
    /// Whether the entry was found in `Contents`. Otherwise `name` is the stream's name.
    pub found: bool,
}

/// A length-prefixed name at `at`: one byte of length, then ASCII, or UTF-16 for newer files.
fn name_at(data: &[u8], at: usize) -> Option<(String, bool, usize)> {
    let len = usize::from(*data.get(at)?);
    if len == 0 {
        return None;
    }
    let body = data.get(at + 1..)?;
    if let Some(text) = body.get(..len)
        && text.iter().all(|b| (0x20..0x7F).contains(b))
    {
        return Some((
            text.iter().map(|&b| char::from(b)).collect(),
            false,
            at + 1 + len,
        ));
    }
    let wide = body.get(..len * 2)?;
    let pairs = wide.as_chunks::<2>().0;
    if pairs
        .iter()
        .all(|c| c[1] == 0 && (0x20..0x7F).contains(&c[0]))
    {
        let text = pairs.iter().map(|c| char::from(c[0])).collect();
        return Some((text, true, at + 1 + len * 2));
    }
    None
}

/// The library name after a stream name: an optional `FF FE FF`, a length and the text. Narrow
/// files store it in the system code page, which is read here as Latin-1.
fn library_name_at(data: &[u8], at: usize, wide: bool) -> Option<(String, usize)> {
    let at = if data.get(at..at + 3) == Some(&[0xFF, 0xFE, 0xFF]) {
        at + 3
    } else {
        at
    };
    let len = usize::from(*data.get(at)?);
    if len > MAX_ITEM_NAME_UNITS {
        return None;
    }
    let body = data.get(at + 1..)?;
    if wide {
        let units: Vec<u16> = body
            .get(..len * 2)?
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| u16::from_le_bytes(*c))
            .collect();
        Some((String::from_utf16_lossy(&units), at + 1 + len * 2))
    } else {
        let text = body.get(..len)?.iter().map(|&b| char::from(b)).collect();
        Some((text, at + 1 + len))
    }
}

fn media_kind(class: Option<&str>) -> ItemKind {
    match class {
        Some("CMediaBits") => ItemKind::Bitmap,
        Some("CMediaSound") => ItemKind::Sound,
        Some("CMediaVideoStream") => ItemKind::Video,
        _ => ItemKind::OtherMedia,
    }
}

/// Reads the library: one item per `Symbol`/`Media` stream, in the order of `stream_names`.
///
/// An item is introduced by a back-reference (`xx 80`) or a class declaration (`FF FF`), then a
/// schema byte, then the stream's name; the library name follows. A symbol then has its own
/// number and a kind byte (0 graphic, 1 button, 2 movie clip).
pub fn library(contents: &[u8], stream_names: &[&str]) -> Vec<LibraryItem> {
    let wanted: HashMap<&str, StreamKind> = stream_names
        .iter()
        .filter_map(|&n| match classify(n) {
            Some((kind @ (StreamKind::Symbol | StreamKind::Media), _)) => Some((n, kind)),
            _ => None,
        })
        .collect();
    let mut found: HashMap<String, LibraryItem> = HashMap::new();

    for (at, &byte) in contents.iter().enumerate() {
        if found.len() == wanted.len() {
            break;
        }
        // Where the schema byte would be, and the class if this spot declares one.
        let (schema_at, class) = match byte {
            0x80 => (at + 1, None),
            0xFF if contents.get(at + 1) == Some(&0xFF) => {
                let len = contents
                    .get(at + 4..at + 6)
                    .map(|b| usize::from(u16::from_le_bytes([b[0], b[1]])))
                    .filter(|l| (1..=64).contains(l));
                let Some(len) = len else { continue };
                let Some(text) = contents.get(at + 6..at + 6 + len) else {
                    continue;
                };
                if !text.starts_with(b"C") || !text.iter().all(u8::is_ascii_alphanumeric) {
                    continue;
                }
                (
                    at + 6 + len,
                    Some(String::from_utf8_lossy(text).into_owned()),
                )
            }
            _ => continue,
        };
        let Some((stream, wide, end)) = name_at(contents, schema_at + 1) else {
            continue;
        };
        let Some(&stream_kind) = wanted.get(stream.as_str()) else {
            continue;
        };
        if found.contains_key(&stream) {
            continue;
        }
        let Some((name, after)) = library_name_at(contents, end, wide) else {
            continue;
        };
        let kind = match stream_kind {
            StreamKind::Media => media_kind(class.as_deref()),
            _ => symbol_kind(contents, after, &stream),
        };
        found.insert(
            stream.clone(),
            LibraryItem {
                stream,
                name,
                kind,
                found: true,
            },
        );
    }

    stream_names
        .iter()
        .filter(|&&name| wanted.contains_key(name))
        .map(|&name| {
            found.remove(name).unwrap_or_else(|| LibraryItem {
                stream: name.to_owned(),
                name: name.to_owned(),
                kind: ItemKind::Unknown,
                found: false,
            })
        })
        .collect()
}

/// The kind of a symbol: its own number, which must be the stream's number, then a kind byte.
fn symbol_kind(contents: &[u8], at: usize, stream: &str) -> ItemKind {
    let number = stream.split(' ').nth(1).and_then(|n| n.parse::<u32>().ok());
    if number.is_none() || u32_at(contents, at) != number {
        return ItemKind::Unknown;
    }
    match contents.get(at + 4) {
        Some(0) => ItemKind::Graphic,
        Some(1) => ItemKind::Button,
        Some(2) => ItemKind::MovieClip,
        _ => ItemKind::Unknown,
    }
}
