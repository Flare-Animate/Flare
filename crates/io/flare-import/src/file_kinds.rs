//! Which files Flare opens and which it imports, going by their names.

use std::path::Path;

/// The file extensions Flare opens itself, and the ones it imports as a new project. Only formats
/// an importer reads belong here: the operating system offers Flare every file of these kinds
/// (`scripts/tests/test_associations.py` checks the bundle, desktop entry and installer agree).
/// OpenToonz (`.tnz`, `.tlv`, `.pli`, `.tzu`), Moho (`.moho`, `.mh`, `.mohobackup`) and the
/// `.fcp`, `.ppm` and `.kwz` files come back here with their importers.
pub const PROJECT_EXTENSIONS: [&str; 1] = ["flare"];
pub const IMPORT_EXTENSIONS: [&str; 16] = [
    "fla",
    "xfl",
    "svg",
    "png",
    "gif",
    "swf",
    "fcp",
    "ppm",
    "kwz",
    "tnz",
    "tlv",
    "pli",
    "tzu",
    "moho",
    "mh",
    "mohobackup",
];

/// The file inside an XFL folder that is the document. A person who has only the folder, with no
/// `.xfl` beside it, picks this one.
pub const XFL_DOCUMENT_FILE: &str = "DOMDocument.xml";

/// Whether `path` is an XFL folder's document file.
pub fn is_xfl_document(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case(XFL_DOCUMENT_FILE))
}

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
    } else if IMPORT_EXTENSIONS.contains(&extension.as_str())
        || is_xfl_document(path)
        || crate::foreign_kind(path).is_some()
    {
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
        // Formats no importer reads are not claimed, so the system never offers them to Flare.
        for name in [
            "a.fcp", "a.ppm", "a.kwz", "a.tnz", "a.tlv", "a.pli", "a.tzu", "a.moho", "a.mh",
        ] {
            assert_eq!(kind(name, false), None, "{name}");
        }
        assert_eq!(
            kind("MyProject", true),
            Some(PathKind::Import),
            "an XFL folder has any name"
        );
        assert_eq!(
            kind("MyProject/DOMDocument.xml", false),
            Some(PathKind::Import),
            "an XFL folder's document opens that folder"
        );
        assert_eq!(
            kind("MyProject/domdocument.XML", false),
            Some(PathKind::Import)
        );
        assert_eq!(kind("notes.xml", false), None);
        assert_eq!(kind("a.png", false), Some(PathKind::Import));
        assert_eq!(kind("a.jpg", false), None);
        assert_eq!(kind("a", false), None);
        assert_eq!(kind("a.flare.txt", false), None);
    }
}
