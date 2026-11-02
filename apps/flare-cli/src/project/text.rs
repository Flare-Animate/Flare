//! Text objects: adding them, changing them and taking them apart.

use anyhow::{Context, Result, bail};
use flare_commands::Command;
use flare_core::{DocumentPoint, ShapeId};
use flare_document::{Content, FrameSpan, Shape, TextSource};
use flare_tools::ops::{SpanRef, break_text_apart, replace_shape};
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::Path;
use uuid::Uuid;

use super::*;

/// What a new text object says and how it looks.
pub struct TextOptions {
    pub text: String,
    pub x: f64,
    pub y: f64,
    pub family: String,
    pub size: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: String,
    pub start: u32,
    pub duration: u32,
    pub layer: Option<Uuid>,
    /// Font files to make known by their family names.
    pub font_files: Vec<std::path::PathBuf>,
}

/// What `set-text` changes; what is `None` stays as it was.
#[derive(Default)]
pub struct TextChanges {
    pub text: Option<String>,
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub family: Option<String>,
    pub size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub color: Option<String>,
    pub font_files: Vec<std::path::PathBuf>,
}

fn origin(x: f64, y: f64) -> Result<DocumentPoint> {
    DocumentPoint::new(x, y).context("the position must be finite")
}

/// Most font files one command may bring, and the biggest each may be.
const MAX_FONT_FILES: usize = 8;
const MAX_FONT_BYTES: u64 = 64 * 1024 * 1024;

/// The fonts in `files`, made known by their family names next to the system's.
fn fonts(files: &[std::path::PathBuf]) -> Result<flare_runtime::ExtraFonts> {
    if files.len() > MAX_FONT_FILES {
        bail!("at most {MAX_FONT_FILES} font files can be given");
    }
    let mut loaded = Vec::new();
    for file in files {
        let size = std::fs::metadata(file)
            .with_context(|| format!("cannot read {}", file.display()))?
            .len();
        if size > MAX_FONT_BYTES {
            bail!("{} is larger than {MAX_FONT_BYTES} bytes", file.display());
        }
        loaded
            .push(std::fs::read(file).with_context(|| format!("cannot read {}", file.display()))?);
    }
    Ok(std::sync::Arc::new(loaded))
}

/// Adds text to a layer of the first scene as an editable text object.
pub fn add_text(path: &Path, options: TextOptions) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, layer) = target_layer(&session, options.layer)?;
    let duration = NonZeroU32::new(options.duration).context("duration must be at least 1")?;
    let source = TextSource::new(
        options.text,
        options.family,
        options.size,
        options.bold,
        options.italic,
        origin(options.x, options.y)?,
    )?;
    let fill = parse_color(&options.color)?;
    let id = ShapeId::new();
    let shape = flare_runtime::text_shape(&source, id, fill, &fonts(&options.font_files)?)
        .map_err(anyhow::Error::msg)?;
    let span = FrameSpan::new(options.start, duration, Content::Shape(shape));
    session.apply([Command::InsertSpan { scene, layer, span }])?;
    commit(&session, path)?;
    Ok(json!({ "shape": id.as_uuid() }))
}

/// Finds the text object `shape` in the first scene.
fn find_text(
    session: &EditorSession,
    shape: Uuid,
) -> Result<(flare_core::SceneId, SpanRef, Shape)> {
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?;
    for layer in scene.timeline().layers() {
        for span in layer.spans() {
            if let Content::Shape(found) = span.content()
                && *found.id().as_uuid() == shape
            {
                if found.text().is_none() {
                    bail!("shape {shape} is not text");
                }
                let at = SpanRef {
                    layer: layer.id(),
                    start: span.start(),
                    element: 0,
                };
                return Ok((scene.id(), at, found.clone()));
            }
        }
    }
    bail!("no shape {shape} in the first scene")
}

/// Changes a text object: its words, its font, its place or its color. It is one undo step.
pub fn set_text(path: &Path, shape: Uuid, changes: TextChanges) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, at, old) = find_text(&session, shape)?;
    let before = old.text().context("shape is not text")?;
    let source = TextSource::new(
        changes.text.unwrap_or_else(|| before.text().to_owned()),
        changes.family.unwrap_or_else(|| before.family().to_owned()),
        changes.size.unwrap_or(before.size()),
        changes.bold.unwrap_or(before.bold()),
        changes.italic.unwrap_or(before.italic()),
        match (changes.x, changes.y) {
            (None, None) => before.origin(),
            (x, y) => {
                let was = before.origin().as_vec();
                origin(x.unwrap_or(was.x), y.unwrap_or(was.y))?
            }
        },
    )?;
    let fill = match changes.color {
        Some(color) => parse_color(&color)?,
        None => old.fill(),
    };
    let new = flare_runtime::text_shape(&source, old.id(), fill, &fonts(&changes.font_files)?)
        .map_err(anyhow::Error::msg)?
        .painted_like(&old);
    let commands = replace_shape(session.document(), scene, at, new)
        .context("nothing about the text changed")?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "shape": shape }))
}

/// Takes text apart into plain outlines, which can then be edited point by point.
pub fn break_text(path: &Path, shape: Uuid) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, at, _) = find_text(&session, shape)?;
    let commands = break_text_apart(session.document(), scene, at).context("shape is not text")?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "shape": shape, "text": false }))
}

/// The font families text can be set in.
pub fn font_families(font_files: &[std::path::PathBuf]) -> Result<Value> {
    Ok(json!({ "families": flare_runtime::font_families(&fonts(font_files)?) }))
}
