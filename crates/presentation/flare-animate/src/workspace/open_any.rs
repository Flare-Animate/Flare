//! Opening whatever file or folder the system hands over: a double-clicked file, a command-line
//! argument or a file dropped on the window.

use std::path::Path;

use flare_import::{PathKind, classify};

use super::*;

impl Workspace {
    /// Opens `path` the way its kind says: a project is opened, an Animate project imported.
    /// Anything else is reported and nothing changes. The reading is done off the UI thread.
    pub fn open_any(&mut self, path: &Path) {
        match classify(path, path.is_dir()) {
            Some(PathKind::Project) => self.open(path),
            Some(PathKind::Import) => self.import_project(path),
            None => self.report(format!(
                "Flare opens .flare projects and imports .fla, .xfl and .swf files and XFL folders, not {}",
                path.file_name().map_or_else(
                    || path.display().to_string(),
                    |name| name.to_string_lossy().into_owned()
                )
            )),
        }
    }

    /// Opens the files dropped on the window this pass, and leaves Home for them. In a browser a
    /// dropped file has a name but no path, so there is nothing to open from here.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn open_dropped(&mut self, ctx: &egui::Context) {
        let dropped: Vec<std::path::PathBuf> = ctx.input(|i| {
            i.raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_owned())
                .take(MAX_DROPPED)
                .collect()
        });
        for path in dropped {
            self.open_any(&path);
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub(super) fn open_dropped(&mut self, _ctx: &egui::Context) {}
}

#[cfg(not(target_arch = "wasm32"))]
/// Most files taken from one drop, so a huge selection cannot start a huge number of jobs.
const MAX_DROPPED: usize = 16;
