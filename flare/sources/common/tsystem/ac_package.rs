//! `.acx`: Flare's native document package.
//!
//! Like a `.fla`, it is a ZIP holding the document as separate parts, so
//! media stays in its own format, big documents stay quick to read, and the
//! parts can be inspected or diffed with ordinary tools:
//!
//! ```text
//! mimetype                    "application/vnd.flare.acx" (first, uncompressed)
//! manifest.json               format version, generator, what is inside
//! document.json               stage size, frame rate, colour, 3D, guides…
//! scenes/001.json …           one timeline per scene, in playing order
//! library/index.json          library items: symbols, bitmaps, sounds
//! library/symbols/<id>.json   one symbol (name, type, timeline) per file
//! library/bitmaps/<id>.png    images as PNG files
//! library/sounds/<id>.<ext>   sounds in the format they were imported in
//! thumbnail.png               optional preview of the first frame
//! ```
//!
//! Every JSON part uses the same serde model as `.acx.json`, so the two
//! formats hold exactly the same information. Readers ignore parts they
//! don't know (with a note) and refuse packages from a newer major version.

use ac_core::bitmap::BitmapAsset;
use ac_core::model::{Document, Symbol, Timeline};
use ac_core::sound::SoundAsset;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};
use std::path::Path;

pub const MIMETYPE: &str = "application/vnd.flare.acx";
/// Packages saved before the app was renamed (AnimationCraft) still open.
pub const LEGACY_MIMETYPE: &str = "application/vnd.animationcraft.acx";
fn mimetype_ok(m: &[u8]) -> bool {
    m == MIMETYPE.as_bytes() || m == LEGACY_MIMETYPE.as_bytes()
}
/// Major.minor of the package layout. Readers accept any minor of their major.
pub const FORMAT_VERSION: (u32, u32) = (1, 0);
pub const EXTENSION: &str = "acx";

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Manifest {
    pub format: String,
    pub version: String,
    pub generator: String,
    pub scenes: Vec<String>,
    pub symbols: usize,
    pub bitmaps: usize,
    pub sounds: usize,
    #[serde(default)]
    pub thumbnail: bool,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum LibraryEntry {
    Symbol { id: u32, name: String, kind: String, file: String },
    Bitmap { id: u32, name: String, file: String, width: u32, height: u32, smoothing: bool },
    Sound { id: u32, name: String, file: String },
}

/// A loaded package plus anything that couldn't be read cleanly.
pub struct Loaded {
    pub doc: Document,
    pub manifest: Manifest,
    pub warnings: Vec<String>,
}

fn sound_ext(data: &[u8]) -> &'static str {
    if data.starts_with(b"RIFF") {
        "wav"
    } else if data.starts_with(b"OggS") {
        "ogg"
    } else if data.starts_with(b"fLaC") {
        "flac"
    } else if data.starts_with(b"FORM") {
        "aif"
    } else {
        "mp3"
    }
}

fn pretty<T: Serialize>(v: &T) -> Result<Vec<u8>, String> {
    let mut out = serde_json::to_vec_pretty(v).map_err(|e| e.to_string())?;
    out.push(b'\n');
    Ok(out)
}

/// The package's parts, by path (for writing, testing or inspecting).
pub fn parts(doc: &Document, thumbnail: Option<&[u8]>) -> Result<BTreeMap<String, Vec<u8>>, String> {
    let doc = doc.with_floats_committed();
    let mut files = BTreeMap::new();
    // document settings: everything except the parts stored elsewhere
    let mut settings = serde_json::to_value(&doc).map_err(|e| e.to_string())?;
    if let Value::Object(m) = &mut settings {
        for k in ["scene", "more_scenes", "library", "bitmaps", "sounds"] {
            m.remove(k);
        }
    }
    files.insert("document.json".into(), pretty(&settings)?);
    let mut scenes = vec![];
    for (i, tl) in doc.scenes().enumerate() {
        let path = format!("scenes/{:03}.json", i + 1);
        files.insert(path.clone(), pretty(tl)?);
        scenes.push(path);
    }
    let mut index = vec![];
    for s in &doc.library {
        let file = format!("library/symbols/{}.json", s.id);
        files.insert(file.clone(), pretty(s)?);
        index.push(LibraryEntry::Symbol { id: s.id, name: s.name.clone(), kind: format!("{:?}", s.kind).to_lowercase(), file });
    }
    for b in &doc.bitmaps {
        let file = format!("library/bitmaps/{}.png", b.id);
        files.insert(file.clone(), b.png.clone());
        index.push(LibraryEntry::Bitmap { id: b.id, name: b.name.clone(), file, width: b.width, height: b.height, smoothing: b.smoothing });
    }
    for s in &doc.sounds {
        let file = format!("library/sounds/{}.{}", s.id, sound_ext(&s.data));
        files.insert(file.clone(), s.data.clone());
        index.push(LibraryEntry::Sound { id: s.id, name: s.name.clone(), file });
    }
    files.insert("library/index.json".into(), pretty(&index)?);
    if let Some(t) = thumbnail {
        files.insert("thumbnail.png".into(), t.to_vec());
    }
    let manifest = Manifest {
        format: "Flare document".into(),
        version: format!("{}.{}", FORMAT_VERSION.0, FORMAT_VERSION.1),
        generator: format!("Flare {}", env!("CARGO_PKG_VERSION")),
        scenes,
        symbols: doc.library.len(),
        bitmaps: doc.bitmaps.len(),
        sounds: doc.sounds.len(),
        thumbnail: thumbnail.is_some(),
    };
    files.insert("manifest.json".into(), pretty(&manifest)?);
    Ok(files)
}

