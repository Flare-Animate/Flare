//! Which files Flare opens and which it imports, going by their names.

use std::path::Path;

/// The file extensions Flare opens itself, and the ones it imports as a new project.
pub const PROJECT_EXTENSIONS: [&str; 1] = ["flare"];
pub const IMPORT_EXTENSIONS: [&str; 3] = ["fla", "xfl", "swf"];

/// What to do with a path.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PathKind {
    /// A Flare project: open it.
    Project,
    /// An Animate project (`.fla`, `.xfl` or an XFL folder) or a Flash movie (`.swf`): import it as a new project.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_told_apart_by_extension_in_any_case_and_folders_are_xfl() {
        let kind = |name: &str, dir: bool| classify(Path::new(name), dir);
        assert_eq!(kind("a.flare", false), Some(PathKind::Project));
        assert_eq!(kind("A.FLARE", false), Some(PathKind::Project));
        assert_eq!(kind("a.fla", false), Some(PathKind::Import));
        assert_eq!(kind("a.SWF", false), Some(PathKind::Import));
        assert_eq!(kind("a.XFL", false), Some(PathKind::Import));
        assert_eq!(
            kind("MyProject", true),
            Some(PathKind::Import),
            "an XFL folder has any name"
        );
        assert_eq!(kind("a.png", false), None);
        assert_eq!(kind("a", false), None);
        assert_eq!(kind("a.flare.txt", false), None);
    }
}
