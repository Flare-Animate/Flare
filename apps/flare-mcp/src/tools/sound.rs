//! Adding audio files from the configured export directory to the open project, and reading
//! how loud each sound is frame by frame.

use flare_commands::Command;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::color::safe_name;
use crate::ops::{apply, first_scene};
use crate::params::{ImportSound, SoundWaveform};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = sound_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Import a WAV, MP3, Ogg Vorbis or FLAC file from the configured export directory as a sound and a new layer with one cue in the open project. The original bytes are kept. Requires write permission."
    )]
    fn import_sound(&self, Parameters(args): Parameters<ImportSound>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let dir = super::export::export_dir(state)?
                .canonicalize()
                .map_err(|error| format!("cannot resolve the export directory: {error}"))?;
            let name = safe_name(&args.source)?;
            let path = dir.join(name).canonicalize().map_err(|error| {
                format!(
                    "cannot resolve {} in the export directory: {error}",
                    args.source
                )
            })?;
            if !path.starts_with(&dir) {
                return Err("sound file must stay inside the export directory".to_owned());
            }
            let metadata = std::fs::metadata(&path).map_err(|error| {
                format!(
                    "cannot inspect {} in the export directory: {error}",
                    args.source
                )
            })?;
            if !metadata.is_file() {
                return Err("sound import requires a regular file".to_owned());
            }
            if metadata.len() > flare_runtime::MAX_AUDIO_INPUT_BYTES as u64 {
                return Err(format!(
                    "sound file is larger than {} bytes",
                    flare_runtime::MAX_AUDIO_INPUT_BYTES
                ));
            }
            let bytes =
                std::fs::read(&path).map_err(|error| format!("cannot read sound: {error}"))?;
            let sound_name = path.file_stem().map_or_else(
                || "Sound".to_owned(),
                |stem| stem.to_string_lossy().into_owned(),
            );
            let made = flare_runtime::sound_layer(&sound_name, bytes, args.start.unwrap_or(0))
                .map_err(|error| error.to_string())?;
            let scene = first_scene(state)?;
            let sound_id = made.sound.id();
            let layer_id = made.layer.id();
            let mut result = apply(
                state,
                vec![
                    Command::AddSound(made.sound),
                    Command::AddLayer {
                        scene,
                        layer: made.layer,
                    },
                ],
            )?;
            result["sound"] = json!(sound_id.as_uuid());
            result["layer"] = json!(layer_id.as_uuid());
            result["name"] = json!(sound_name);
            result["start"] = json!(args.start.unwrap_or(0));
            result["seconds"] = json!(made.seconds);
            Ok(result)
        })
    }

    #[tool(
        description = "How loud each sound of the project is on every frame from its cue: for each sound its cues (layer and start frame) and a [low, high] sample pair per frame, in -1..1, at the project's frame rate. Use it to time mouth shapes and beats to the audio."
    )]
    fn sound_waveform(&self, Parameters(args): Parameters<SoundWaveform>) -> Outcome {
        self.read(|state| {
            flare_runtime::sound_waveforms(state.session.document(), args.sound.as_deref())
                .map_err(|error| error.to_string())
        })
    }
}
