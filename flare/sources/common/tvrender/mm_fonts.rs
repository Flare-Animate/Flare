//! The system's fonts: an index of every face in the OS font folders, found
//! by the names XFL uses, PostScript names ("ArialMT", "Arial-BoldMT") or
//! family names ("Arial"). A face's file is read on first use and kept.

use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use skrifa::Tag;
use skrifa::raw::tables::name::{Name, NameId, NameRecord};
use skrifa::raw::tables::os2::{Os2, SelectionFlags};
use skrifa::raw::{FileRef, FontData, FontRead};

/// One face of an installed font.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceInfo {
    pub path: PathBuf,
    /// The face's index in a font collection (.ttc), else 0.
    pub index: u32,
    /// The typographic family: "Segoe UI" holds Segoe UI Semibold too.
    pub family: String,
    /// "Regular", "Bold Italic", "Semibold"...
    pub style: String,
    pub postscript: String,
    /// 100 (thin) to 900 (black): 400 is regular, 700 bold.
    pub weight: u16,
    /// 1 (ultra-condensed) to 9 (ultra-expanded): 5 is normal, Arial Narrow 3.
    pub stretch: u16,
    pub italic: bool,
}

/// Every face in the system's font folders, by family, then normal widths
/// first, then weight. The folders are scanned on first use.
pub fn system() -> &'static [FaceInfo] {
    static INDEX: OnceLock<Vec<FaceInfo>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut faces = Vec::new();
        for dir in font_dirs() {
            scan(&dir, 0, &mut faces);
        }
        faces.sort_by_cached_key(|f| (f.family.to_lowercase(), f.family.clone(), f.stretch.abs_diff(5), f.weight, f.italic, f.style.clone()));
        faces
    })
}

/// The installed families, in order, each once.
pub fn families() -> Vec<&'static str> {
    let mut names: Vec<&str> = system().iter().map(|f| f.family.as_str()).collect();
    names.dedup();
    names
}

/// The face XFL's `face` attribute names. A PostScript name picks that face, a
/// family name its regular one; `bold` and `italic` then ask for a bolder or
/// italic face of the family. Device fonts (_sans, _serif, _typewriter) are
/// Arial, Times New Roman and Courier New, and missing fonts fall back to
/// Arial, then to any font. None only if no fonts are installed.
pub fn resolve(face: &str, bold: bool, italic: bool) -> Option<&'static FaceInfo> {
    resolve_in(system(), face, bold, italic)
}

/// The installed face `face` names, a PostScript name or a family's regular
/// face, without falling back to another font.
pub fn find(face: &str) -> Option<&'static FaceInfo> {
    let key = squash(face);
    system().iter().find(|f| squash(&f.postscript) == key).or_else(|| style_in(system(), face, false, false))
}

/// The font file holding `face`, read once and kept.
pub fn load(face: &FaceInfo) -> Option<Arc<Vec<u8>>> {
    static FILES: OnceLock<Mutex<HashMap<PathBuf, Arc<Vec<u8>>>>> = OnceLock::new();
    let mut files = FILES.get_or_init(Default::default).lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(data) = files.get(&face.path) {
        return Some(data.clone());
    }
    let data = Arc::new(std::fs::read(&face.path).ok()?);
    files.insert(face.path.clone(), data.clone());
    Some(data)
}

// ponytail: every lookup scans all faces, squashing each name, once per run per layout. Key a
// HashMap by squashed name if documents with much text render slowly.
fn resolve_in<'a>(faces: &'a [FaceInfo], face: &str, bold: bool, italic: bool) -> Option<&'a FaceInfo> {
    let key = squash(face);
    if let Some(f) = faces.iter().find(|f| squash(&f.postscript) == key) {
        if (bold && f.weight < 600) || (italic && !f.italic) {
            // Only a face with both styles will do; otherwise the face named wins.
            let (bold, italic) = (bold || f.weight >= 600, italic || f.italic);
            let both = style_in(faces, &f.family, bold, italic).filter(|g| (g.weight >= 600) == bold && g.italic == italic);
            return both.or(Some(f));
        }
        return Some(f);
    }
    let family = match face {
        "_sans" => "Arial",
        "_serif" => "Times New Roman",
        "_typewriter" => "Courier New",
        name => name,
    };
    style_in(faces, family, bold, italic).or_else(|| style_in(faces, "Arial", bold, italic)).or(faces.first())
}

