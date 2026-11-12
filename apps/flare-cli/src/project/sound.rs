//! Adding a decoded audio file to an existing project, and reading a sound's waveform.

use anyhow::{Context, Result, bail};
use flare_commands::Command;
use serde_json::{Value, json};
use std::path::Path;

use super::{commit, open};

/// Adds a sound as a new layer and one cue, in one validated edit.
pub fn sound_import(path: &Path, sound_path: &Path, at: u32) -> Result<Value> {
    let metadata = std::fs::metadata(sound_path)
        .with_context(|| format!("cannot inspect {}", sound_path.display()))?;
    if !metadata.is_file() {
        bail!("sound import requires a regular file");
    }
    if metadata.len() > flare_runtime::MAX_AUDIO_INPUT_BYTES as u64 {
        bail!(
            "sound file is larger than {} bytes",
            flare_runtime::MAX_AUDIO_INPUT_BYTES
        );
    }
    let bytes = std::fs::read(sound_path)
        .with_context(|| format!("cannot read {}", sound_path.display()))?;
    let name = sound_path.file_stem().map_or_else(
        || "Sound".to_owned(),
        |stem| stem.to_string_lossy().into_owned(),
    );
    let made = flare_runtime::sound_layer(&name, bytes, at)?;
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .map(|scene| scene.id())
        .context("the project has no scene")?;
    let sound_id = made.sound.id();
    let layer_id = made.layer.id();
    session.apply([
        Command::AddSound(made.sound),
        Command::AddLayer {
            scene,
            layer: made.layer,
        },
    ])?;
    commit(&session, path)?;
    Ok(json!({
        "sound": sound_id.as_uuid(),
        "layer": layer_id.as_uuid(),
        "name": name,
        "start": at,
        "seconds": made.seconds,
    }))
}

/// Each sound's `[low, high]` on every frame from its cue, or only the one `sound` names.
pub fn sound_waveform(path: &Path, sound: Option<&str>) -> Result<Value> {
    let session = open(path)?;
    Ok(flare_runtime::sound_waveforms(session.document(), sound)?)
}
