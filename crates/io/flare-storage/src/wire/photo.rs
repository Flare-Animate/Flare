//! Photo records: a bitmap kept as a JPEG (see `flare_document::Photo`). The length of the fields
//! (4 bytes, little endian), the fields as CBOR, the JPEG, then the transparency when it has any.
//! The pixels are not stored: loading decodes them from the JPEG, which is what the document
//! held.

use crate::StorageError;
use flare_core::BitmapId;
use flare_document::Bitmap;
use serde::{Deserialize, Serialize};

use super::uuid;

const MAX_META_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
struct PhotoMeta {
    name: String,
    width: u32,
    height: u32,
    quality: u8,
    jpeg: u64,
    alpha: bool,
}

/// `bitmap`, which is kept as a photo, as one record; `None` for one that is not.
pub(crate) fn frame(bitmap: &Bitmap) -> Result<Option<Vec<u8>>, StorageError> {
    let Some(photo) = bitmap.photo() else {
        return Ok(None);
    };
    let meta = crate::codec::to_cbor(&PhotoMeta {
        name: bitmap.name().to_owned(),
        width: bitmap.width(),
        height: bitmap.height(),
        quality: photo.quality(),
        jpeg: photo.jpeg().len() as u64,
        alpha: photo.alpha().is_some(),
    })?;
    let mut out = Vec::with_capacity(4 + meta.len() + photo.byte_len());
    out.extend_from_slice(&u32::try_from(meta.len()).unwrap_or(u32::MAX).to_le_bytes());
    out.extend_from_slice(&meta);
    out.extend_from_slice(photo.jpeg());
    if let Some(alpha) = photo.alpha() {
        out.extend_from_slice(alpha);
    }
    Ok(Some(out))
}

/// The bitmap in a photo record, decoded and checked by `Bitmap::from_photo`.
pub(crate) fn unframe(id: &str, record: &[u8]) -> Result<Bitmap, StorageError> {
    let corrupt = |why: &str| StorageError::Corrupt(format!("a photo record {why}"));
    let (length, rest) = record
        .split_first_chunk::<4>()
        .ok_or_else(|| corrupt("is too short"))?;
    let length = u32::from_le_bytes(*length) as usize;
    if length > MAX_META_BYTES || length > rest.len() {
        return Err(corrupt("has fields that do not fit it"));
    }
    let (meta, body) = rest.split_at(length);
    let meta: PhotoMeta = crate::codec::from_cbor(meta)?;
    let jpeg_len = usize::try_from(meta.jpeg).map_err(|_| corrupt("claims too much"))?;
    if jpeg_len > body.len() {
        return Err(corrupt("is shorter than its JPEG"));
    }
    let (jpeg, alpha) = body.split_at(jpeg_len);
    let alpha = match (meta.alpha, alpha.is_empty()) {
        (true, false) => Some(alpha.to_vec()),
        (false, true) => None,
        _ => return Err(corrupt("does not hold the transparency it says")),
    };
    Bitmap::from_photo(
        BitmapId::from_uuid(uuid(id)?),
        meta.name,
        meta.width,
        meta.height,
        meta.quality,
        jpeg.to_vec(),
        alpha,
    )
    .map_err(|error| StorageError::Corrupt(error.to_string()))
}
