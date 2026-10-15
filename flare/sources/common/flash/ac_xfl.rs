//! XFL / .fla interchange (File ▸ Open / Import / Export).
//!
//! A CS5-or-later `.fla` is a ZIP of an XFL folder:
//! `DOMDocument.xml` (document, scenes, media and symbol list),
//! `LIBRARY/<symbol>.xml` (one `DOMSymbolItem` per symbol, plus imported
//! media files), `bin/*.dat` (bitmap data), `META-INF/`, `PublishSettings.xml`.
//! An uncompressed XFL folder has the same files plus a `<name>.xfl` marker.
//!
//! Everything here is built from public descriptions of the format and from
//! files this program writes itself (clean-room; no third-party samples, no
//! disassembly or debugging of other programs). See CONTRIBUTING.md.

#[path = "ac_xfl_brush.rs"]
pub mod brush;
#[path = "ac_xfl_dat.rs"]
pub mod dat;
#[path = "ac_xfl_edges.rs"]
pub mod edges;
#[path = "ac_xfl_read.rs"]
mod read;
#[path = "ac_xfl_write.rs"]
mod write;

use ac_core::model::Document;
use std::collections::BTreeMap;
use std::io::{Cursor, Read as _, Write as _};
use std::path::Path;

/// The files of an XFL package by path ("DOMDocument.xml", "LIBRARY/a.xml"…).
pub type Package = BTreeMap<String, Vec<u8>>;

/// A loaded document plus what could not be carried over.
pub struct Imported {
    pub doc: Document,
    pub warnings: Vec<String>,
}

/// Read a package (already unpacked).
pub fn read_package(pkg: &Package) -> Result<Imported, String> {
    read::read(pkg)
}

/// Write a document as package files. Returns (files, warnings).
pub fn write_package(doc: &Document) -> (Package, Vec<String>) {
    write::write(doc)
}

/// Unzip a `.fla`. Archives whose end record is slightly off (real files
/// from the authoring tool can state the central directory's size wrongly,
/// which strict readers reject) are repaired in memory first.
pub fn unzip(bytes: &[u8]) -> Result<Package, String> {
    let mut z = match zip::ZipArchive::new(Cursor::new(bytes)) {
        Ok(z) => z,
        Err(first) => {
            let fixed = repair_end_record(bytes).ok_or_else(|| format!("not a CS5+ .fla (ZIP) file: {first}"))?;
            return unzip_strict(&fixed).map_err(|e| format!("damaged .fla (ZIP) file: {first}; after repair: {e}"));
        }
    };
    read_all(&mut z)
}

fn unzip_strict(bytes: &[u8]) -> Result<Package, String> {
    let mut z = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
    read_all(&mut z)
}

/// Rewrite the end-of-central-directory record so its size and entry counts
/// match the central directory that is actually there. None if it can't be found.
pub fn repair_end_record(bytes: &[u8]) -> Option<Vec<u8>> {
    let eocd = bytes.windows(4).rposition(|w| w == b"PK\x05\x06")?;
    if eocd + 22 > bytes.len() {
        return None;
    }
    let u32_at = |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as usize;
    let u16_at = |i: usize| u16::from_le_bytes([bytes[i], bytes[i + 1]]) as usize;
    // the stated directory start, or failing that the first central header
    let stated = u32_at(eocd + 16);
    let cd = if bytes.get(stated..stated + 4) == Some(b"PK\x01\x02") { stated } else { bytes.windows(4).position(|w| w == b"PK\x01\x02")? };
    // count the central headers that are really there
    let (mut k, mut n) = (cd, 0usize);
    while k + 46 <= eocd && &bytes[k..k + 4] == b"PK\x01\x02" {
        k += 46 + u16_at(k + 28) + u16_at(k + 30) + u16_at(k + 32);
        n += 1;
    }
    if n == 0 || n > 0xFFFF {
        return None;
    }
    let mut out = bytes.to_vec();
    out[eocd + 8..eocd + 10].copy_from_slice(&(n as u16).to_le_bytes());
    out[eocd + 10..eocd + 12].copy_from_slice(&(n as u16).to_le_bytes());
    out[eocd + 12..eocd + 16].copy_from_slice(&((k - cd) as u32).to_le_bytes());
    out[eocd + 16..eocd + 20].copy_from_slice(&(cd as u32).to_le_bytes());
    out.truncate(eocd + 22 + u16_at(eocd + 20).min(bytes.len() - eocd - 22));
    Some(out)
}

