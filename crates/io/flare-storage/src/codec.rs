//! How a record is stored: its fields as CBOR, compressed with zstd when that is smaller, with the
//! BLAKE3 digest of what is stored beside it.
//!
//! Everything read back is untrusted. The digest is checked before anything is decoded, the size
//! the record claims is checked against a limit before anything is allocated, and the zstd frame's
//! own header is read here, in code of ours, and must agree: the content size it declares is the
//! size the record states, its window is no bigger than the writer's, it carries a checksum and
//! it names no dictionary. Only then is a decompressor asked, and told to stop at that size. So a
//! record cannot be a decompression bomb whichever zstd is behind it (a library in the desktop
//! app, WebAssembly in the browser).

use std::borrow::Cow;

use serde::{Serialize, de::DeserializeOwned};

use crate::StorageError;

/// Records shorter than this are stored as they are: a zstd frame costs more than it saves.
const COMPRESS_FROM: usize = 64;
/// How far back the compressor looks (2^18 bytes), which is also all a frame may declare.
pub(crate) const WINDOW_LOG: u32 = 18;
/// Deepest nesting of arrays and maps a record may have.
const MAX_DEPTH: usize = 128;

/// A zstd implementation. The desktop app's is a library, the browser's is a WebAssembly module
/// the page loads; the records are the same bytes either way.
pub trait Zstd {
    /// `raw` as one zstd frame at the level and window the format uses, with the content size and
    /// a checksum in its header.
    fn compress(&mut self, raw: &[u8]) -> Result<Vec<u8>, StorageError>;

    /// The content of `frame`, which is `size` bytes (the frame's header has been checked to say
    /// so already). Must not produce more than that.
    fn decompress(&mut self, frame: &[u8], size: usize) -> Result<Vec<u8>, StorageError>;
}

/// What is in a record's payload.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    /// The bytes themselves.
    Plain,
    /// One zstd frame (with content size and checksum) holding them.
    Zstd,
}

impl Codec {
    pub fn name(self) -> &'static str {
        match self {
            Self::Plain => "cbor",
            Self::Zstd => "cbor+zstd",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "cbor" => Some(Self::Plain),
            "cbor+zstd" => Some(Self::Zstd),
            _ => None,
        }
    }
}

/// A record ready to be written.
pub struct Stored<'a> {
    pub codec: Codec,
    /// The length before compression.
    pub size: usize,
    pub digest: [u8; 32],
    pub bytes: Cow<'a, [u8]>,
}

/// Compresses records with the zstd it is given, reusing it for a whole save.
pub struct Packer<'z>(&'z mut dyn Zstd);

impl<'z> Packer<'z> {
    pub fn new(zstd: &'z mut dyn Zstd) -> Self {
        Self(zstd)
    }

    /// `raw` as it will be stored: compressed if that is smaller.
    pub fn pack<'a>(&mut self, raw: &'a [u8]) -> Result<Stored<'a>, StorageError> {
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

/// What a zstd frame's header says about it (RFC 8878, section 3.1.1.1).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct FrameHeader {
    pub(crate) window: u64,
    pub(crate) content_size: Option<u64>,
    pub(crate) checksum: bool,
    pub(crate) dictionary: bool,
}

/// Reads the header of the zstd frame at the start of `frame`, or `None` if it is not one.
pub(crate) fn frame_header(frame: &[u8]) -> Option<FrameHeader> {
    const MAGIC: [u8; 4] = [0x28, 0xB5, 0x2F, 0xFD];
    let (magic, rest) = frame.split_first_chunk::<4>()?;
    if *magic != MAGIC {
        return None;
    }
    let (&descriptor, mut rest) = rest.split_first()?;
    let size_flag = descriptor >> 6;
    let single_segment = descriptor & 0x20 != 0;
    if descriptor & 0x08 != 0 {
        return None; // a reserved bit
    }
    let checksum = descriptor & 0x04 != 0;
    let dictionary_bytes = [0usize, 1, 2, 4][usize::from(descriptor & 3)];
    let mut window_descriptor = None;
    if !single_segment {
        let (&byte, after) = rest.split_first()?;
        window_descriptor = Some(byte);
        rest = after;
    }
    let (_dictionary, rest) = rest.split_at_checked(dictionary_bytes)?;
    let size_bytes = match size_flag {
        0 => usize::from(single_segment),
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let (size, _) = rest.split_at_checked(size_bytes)?;
    let content_size = (size_bytes > 0).then(|| {
        let mut value = 0u64;
        for (i, byte) in size.iter().enumerate() {
            value |= u64::from(*byte) << (8 * i);
        }
        if size_flag == 1 { value + 256 } else { value }
    });
    let window = match window_descriptor {
        Some(byte) => {
            let log = 10 + u32::from(byte >> 3);
            let base = 1u64.checked_shl(log)?;
            base + base / 8 * u64::from(byte & 7)
        }
        None => content_size?,
    };
    Some(FrameHeader {
        window,
        content_size,
        checksum,
        dictionary: dictionary_bytes > 0,
    })
}

/// What a record held, checked: the codec named, the size against `limit`, the digest of what was
/// stored and, for a zstd frame, its header against the stated size, before the decompressor
/// is asked for anything.
pub fn unpack(
    zstd: &mut dyn Zstd,
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
            let header = frame_header(stored)
                .ok_or_else(|| StorageError::Corrupt("a record is not a zstd frame".into()))?;
            if header.content_size != Some(size as u64) {
                return Err(StorageError::Corrupt(
                    "a record's frame does not declare its stated size".into(),
                ));
            }
            if header.window > 1 << WINDOW_LOG || !header.checksum || header.dictionary {
                return Err(StorageError::Corrupt(
                    "a record's frame is not one this format writes".into(),
                ));
            }
            let raw = zstd.decompress(stored, size)?;
            if raw.len() != size {
                return Err(StorageError::Corrupt(
                    "a record is not its stated size".into(),
                ));
            }
            Ok(raw)
        }
    }
}

/// The zstd of the desktop app: the library.
#[cfg(not(target_arch = "wasm32"))]
pub struct NativeZstd {
    compressor: zstd::bulk::Compressor<'static>,
    decompressor: zstd::bulk::Decompressor<'static>,
}

#[cfg(not(target_arch = "wasm32"))]
impl NativeZstd {
    pub fn new() -> Result<Self, StorageError> {
        use zstd::zstd_safe::{CParameter, DParameter};
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
        let mut decompressor = zstd::bulk::Decompressor::new()?;
        decompressor
            .set_parameter(DParameter::WindowLogMax(WINDOW_LOG))
            .map_err(|e| StorageError::Corrupt(format!("cannot set up decompression: {e}")))?;
        Ok(Self {
            compressor,
            decompressor,
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Zstd for NativeZstd {
    fn compress(&mut self, raw: &[u8]) -> Result<Vec<u8>, StorageError> {
        Ok(self.compressor.compress(raw)?)
    }

    fn decompress(&mut self, frame: &[u8], size: usize) -> Result<Vec<u8>, StorageError> {
        self.decompressor
            .decompress(frame, size)
            .map_err(|e| StorageError::Corrupt(format!("a record cannot be decompressed: {e}")))
    }
}
