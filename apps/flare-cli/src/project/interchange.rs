//! Importing, exporting and publishing other formats.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::*;

/// Imports an XFL directory, `.xfl` file or zipped `.fla` into a new project file and
/// reports everything that could not be carried over.
pub fn import(source: &Path, out: &Path) -> Result<Value> {
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let imported = flare_import::import_path(source)
        .with_context(|| format!("cannot import {}", source.display()))?;
    flare_storage::save(&imported.document, out)
        .with_context(|| format!("cannot save {}", out.display()))?;
    let count = |severity| {
        imported
            .diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    };
    let diagnostics: Vec<Value> = imported
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "severity": format!("{:?}", d.severity).to_lowercase(),
                "location": d.location,
                "message": d.message,
            })
        })
        .collect();
    Ok(json!({
        "imported": out.to_string_lossy(),
        "scenes": imported.document.scenes().len(),
        "symbols": imported.document.symbols().len(),
        "kept_source_data": imported.document.attachments().len(),
        "original_kept": source_json(&imported.document),
        "info": count(flare_import::Severity::Info),
        "warnings": count(flare_import::Severity::Warning),
        "unsupported": count(flare_import::Severity::Unsupported),
        "diagnostics": diagnostics,
    }))
}

/// What the project remembers of the file it was imported from: with it, exporting the project
/// unchanged writes that file back byte for byte.
fn source_json(document: &flare_document::Document) -> Value {
    match document.origin() {
        Some(origin) => json!({
            "kept": true,
            "format": origin.kind().name(),
            "bytes": origin.len(),
            "digest": origin.digest().iter().map(|b| format!("{b:02x}")).collect::<String>(),
        }),
        None => json!({ "kept": false }),
    }
}

/// Exports a project as an XFL folder, or as a zipped modern `.fla` when `out` ends in
/// `.fla`. Never overwrites, and reports everything the format could not hold exactly.
pub fn export_xfl(path: &Path, out: &Path) -> Result<Value> {
    let session = open(path)?;
    let output = flare_export::export_xfl(session.document())?;
    let zipped = out
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("fla"));
    if zipped {
        flare_export::write_fla(&output, out)
    } else {
        flare_export::write_directory(&output, out)
    }
    .with_context(|| format!("cannot write {}", out.display()))?;
    let count = |severity| {
        output
            .diagnostics
            .iter()
            .filter(|d| d.severity == severity)
            .count()
    };
    let diagnostics: Vec<Value> = output
        .diagnostics
        .iter()
        .map(|d| {
            json!({
                "severity": format!("{:?}", d.severity).to_lowercase(),
                "location": d.location,
                "message": d.message,
            })
        })
        .collect();
    Ok(json!({
        "exported": out.to_string_lossy(),
        "format": if zipped { "fla" } else { "xfl" },
        "files": output.file_count(),
        "lossless": { "exact": output.lossless.exact, "reason": output.lossless.reason },
        "info": count(flare_export::Severity::Info),
        "warnings": count(flare_export::Severity::Warning),
        "diagnostics": diagnostics,
    }))
}

/// Renders every frame of the first scene as an endlessly looping GIF. Never overwrites; the
/// file appears only when complete.
pub fn export_gif(path: &Path, out: &Path) -> Result<Value> {
    export_animation(path, out, flare_runtime::gif_bytes)
}

/// Renders every frame of the first scene as an endlessly repeating animated WebP (lossless). Never
/// overwrites; the file appears only when complete.
pub fn export_animated_webp(path: &Path, out: &Path) -> Result<Value> {
    export_animation(path, out, flare_runtime::animated_webp_bytes)
}

/// Renders every frame of the first scene as an endlessly looping APNG (animated PNG). Never
/// overwrites; the file appears only when complete.
pub fn export_apng(path: &Path, out: &Path) -> Result<Value> {
    export_animation(path, out, flare_runtime::apng_bytes)
}

/// Writes an animation of the document made by `encode`, as a whole file or not at all.
fn export_animation(
    path: &Path,
    out: &Path,
    encode: fn(&flare_document::Document) -> Result<Vec<u8>, String>,
) -> Result<Value> {
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let session = open(path)?;
    let bytes = encode(session.document()).map_err(anyhow::Error::msg)?;
    write_new(out, &bytes)
}

