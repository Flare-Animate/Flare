//! How the library's bitmaps are stored, as Animate's Bitmap Properties choose it: lossless, or as
//! a photo (a JPEG at a quality). A frame of video kept as a photo takes about a ninth of the
//! room in the file and looks the same; a drawing with hard edges is better lossless.
//!
//! The change is one undo step of `ReplaceBitmap` commands: each bitmap keeps its identity and
//! size, so every frame that shows it shows it still.

use flare_commands::Command;
use flare_core::BitmapId;
use flare_document::{Bitmap, BitmapError, Document};
use thiserror::Error;

/// How a bitmap is stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Every pixel exactly.
    Lossless,
    /// As a JPEG at this quality, 1 to 100.
    Photo { quality: u8 },
}

/// What [`set_bitmap_compression`] would do.
#[derive(Debug, Clone, PartialEq)]
pub struct Compressed {
    /// The batch to apply: one undo step.
    pub commands: Vec<Command>,
    /// Bitmaps whose storage changes.
    pub changed: usize,
    /// Bitmaps already stored that way.
    pub unchanged: usize,
    /// Bitmaps a JPEG would not make smaller, which stay lossless.
    pub not_smaller: usize,
    /// The bytes the chosen bitmaps take to store before and after, before the file's own
    /// compression (which shrinks lossless pixels a little more).
    pub bytes_before: u64,
    pub bytes_after: u64,
}

/// Why nothing could be done.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompressionError {
    #[error("there is no bitmap {0:?} in the library")]
    NoSuchBitmap(BitmapId),
    #[error("the library has no bitmaps")]
    NoBitmaps,
    #[error("a photo's quality is from 1 to 100, not {0}")]
    Quality(u8),
    #[error("bitmap {name:?} could not be stored that way: {why}")]
    Failed { name: String, why: String },
}

/// The bytes `bitmap` takes to store as it is.
pub fn stored_bytes(bitmap: &Bitmap) -> u64 {
    bitmap
        .photo()
        .map_or(bitmap.byte_len(), |photo| photo.byte_len() as u64)
}

/// The library's bitmaps in brief, for `info` and `get_info`: how many, how many are photos,
/// and the bytes they take to store and to hold open.
pub fn bitmap_summary(document: &Document) -> serde_json::Value {
    let bitmaps = document.bitmaps();
    serde_json::json!({
        "count": bitmaps.len(),
        "photos": bitmaps.iter().filter(|b| b.photo().is_some()).count(),
        "stored_bytes": bitmaps.iter().map(stored_bytes).sum::<u64>(),
        "pixel_bytes": bitmaps.iter().map(Bitmap::byte_len).sum::<u64>(),
    })
}

/// Stores the bitmaps `which` names (all of the library's when empty) as `compression` says.
/// A bitmap already stored that way is left alone, and one a JPEG would not shrink stays
/// lossless; neither is an error. Compressing is slow for many large bitmaps (about 10 ms
/// a frame of video), so a window runs it off its own thread.
///
/// # Errors
/// A name that is not in the library, an empty library, a quality out of range, or a bitmap the
/// encoder could not compress.
pub fn set_bitmap_compression(
    document: &Document,
    which: &[BitmapId],
    compression: Compression,
) -> Result<Compressed, CompressionError> {
    if let Compression::Photo { quality } = compression
        && !(1..=100).contains(&quality)
    {
        return Err(CompressionError::Quality(quality));
    }
    let chosen: Vec<&Bitmap> = if which.is_empty() {
        document.bitmaps().iter().collect()
    } else {
        which
            .iter()
            .map(|id| {
                document
                    .bitmap(*id)
                    .ok_or(CompressionError::NoSuchBitmap(*id))
            })
            .collect::<Result<_, _>>()?
    };
    if chosen.is_empty() {
        return Err(CompressionError::NoBitmaps);
    }
    let mut out = Compressed {
        commands: Vec::new(),
        changed: 0,
        unchanged: 0,
        not_smaller: 0,
        bytes_before: 0,
        bytes_after: 0,
    };
    for bitmap in chosen {
        out.bytes_before += stored_bytes(bitmap);
        let next = match compression {
            Compression::Lossless if bitmap.photo().is_none() => None,
            Compression::Lossless => Some(bitmap.as_lossless()),
            Compression::Photo { quality }
                if bitmap.photo().is_some_and(|p| p.quality() == quality) =>
            {
                None
            }
            Compression::Photo { quality } => match bitmap.as_photo(quality) {
                Ok(photo) => Some(photo),
                Err(BitmapError::PhotoNotSmaller) => {
                    out.not_smaller += 1;
                    out.bytes_after += stored_bytes(bitmap);
                    continue;
                }
                Err(why) => {
                    return Err(CompressionError::Failed {
                        name: bitmap.name().to_owned(),
                        why: why.to_string(),
                    });
                }
            },
        };
        match next {
            Some(next) => {
                out.bytes_after += stored_bytes(&next);
                out.changed += 1;
                out.commands.push(Command::ReplaceBitmap(next));
            }
            None => {
                out.bytes_after += stored_bytes(bitmap);
                out.unchanged += 1;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[path = "compression_tests.rs"]
mod tests;