/// Package bytes (a ZIP: `mimetype` first and stored, the rest deflated;
/// media that is already compressed is stored as is).
pub fn write(doc: &Document, thumbnail: Option<&[u8]>) -> Result<Vec<u8>, String> {
    let files = parts(doc, thumbnail)?;
    let mut z = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    z.start_file("mimetype", stored).map_err(|e| e.to_string())?;
    z.write_all(MIMETYPE.as_bytes()).map_err(|e| e.to_string())?;
    // manifest first after the mimetype, so tools can peek cheaply
    let mut order: Vec<&String> = files.keys().collect();
    order.sort_by_key(|k| (k.as_str() != "manifest.json", k.as_str() != "document.json", (*k).clone()));
    for name in order {
        let compressed = name.ends_with(".png") || name.ends_with(".mp3") || name.ends_with(".ogg") || name.ends_with(".flac") || name.ends_with(".aif");
        z.start_file(name.as_str(), if compressed { stored } else { deflated }).map_err(|e| e.to_string())?;
        z.write_all(&files[name]).map_err(|e| e.to_string())?;
    }
    Ok(z.finish().map_err(|e| e.to_string())?.into_inner())
}

/// True for bytes that look like an `.acx` package.
pub fn is_package(bytes: &[u8]) -> bool {
    bytes.len() > 38 && bytes.starts_with(b"PK\x03\x04") && bytes[30..38] == *b"mimetype" && (bytes.windows(MIMETYPE.len()).take(200).any(|w| w == MIMETYPE.as_bytes()) || bytes.windows(LEGACY_MIMETYPE.len()).take(200).any(|w| w == LEGACY_MIMETYPE.as_bytes()))
}

/// Read a package.
pub fn read(bytes: &[u8]) -> Result<Loaded, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| format!("not a Flare package ({e})"))?;
    let mut files: BTreeMap<String, Vec<u8>> = BTreeMap::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        let mut v = vec![];
        f.read_to_end(&mut v).map_err(|e| format!("{name}: {e}"))?;
        files.insert(name, v);
    }
    from_parts(&files)
}

/// Rebuild a document from package parts.
pub fn from_parts(files: &BTreeMap<String, Vec<u8>>) -> Result<Loaded, String> {
    let get = |p: &str| files.get(p).ok_or_else(|| format!("the package has no {p}"));
    if !files.get("mimetype").is_some_and(|m| mimetype_ok(m)) {
        return Err("not a Flare package (wrong or missing mimetype)".into());
    }
    let manifest: Manifest = serde_json::from_slice(get("manifest.json")?).map_err(|e| format!("manifest.json: {e}"))?;
    let major: u32 = manifest.version.split('.').next().and_then(|v| v.parse().ok()).unwrap_or(0);
    if major > FORMAT_VERSION.0 {
        return Err(format!("this document was made by a newer Flare (package version {}); please update", manifest.version));
    }
    let mut warnings = vec![];
    let mut settings: Value = serde_json::from_slice(get("document.json")?).map_err(|e| format!("document.json: {e}"))?;
    let mut scenes: Vec<Timeline> = vec![];
    for p in &manifest.scenes {
        match files.get(p) {
            Some(b) => scenes.push(serde_json::from_slice(b).map_err(|e| format!("{p}: {e}"))?),
            None => warnings.push(format!("scene file {p} is missing")),
        }
    }
    if scenes.is_empty() {
        scenes.push(Timeline::new("Scene 1"));
    }
    let index: Vec<LibraryEntry> = serde_json::from_slice(get("library/index.json")?).map_err(|e| format!("library/index.json: {e}"))?;
    let mut library: Vec<Symbol> = vec![];
    let mut bitmaps: Vec<BitmapAsset> = vec![];
    let mut sounds: Vec<SoundAsset> = vec![];
    let mut known: Vec<&str> = vec!["mimetype", "manifest.json", "document.json", "library/index.json", "thumbnail.png"];
    known.extend(manifest.scenes.iter().map(String::as_str));
    for e in &index {
        match e {
            LibraryEntry::Symbol { name, file, .. } => {
                known.push(file);
                match files.get(file) {
                    Some(b) => library.push(serde_json::from_slice(b).map_err(|err| format!("{file}: {err}"))?),
                    None => warnings.push(format!("symbol '{name}' is missing ({file})")),
                }
            }
            LibraryEntry::Bitmap { id, name, file, width, height, smoothing } => {
                known.push(file);
                match files.get(file) {
                    Some(b) => bitmaps.push(BitmapAsset { id: *id, name: name.clone(), width: *width, height: *height, smoothing: *smoothing, png: b.clone() }),
                    None => warnings.push(format!("bitmap '{name}' is missing ({file})")),
                }
            }
            LibraryEntry::Sound { id, name, file } => {
                known.push(file);
                match files.get(file) {
                    Some(b) => sounds.push(SoundAsset { id: *id, name: name.clone(), data: b.clone() }),
                    None => warnings.push(format!("sound '{name}' is missing ({file})")),
                }
            }
        }
    }
    for f in files.keys() {
        if !known.contains(&f.as_str()) {
            warnings.push(format!("ignored unknown part {f}"));
        }
    }
    let first = scenes.remove(0);
    if let Value::Object(m) = &mut settings {
        m.insert("scene".into(), serde_json::to_value(&first).map_err(|e| e.to_string())?);
        m.insert("more_scenes".into(), serde_json::to_value(&scenes).map_err(|e| e.to_string())?);
        m.insert("library".into(), Value::Array(vec![]));
    }
    let mut doc: Document = serde_json::from_value(settings).map_err(|e| format!("document.json: {e}"))?;
    doc.library = library;
    doc.bitmaps = bitmaps;
    doc.sounds = sounds;
    let max_sym = doc.library.iter().map(|s| s.id).max().unwrap_or(0);
    doc.next_symbol_id = doc.next_symbol_id.max(max_sym + 1);
    Ok(Loaded { doc, manifest, warnings })
}