/// Writes `bytes` to `out`, which must not exist: the file appears only when it is whole.
fn write_new(out: &Path, bytes: &[u8]) -> Result<Value> {
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    std::io::Write::write_all(&mut staged, bytes)?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(json!({ "exported": out.to_string_lossy(), "bytes": bytes.len() }))
}

/// Writes the first scene as a movie made by ffmpeg, in the container `id` (see `flare_ffmpeg`).
/// Never overwrites; the file appears only when whole.
pub fn export_movie(path: &Path, out: &Path, id: &str) -> Result<Value> {
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let container =
        flare_ffmpeg::container(id).with_context(|| format!("no movie format called {id}"))?;
    let session = open(path)?;
    flare_video::write_with_ffmpeg(session.document(), out, container)
        .map_err(anyhow::Error::msg)?;
    let bytes = std::fs::metadata(out)?.len();
    Ok(json!({ "exported": out.to_string_lossy(), "bytes": bytes }))
}

/// Renders one frame of the first scene as a flattened Photoshop (PSD) file at the stage's size.
/// Never overwrites.
pub fn export_psd(path: &Path, out: &Path, frame: u32) -> Result<Value> {
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let session = open(path)?;
    let bytes = flare_runtime::psd_export(session.document(), frame).map_err(anyhow::Error::msg)?;
    write_new(out, &bytes)
}

/// Renders one frame of the first scene as an SVG picture (vector: at the stage size). Never
/// overwrites. The notes say what SVG could not carry, and they are reported with the file.
pub fn export_svg(path: &Path, out: &Path, frame: u32) -> Result<Value> {
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let session = open(path)?;
    let svg = flare_runtime::svg_export(session.document(), frame).map_err(anyhow::Error::msg)?;
    let mut value = write_new(out, svg.text.as_bytes())?;
    value["notes"] = json!(svg.notes);
    Ok(value)
}

/// Renders the first scene as an H.264 and AAC video, with its sound. Never overwrites; the file
/// appears only when whole.
pub fn export_mp4(path: &Path, out: &Path) -> Result<Value> {
    let session = open(path)?;
    flare_video::write_mp4(session.document(), out).map_err(anyhow::Error::msg)?;
    let bytes = std::fs::metadata(out)?.len();
    Ok(json!({ "exported": out.to_string_lossy(), "bytes": bytes }))
}

/// Renders the first scene as a QuickTime movie with its sound, as [`export_mp4`] does, with no
/// ffmpeg needed.
pub fn export_mov(path: &Path, out: &Path) -> Result<Value> {
    let session = open(path)?;
    flare_video::write_mov(session.document(), out).map_err(anyhow::Error::msg)?;
    let bytes = std::fs::metadata(out)?.len();
    Ok(json!({ "exported": out.to_string_lossy(), "bytes": bytes }))
}

/// Publishes a project as a SWF file. Never overwrites; the file appears only when complete.
pub fn publish_swf(path: &Path, out: &Path, compress: bool) -> Result<Value> {
    let session = open(path)?;
    let published = flare_publish::publish_to_file(
        session.document(),
        &flare_publish::PublishOptions { compress },
        out,
    )?;
    let diagnostics: Vec<Value> = published
        .diagnostics
        .iter()
        .map(|d| json!({ "severity": format!("{:?}", d.severity).to_lowercase(), "message": d.message }))
        .collect();
    Ok(json!({
        "published": out.to_string_lossy(),
        "frames": published.frames,
        "bytes": published.bytes.len(),
        "original": published.original,
        "diagnostics": diagnostics,
    }))
}

/// Publishes a project as an HTML5 Canvas page. Never overwrites; the file appears only when
/// complete.
pub fn publish_html5(
    path: &Path,
    out: &Path,
    loop_timeline: bool,
    center_stage: bool,
    responsive: bool,
) -> Result<Value> {
    let session = open(path)?;
    let title = out
        .file_stem()
        .map_or_else(|| "movie".to_owned(), |s| s.to_string_lossy().into_owned());
    let options = flare_publish::Html5Options {
        title,
        loop_timeline,
        center_stage,
        responsive,
    };
    let published = flare_publish::publish_html5_to_file(session.document(), &options, out)?;
    let diagnostics: Vec<Value> = published
        .diagnostics
        .iter()
        .map(|d| json!({ "severity": format!("{:?}", d.severity).to_lowercase(), "message": d.message }))
        .collect();
    Ok(json!({
        "published": out.to_string_lossy(),
        "frames": published.frames,
        "bytes": published.html.len(),
        "diagnostics": diagnostics,
    }))
}

