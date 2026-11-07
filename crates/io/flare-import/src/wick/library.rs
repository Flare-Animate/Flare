//! The bitmap library: each picture decoded once, and identical pictures sharing one bitmap, with
//! the decoded bytes bounded. Only PNG is decoded; Wick's other pictures are reported by the caller.

use std::collections::HashMap;

use flare_core::BitmapId;
use flare_document::{Bitmap, MAX_BITMAPS};

use super::error::WickError;
use super::limits::MAX_DECODED_BYTES;

/// A bitmap in the library and its size in pixels.
#[derive(Debug, Clone, Copy)]
pub(super) struct Picked {
    pub id: BitmapId,
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
pub(super) struct Library {
    bitmaps: Vec<Bitmap>,
    by_hash: HashMap<[u8; 32], Picked>,
    /// Each asset decoded once, by its file name; `None` when it could not be read.
    by_asset: HashMap<String, Option<Picked>>,
    decoded: u64,
}

impl Library {
    /// The bitmap for an asset file, decoding it on first use. `Ok(None)` when the file is absent
    /// or not a PNG this importer reads.
    pub fn asset(
        &mut self,
        file: &str,
        bytes: Option<&Vec<u8>>,
    ) -> Result<Option<Picked>, WickError> {
        if let Some(known) = self.by_asset.get(file) {
            return Ok(*known);
        }
        let picked = match bytes {
            Some(data) => match crate::bitmap::decode_png(file, data) {
                Ok(picture) => Some(self.add(file, picture.width, picture.height, picture.rgba)?),
                Err(_) => None,
            },
            None => None,
        };
        self.by_asset.insert(file.to_owned(), picked);
        Ok(picked)
    }

    fn add(
        &mut self,
        name: &str,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<Picked, WickError> {
        let bytes = rgba.len() as u64;
        let bitmap = Bitmap::new(BitmapId::new(), name, width, height, rgba)
            .map_err(|e| WickError::Picture(e.to_string()))?;
        if let Some(picked) = self.by_hash.get(bitmap.content_hash()) {
            return Ok(*picked);
        }
        if self.bitmaps.len() >= MAX_BITMAPS {
            return Err(WickError::TooLarge("the project's bitmaps"));
        }
        self.decoded += bytes;
        if self.decoded > MAX_DECODED_BYTES {
            return Err(WickError::TooLarge("the pictures kept"));
        }
        let picked = Picked {
            id: bitmap.id(),
            width,
            height,
        };
        self.by_hash.insert(*bitmap.content_hash(), picked);
        self.bitmaps.push(bitmap);
        Ok(picked)
    }

    pub fn len(&self) -> usize {
        self.bitmaps.len()
    }

    pub fn into_bitmaps(self) -> Vec<Bitmap> {
        self.bitmaps
    }
}
