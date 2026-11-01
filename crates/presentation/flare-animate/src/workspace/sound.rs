//! Importing sounds and keeping the sound card in step.

use flare_commands::Command;
use flare_core::LayerId;
use flare_document::Layer;
use flare_runtime::{AudioJob, Mode};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use super::*;

impl Workspace {
    // ---- sound ------------------------------------------------------------------------

    /// Imports a sound file: it is read and decoded on a worker, then added to the project with
    /// a cue at the current frame on a new layer, as one undo step.
    pub fn import_sound(&mut self, path: &Path) {
        if self.in_symbol() {
            self.report("Leave symbol editing before importing a sound");
            return;
        }
        let id = self.background.next_id();
        self.audio.importing = Some(id);
        self.background.submit_audio(AudioJob::Import {
            id,
            path: path.to_owned(),
        });
        if self.background.mode() == Mode::Threads {
            self.status = Some(format!("Importing {}...", path.display()));
        }
        self.settle_inline();
    }

    pub(super) fn add_sound(
        &mut self,
        path: &Path,
        bytes: Vec<u8>,
        audio: Arc<flare_audio::Audio>,
    ) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let name = path
            .file_stem()
            .map_or_else(|| "Sound".to_owned(), |n| n.to_string_lossy().into_owned());
        let id = flare_core::SoundId::new();
        let sound = match flare_document::Sound::new(
            id,
            name.clone(),
            bytes,
            audio.sample_rate(),
            audio.channels(),
            audio.frames() as u64,
        ) {
            Ok(sound) => sound,
            Err(error) => return self.report(format!("Cannot import {name}: {error}")),
        };
        let layer_id = LayerId::new();
        let cue = flare_document::SoundCue {
            start: self.frame,
            sound: id,
        };
        let layer = match Layer::new(layer_id, format!("Sound: {name}"), vec![])
            .map_err(|e| e.to_string())
            .and_then(|l| l.with_cues(vec![cue]).map_err(|e| e.to_string()))
        {
            Ok(layer) => layer,
            Err(message) => return self.report(format!("Cannot import {name}: {message}")),
        };
        self.run_batch(
            vec![Command::AddSound(sound), Command::AddLayer { scene, layer }],
            "import the sound",
        );
        if self.status.is_none() {
            self.audio.decoded.insert(id, audio);
            self.current_layer = Some(layer_id);
        }
    }

    /// Asks for every sound in the project to be decoded (once), and forgets decodings of
    /// sounds that are gone.
    pub(super) fn ensure_sounds_decoded(&mut self) {
        let present: HashSet<flare_core::SoundId> = self
            .session
            .document()
            .sounds()
            .iter()
            .map(flare_document::Sound::id)
            .collect();
        self.audio.decoded.retain(|id, _| present.contains(id));
        self.audio.decode_failed.retain(|id| present.contains(id));
        let wanted: Vec<(flare_core::SoundId, Arc<[u8]>)> = self
            .session
            .document()
            .sounds()
            .iter()
            .filter(|s| {
                !self.audio.decoded.contains_key(&s.id())
                    && !self.audio.decoding.contains(&s.id())
                    && !self.audio.decode_failed.contains(&s.id())
            })
            .map(|s| (s.id(), s.shared_data()))
            .collect();
        for (sound, data) in wanted {
            self.audio.decoding.insert(sound);
            self.background
                .submit_audio(AudioJob::Decode { sound, data });
        }
        self.settle_inline();
    }

    /// Keeps the sound card in step with the timeline: sounds start when playing starts, again
    /// when the timeline wraps or jumps back, and stop when it pauses.
    pub(super) fn sync_audio(&mut self) {
        let now = (self.playing, self.frame);
        let before = self.audio.synced.replace(now);
        let rate = self.document().frame_rate();
        let fps = rate.per_second();
        let (was_playing, was_frame) = before.unwrap_or((false, 0));
        if !self.playing {
            if was_playing {
                self.audio.events.push(AudioEvent::Stop);
            }
            return;
        }
        // A jump back (the loop wrapping, or a scrub) or a jump of over a second forward starts
        // the sounds over from where the playhead is; ordinary playback does not.
        let jumped = self.frame < was_frame || f64::from(self.frame - was_frame) > fps;
        if was_playing && !jumped {
            return;
        }
        self.audio.events.push(AudioEvent::Stop);
        let Some(scene) = self.session.document().scenes().first() else {
            return;
        };
        let cues = scene
            .timeline()
            .cues()
            .filter(|(layer, cue)| layer.is_visible() && cue.start >= self.frame && !self.muted);
        for (_, cue) in cues {
            if let Some(audio) = self.audio.decoded.get(&cue.sound) {
                self.audio.events.push(AudioEvent::Play {
                    audio: Arc::clone(audio),
                    delay_seconds: f64::from(cue.start - self.frame) / fps,
                });
            }
        }
    }

    /// What to do with the sound card since the last call.
    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio.events)
    }
}
