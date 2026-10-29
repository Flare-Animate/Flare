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

#[derive(Serialize, Deserialize)]
pub(crate) struct SoundWire {
    pub(super) name: String,
    /// The imported file, base64.
    pub(super) data: String,
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) frames: u64,
}

impl SoundWire {
    pub(crate) fn of(sound: &Sound) -> Self {
        use base64::Engine as _;
        Self {
            name: sound.name().to_owned(),
            data: base64::engine::general_purpose::STANDARD.encode(sound.data()),
            sample_rate: sound.sample_rate(),
            channels: sound.channels(),
            frames: sound.frames(),
        }
    }

    pub(crate) fn build(self, id: &str) -> Result<Sound, StorageError> {
        use base64::Engine as _;
        let data = base64::engine::general_purpose::STANDARD
            .decode(self.data)
            .map_err(|_| StorageError::Corrupt("sound data is not valid base64".into()))?;
        Sound::new(
            SoundId::from_uuid(uuid(id)?),
            self.name,
            data,
            self.sample_rate,
            self.channels,
            self.frames,
        )
        .map_err(|error| StorageError::Corrupt(error.to_string()))
    }
}
