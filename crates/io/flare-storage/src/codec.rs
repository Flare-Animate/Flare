//! How a record is stored: its fields as CBOR, compressed with zstd when that is smaller, with the
//! BLAKE3 digest of what is stored beside it.
//!
//! Everything read back is untrusted. The digest is checked before anything is decoded, the size
//! the row claims is checked against a limit before anything is allocated, and the decompressor
//! is told not to produce more than that size, so a record cannot be a decompression bomb.

use std::borrow::Cow;

use serde::{Serialize, de::DeserializeOwned};
use zstd::zstd_safe::{CParameter, DParameter};

use crate::StorageError;

/// Records shorter than this are stored as they are: a zstd frame costs more than it saves.
const COMPRESS_FROM: usize = 64;
/// How far back the compressor looks (2^18 bytes), which is also all the decoder is allowed.
const WINDOW_LOG: u32 = 18;
/// Deepest nesting of arrays and maps a record may have.
const MAX_DEPTH: usize = 128;

/// What is in a record's `payload` column.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Codec {
    /// The bytes themselves.
    Plain,
    /// One zstd frame (with content size and checksum) holding them.
    Zstd,
}

impl Codec {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Plain => "cbor",
            Self::Zstd => "cbor+zstd",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<Self> {
        match name {
            "cbor" => Some(Self::Plain),
            "cbor+zstd" => Some(Self::Zstd),
            _ => None,
        }
    }
}

/// A record ready to be written.
pub(crate) struct Stored<'a> {
    pub(crate) codec: Codec,
    /// The length before compression.
    pub(crate) size: usize,
    pub(crate) digest: [u8; 32],
    pub(crate) bytes: Cow<'a, [u8]>,
}

/// Compresses records, reusing one compressor for a whole save.
pub(crate) struct Packer(zstd::bulk::Compressor<'static>);

impl Packer {
    pub(crate) fn new() -> Result<Self, StorageError> {
        let mut compressor = zstd::bulk::Compressor::new(3)?;
        for parameter in [
            CParameter::WindowLog(WINDOW_LOG),
            CParameter::ChecksumFlag(true),
            CParameter::ContentSizeFlag(true),
        ] {
            compressor
                .set_parameter(parameter)
                .map_err(|e| StorageError::Corrupt(format!("cannot set up compression: {e}")))?;
        }
        Ok(Self(compressor))
    }

    /// `raw` as it will be stored: compressed if that is smaller.
    pub(crate) fn pack<'a>(&mut self, raw: &'a [u8]) -> Result<Stored<'a>, StorageError> {
        if raw.len() >= COMPRESS_FROM {
            let packed = self.0.compress(raw)?;
            if packed.len() < raw.len() {
                return Ok(Stored {
                    codec: Codec::Zstd,
                    size: raw.len(),
                    digest: *blake3::hash(&packed).as_bytes(),
                    bytes: Cow::Owned(packed),
                });
            }
        }
        Ok(Stored {
            codec: Codec::Plain,
            size: raw.len(),
            digest: *blake3::hash(raw).as_bytes(),
            bytes: Cow::Borrowed(raw),
        })
    }
}

/// `value` as CBOR bytes.
pub(crate) fn to_cbor<T: Serialize>(value: &T) -> Result<Vec<u8>, StorageError> {
    let mut out = Vec::new();
    ciborium::into_writer(value, &mut out)
        .map_err(|e| StorageError::Corrupt(format!("cannot encode a record: {e}")))?;
    Ok(out)
}

/// A value from CBOR bytes, with the nesting bounded.
pub(crate) fn from_cbor<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, StorageError> {
    ciborium::de::from_reader_with_recursion_limit(bytes, MAX_DEPTH)
        .map_err(|e| StorageError::Corrupt(format!("a record cannot be read: {e}")))
}

/// What a row held, checked: the digest of what was stored, the size against `limit`, and the
/// decompression against the size. `None` for a codec this build does not know.
pub(crate) fn unpack(
    codec: &str,
    size: i64,
    digest: &[u8],
    stored: &[u8],
    limit: usize,
) -> Result<Vec<u8>, StorageError> {
    let codec = Codec::from_name(codec)
        .ok_or_else(|| StorageError::Corrupt(format!("unknown record codec {codec:?}")))?;
    let size = usize::try_from(size)
        .map_err(|_| StorageError::Corrupt("a record has a negative size".into()))?;
    if size > limit {
        return Err(StorageError::Limit("entity payload size"));
    }
    if digest != blake3::hash(stored).as_bytes() {
        return Err(StorageError::Corrupt(
            "a record does not match its digest".into(),
        ));
    }
    match codec {
        Codec::Plain => {
            if stored.len() != size {
                return Err(StorageError::Corrupt(
                    "a record is not its stated size".into(),
                ));
            }
            Ok(stored.to_vec())
        }
        Codec::Zstd => {
            let mut decompressor = zstd::bulk::Decompressor::new()?;
            decompressor
                .set_parameter(DParameter::WindowLogMax(WINDOW_LOG))
                .map_err(|e| StorageError::Corrupt(format!("cannot set up decompression: {e}")))?;
            let raw = decompressor.decompress(stored, size).map_err(|e| {
                StorageError::Corrupt(format!("a record cannot be decompressed: {e}"))
            })?;
            if raw.len() != size {
                return Err(StorageError::Corrupt(
                    "a record is not its stated size".into(),
                ));
            }
            Ok(raw)
        }
    }
}
