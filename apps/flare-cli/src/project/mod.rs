//! Headless project commands. They use the same edit session and storage as any future UI.

use anyhow::{Context, Result, bail};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Stage, Stroke, Timeline};
use flare_history::EditorSession;
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::Path;

mod draw;
mod interchange;
mod render;
mod timeline;

#[allow(unused_imports)]
use draw::*;
pub use draw::{PlaceOptions, add_layer, add_symbol, draw_rect, eval, place, set_stage};
#[allow(unused_imports)]
use interchange::*;
pub use interchange::{export_xfl, import, publish_html5, publish_swf, test_movie};
#[allow(unused_imports)]
use render::*;
pub use render::{export, export_frames};
#[allow(unused_imports)]
use timeline::*;
pub use timeline::{FrameOperation, LayerOperation, frame_op, layer_op};

fn open(path: &Path) -> Result<EditorSession> {
    let document =
        flare_storage::load(path).with_context(|| format!("cannot open {}", path.display()))?;
    Ok(EditorSession::new(document))
}

fn commit(session: &EditorSession, path: &Path) -> Result<()> {
    flare_storage::save(session.document(), path)
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
    flare_storage::save(&document, path)?;
    Ok(json!({ "created": path.to_string_lossy(), "id": document.id().as_uuid() }))
}

pub fn info(path: &Path) -> Result<Value> {
    let session = open(path)?;
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
                    json!({ "id": layer.id().as_uuid(), "name": layer.name(),
                            "visible": layer.is_visible(), "locked": layer.is_locked(),
                            "spans": layer.spans().len() })
                })
                .collect();
            json!({ "id": scene.id().as_uuid(), "name": scene.name(), "layers": layers })
        })
        .collect();
    let symbols: Vec<Value> = document
        .symbols()
        .iter()
        .map(|symbol| json!({ "id": symbol.id().as_uuid(), "name": symbol.name() }))
        .collect();
    Ok(json!({
        "id": document.id().as_uuid(),
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
}

fn rect_content(rect: [f64; 4], style: RectStyle) -> Result<Content> {
    let [x, y, w, h] = rect;
    if !(w > 0.0 && h > 0.0) {
        bail!("rectangle width and height must be positive");
    }
    let mut shape = Shape::rect(ShapeId::new(), x, y, w, h, style.fill)
        .context("rectangle coordinates must be finite")?;
    if let Some((width, color)) = style.stroke {
        shape = shape.with_stroke(Stroke::new(width, color)?);
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
pub fn rect_style(color: &str, stroke: Option<&str>, stroke_width: f64) -> Result<RectStyle> {
    Ok(RectStyle {
        fill: parse_color(color)?,
        stroke: stroke
            .map(|c| Ok::<_, anyhow::Error>((stroke_width, parse_color(c)?)))
            .transpose()?,
    })
}
