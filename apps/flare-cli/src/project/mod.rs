//! Headless project commands. They use the same edit session and storage as any future UI.

use anyhow::{Context, Result, bail};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{
    Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Stroke, StrokeStyle, Timeline,
};
use flare_history::EditorSession;
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::Path;

mod bones;
mod camera;
mod draw;
mod interchange;
mod magic_wand;
mod multi_frame;
mod render;
mod sound;
mod text;
mod timeline;

pub use bones::{BoneAction, bone, bones_list};
#[allow(unused_imports)]
use camera::*;
pub use camera::{CameraMotion, camera};
#[allow(unused_imports)]
use draw::*;
pub use draw::{
    PlaceOptions, add_layer, add_symbol, draw_rect, eval, place, rename_scene, set_stage,
};
#[allow(unused_imports)]
use interchange::*;
pub use interchange::{
    convert, export_animated_webp, export_apng, export_gif, export_movie, export_mp4, export_psd,
    export_sprites, export_svg, export_xfl, import, open_app, publish_html5, publish_swf,
    test_movie,
};
pub use magic_wand::{WandRequest, magic_wand};
pub use multi_frame::transform_frames;
#[allow(unused_imports)]
use render::*;
pub use render::{ExportFormat, export, export_frames};
pub use sound::{sound_import, sound_waveform};
#[allow(unused_imports)]
use text::*;
pub use text::{TextChanges, TextOptions, add_text, break_text, font_families, set_text};
#[allow(unused_imports)]
use timeline::*;
pub use timeline::{FrameOperation, LayerOperation, frame_op, layer_op};

/// Opens a project: a `.flare`, or an Animate `.fla`, `.xfl` or XFL folder (a movie is read too,
/// to inspect or convert, but never written back).
fn open(path: &Path) -> Result<EditorSession> {
    let loaded =
        flare_runtime::load(path).with_context(|| format!("cannot open {}", path.display()))?;
    Ok(EditorSession::new(loaded.document))
}

/// Writes an edit back to the file it was opened from, in that file's format, replacing it only
/// once the new one is whole. A file Flare cannot write back whole (a movie, a pre-CS5 `.fla`, an
/// Animate file too big to keep) is refused; `flare convert` makes a copy of it.
fn commit(session: &EditorSession, path: &Path) -> Result<()> {
    if !flare_runtime::saves_in_place(session.document(), path) {
        bail!(
            "{} cannot be written back as it is (a movie, a pre-CS5 .fla or a file too big to keep); use `flare convert` to make a project from it",
            path.display()
        );
    }
    flare_runtime::save(session.document(), path)
        .map(|_| ())
        .with_context(|| format!("cannot save {}", path.display()))
}

pub fn new(path: &Path, numerator: u32, denominator: u32, stage: Stage) -> Result<Value> {
    if path.exists() {
        bail!("{} already exists", path.display());
    }
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![])?;
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(numerator, denominator)?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .map_err(|errors| anyhow::anyhow!("{errors:?}"))?
        .with_stage(stage);
    // The ending picks the format, so `flare new a.fla` makes a document Animate opens.
    flare_runtime::save(&document, path)?;
    Ok(json!({ "created": path.to_string_lossy(), "id": document.id().as_uuid() }))
}

