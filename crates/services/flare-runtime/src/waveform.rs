//! A project's sounds frame by frame: how loud each is on every frame of the timeline from its
//! cue, so a script or an agent can key a mouth or a beat to them as a person does by looking at
//! the waveform. The command line and the MCP server read it; the editor draws the same
//! [`Peaks`](flare_audio::Peaks).

use flare_audio::{AudioError, Peaks};
use flare_document::{Document, Sound};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum WaveformError {
    #[error("the project has no sound named or with the id {0:?}")]
    UnknownSound(String),
    #[error("cannot read the sound {name:?}: {error}")]
    Audio { name: String, error: AudioError },
}

/// Rounds a sample to three places: finer is noise to a reader and only makes the reply longer.
fn rounded(sample: f32) -> f64 {
    (f64::from(sample) * 1000.0).round() / 1000.0
}

/// The `[low, high]` sample of `sound` on each frame of the timeline from the frame it starts on,
/// at `per_second` frames a second. Decodes the sound, so it is bounded as decoding is.
pub fn frame_peaks(sound: &Sound, per_second: f64) -> Result<Vec<[f32; 2]>, WaveformError> {
    let audio =
        flare_audio::decode(&sound.shared_data()).map_err(|error| WaveformError::Audio {
            name: sound.name().to_owned(),
            error,
        })?;
    let peaks = Peaks::of(&audio);
    let frames = (audio.seconds() * per_second).ceil() as usize;
    Ok((0..frames)
        .map(|frame| {
            let from = frame as f64 / per_second;
            peaks
                .between(from, from + 1.0 / per_second)
                .unwrap_or([0.0, 0.0])
        })
        .collect())
}

/// Every sound of `document`, or only the one `only` names (by name or id), with where it is cued
/// in the first scene and its `[low, high]` on each frame from its start.
pub fn sound_waveforms(document: &Document, only: Option<&str>) -> Result<Value, WaveformError> {
    let per_second = document.frame_rate().per_second();
    let chosen: Vec<&Sound> = document
        .sounds()
        .iter()
        .filter(|s| only.is_none_or(|o| s.name() == o || s.id().as_uuid().to_string() == o))
        .collect();
    if let Some(wanted) = only
        && chosen.is_empty()
    {
        return Err(WaveformError::UnknownSound(wanted.to_owned()));
    }
    let cues: Vec<(flare_core::LayerId, &flare_document::SoundCue)> = document
        .scenes()
        .first()
        .map(|scene| {
            scene
                .timeline()
                .cues()
                .map(|(layer, cue)| (layer.id(), cue))
                .collect()
        })
        .unwrap_or_default();
    let sounds = chosen
        .into_iter()
        .map(|sound| {
            let frames: Vec<[f64; 2]> = frame_peaks(sound, per_second)?
                .into_iter()
                .map(|[low, high]| [rounded(low), rounded(high)])
                .collect();
            let cued: Vec<Value> = cues
                .iter()
                .filter(|(_, cue)| cue.sound == sound.id())
                .map(|(layer, cue)| json!({ "layer": layer.as_uuid(), "start": cue.start }))
                .collect();
            Ok(json!({
                "sound": sound.id().as_uuid(),
                "name": sound.name(),
                "seconds": sound.seconds(),
                "cues": cued,
                "frames": frames,
            }))
        })
        .collect::<Result<Vec<Value>, WaveformError>>()?;
    Ok(json!({ "frame_rate": per_second, "sounds": sounds }))
}

#[cfg(test)]
#[path = "waveform_tests.rs"]
mod tests;
