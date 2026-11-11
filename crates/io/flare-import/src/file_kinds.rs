//! Which files Flare opens and which it imports, going by their names.

use std::path::Path;

/// The file extensions Flare opens itself, and the ones it imports as a new project. Only formats
/// an importer reads belong here: the operating system offers Flare every file of these kinds
/// (`scripts/tests/test_associations.py` checks the bundle, desktop entry and installer agree).
/// OpenToonz (`.tnz`, `.tpl`, `.plt`, `.tlv`, `.pli`), Moho, BVH, FlipaClip and Flipnote files
/// come back here with their importers.
pub const PROJECT_EXTENSIONS: [&str; 1] = ["flare"];
pub const IMPORT_EXTENSIONS: [&str; 46] = [
    "fla",
    "xfl",
    "svg",
    "png",
    "jpg",
    "jpeg",
    "bmp",
    "tga",
    "tif",
    "tiff",
    "webp",
    "ico",
    "qoi",
    "pnm",
    "pbm",
    "pgm",
    "pam",
    "hdr",
    "exr",
    "dds",
    "psd",
    "mp4",
    "mov",
    "avi",
    "mkv",
    "webm",
    "mpeg",
    "mpg",
    "flv",
    "gif",
    "swf",
    "bvh",
    "fc",
    "ppm",
    "kwz",
    "tnz",
    "tpl",
    "plt",
    "tlv",
    "pli",
    "moho",
    "anime",
    "mohoproj",
    "animeproj",
    "anme",
    "wick",
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
    /// An Animate project (`.fla`, `.xfl` or an XFL folder), a Flash movie (`.swf`) or a ZIP wrapper
    /// containing one `.fla`: import it as a new project.
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
        // Accept ZIP wrappers without advertising all ZIP archives as Flare-associated files.
        || extension == "zip"
        || is_xfl_document(path)
        || crate::foreign_kind(path).is_some()
    {
        Some(PathKind::Import)
    } else {
        // An XFL project is a folder, which can have any name.
        is_dir.then_some(PathKind::Import)
    }
}

/// Formats people meet beside Flash and Toonz files, which Flare does not import, with the reason
/// the refusal names. Anything else gets the generic message.
const UNSUPPORTED: [(&str, &str); 7] = [
    (
        "as",
        "an ActionScript source file (.as) holds code, not an animation; open the .fla or .swf it belongs to",
    ),
    (
        "as2",
        "an ActionScript 2 source file (.as2) holds code, not an animation; open the .fla or .swf it belongs to",
    ),
    (
        "as3",
        "an ActionScript 3 source file (.as3) holds code, not an animation; open the .fla or .swf it belongs to",
    ),
    (
        "jsfl",
        "a JSFL script (.jsfl) runs inside Animate and is not an animation",
    ),
    (
        "sol",
        "a Flash shared object (.sol) holds saved data, not an animation",
    ),
    (
        "swc",
        "a component library (.swc) has no timeline of its own; the .swf that uses it is what opens",
    ),
    (
        "tnx",
        "the OpenToonz .tnx format is not read; open the .tnz scene instead",
    ),
];

/// Why `path` is not imported, for a name Flare knows and does not read. `None` when the name
/// is not one of those (the caller then says the file is not a kind it opens).
pub fn unsupported_reason(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    UNSUPPORTED
        .iter()
        .find(|(ext, _)| *ext == extension)
        .map(|(_, reason)| *reason)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_unsupported_kind_names_its_reason_and_others_do_not() {
        for name in [
            "a.as", "a.AS2", "a.as3", "a.jsfl", "a.sol", "a.swc", "a.tnx",
        ] {
            let reason = unsupported_reason(Path::new(name));
            assert!(reason.is_some_and(|r| !r.is_empty()), "{name}");
        }
        assert_eq!(unsupported_reason(Path::new("a.fla")), None);
        assert_eq!(unsupported_reason(Path::new("a.txt")), None);
        assert_eq!(unsupported_reason(Path::new("a")), None);
    }

    #[test]
    fn paths_are_told_apart_by_extension_in_any_case_and_folders_are_xfl() {
        let kind = |name: &str, dir: bool| classify(Path::new(name), dir);
        assert_eq!(kind("a.flare", false), Some(PathKind::Project));
        assert_eq!(kind("A.FLARE", false), Some(PathKind::Project));
        assert_eq!(kind("a.fla", false), Some(PathKind::Import));
        assert_eq!(kind("a.SWF", false), Some(PathKind::Import));
        assert_eq!(kind("a.XFL", false), Some(PathKind::Import));
        assert_eq!(kind("wrapper.zip", false), Some(PathKind::Import));
        // Formats no importer reads are not claimed, so the system never offers them to Flare.
        for name in ["a.fcp", "a.tzu", "a.mh", "a.mohobackup"] {
            assert_eq!(kind(name, false), None, "{name}");
        }
        for name in [
            "a.bvh",
            "a.fc",
            "a.ppm",
            "a.kwz",
            "a.tnz",
            "a.tpl",
            "a.plt",
            "a.tlv",
            "a.pli",
            "a.moho",
            "a.anime",
            "a.mohoproj",
            "a.animeproj",
            "a.anme",
            "a.wick",
        ] {
            assert_eq!(kind(name, false), Some(PathKind::Import), "{name}");
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
        assert_eq!(kind("a.jpg", false), Some(PathKind::Import));
        assert_eq!(kind("a.JPEG", false), Some(PathKind::Import));
        assert_eq!(kind("a.bmp", false), Some(PathKind::Import));
        assert_eq!(kind("a.TGA", false), Some(PathKind::Import));
        for name in [
            "a.tif", "a.TIFF", "a.webp", "a.ico", "a.qoi", "a.pnm", "a.pam", "a.hdr", "a.exr",
            "a.dds", "a.psd", "a.mp4", "a.MOV", "a.avi", "a.mkv", "a.webm", "a.mpeg", "a.mpg",
            "a.flv",
        ] {
            assert_eq!(kind(name, false), Some(PathKind::Import), "{name}");
        }
        assert_eq!(kind("a", false), None);
        assert_eq!(kind("a.flare.txt", false), None);
    }
}
