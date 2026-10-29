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

    /// The sounds that only the cues of `layer` play: what deleting the layer would leave unused.
    pub(super) fn orphaned_sounds(&self, layer: LayerId) -> Vec<flare_core::SoundId> {
        let document = self.session.document();
        let timelines = document
            .scenes()
            .iter()
            .map(|s| s.timeline())
            .chain(document.symbols().iter().map(|s| s.timeline()));
        let (mut own, mut elsewhere) = (Vec::new(), HashSet::new());
        for timeline in timelines {
            for l in timeline.layers() {
                if l.id() == layer {
                    own.extend(l.cues().iter().map(|c| c.sound));
                } else {
                    elsewhere.extend(l.cues().iter().map(|c| c.sound));
                }
            }
        }
        own.sort_by_key(|id| *id.as_uuid());
        own.dedup();
        own.retain(|id| !elsewhere.contains(id));
        own
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
        let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
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
        for layer in scene.timeline().layers().iter().filter(|l| l.is_visible()) {
            for cue in layer
                .cues()
                .iter()
                .filter(|c| c.start >= self.frame && !self.muted)
            {
                if let Some(audio) = self.audio.decoded.get(&cue.sound) {
                    self.audio.events.push(AudioEvent::Play {
                        audio: Arc::clone(audio),
                        delay_seconds: f64::from(cue.start - self.frame) / fps,
                    });
                }
            }
        }
    }

    /// What to do with the sound card since the last call.
    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio.events)
    }
}