/// The face of `family` closest to the style asked for: the right slant first,
/// then the nearest weight, then the most normal width.
fn style_in<'a>(faces: &'a [FaceInfo], family: &str, bold: bool, italic: bool) -> Option<&'a FaceInfo> {
    let (key, weight) = (squash(family), if bold { 700 } else { 400 });
    let same = faces.iter().filter(|f| squash(&f.family) == key);
    same.min_by_key(|f| (f.italic != italic, f.weight.abs_diff(weight), f.stretch.abs_diff(5)))
}

/// A name as XFL may spell it: "Times New Roman" and "TimesNewRoman" match.
fn squash(name: &str) -> String {
    name.chars().filter(char::is_ascii_alphanumeric).map(|c| c.to_ascii_lowercase()).collect()
}

/// Windows' font folders, then macOS's and Linux's.
fn font_dirs() -> Vec<PathBuf> {
    let var = |name: &str| std::env::var_os(name).map(PathBuf::from);
    let mut dirs: Vec<PathBuf> = Vec::new();
    dirs.extend(var("WINDIR").map(|d| d.join("Fonts")));
    dirs.extend(var("LOCALAPPDATA").map(|d| d.join("Microsoft").join("Windows").join("Fonts")));
    dirs.extend(["/Library/Fonts", "/System/Library/Fonts", "/usr/share/fonts", "/usr/local/share/fonts"].map(PathBuf::from));
    dirs.extend(var("HOME").map(|d| d.join(".local").join("share").join("fonts")));
    dirs
}

fn scan(dir: &Path, depth: u32, faces: &mut Vec<FaceInfo>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|e| e.path()) {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or_default().to_ascii_lowercase();
        if path.is_dir() {
            if depth < 4 {
                scan(&path, depth + 1, faces);
            }
        } else if matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc") {
            faces.extend(read_faces(&path));
        }
    }
}

/// The faces in one font file. Only its table directories and its name and
/// OS/2 tables are read: whole files take seconds for a full Windows font folder.
fn read_faces(path: &Path) -> Vec<FaceInfo> {
    let Ok(mut file) = File::open(path) else { return Vec::new() };
    let mut head = Vec::new();
    // ponytail: a collection's table directories must sit in its first 16 KiB, as in every
    // font seen so far; read on until they're all in if one turns up that's laid out otherwise.
    if (&mut file).take(16 * 1024).read_to_end(&mut head).is_err() {
        return Vec::new();
    }
    let Ok(fonts) = FileRef::new(&head) else { return Vec::new() };
    let mut faces = Vec::new();
    for (index, font) in fonts.fonts().enumerate() {
        let Ok(font) = font else { continue };
        let mut table = |tag: &[u8; 4]| -> Option<Vec<u8>> {
            let record = font.table_directory().table_records().iter().find(|r| r.tag() == Tag::new(tag))?;
            let mut data = vec![0; record.length() as usize];
            file.seek(SeekFrom::Start(record.offset().into())).ok()?;
            file.read_exact(&mut data).ok()?;
            Some(data)
        };
        let (name, os2) = (table(b"name"), table(b"OS/2"));
        if let Some(face) = name.and_then(|name| face_info(path, index as u32, &name, os2.as_deref())) {
            faces.push(face);
        }
    }
    faces
}

fn face_info(path: &Path, index: u32, name: &[u8], os2: Option<&[u8]>) -> Option<FaceInfo> {
    let name = Name::read(FontData::new(name)).ok()?;
    let get = |ids: &[NameId]| ids.iter().find_map(|&id| english(&name, id));
    let family = get(&[NameId::TYPOGRAPHIC_FAMILY_NAME, NameId::FAMILY_NAME])?;
    let style = get(&[NameId::TYPOGRAPHIC_SUBFAMILY_NAME, NameId::SUBFAMILY_NAME]).unwrap_or_else(|| "Regular".into());
    let os2 = os2.and_then(|data| Os2::read(FontData::new(data)).ok());
    let lower = style.to_lowercase();
    Some(FaceInfo {
        path: path.to_path_buf(),
        index,
        postscript: get(&[NameId::POSTSCRIPT_NAME]).unwrap_or_else(|| family.replace(' ', "")),
        weight: os2.as_ref().map_or(if lower.contains("bold") { 700 } else { 400 }, |o| o.us_weight_class()),
        stretch: os2.as_ref().map_or(5, |o| o.us_width_class()),
        italic: os2.as_ref().is_some_and(|o| o.fs_selection().contains(SelectionFlags::ITALIC))
            || lower.contains("italic")
            || lower.contains("oblique"),
        family,
        style,
    })
}