/// Test Movie: publish the project and play it in an external Ruffle, waiting for it to close.
/// Starts the desktop app on `paths` and returns without waiting for it.
pub fn open_app(paths: &[PathBuf]) -> Result<Value> {
    let app = flare_runtime::find_desktop().context(
        "the Flare app was not found: put flare-desktop next to this program or on PATH, or set FLARE_DESKTOP",
    )?;
    flare_runtime::open_desktop(&app, paths)
        .with_context(|| format!("cannot start {}", app.display()))?;
    Ok(json!({ "app": app.to_string_lossy(), "opened": paths.len() }))
}

pub fn test_movie(path: &Path, ruffle: Option<PathBuf>) -> Result<Value> {
    let session = open(path)?;
    let runner = ruffle.or_else(flare_ruffle::find_ruffle).context(
        "Ruffle was not found: install it and put it on PATH, set FLARE_RUFFLE, or pass --ruffle <path>",
    )?;
    let movie = flare_ruffle::launch_test_movie(session.document(), &runner)
        .with_context(|| format!("cannot start {}", runner.display()))?;
    let status = movie.wait().context("waiting for the player")?;
    Ok(json!({ "player": runner.to_string_lossy(), "exit_code": status.code() }))
}

/// Converts any project (`.flare`, `.fla`, `.xfl` or an XFL folder, or a `.swf` to read) to the
/// format the ending of `out` says, as Save As does. Refuses an existing `out` unless `force`,
/// and then replaces it only with a whole new file. An unchanged Animate document comes out as
/// the original; `lossless` and the diagnostics say what an edited one lost.
pub fn convert(source: &Path, out: &Path, force: bool) -> Result<Value> {
    let (target, format) = flare_runtime::resolve_save_path(out);
    if !force && (target.exists() || flare_runtime::xfl_folder(&target).exists()) {
        bail!(
            "{} already exists; pass --force to replace it",
            target.display()
        );
    }
    let loaded =
        flare_runtime::load(source).with_context(|| format!("cannot open {}", source.display()))?;
    let saved = flare_runtime::save(&loaded.document, &target)
        .with_context(|| format!("cannot write {}", target.display()))?;
    let mut answer = json!({
        "converted": saved.path.to_string_lossy(),
        "format": format.extension(),
        "from": source.to_string_lossy(),
        "import_notes": loaded.notes,
    });
    if let (Some(summary), Some(map)) = (&saved.summary, answer.as_object_mut()) {
        map.insert("files".into(), json!(summary.files));
        map.insert(
            "lossless".into(),
            json!({ "exact": summary.exact, "reason": summary.reason }),
        );
        map.insert("rebuilt".into(), json!(summary.rebuilt));
        map.insert("warnings".into(), json!(summary.warnings));
        map.insert("notes".into(), json!(summary.notes));
    }
    Ok(answer)
}

/// Writes the sprite sheet PNG to `out` and its JSON atlas beside it (the same name, `.json`).
/// Never overwrites either; each appears only when whole.
pub fn export_sprites(
    path: &Path,
    out: &Path,
    start: u32,
    end: Option<u32>,
    columns: Option<u32>,
) -> Result<Value> {
    let atlas = out.with_extension("json");
    for file in [out, atlas.as_path()] {
        if file.exists() {
            bail!("{} already exists", file.display());
        }
    }
    let session = open(path)?;
    let name = out
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sheet.png");
    let sheet = flare_runtime::sprite_sheet(session.document(), start, end, columns, name)
        .map_err(anyhow::Error::msg)?;
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    for (file, bytes) in [
        (out, sheet.png.as_slice()),
        (atlas.as_path(), sheet.json.as_bytes()),
    ] {
        let mut staged = tempfile::NamedTempFile::new_in(directory)?;
        std::io::Write::write_all(&mut staged, bytes)?;
        staged.persist(file).map_err(|error| error.error)?;
    }
    Ok(json!({
        "exported": out.to_string_lossy(),
        "atlas": atlas.to_string_lossy(),
        "width": sheet.width,
        "height": sheet.height,
        "frames": sheet.frames,
    }))
}
