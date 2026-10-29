//! Opening, saving and crash recovery.

use flare_document::Document;
use flare_runtime::{IoJob, Mode};
use std::path::{Path, PathBuf};

use super::*;

impl Workspace {
    // ---- file operations -------------------------------------------------------------

    /// Replaces the document being edited, keeping the user's tool and environment settings
    /// (tool, colors, native menu, recovery, panel layout, onion skin) rather than resetting them with it.
    pub(super) fn install_document(&mut self, document: Document, path: Option<PathBuf>) {
        let mut fresh = Self::new(document, path);
        fresh.tool = self.tool;
        fresh.fill = self.fill;
        fresh.gradient_enabled = self.gradient_enabled;
        fresh.gradient_end = self.gradient_end;
        fresh.gradient_vertical = self.gradient_vertical;
        fresh.text_family = std::mem::take(&mut self.text_family);
        fresh.text_size = self.text_size;
        fresh.text_bold = self.text_bold;
        fresh.text_italic = self.text_italic;
        fresh.stroke_enabled = self.stroke_enabled;
        fresh.stroke_color = self.stroke_color;
        fresh.stroke_width = self.stroke_width;
        fresh.stroke_cap = self.stroke_cap;
        fresh.stroke_join = self.stroke_join;
        fresh.native_menu = self.native_menu;
        fresh.titlebar_inset = self.titlebar_inset;
        fresh.recovery_dir = self.recovery_dir.take();
        fresh.autosave_interval = self.autosave_interval;
        fresh.layout = std::mem::replace(
            &mut self.layout,
            egui_tiles::Tree::empty(egui::Id::unique("taken")),
        );
        fresh.onion = self.onion;
        // Whatever was playing belonged to the old project.
        fresh.audio_events = std::mem::take(&mut self.audio_events);
        fresh.audio_events.push(AudioEvent::Stop);
        // The workers belong to the app, not to a document; results of the old document's jobs
        // find nothing waiting for them and are dropped.
        fresh.background = std::mem::replace(&mut self.background, new_runtime());
        *self = fresh;
    }

    /// Opens a project, refusing to replace unsaved work. The file is read off the UI thread.
    pub fn open(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before opening another project");
            return;
        }
        self.start_open(path, false);
    }

    /// Reads an XFL folder or a `.fla` into a new, unsaved project, refusing to replace unsaved
    /// work. The file is read off the UI thread.
    pub fn import_project(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before importing another project");
            return;
        }
        let id = self.background.next_id();
        self.importing = Some(id);
        self.background.submit_io(IoJob::Import {
            id,
            path: path.to_owned(),
        });
        self.settle_inline();
    }

    /// Like [`Self::import_project`] for a `.fla` already in memory, named `name`.
    pub fn import_project_bytes(&mut self, name: &str, bytes: Vec<u8>) {
        if self.is_dirty() {
            self.report("Save or undo your changes before importing another project");
            return;
        }
        let id = self.background.next_id();
        self.importing = Some(id);
        self.background.submit_io(IoJob::ImportBytes {
            id,
            name: name.to_owned(),
            bytes,
        });
        self.settle_inline();
    }

    /// Starts an empty project, refusing to replace unsaved work.
    pub(super) fn new_document(&mut self) {
        if self.is_dirty() {
            self.report("Save or undo your changes before starting a new project");
            return;
        }
        match Self::untitled() {
            Ok(fresh) => {
                let document = fresh.session.document().clone();
                self.discard_recovery();
                self.install_document(document, None);
            }
            Err(error) => self.report(error),
        }
    }

    /// Goes back to the saved copy of the project, giving up the changes made since.
    pub(super) fn revert(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        // Giving up the changes is the point, so the dirty check does not apply to this open.
        self.revert_pending = true;
        self.start_open(&path, false);
    }

    pub(super) fn start_open(&mut self, path: &Path, recovery: bool) {
        let id = self.background.next_id();
        self.opening = Some(id);
        if std::mem::take(&mut self.revert_pending) {
            self.reverting = Some(id);
        }
        self.background.submit_io(IoJob::Open {
            id,
            path: path.to_owned(),
            recovery,
        });
        if self.background.mode() == Mode::Threads {
            self.status = Some(format!("Opening {}...", path.display()));
        }
        self.settle_inline();
    }

    // ---- crash recovery --------------------------------------------------------------

    /// Turns on autosave into `dir`. Recovery files are separate from the project file.
    pub fn enable_recovery(&mut self, dir: PathBuf) {
        self.recovery_dir = Some(dir);
    }

    pub fn set_autosave_interval(&mut self, seconds: f64) {
        self.autosave_interval = seconds.max(1.0);
    }

    /// Loads a recovery file as unsaved work (it counts as dirty until saved to a project).
    pub fn recover(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before recovering other work");
            return;
        }
        self.start_open(path, true);
    }

    /// Deletes the recovery file for the current document, if there is one.
    pub fn discard_recovery(&mut self) {
        self.recovery_written = false;
        if let Some(dir) = &self.recovery_dir
            && let Err(error) = flare_storage::discard_recovery(dir, self.session.document().id())
        {
            self.status = Some(format!("Could not remove the recovery file: {error}"));
        }
    }

    /// Writes a recovery file when there are unsaved changes and the interval has passed.
    /// Only the document itself is saved: an in-progress symbol edit is not applied yet.
    pub(super) fn autosave(&mut self, now: f64) {
        let Some(dir) = self.recovery_dir.clone() else {
            return;
        };
        if !self.session.is_dirty() {
            // Clean again (saved, or undone back to the saved state): nothing to recover.
            if self.recovery_written {
                self.discard_recovery();
            }
            self.last_autosave = Some(now);
            return;
        }
        if self
            .last_autosave
            .is_some_and(|last| now - last < self.autosave_interval)
        {
            return;
        }
        if self.autosaving.is_some() {
            return; // the last one is still being written
        }
        self.last_autosave = Some(now);
        let id = self.background.next_id();
        self.autosaving = Some(id);
        self.background.submit_io(IoJob::Autosave {
            id,
            dir,
            document: self.session.snapshot(),
        });
        self.settle_inline();
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(path) => self.save_as(&path),
            None => self.dialog = Some(DialogRequest::SaveAs),
        }
    }

    pub fn save_as(&mut self, path: &Path) {
        // Saving writes the whole document, so pending symbol edits are applied first.
        if !self.exit_all_symbols() {
            return;
        }
        // The file is written off the UI thread from a snapshot, so editing carries on while it
        // is written; whatever changes meanwhile simply stays unsaved.
        self.background.submit_io(IoJob::Save {
            path: path.to_owned(),
            document: self.session.snapshot(),
        });
        self.settle_inline();
    }
}