/// Name `id`, in English if the font has it, preferring Windows' records.
fn english(name: &Name, id: NameId) -> Option<String> {
    let rank = |r: &&NameRecord| {
        let (platform, language) = (r.platform_id(), r.language_id());
        ((platform == 3 && language == 0x409) || (platform == 1 && language == 0), platform == 3)
    };
    let record = name.name_record().iter().filter(|r| r.name_id() == id).max_by_key(rank)?;
    let text: String = record.string(name.string_data()).ok()?.chars().collect();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn face(family: &str, style: &str, postscript: &str, weight: u16, italic: bool) -> FaceInfo {
        let (family, style, postscript) = (family.into(), style.into(), postscript.into());
        FaceInfo { path: PathBuf::from("x.ttf"), index: 0, family, style, postscript, weight, stretch: 5, italic }
    }

    #[test]
    fn resolves_xfl_names() {
        let narrow = FaceInfo { stretch: 3, ..face("Arial", "Narrow", "ArialNarrow", 400, false) };
        let faces = [
            narrow,
            face("Arial", "Regular", "ArialMT", 400, false),
            face("Arial", "Bold", "Arial-BoldMT", 700, false),
            face("Arial", "Italic", "Arial-ItalicMT", 400, true),
            face("Segoe UI", "Semibold", "SegoeUI-Semibold", 600, false),
            face("Segoe UI", "Regular", "SegoeUI", 400, false),
            face("Segoe UI", "Light", "SegoeUI-Light", 300, false),
            face("Times New Roman", "Regular", "TimesNewRomanPSMT", 400, false),
        ];
        let name = |face: &str, bold, italic| resolve_in(&faces, face, bold, italic).map(|f| f.postscript.as_str());
        assert_eq!(name("ArialMT", false, false), Some("ArialMT"));
        assert_eq!(name("Arial-BoldMT", false, false), Some("Arial-BoldMT"), "the face named, bold by itself");
        assert_eq!(name("ArialMT", true, false), Some("Arial-BoldMT"), "bold asked on top");
        assert_eq!(name("Arial-BoldMT", false, true), Some("Arial-BoldMT"), "no bold italic: the face named");
        assert_eq!(name("SegoeUI-Light", false, false), Some("SegoeUI-Light"));
        assert_eq!(name("Arial", false, true), Some("Arial-ItalicMT"));
        assert_eq!(name("Arial", false, false), Some("ArialMT"), "normal width before Arial Narrow");
        assert_eq!(name("ArialNarrow", false, false), Some("ArialNarrow"));
        assert_eq!(name("segoe ui", false, false), Some("SegoeUI"), "the regular weight, not light or semibold");
        assert_eq!(name("Segoe UI", true, false), Some("SegoeUI-Semibold"), "the boldest there is");
        assert_eq!(name("TimesNewRoman", false, false), Some("TimesNewRomanPSMT"), "spaces don't matter");
        assert_eq!(name("_serif", false, false), Some("TimesNewRomanPSMT"));
        assert_eq!(name("MyriadPro-Regular", false, false), Some("ArialMT"), "missing fonts fall back to Arial");
        assert_eq!(resolve_in(&faces[4..7], "Nope", false, false).map(|f| f.family.as_str()), Some("Segoe UI"), "then to anything");
        assert_eq!(resolve_in(&[], "Arial", false, false), None);
    }

    #[test]
    fn indexes_system_fonts() {
        let faces = system();
        if faces.is_empty() {
            eprintln!("no system fonts: skipped");
            return;
        }
        assert!(faces.iter().all(|f| !f.family.is_empty() && !f.style.is_empty()));
        let families = families();
        assert!(families.windows(2).all(|w| w[0] != w[1]), "each family once");
        if let Some(arial) = faces.iter().find(|f| f.postscript == "ArialMT") {
            assert_eq!((arial.family.as_str(), arial.weight, arial.stretch, arial.italic), ("Arial", 400, 5, false));
            assert_eq!(resolve("Arial", false, false).map(|f| f.postscript.as_str()), Some("ArialMT"));
            assert_eq!(resolve("Arial", true, false).map(|f| f.postscript.as_str()), Some("Arial-BoldMT"));
            assert!(load(arial).is_some_and(|data| data.len() > 100_000));
        }
    }
}
