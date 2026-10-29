//! Opening, saving and crash recovery.

use flare_document::Document;
use flare_runtime::{IoJob, Mode};
use std::path::{Path, PathBuf};

use super::*;

impl Workspace {
    // ---- file operations -------------------------------------------------------------

    /// Takes over what belongs to the person and the app rather than to a document: the tool,
    /// colors, settings, panel layout, the runtime's workers and what is playing. `from` is left
    /// with placeholders, so it should be dropped or parked, not used.
    pub(super) fn adopt_environment(&mut self, from: &mut Workspace) {
        self.tool = from.tool;
        self.paint.fill = from.paint.fill;
        self.paint.gradient_enabled = from.paint.gradient_enabled;
        self.paint.gradient_end = from.paint.gradient_end;
        self.paint.gradient_vertical = from.paint.gradient_vertical;
        self.text.family = std::mem::take(&mut from.text.family);
        self.text.size = from.text.size;
        self.text.bold = from.text.bold;
        self.text.italic = from.text.italic;
        self.paint.stroke_enabled = from.paint.stroke_enabled;
        self.paint.stroke_color = from.paint.stroke_color;
        self.paint.stroke_width = from.paint.stroke_width;
        self.paint.stroke_cap = from.paint.stroke_cap;
        self.paint.stroke_join = from.paint.stroke_join;
        self.native_menu = from.native_menu;
        self.titlebar_inset = from.titlebar_inset;
        self.recovery.dir = from.recovery.dir.take();
        self.apply_preferences(&from.preferences());
        self.layout = std::mem::replace(
            &mut from.layout,
            egui_tiles::Tree::empty(egui::Id::unique("taken")),
        );
        self.onion = from.onion;
        // Whatever was playing belonged to the old project.
        self.audio.events = std::mem::take(&mut from.audio.events);
        self.audio.events.push(AudioEvent::Stop);
        // The workers belong to the app, not to a document; results of the old document's jobs
        // find nothing waiting for them and are dropped.
        self.background = std::mem::replace(&mut from.background, new_runtime());
    }

    /// Replaces the document being edited in its tab, keeping the person's environment.
    pub(super) fn install_document(&mut self, document: Document, path: Option<PathBuf>) {
        let mut fresh = Self::new(document, path);
        fresh.adopt_environment(self);
        fresh.parked = std::mem::take(&mut self.parked);
        fresh.position = self.position;
        *self = fresh;
    }

    /// Opens a project, refusing to replace unsaved work. The file is read off the UI thread.
    pub fn open(&mut self, path: &Path) {
        self.start_open(path, false);
    }

    /// Reads an XFL folder or a `.fla` into a new, unsaved project, in a new tab unless the current
    /// document is untouched. The file is read off the UI thread.
    pub fn import_project(&mut self, path: &Path) {
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
        let id = self.background.next_id();
        self.importing = Some(id);
        self.background.submit_io(IoJob::ImportBytes {
            id,
            name: name.to_owned(),
            bytes,
        });
        self.settle_inline();
    }

    /// Replaces the document in its tab with an empty one. The caller has made sure that loses
    /// nothing.
    pub(super) fn new_document_in_place(&mut self) {
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
        self.recovery.dir = Some(dir);
    }

    pub fn set_autosave_interval(&mut self, seconds: f64) {
        self.recovery.interval = seconds.max(1.0);
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
        self.recovery.written = false;
        if let Some(dir) = &self.recovery.dir
            && let Err(error) = flare_storage::discard_recovery(dir, self.session.document().id())
        {
            self.status = Some(format!("Could not remove the recovery file: {error}"));
        }
    }

    /// Writes a recovery file when there are unsaved changes and the interval has passed.
    /// Only the document itself is saved: an in-progress symbol edit is not applied yet.
    pub(super) fn autosave(&mut self, now: f64) {
        let Some(dir) = self.recovery.dir.clone() else {
            return;
        };
        if !self.session.is_dirty() {
            // Clean again (saved, or undone back to the saved state): nothing to recover.
            if self.recovery.written {
                self.discard_recovery();
            }
            self.recovery.last = Some(now);
            return;
        }
        if self
            .recovery
            .last
            .is_some_and(|last| now - last < self.recovery.interval)
        {
            return;
        }
        if self.recovery.in_flight.is_some() {
            return; // the last one is still being written
        }
        self.recovery.last = Some(now);
        let id = self.background.next_id();
        self.recovery.in_flight = Some(id);
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