/// Open `.acx` from disk.
pub fn open(path: &Path) -> Result<Loaded, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    read(&bytes)
}

/// Save `.acx` safely: written next to the target, then renamed over it, so
/// a crash mid-save never leaves a half-written document.
pub fn save(doc: &Document, path: &Path, thumbnail: Option<&[u8]>) -> Result<(), String> {
    let bytes = write(doc, thumbnail)?;
    let tmp = path.with_extension("acx.saving");
    std::fs::write(&tmp, &bytes).map_err(|e| format!("{}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("{}: {e}", path.display())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rich_doc() -> Document {
        let mut d = ac_core::demo::demo_document();
        d.more_scenes.push(Timeline::new("Credits"));
        d.bitmaps.push(BitmapAsset::from_rgba(5, "dot.png", 2, 2, &[255u8; 16]).unwrap());
        d.sounds.push(SoundAsset { id: 3, name: "voice.mp3".into(), data: b"ID3fake".to_vec() });
        d.scene.layers[0].keyframes.get_mut(&0).unwrap().sound = Some(ac_core::sound::FrameSound::new(3));
        d
    }

    #[test]
    fn round_trips_everything() {
        let d = rich_doc();
        let bytes = write(&d, Some(b"\x89PNGthumb")).unwrap();
        assert!(is_package(&bytes));
        assert!(!is_package(b"{\"json\": true}"));
        let back = read(&bytes).unwrap();
        assert!(back.warnings.is_empty(), "{:?}", back.warnings);
        assert_eq!(back.doc, d.with_floats_committed());
        assert_eq!(back.manifest.scenes, ["scenes/001.json", "scenes/002.json"]);
        assert_eq!((back.manifest.symbols, back.manifest.bitmaps, back.manifest.sounds, back.manifest.thumbnail), (d.library.len(), 1, 1, true));
        // media is stored as plain files
        let p = parts(&d, None).unwrap();
        assert_eq!(p["library/sounds/3.mp3"], b"ID3fake");
        assert!(p["library/bitmaps/5.png"].starts_with(b"\x89PNG"));
        assert!(!String::from_utf8_lossy(&p["document.json"]).contains("\"library\""));
    }

    #[test]
    fn refuses_newer_majors_and_tolerates_extras() {
        let d = rich_doc();
        let mut p = parts(&d, None).unwrap();
        p.insert("mimetype".into(), MIMETYPE.as_bytes().to_vec());
        p.insert("plugins/future.json".into(), b"{}".to_vec());
        let l = from_parts(&p).unwrap();
        assert_eq!(l.warnings, ["ignored unknown part plugins/future.json"]);
        // saved before the rename
        let mut old = p.clone();
        old.insert("mimetype".into(), LEGACY_MIMETYPE.as_bytes().to_vec());
        assert!(from_parts(&old).is_ok());
        let mut m: Manifest = serde_json::from_slice(&p["manifest.json"]).unwrap();
        m.version = "2.0".into();
        p.insert("manifest.json".into(), serde_json::to_vec(&m).unwrap());
        assert!(from_parts(&p).err().unwrap().contains("newer Flare"));
        p.insert("mimetype".into(), b"application/zip".to_vec());
        assert!(from_parts(&p).is_err());
    }

    #[test]
    fn saves_atomically_to_disk() {
        let dir = std::env::temp_dir().join(format!("ac-package-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("movie.acx");
        save(&rich_doc(), &path, None).unwrap();
        save(&rich_doc(), &path, None).unwrap(); // overwrite
        assert!(!dir.join("movie.acx.saving").exists());
        assert_eq!(open(&path).unwrap().doc.sounds.len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }
}