pub fn info(path: &Path) -> Result<Value> {
    let session = open(path)?;
    let format = if path.is_dir() {
        Some(flare_runtime::FileFormat::Xfl)
    } else {
        flare_runtime::FileFormat::of_path(path)
    };
    let document = session.document();
    let scenes: Vec<Value> = document
        .scenes()
        .iter()
        .map(|scene| {
            let layers: Vec<Value> = scene
                .timeline()
                .layers()
                .iter()
                .map(|layer| {
                    let elements: usize = layer.spans().iter().map(|s| s.element_count()).sum();
                    let armature = flare_tools::bones::is_armature_layer(layer);
                    json!({ "id": layer.id().as_uuid(), "name": layer.name(),
                            "visible": layer.is_visible(), "locked": layer.is_locked(),
                            "spans": layer.spans().len(), "elements": elements,
                            "armature": armature })
                })
                .collect();
            json!({ "id": scene.id().as_uuid(), "name": scene.name(), "layers": layers })
        })
        .collect();
    let symbols: Vec<Value> = document
        .symbols()
        .iter()
        .map(|symbol| {
            json!({ "id": symbol.id().as_uuid(), "name": symbol.name(), "kind": symbol.kind().name() })
        })
        .collect();
    Ok(json!({
        "id": document.id().as_uuid(),
        "format": format.map_or("flare", flare_runtime::FileFormat::extension),
        "frame_rate": document.frame_rate(),
        "stage": {
            "width": document.stage().width(),
            "height": document.stage().height(),
            "background": document.stage().background().to_array(),
        },
        "scenes": scenes,
        "symbols": symbols,
        "attachments": document.attachments().len(),
    }))
}

/// `#RRGGBB` or `#RRGGBBAA` (the `#` is optional).
pub fn parse_color(text: &str) -> Result<Rgba8> {
    let hex = text.trim_start_matches('#');
    let byte = |i: usize| {
        hex.get(i..i + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
            .with_context(|| format!("bad color {text:?}, expected RRGGBB or RRGGBBAA"))
    };
    match hex.len() {
        6 => Ok(Rgba8::opaque(byte(0)?, byte(2)?, byte(4)?)),
        8 => Ok(Rgba8::new(byte(0)?, byte(2)?, byte(4)?, byte(6)?)),
        _ => bail!("bad color {text:?}, expected RRGGBB or RRGGBBAA"),
    }
}

/// Fill, and optionally an outline of (width, color), for a rectangle.
#[derive(Clone, Copy)]
pub struct RectStyle {
    pub fill: Rgba8,
    pub stroke: Option<(f64, Rgba8)>,
    pub stroke_style: StrokeStyle,
}

fn rect_content(rect: [f64; 4], style: RectStyle) -> Result<Content> {
    let [x, y, w, h] = rect;
    if !(w > 0.0 && h > 0.0) {
        bail!("rectangle width and height must be positive");
    }
    let mut shape = Shape::rect(ShapeId::new(), x, y, w, h, style.fill)
        .context("rectangle coordinates must be finite")?;
    if let Some((width, color)) = style.stroke {
        shape = shape.with_stroke(Stroke::new(width, color)?.with_style(style.stroke_style));
    }
    Ok(Content::Shape(shape))
}

fn one_frame_timeline(content: Content) -> Result<Timeline> {
    let span = FrameSpan::new(0, NonZeroU32::MIN, content);
    Ok(Timeline::new(vec![Layer::new(
        LayerId::new(),
        "Layer 1",
        vec![span],
    )?]))
}

/// Builds a rectangle style from the command-line color strings.
pub fn rect_style(
    color: &str,
    stroke: Option<&str>,
    stroke_width: f64,
    stroke_style: Option<&str>,
) -> Result<RectStyle> {
    let stroke_style = match stroke_style {
        None => StrokeStyle::Solid,
        Some(name) => StrokeStyle::parse(name).with_context(|| {
            format!(
                "unknown stroke style {name:?}, expected one of {}",
                StrokeStyle::name_list()
            )
        })?,
    };
    Ok(RectStyle {
        stroke_style,
        fill: parse_color(color)?,
        stroke: stroke
            .map(|c| Ok::<_, anyhow::Error>((stroke_width, parse_color(c)?)))
            .transpose()?,
    })
}

/// For the `tween` command: open a project.
pub fn open_session(path: &Path) -> Result<EditorSession> {
    open(path)
}

/// For the `tween` command: write an edit back.
pub fn commit_session(session: &EditorSession, path: &Path) -> Result<()> {
    commit(session, path)
}

/// For the `tween` command: the first scene and the layer given, else the first that is not the
/// camera.
pub fn first_scene_layer(
    session: &EditorSession,
    layer: Option<uuid::Uuid>,
) -> Result<(flare_core::SceneId, flare_core::LayerId)> {
    draw::target_layer(session, layer)
}
