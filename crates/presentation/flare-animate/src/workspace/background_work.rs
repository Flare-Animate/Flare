//! Handing work to the runtime and taking its results.

use flare_runtime::{Done, Mode};

use super::*;

impl Workspace {
    // ---- background work -------------------------------------------------------------

    /// Moves drawing and file work onto worker threads. Until this is called it runs on the
    /// calling thread, which keeps headless use and tests deterministic.
    pub fn start_background_threads(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        self.background
            .start_threads(std::sync::Arc::new(move || ctx.request_repaint()));
    }

    /// Whether a save, open or autosave is still being worked on.
    pub fn has_pending_io(&self) -> bool {
        self.background.io_pending()
    }

    /// Waits for every save, open and autosave started so far and applies their results. For
    /// closing the app only: everything else keeps the UI free of waiting.
    pub fn finish_background(&mut self) {
        for done in self.background.finish_io() {
            self.handle_done(done);
        }
    }

    /// Applies the results of finished work. Never blocks.
    pub(super) fn drain_background(&mut self) {
        for done in self.background.poll() {
            self.handle_done(done);
        }
    }

    /// With no worker threads a job has already finished by the time it was handed over, so its
    /// result is applied at once, as though the work had been instant.
    pub(super) fn settle_inline(&mut self) {
        if self.background.mode() == Mode::Inline {
            self.drain_background();
        }
    }

    pub(super) fn handle_done(&mut self, done: Done) {
        match done {
            Done::Rendered { id, result } => {
                self.render_inbox.push((id, result.map(into_color_image)))
            }
            Done::SoundImported { id, path, result } => {
                if self
                    .importing_sound
                    .take_if(|pending| *pending == id)
                    .is_none()
                {
                    return;
                }
                match result {
                    Ok((bytes, audio)) => self.add_sound(&path, bytes, audio),
                    Err(message) => {
                        self.report(format!("Cannot import {}: {message}", path.display()));
                    }
                }
            }
            Done::SoundDecoded { sound, result } => {
                self.decoding.remove(&sound);
                match result {
                    Ok(audio) => {
                        let held: usize = self.decoded.values().map(|a| a.frames()).sum();
                        if held + audio.frames() > DECODED_AUDIO_SAMPLES / 2 {
                            self.decode_failed.insert(sound);
                            self.report("Too much sound is loaded to play every sound at once");
                        } else {
                            self.decoded.insert(sound, audio);
                        }
                    }
                    Err(message) => {
                        self.decode_failed.insert(sound);
                        self.report(format!("Cannot play a sound: {message}"));
                    }
                }
            }
            Done::Text { id, result } => {
                let Some((_, at)) = self.text_pending.take_if(|(pending, _)| *pending == id) else {
                    return; // asked for again since
                };
                match result {
                    Ok(outline) => self.place_outline(outline, at),
                    Err(message) => self.report(format!("Cannot place text: {message}")),
                }
            }
            Done::Saved {
                path,
                document,
                result,
            } => match result {
                Ok(()) => {
                    self.session.mark_snapshot_saved(&document);
                    // Once saved, the recovery file for the work is obsolete.
                    if !self.session.is_dirty() {
                        self.discard_recovery();
                    }
                    self.path = Some(path.clone());
                    self.report(format!("Saved {}", path.display()));
                }
                Err(error) => self.report(format!("Cannot save: {error}")),
            },
            Done::Autosaved { id, result } => {
                if self.autosaving == Some(id) {
                    self.autosaving = None;
                }
                match result {
                    Ok(_) => self.recovery_written = true,
                    Err(error) => self.report(format!("Autosave failed: {error}")),
                }
            }
            Done::Opened {
                id,
                path,
                recovery,
                result,
            } => {
                if self.opening != Some(id) {
                    return; // superseded by a later request
                }
                self.opening = None;
                match result {
                    Err(error) => {
                        let what = if recovery { "recover" } else { "open" };
                        self.report(format!("Cannot {what} {}: {error}", path.display()));
                    }
                    Ok(_) if self.is_dirty() => {
                        self.report("Your project changed while the file was opening, so it was not replaced");
                    }
                    Ok(document) if recovery => {
                        self.install_document(document, None);
                        self.session.mark_unsaved();
                        // The file stays until the work is saved or discarded, so a second
                        // crash before saving still leaves it recoverable.
                        self.recovery_written = true;
                        self.report("Recovered unsaved work");
                    }
                    Ok(document) => {
                        // The old document is clean, so any recovery file for it is obsolete.
                        self.discard_recovery();
                        self.install_document(document, Some(path));
                    }
                }
            }
        }
    }
}
