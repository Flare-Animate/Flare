//! Importing sounds and keeping the sound card in step.

use flare_commands::Command;
use flare_core::LayerId;
use flare_document::Layer;
use flare_runtime::{AudioJob, Mode};
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use flare_audio::Excerpt;

use super::*;

/// The shortest slice a scrub plays. One frame at 24 fps is about 42 ms, which is heard; at 60
/// fps a frame is a click, so a slice is never shorter than this.
const MIN_SCRUB_SECONDS: f64 = 0.05;

impl Workspace {
    // ---- sound ------------------------------------------------------------------------

    /// Imports a sound file: it is read and decoded on a worker, then added to the project with
    /// a cue at the current frame on a new layer, as one undo step.
    pub fn import_sound(&mut self, path: &Path) {
        if self.in_symbol() {
            self.report(tr!(self, "msg-leave-symbol-sound"));
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

    /// Imports an audio file already held in memory, as by a browser file picker.
    pub fn import_sound_bytes(&mut self, name: &str, bytes: Vec<u8>) {
        if self.in_symbol() {
            self.report(tr!(self, "msg-leave-symbol-sound"));
            return;
        }
        let id = self.background.next_id();
        self.audio.importing = Some(id);
        self.background.submit_audio(AudioJob::ImportBytes {
            id,
            name: name.to_owned(),
            bytes,
        });
        if self.background.mode() == Mode::Threads {
            self.status = Some(format!("Importing {name}..."));
        }
        self.settle_inline();
    }

    pub(super) fn add_sound(&mut self, path: &Path, bytes: Vec<u8>, audio: flare_audio::Decoded) {
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
            audio.audio.sample_rate(),
            audio.audio.channels(),
            audio.audio.frames() as u64,
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
            // The layer being drawn on stays the active one, so frames added after the import
            // (F5 at a frame ahead) lengthen the drawing, not the sound's own layer.
            self.audio.decoded.insert(id, audio);
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
                self.audio.scrubbing = false;
            } else if before.is_some_and(|(_, frame)| frame != self.frame) {
                self.scrub_audio(fps);
            }
            return;
        }
        self.audio.scrubbing = false;
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
            if let Some(decoded) = self.audio.decoded.get(&cue.sound) {
                self.audio.events.push(AudioEvent::Play {
                    audio: Arc::clone(&decoded.audio),
                    delay_seconds: f64::from(cue.start - self.frame) / fps,
                    excerpt: Excerpt::WHOLE,
                });
            }
        }
    }

    /// The paused playhead moved (dragged along the timeline, or stepped): what was sounding
    /// from the last move stops, and each sound under the playhead plays the stretch of it that
    /// this frame covers, so a person can find a word by ear as in Animate.
    fn scrub_audio(&mut self, fps: f64) {
        let slices = self.scrub_slices(fps);
        if self.audio.scrubbing || !slices.is_empty() {
            self.audio.events.push(AudioEvent::Stop);
        }
        self.audio.scrubbing = !slices.is_empty();
        for (audio, excerpt) in slices {
            self.audio.events.push(AudioEvent::Play {
                audio,
                delay_seconds: 0.0,
                excerpt,
            });
        }
    }

    /// The sounds a scrub at the playhead plays, each with the slice of it under the playhead.
    fn scrub_slices(&self, fps: f64) -> Vec<(Arc<flare_audio::Audio>, Excerpt)> {
        let Some(scene) = self.session.document().scenes().first() else {
            return Vec::new();
        };
        if self.muted || self.in_symbol() {
            return Vec::new();
        }
        let length = (1.0 / fps).max(MIN_SCRUB_SECONDS);
        scene
            .timeline()
            .cues()
            .filter(|(layer, cue)| layer.is_visible() && cue.start <= self.frame)
            .filter_map(|(_, cue)| {
                let decoded = self.audio.decoded.get(&cue.sound)?;
                let offset = f64::from(self.frame - cue.start) / fps;
                (offset < decoded.audio.seconds())
                    .then(|| (Arc::clone(&decoded.audio), Excerpt::slice(offset, length)))
            })
            .collect()
    }

    /// The waveform of a sound in the project, once it has been decoded.
    pub(super) fn sound_peaks(&self, sound: flare_core::SoundId) -> Option<&flare_audio::Peaks> {
        self.audio.decoded.get(&sound).map(|d| d.peaks.as_ref())
    }

    /// What to do with the sound card since the last call.
    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio.events)
    }
}
