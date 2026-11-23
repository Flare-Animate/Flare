//! File > Import > Import to Stage and Import to Library: a Flash movie (or anything else Flare
//! imports) read off the UI thread and brought into the open document by
//! `flare_editor::import_into`, as one undo step.

use std::path::Path;

use flare_editor::import_into::{Destination, import_into};

use super::*;

/// The endings Import to Stage and Import to Library offer: Flash movies, compiled clips and
/// documents.
pub const IMPORT_INTO_EXTENSIONS: [&str; 5] = ["swf", "spl", "swt", "swc", "fla"];

/// The largest file a browser reads whole to import; the desktop app reads from the disk under
/// the importers' own limits.
pub const MAX_IMPORT_INTO_BYTES: usize = 256 * 1024 * 1024;

impl Workspace {
    /// Reads `path` and brings it into this document: onto a new layer at the playhead when
    /// `to_stage`, else into the library only.
    pub fn import_into_document(&mut self, path: &Path, to_stage: bool) {
        let id = self.background.next_id();
        self.importing_into = Some((id, to_stage));
        self.background.submit_io(flare_runtime::IoJob::Import {
            id,
            path: path.to_owned(),
        });
        self.settle_inline();
    }

    /// Like [`Self::import_into_document`] for a file already in memory, named `name` (a
    /// browser has no path to read).
    pub fn import_into_document_bytes(&mut self, name: &str, bytes: Vec<u8>, to_stage: bool) {
        let id = self.background.next_id();
        self.importing_into = Some((id, to_stage));
        self.background
            .submit_io(flare_runtime::IoJob::ImportBytes {
                id,
                name: name.to_owned(),
                bytes,
            });
        self.settle_inline();
    }

    /// The read file, `result`, brought in; false when it was not this import's.
    pub(super) fn finish_import_into(
        &mut self,
        id: u64,
        path: &Path,
        result: Result<(Document, Vec<String>), String>,
    ) -> bool {
        let Some((wanted, to_stage)) = self.importing_into else {
            return false;
        };
        if wanted != id {
            return false;
        }
        self.importing_into = None;
        let shown = path.display().to_string();
        let cannot = |ws: &Self, reason: &str| {
            tr!(
                ws,
                "msg-cannot-import",
                &[("path", &shown), ("reason", reason)]
            )
        };
        let (source, mut notes) = match result {
            Ok(read) => read,
            Err(error) => {
                let text = cannot(self, &error);
                self.report(text);
                return true;
            }
        };
        let destination = match (to_stage, self.scene_id()) {
            (true, Some(scene)) => Destination::Stage {
                scene,
                frame: self.frame,
            },
            (true, None) => {
                let text = cannot(self, "there is no scene to place it in");
                self.report(text);
                return true;
            }
            (false, _) => Destination::Library,
        };
        let name = path.file_stem().map_or_else(
            || "Imported".to_owned(),
            |s| s.to_string_lossy().into_owned(),
        );
        match import_into(self.document(), &source, &name, destination) {
            Ok(done) => {
                self.run_batch(done.commands, "import the file");
                if let Some(layer) = done.layer
                    && self.layer(layer).is_some()
                {
                    self.select_layer(layer);
                }
                self.validate_selection();
                notes.extend(done.notes.iter().map(ToString::to_string));
                let count = notes.len();
                for note in notes {
                    self.log.push(note);
                }
                let text = tr_n!(self, "msg-imported", count, &[("path", &shown)]);
                self.report(text);
            }
            Err(refusal) => {
                let text = cannot(self, &refusal.to_string());
                self.report(text);
            }
        }
        true
    }
}
