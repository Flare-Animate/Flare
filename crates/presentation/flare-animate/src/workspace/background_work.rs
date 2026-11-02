//! Handing work to the runtime and taking its results.

use flare_runtime::{Done, Mode};
use std::path::Path;

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
                    .audio
                    .importing
                    .take_if(|pending| *pending == id)
                    .is_none()
                {
                    return;
                }
                match result {
                    Ok((bytes, audio)) => self.add_sound(&path, bytes, audio),
                    Err(message) => {
                        let text = tr!(
                            self,
                            "msg-cannot-import-sound",
                            &[("path", &path.display().to_string()), ("reason", &message)],
                        );
                        self.report(text);
                    }
                }
            }
            Done::SoundDecoded { sound, result } => {
                self.audio.decoding.remove(&sound);
                match result {
                    Ok(audio) => {
                        let held: usize = self.audio.decoded.values().map(|a| a.frames()).sum();
                        if held + audio.frames() > DECODED_AUDIO_SAMPLES / 2 {
                            self.audio.decode_failed.insert(sound);
                            self.report(tr!(self, "msg-too-much-sound"));
                        } else {
                            self.audio.decoded.insert(sound, audio);
                        }
                    }
                    Err(message) => {
                        self.audio.decode_failed.insert(sound);
                        let text = tr!(self, "msg-cannot-play-sound", &[("reason", &message)]);
                        self.report(text);
                    }
                }
            }
            Done::Text { id, result } => {
                let Some((_, target)) = self.text.pending.take_if(|(pending, _)| *pending == id)
                else {
                    return; // asked for again since
                };
                match result {
                    Ok(shape) => self.text_ready(shape, target),
                    Err(message) => {
                        let text = tr!(self, "msg-cannot-place-text", &[("reason", &message)]);
                        self.report(text);
                    }
                }
                // The stored letters are back in charge, or the new ones are.
                self.clear_text_preview();
            }
            Done::Families(families) => self.text.families = families,
            Done::TextPreview { id, result } => self.text_preview_arrived(id, result),
            Done::Saved {
                path: _,
                document,
                result,
            } => match result {
                Ok(saved) => {
                    self.session.mark_snapshot_saved(&document);
                    // Once saved, the recovery file for the work is obsolete.
                    if !self.session.is_dirty() {
                        self.discard_recovery();
                    }
                    self.note_recent(&saved.path);
                    self.report_saved(&saved);
                    self.path = Some(saved.path);
                }
                Err(error) => {
                    let text = tr!(self, "msg-cannot-save", &[("reason", &error.to_string())]);
                    self.report(text);
                }
            },
            Done::Autosaved { id, result } => {
                if self.recovery.in_flight == Some(id) {
                    self.recovery.in_flight = None;
                }
                match result {
                    Ok(_) => self.recovery.written = true,
                    Err(error) => {
                        let text = tr!(
                            self,
                            "msg-autosave-failed",
                            &[("reason", &error.to_string())]
                        );
                        self.report(text);
                    }
                }
            }
            Done::Imported { id, path, result } => {
                if self.importing != Some(id) {
                    return;
                }
                self.importing = None;
                match result {
                    Err(error) => {
                        let text = tr!(
                            self,
                            "msg-cannot-import",
                            &[
                                ("path", &path.display().to_string()),
                                ("reason", &error.to_string()),
                            ],
                        );
                        self.report(text);
                    }
                    Ok((document, notes)) => {
                        if self.is_blank() {
                            self.discard_recovery();
                        }
                        self.show_document(document, None);
                        // An import is somebody else's file: it counts as unsaved work until it
                        // is saved as a project.
                        self.session.mark_unsaved();
                        let skipped = notes.len();
                        for note in notes {
                            self.log.push(note);
                        }
                        let text = tr_n!(
                            self,
                            "msg-imported",
                            skipped,
                            &[("path", &path.display().to_string())],
                        );
                        self.report(text);
                    }
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
                let reverting = self.reverting.take();
                match result {
                    Err(error) => {
                        let key = if recovery {
                            "msg-cannot-recover"
                        } else {
                            "msg-cannot-open"
                        };
                        let text = self.tr_with(
                            key,
                            &[
                                ("path", &path.display().to_string()),
                                ("reason", &error.to_string()),
                            ],
                        );
                        self.report(text);
                    }
                    Ok(_) if recovery && self.is_dirty() => {
                        self.report(tr!(self, "msg-project-changed"));
                    }
                    Ok(loaded) if recovery => {
                        self.install_document(loaded.document, None);
                        self.session.mark_unsaved();
                        // The file stays until the work is saved or discarded, so a second
                        // crash before saving still leaves it recoverable.
                        self.recovery.written = true;
                        self.report(tr!(self, "msg-recovered"));
                    }
                    Ok(loaded) if !loaded.backed => {
                        // A movie, or an Animate file Flare cannot write back: somebody else's
                        // file, which counts as unsaved work until it is saved under a name.
                        if self.is_blank() {
                            self.discard_recovery();
                        }
                        self.show_document(loaded.document, None);
                        self.session.mark_unsaved();
                        self.imported_report(&path, loaded.notes);
                    }
                    Ok(loaded) if reverting == Some(id) => {
                        // Going back to the saved copy replaces the document in its own tab.
                        self.discard_recovery();
                        self.note_recent(&loaded.path);
                        self.install_document(loaded.document, Some(loaded.path));
                        self.log_import_notes(loaded.notes);
                    }
                    Ok(loaded) => {
                        // An untouched new document gives way to it; otherwise it opens beside.
                        if self.is_blank() {
                            self.discard_recovery();
                        }
                        self.note_recent(&loaded.path);
                        self.show_document(loaded.document, Some(loaded.path));
                        self.log_import_notes(loaded.notes);
                    }
                }
            }
        }
    }

    /// Tells the person an import finished, with what could not be carried over in the Output
    /// panel.
    fn imported_report(&mut self, path: &Path, notes: Vec<String>) {
        let skipped = notes.len();
        self.log_import_notes(notes);
        let text = tr_n!(
            self,
            "msg-imported",
            skipped,
            &[("path", &path.display().to_string())],
        );
        self.report(text);
    }

    fn log_import_notes(&mut self, notes: Vec<String>) {
        for note in notes {
            self.log.push(note);
        }
    }

    /// Tells the person a save finished: where, and for an Animate format what was written, with
    /// what the format could not hold in the Output panel. For a front end that wrote the file
    /// itself, as the browser build does with a download.
    pub fn report_saved(&mut self, saved: &flare_runtime::Saved) {
        let text = self.saved_message(saved);
        if let Some(summary) = &saved.summary {
            if !summary.exact && !summary.rebuilt.is_empty() {
                let names = summary.rebuilt.join(", ");
                let line = tr!(
                    self,
                    "msg-saved-rebuilt-names",
                    &[
                        ("path", &saved.path.display().to_string()),
                        ("names", &names)
                    ],
                );
                self.log.push(line);
            }
            for note in &summary.notes {
                self.log.push(note.clone());
            }
            if self.log.len() > MAX_LOG_LINES {
                let excess = self.log.len() - MAX_LOG_LINES;
                self.log.drain(..excess);
            }
        }
        self.report(text);
    }

    /// What a finished save says: where it went and, for an Animate format, what was written.
    pub(super) fn saved_message(&self, saved: &flare_runtime::Saved) -> String {
        let path = saved.path.display().to_string();
        let Some(summary) = &saved.summary else {
            return tr!(self, "msg-saved", &[("path", &path)]);
        };
        let detail = if summary.exact {
            tr!(self, "msg-saved-detail-original")
        } else if summary.carried == 0 {
            tr!(
                self,
                "msg-saved-detail-new",
                &[("files", &summary.files.to_string())],
            )
        } else {
            tr!(
                self,
                "msg-saved-detail-edited",
                &[
                    ("rebuilt", &summary.rebuilt_count.to_string()),
                    ("files", &summary.files.to_string()),
                ],
            )
        };
        if summary.warnings == 0 {
            tr!(
                self,
                "msg-saved-with",
                &[("path", &path), ("detail", &detail)]
            )
        } else {
            tr_n!(
                self,
                "msg-saved-with-notes",
                summary.warnings,
                &[("path", &path), ("detail", &detail)],
            )
        }
    }
}
