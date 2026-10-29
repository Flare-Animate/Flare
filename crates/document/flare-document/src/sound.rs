//! Sounds: a library of imported audio, and cues that start a sound at a frame of a layer.

use std::sync::Arc;

use flare_core::SoundId;
use thiserror::Error;

/// Most sounds one document may hold.
pub const MAX_SOUNDS: usize = 256;
/// Largest single sound, as the file it was imported from.
pub const MAX_SOUND_BYTES: usize = 64 * 1024 * 1024;
/// Most bytes of sound files in one document.
pub const MAX_TOTAL_SOUND_BYTES: usize = 256 * 1024 * 1024;
/// Most cues one layer may hold.
pub const MAX_CUES_PER_LAYER: usize = 10_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SoundError {
    #[error("a sound file is limited to {MAX_SOUND_BYTES} bytes")]
    TooLarge,
    #[error("a sound needs a sample rate, at least one channel and at least one sample")]
    Empty,
    #[error("a layer holds at most {MAX_CUES_PER_LAYER} sound cues")]
    TooManyCues,
}

/// An imported sound: the original file, kept as it was so nothing is lost to re-encoding, and
/// what decoding it showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    id: SoundId,
    name: String,
    /// Shared, so a snapshot of the document never copies megabytes of audio.
    data: Arc<[u8]>,
    sample_rate: u32,
    channels: u16,
    /// Samples per channel.
    frames: u64,
}

impl Sound {
    pub fn new(
        id: SoundId,
        name: impl Into<String>,
        data: Vec<u8>,
        sample_rate: u32,
        channels: u16,
        frames: u64,
    ) -> Result<Self, SoundError> {
        if data.len() > MAX_SOUND_BYTES {
            return Err(SoundError::TooLarge);
        }
        if data.is_empty() || sample_rate == 0 || channels == 0 || frames == 0 {
            return Err(SoundError::Empty);
        }
        Ok(Self {
            id,
            name: name.into(),
            data: data.into(),
            sample_rate,
            channels,
            frames,
        })
    }

    pub fn id(&self) -> SoundId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The imported file, byte for byte.
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The same bytes, shared: cheap to hand to another thread.
    pub fn shared_data(&self) -> Arc<[u8]> {
        Arc::clone(&self.data)
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> u16 {
        self.channels
    }

    pub fn frames(&self) -> u64 {
        self.frames
    }

    pub fn seconds(&self) -> f64 {
        self.frames as f64 / f64::from(self.sample_rate)
    }
}

/// Starts a sound at a frame of the layer that holds the cue. It plays until it ends.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundCue {
    pub start: u32,
    pub sound: SoundId,
}
