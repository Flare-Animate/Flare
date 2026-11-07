//! Import of other animation tools' files: Moho projects, FlipaClip projects, BVH motion and Toonz scenes and levels.
//! The readers and converters live in `flare-moho` and `flare-toonz`; this only picks one by the
//! file's name (a Moho container and a `.fla` are both zip files, so the bytes cannot tell) and
//! turns their notes into the importer's diagnostics.

use std::io::Read;
use std::path::Path;

use crate::xfl::{Diagnostic, ImportError, Imported, Severity};

/// Which foreign reader handles `path`, going by its extension.
pub fn foreign_kind(path: &Path) -> Option<&'static str> {
    if flare_moho::is_moho_path(path) {
        return Some("moho");
    }
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "bvh" => Some("bvh"),
        "svg" => Some("svg"),
        "png" => Some("png"),
        "jpg" | "jpeg" => Some("jpeg"),
        "bmp" => Some("bmp"),
        "tga" => Some("tga"),
        "tif" | "tiff" => Some("tiff"),
        "webp" => Some("webp"),
        "ico" => Some("ico"),
        "qoi" => Some("qoi"),
        "pnm" | "pbm" | "pgm" | "pam" => Some("pnm"),
        "hdr" => Some("hdr"),
        "exr" => Some("exr"),
        "dds" => Some("dds"),
        "psd" => Some("psd"),
        "gif" => Some("gif"),
        "tnz" | "tpl" | "plt" | "pli" | "tlv" => Some("toonz"),
        "ppm" | "kwz" | "kwc" => Some("flipnote"),
        "fc" => Some("flipaclip"),
        "wick" => Some("wick"),
        _ => None,
    }
}

const MAX_BYTES: u64 = 64 * 1024 * 1024;

/// A `.ppm` that is a plain Netpbm picture (`P1` to `P7`) rather than a Flipnote animation.
fn is_plain_netpbm(name: &Path, bytes: &[u8]) -> bool {
    let ppm = name
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("ppm"));
    ppm && !crate::flipnote::looks_like_ppm(bytes) && matches!(bytes, [b'P', b'1'..=b'7', ..])
}

pub fn import_foreign_path(path: &Path) -> Result<Imported, ImportError> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if foreign_kind(path) == Some("moho") {
        // A Moho project's pictures are read from its own folder, and nowhere else.
        let folder = match path.parent() {
            Some(dir) if !dir.as_os_str().is_empty() => dir,
            _ => Path::new("."),
        };
        return import_moho(path, bytes, Some(folder));
    }
    import_foreign_bytes(path, bytes)
}

/// Imports a Moho, BVH or Toonz file held in memory; `name` says which.
pub fn import_foreign_bytes(name: &Path, bytes: Vec<u8>) -> Result<Imported, ImportError> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err(ImportError::TooLarge("the file".into()));
    }
    let file = name
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let foreign = |e: &dyn std::fmt::Display| ImportError::Foreign(format!("{file}: {e}"));
    match foreign_kind(name) {
        Some("moho") => import_moho(name, bytes, None),
        Some("bvh") => {
            let text = String::from_utf8(bytes).map_err(|_| foreign(&"not UTF-8 text"))?;
            let parsed = flare_moho::bvh::parse(&text).map_err(|e| foreign(&e))?;
            let out = flare_moho::bvh::to_document(&parsed).map_err(|e| foreign(&e))?;
            Ok(from_moho(out))
        }
        Some("flipaclip") => crate::flipaclip::import(file, &bytes),
        Some("wick") => crate::wick::import(file, &bytes),
        Some("png") => crate::bitmap::import_png(file, &bytes),
        Some("jpeg") => crate::bitmap::import_jpeg(file, &bytes),
        Some("bmp") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Bmp),
        Some("tiff") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Tiff),
        Some("webp") => crate::webp::import(file, &bytes),
        Some("ico") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Ico),
        Some("qoi") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Qoi),
        Some("pnm") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Pnm),
        Some("hdr") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Hdr),
        Some("exr") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::OpenExr),
        Some("dds") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Dds),
        Some("psd") => crate::psd::import(file, &bytes),
        Some("tga") => crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Tga),
        Some("gif") => crate::bitmap::import_gif(file, &bytes),
        Some("svg") => crate::svg::import(&bytes).map_err(|e| foreign(&e)),
        Some("flipnote") if is_plain_netpbm(name, &bytes) => {
            crate::bitmap::import_raster(file, &bytes, image::ImageFormat::Pnm)
        }
        Some("flipnote") => crate::flipnote::import(file, &bytes),
        Some("toonz") => crate::toonz::import(name, &bytes).map_err(|e| foreign(&e)),
        _ => Err(ImportError::NotXfl("unknown file type".into())),
    }
}

/// A Moho project; with `folder`, its image layers' pictures are read from that folder.
fn import_moho(
    name: &Path,
    bytes: Vec<u8>,
    folder: Option<&Path>,
) -> Result<Imported, ImportError> {
    let file = name
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project");
    let foreign = |e: &dyn std::fmt::Display| ImportError::Foreign(format!("{file}: {e}"));
    let project = flare_moho::parse_bytes(&bytes, file).map_err(|e| foreign(&e))?;
    let out = match folder {
        Some(dir) => {
            let mut pictures =
                crate::moho_images::ProjectFolder::new(dir).map_err(|e| foreign(&e))?;
            flare_moho::to_flare_document_with(&project, &mut pictures)
        }
        None => flare_moho::to_flare_document(&project),
    }
    .map_err(|e| foreign(&e))?;
    Ok(from_moho(out))
}

fn from_moho(out: flare_moho::Converted) -> Imported {
    Imported {
        document: out.document,
        diagnostics: out
            .notes
            .into_iter()
            .map(|n| Diagnostic {
                severity: match n.severity {
                    flare_moho::Severity::Info => Severity::Info,
                    flare_moho::Severity::Warning => Severity::Warning,
                    flare_moho::Severity::Unsupported => Severity::Unsupported,
                },
                location: n.location,
                message: n.message,
            })
            .collect(),
    }
}