fn read_all(z: &mut zip::ZipArchive<Cursor<&[u8]>>) -> Result<Package, String> {
    let mut pkg = Package::new();
    for i in 0..z.len() {
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        if f.is_dir() {
            continue;
        }
        let name = f.name().replace('\\', "/");
        let mut v = vec![];
        f.read_to_end(&mut v).map_err(|e| format!("{name}: {e}"))?;
        pkg.insert(name, v);
    }
    Ok(pkg)
}

/// Zip a package as a `.fla` (`mimetype` first and stored, like ODF/EPUB).
pub fn zip(pkg: &Package) -> Result<Vec<u8>, String> {
    let mut w = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let stored = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    let deflated = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    if let Some(m) = pkg.get("mimetype") {
        w.start_file("mimetype", stored).map_err(|e| e.to_string())?;
        w.write_all(m).map_err(|e| e.to_string())?;
    }
    for (name, data) in pkg {
        if name == "mimetype" {
            continue;
        }
        w.start_file(name.as_str(), deflated).map_err(|e| e.to_string())?;
        w.write_all(data).map_err(|e| e.to_string())?;
    }
    Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
}

/// Open a `.fla` (zipped), an `.xfl` marker file or an XFL folder.
pub fn open(path: &Path) -> Result<Imported, String> {
    let pkg = if path.is_dir() {
        read_dir(path)?
    } else if path.extension().map(|e| e.eq_ignore_ascii_case("xfl")).unwrap_or(false) {
        read_dir(path.parent().ok_or("no folder")?)?
    } else {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        unzip(&bytes)?
    };
    read_package(&pkg)
}

/// Save as `.fla` (zip) or, for a path ending in `.xfl` or a folder, an
/// uncompressed XFL folder. Returns warnings.
pub fn save(doc: &Document, path: &Path) -> Result<Vec<String>, String> {
    let (pkg, warnings) = write_package(doc);
    let is_xfl = path.extension().map(|e| e.eq_ignore_ascii_case("xfl")).unwrap_or(false);
    if is_xfl || path.is_dir() {
        let (dir, stem) = if is_xfl {
            // name.xfl → folder "name/" holding "name.xfl"
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("document").to_string();
            (path.parent().unwrap_or(Path::new(".")).join(&stem), stem)
        } else {
            (path.to_path_buf(), path.file_name().and_then(|s| s.to_str()).unwrap_or("document").to_string())
        };
        write_dir(&pkg, &dir, &stem)?;
    } else {
        std::fs::write(path, zip(&pkg)?).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(warnings)
}

fn read_dir(dir: &Path) -> Result<Package, String> {
    fn walk(base: &Path, d: &Path, pkg: &mut Package) -> Result<(), String> {
        for e in std::fs::read_dir(d).map_err(|e| format!("{}: {e}", d.display()))? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.is_dir() {
                walk(base, &p, pkg)?;
            } else {
                let rel = p.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                pkg.insert(rel, std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?);
            }
        }
        Ok(())
    }
    let mut pkg = Package::new();
    walk(dir, dir, &mut pkg)?;
    if !pkg.contains_key("DOMDocument.xml") {
        return Err(format!("{} has no DOMDocument.xml", dir.display()));
    }
    Ok(pkg)
}

fn write_dir(pkg: &Package, dir: &Path, stem: &str) -> Result<(), String> {
    for (name, data) in pkg {
        if name == "mimetype" {
            continue;
        }
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&p, data).map_err(|e| format!("{}: {e}", p.display()))?;
    }
    std::fs::write(dir.join(format!("{stem}.xfl")), b"PROXY-CS5").map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
#[path = "ac_xfl_tests.rs"]
mod tests;
