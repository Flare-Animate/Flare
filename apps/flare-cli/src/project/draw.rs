//! Adding shapes, symbols and instances.

use anyhow::{Context, Result};
use flare_commands::Command;
use flare_core::{InstanceId, LayerId, Rgba8, SceneId, SymbolId, Transform2D};
use flare_document::{Content, FrameSpan, Instance, Layer, Stage, Symbol, Timeline, Tween};
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::Path;
use uuid::Uuid;

use super::*;

/// Adds a Graphic symbol, optionally containing one filled rectangle.
pub fn add_symbol(path: &Path, name: &str, rect: Option<([f64; 4], RectStyle)>) -> Result<Value> {
    let mut session = open(path)?;
    let id = SymbolId::new();
    let timeline = match rect {
        Some((rect, style)) => one_frame_timeline(rect_content(rect, style)?)?,
        None => Timeline::default(),
    };
    session.apply([Command::AddSymbol(Symbol::new(id, name, timeline))])?;
    commit(&session, path)?;
    Ok(json!({ "symbol": id.as_uuid() }))
}

/// Draws a filled rectangle directly on the first layer of the first scene.
pub fn draw_rect(
    path: &Path,
    rect: [f64; 4],
    style: RectStyle,
    start: u32,
    duration: u32,
    layer: Option<Uuid>,
) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, layer) = target_layer(&session, layer)?;
    let duration = NonZeroU32::new(duration).context("duration must be at least 1")?;
    let content = rect_content(rect, style)?;
    let span = FrameSpan::new(start, duration, content);
    session.apply([Command::InsertSpan { scene, layer, span }])?;
    commit(&session, path)?;
    Ok(json!({ "drawn": true }))
}

/// The first scene and the requested layer (default: its first layer).
pub(super) fn target_layer(
    session: &EditorSession,
    layer: Option<Uuid>,
) -> Result<(SceneId, LayerId)> {
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?;
    let layers = scene.timeline().layers();
    let layer = match layer {
        Some(id) => layers
            .iter()
            .find(|candidate| *candidate.id().as_uuid() == id)
            .with_context(|| format!("no layer {id} in the first scene"))?,
        None => layers.first().context("scene has no layer")?,
    };
    Ok((scene.id(), layer.id()))
}

/// Adds an empty layer below the existing ones in the first scene.
pub fn add_layer(path: &Path, name: &str) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let id = LayerId::new();
    let layer = Layer::new(id, name, vec![])?;
    session.apply([Command::AddLayer { scene, layer }])?;
    commit(&session, path)?;
    Ok(json!({ "layer": id.as_uuid() }))
}

/// Places an instance on the first layer of the first scene.
pub struct PlaceOptions {
    pub symbol: Uuid,
    pub start: u32,
    pub duration: u32,
    pub x: f64,
    pub y: f64,
    pub tween: bool,
    pub layer: Option<Uuid>,
}

pub fn place(path: &Path, options: PlaceOptions) -> Result<Value> {
    let PlaceOptions {
        symbol,
        start,
        duration,
        x,
        y,
        tween,
        layer,
    } = options;
    let mut session = open(path)?;
    let (scene, layer) = target_layer(&session, layer)?;
    let transform = Transform2D::new(DAffine2::from_translation(DVec2::new(x, y)))?;
    let instance = Instance::new(InstanceId::new(), SymbolId::from_uuid(symbol), transform);
    let duration = NonZeroU32::new(duration).context("duration must be at least 1")?;
    let mut span = FrameSpan::new(start, duration, Content::Instance(instance.clone()));
    if tween {
        span = span.with_tween(Tween::Linear);
    }
    session.apply([Command::InsertSpan { scene, layer, span }])?;
    commit(&session, path)?;
    Ok(json!({ "instance": instance.id().as_uuid() }))
}

/// Evaluates one frame of a scene (default: the first) using the shared evaluator.
pub fn eval(path: &Path, frame: u32, scene: Option<Uuid>) -> Result<Value> {
    let session = open(path)?;
    let document = session.document();
    let scene = match scene {
        Some(id) => SceneId::from_uuid(id),
        None => document
            .scenes()
            .first()
            .context("document has no scene")?
            .id(),
    };
    let placed = flare_eval::evaluate(document, scene, frame)?;
    let items: Vec<Value> = placed
        .iter()
        .map(|item| {
            json!({
                "path": item.path.iter().map(|id| id.as_uuid()).collect::<Vec<_>>(),
                "symbol": item.symbol.as_uuid(),
                "matrix": item.transform.to_cols_array(),
            })
        })
        .collect();
    Ok(json!({ "frame": frame, "placed": items }))
}

/// Replaces any of the stage's width, height and background.
pub fn set_stage(
    path: &Path,
    width: Option<u32>,
    height: Option<u32>,
    background: Option<Rgba8>,
) -> Result<Value> {
    let mut session = open(path)?;
    let current = session.document().stage();
    let stage = Stage::new(
        width.unwrap_or(current.width()),
        height.unwrap_or(current.height()),
        background.unwrap_or(current.background()),
    )?;
    session.apply([Command::SetStage(stage)])?;
    commit(&session, path)?;
    Ok(json!({ "width": stage.width(), "height": stage.height() }))
}
