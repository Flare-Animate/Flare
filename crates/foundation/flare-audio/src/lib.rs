//! Sound: decoding, and a mixer that turns sounds placed on a timeline into output samples.
//!
//! Everything here is pure and testable without a sound card; the device itself is opened by
//! `flare-platform`. Decoding handles WAV,
//! MP3, Ogg Vorbis and FLAC (`symphonia`) with limits on input size and decoded length, so a
//! hostile or huge file is refused instead of exhausting memory.
#![forbid(unsafe_code)]

use std::sync::Arc;

use thiserror::Error;

/// Something to do with the sound card. A front end decides what plays when and says so with
/// these; whoever owns the device carries them out (see `flare-platform`).
#[derive(Clone, Debug)]
pub enum AudioEvent {
    /// Silence everything that is playing.
    Stop,
    /// Start a sound after a delay.
    Play {
        audio: Arc<Audio>,
        delay_seconds: f64,
    },
}

/// Largest encoded file accepted.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Most samples (all channels) one decoded sound may have: ten minutes of 48 kHz stereo.
pub const MAX_DECODED_SAMPLES: usize = 57_600_000;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AudioError {
    #[error("the file is larger than {MAX_INPUT_BYTES} bytes")]
    InputTooLarge,
    #[error("the sound is longer than this editor handles (about ten minutes)")]
    TooLong,
    #[error(
        "this is not a sound file Flare can read (WAV, MP3, Ogg Vorbis and FLAC are supported)"
    )]
    Unsupported,
    #[error("the sound file is damaged: {0}")]
    Corrupt(String),
    #[error("the sound has no audio in it")]
    Empty,
}

mod audio;
mod mixer;
#[cfg(test)]
mod tests;

pub use audio::{Audio, decode};
pub use mixer::Mixer;
