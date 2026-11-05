//! Bitmap records: the fields, then the raw RGBA pixels (compressed with the rest of the record).

use crate::StorageError;
use flare_core::BitmapId;
use flare_document::Bitmap;
use serde::{Deserialize, Serialize};

use super::uuid;

/// The most a bitmap record may be: the most pixels one bitmap may hold, and its fields.
pub const MAX_BITMAP_RECORD_BYTES: usize =
    flare_document::MAX_BITMAP_PIXELS as usize * 4 + 64 * 1024 + 4;
const MAX_META_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
pub(crate) struct BitmapMeta {
    name: String,
    width: u32,
    height: u32,
}

impl BitmapMeta {
    /// A bitmap as one record: the length of its fields (4 bytes, little endian), the fields as
    /// CBOR, then the pixels.
    pub(crate) fn frame(bitmap: &Bitmap) -> Result<Vec<u8>, StorageError> {
        let meta = crate::codec::to_cbor(&Self {
            name: bitmap.name().to_owned(),
            width: bitmap.width(),
            height: bitmap.height(),
        })?;
        let mut out = Vec::with_capacity(4 + meta.len() + bitmap.pixels().len());
        out.extend_from_slice(&u32::try_from(meta.len()).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(&meta);
        out.extend_from_slice(bitmap.pixels());
        Ok(out)
    }

    /// Rebuilds a bitmap through its validating constructor, which checks the size against the
    /// pixels.
    pub(crate) fn unframe(id: &str, record: &[u8]) -> Result<Bitmap, StorageError> {
        let corrupt = |why: &str| StorageError::Corrupt(format!("a bitmap record {why}"));
        let (length, rest) = record
            .split_first_chunk::<4>()
            .ok_or_else(|| corrupt("is too short"))?;
        let length = u32::from_le_bytes(*length) as usize;
        if length > MAX_META_BYTES || length > rest.len() {
            return Err(corrupt("has fields that do not fit it"));
        }
        let (meta, pixels) = rest.split_at(length);
        let meta: BitmapMeta = crate::codec::from_cbor(meta)?;
        Bitmap::new(
            BitmapId::from_uuid(uuid(id)?),
            meta.name,
            meta.width,
            meta.height,
            pixels.to_vec(),
        )
        .map_err(|error| StorageError::Corrupt(error.to_string()))
    }
}
