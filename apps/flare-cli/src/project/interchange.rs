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
    if out.exists() {
        bail!("{} already exists", out.display());
    }
    let session = open(path)?;
    let bytes = flare_runtime::gif_bytes(session.document()).map_err(anyhow::Error::msg)?;
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    std::io::Write::write_all(&mut staged, &bytes)?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(json!({ "exported": out.to_string_lossy(), "bytes": bytes.len() }))
}

/// Renders the first scene as an H.264 and AAC video, with its sound. Never overwrites; the file
/// appears only when whole.
pub fn export_mp4(path: &Path, out: &Path) -> Result<Value> {
    let session = open(path)?;
    flare_video::write_mp4(session.document(), out).map_err(anyhow::Error::msg)?;
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
