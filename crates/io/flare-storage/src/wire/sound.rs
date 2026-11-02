//! Sound and cue records.

use crate::StorageError;
use flare_core::SoundId;
use flare_document::Sound;
use serde::{Deserialize, Serialize};

use super::*;

#[derive(Serialize, Deserialize)]
pub(super) struct CueWire {
    pub(super) start: u32,
    pub(super) sound: String,
}

/// Everything about a sound but its file. The file is not a field: it follows the fields as the
/// bytes it is, with no text encoding of it (base64 would add a third).
#[derive(Serialize, Deserialize)]
pub(crate) struct SoundMeta {
    pub(super) name: String,
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) frames: u64,
}

/// The most the fields of a sound may take.
const MAX_META_BYTES: usize = 64 * 1024;

impl SoundMeta {
    pub(crate) fn of(sound: &Sound) -> Self {
        Self {
            name: sound.name().to_owned(),
            sample_rate: sound.sample_rate(),
            channels: sound.channels(),
            frames: sound.frames(),
        }
    }

    /// A sound as one record: the length of its fields (4 bytes, little endian), the fields as
    /// CBOR, then the file.
    pub(crate) fn frame(sound: &Sound) -> Result<Vec<u8>, StorageError> {
        let meta = crate::codec::to_cbor(&Self::of(sound))?;
        let mut out = Vec::with_capacity(4 + meta.len() + sound.data().len());
        out.extend_from_slice(&u32::try_from(meta.len()).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(&meta);
        out.extend_from_slice(sound.data());
        Ok(out)
    }

    /// Takes a record made by [`SoundMeta::frame`] apart, and rebuilds the sound through its
    /// validating constructor.
    pub(crate) fn unframe(id: &str, record: &[u8]) -> Result<Sound, StorageError> {
        let corrupt = |why: &str| StorageError::Corrupt(format!("a sound record {why}"));
        let (length, rest) = record
            .split_first_chunk::<4>()
            .ok_or_else(|| corrupt("is too short"))?;
        let length = u32::from_le_bytes(*length) as usize;
        if length > MAX_META_BYTES || length > rest.len() {
            return Err(corrupt("has fields that do not fit it"));
        }
        let (meta, data) = rest.split_at(length);
        let meta: SoundMeta = crate::codec::from_cbor(meta)?;
        Sound::new(
            SoundId::from_uuid(uuid(id)?),
            meta.name,
            data.to_vec(),
            meta.sample_rate,
            meta.channels,
            meta.frames,
        )
        .map_err(|error| StorageError::Corrupt(error.to_string()))
    }
}
