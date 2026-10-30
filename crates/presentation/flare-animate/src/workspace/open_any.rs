//! Opening whatever file or folder the system hands over: a double-clicked file, a command-line
//! argument or a file dropped on the window.

use std::path::Path;

use super::*;

/// The file extensions Flare opens itself, and the ones it imports as a new project.
pub const PROJECT_EXTENSIONS: [&str; 1] = ["flare"];
pub const IMPORT_EXTENSIONS: [&str; 2] = ["fla", "xfl"];

/// What to do with a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    /// A Flare project: open it.
    Project,
    /// An Animate project (`.fla`, `.xfl` or an XFL folder): import it as a new project.
    Import,
}

/// What `path` is, going by its name, or `is_dir` for a folder. `None` for anything else.
pub fn classify(path: &Path, is_dir: bool) -> Option<PathKind> {
    let extension = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase)
        .unwrap_or_default();
    if PROJECT_EXTENSIONS.contains(&extension.as_str()) {
        Some(PathKind::Project)
    } else if IMPORT_EXTENSIONS.contains(&extension.as_str()) {
        Some(PathKind::Import)
    } else {
        // An XFL project is a folder, which can have any name.
        is_dir.then_some(PathKind::Import)
    }
}

impl Workspace {
    /// Opens `path` the way its kind says: a project is opened, an Animate project imported.
    /// Anything else is reported and nothing changes. The reading is done off the UI thread.
    pub fn open_any(&mut self, path: &Path) {
        match classify(path, path.is_dir()) {
            Some(PathKind::Project) => self.open(path),
            Some(PathKind::Import) => self.import_project(path),
            None => self.report(format!(
                "Flare opens .flare projects and imports .fla and .xfl files, not {}",
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
