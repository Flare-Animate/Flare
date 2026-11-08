//! Making a sound layer from the bytes of an audio file, for every surface that has no worker
//! pool to decode on (the command line and the MCP server).

use flare_audio::AudioError;
use flare_core::{LayerId, SoundId};
use flare_document::{Layer, Sound, SoundCue, SoundError, TimelineError};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum SoundImportError {
    #[error("cannot read the sound: {0}")]
    Audio(#[from] AudioError),
    #[error("cannot keep the sound: {0}")]
    Sound(#[from] SoundError),
    #[error("cannot make the sound layer: {0}")]
    Layer(#[from] TimelineError),
}

/// A decoded sound and the layer whose one cue plays it, ready to become `AddSound` and
/// `AddLayer` in one batch.
#[derive(Debug, Clone)]
pub struct SoundLayer {
    pub sound: Sound,
    pub layer: Layer,
    pub seconds: f64,
}

/// Decodes `bytes` (WAV, MP3, Ogg Vorbis or FLAC) and builds a layer named after `name` that
/// starts the sound at frame `start`. The original file bytes are kept, as the editor does.
pub fn sound_layer(name: &str, bytes: Vec<u8>, start: u32) -> Result<SoundLayer, SoundImportError> {
    let audio = flare_audio::decode(&bytes)?;
    let id = SoundId::new();
    let sound = Sound::new(
        id,
        name,
        bytes,
        audio.sample_rate(),
        audio.channels(),
        audio.frames() as u64,
    )?;
    let layer = Layer::new(LayerId::new(), format!("Sound: {name}"), vec![])?
        .with_cues(vec![SoundCue { start, sound: id }])?;
    Ok(SoundLayer {
        sound,
        layer,
        seconds: audio.seconds(),
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use super::*;

    fn wav(frames: u32) -> Vec<u8> {
        let data_len = frames * 2;
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data_len).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16_u32.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&1_u16.to_le_bytes());
        out.extend_from_slice(&8000_u32.to_le_bytes());
        out.extend_from_slice(&16000_u32.to_le_bytes());
        out.extend_from_slice(&2_u16.to_le_bytes());
        out.extend_from_slice(&16_u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data_len.to_le_bytes());
        for i in 0..frames {
            out.extend_from_slice(&(((i % 50) as i16 - 25) * 300).to_le_bytes());
        }
        out
    }

    #[test]
    fn a_wav_becomes_a_layer_with_one_cue() {
        let made = sound_layer("Beep", wav(8000), 12).expect("sound layer");
        assert_eq!(made.sound.name(), "Beep");
        assert_eq!(made.sound.frames(), 8000);
        assert!((made.seconds - 1.0).abs() < 1e-6);
        assert_eq!(made.layer.name(), "Sound: Beep");
        assert_eq!(made.layer.cues().len(), 1);
        assert_eq!(made.layer.cues()[0].start, 12);
        assert_eq!(made.layer.cues()[0].sound, made.sound.id());
    }

    #[test]
    fn something_that_is_not_sound_is_refused() {
        let error = sound_layer("x", b"not audio at all".to_vec(), 0).expect_err("refused");
        assert!(matches!(error, SoundImportError::Audio(_)));
    }
}
