//! Adding shapes, symbols and instances.

use anyhow::{Context, Result};
use flare_commands::Command;
use flare_core::{InstanceId, LayerId, Rgba8, SceneId, SymbolId};
use flare_document::{Content, FrameSpan, Instance, Layer, Stage, Timeline, Tween};
use flare_history::EditorSession;
use flare_tools::ops::{self, Target};
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::Path;
use uuid::Uuid;

use super::*;

/// Adds a symbol of `kind`, optionally containing one filled rectangle, as the desktop's Create
/// New Symbol does: a blank name or one the library already has is refused.
pub fn add_symbol(
    path: &Path,
    name: &str,
    kind: flare_document::SymbolKind,
    rect: Option<([f64; 4], RectStyle)>,
) -> Result<Value> {
    let mut session = open(path)?;
    let timeline = match rect {
        Some((rect, style)) => one_frame_timeline(rect_content(rect, style)?)?,
        None => Timeline::default(),
    };
    let (commands, id) =
        flare_tools::library::create_symbol_with(session.document(), name, kind, timeline)?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "symbol": id.as_uuid() }))
}

/// Draws a filled rectangle into a layer of the first scene, as the desktop app does: it joins the
/// layer's keyframe at `start` as one more element, or starts a keyframe of `duration` frames
/// there. The layer is the one named, else the topmost that is not a folder or the camera; a
/// document with none gets "Layer 1". A locked, hidden, folder, mask, guide or camera layer, or a
/// keyframe that tweens, is refused.
pub fn draw_rect(
    path: &Path,
    rect: [f64; 4],
    style: RectStyle,
    start: u32,
    duration: u32,
    layer: Option<Uuid>,
) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let duration = NonZeroU32::new(duration).context("duration must be at least 1")?;
    let Content::Shape(shape) = rect_content(rect, style)? else {
        bail!("a rectangle is a shape");
    };
    let target = match layer {
        Some(id) => Target::Layer(
            session.document().scenes()[0]
                .timeline()
                .layers()
                .iter()
                .find(|candidate| *candidate.id().as_uuid() == id)
                .with_context(|| format!("no layer {id} in the first scene"))?
                .id(),
        ),
        None => Target::Default,
    };
    let plan = ops::draw_shapes(
        session.document(),
        scene,
        target,
        start,
        Some(duration),
        vec![shape],
        "Layer 1",
    )
    .context("cannot draw")?;
    let element = plan.placed.first().map_or(0, |at| at.element);
    session.apply(plan.commands)?;
    commit(&session, path)?;
    Ok(json!({ "drawn": true, "layer": plan.layer.as_uuid(), "element": element }))
}

/// The first scene and the requested layer (default: its first layer that is not the camera).
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
        None => layers
            .iter()
            .find(|l| !l.is_camera())
            .context("scene has no layer to draw on")?,
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

/// What `flare place` takes.
#[derive(clap::Args)]
pub struct PlaceArgs {
    pub path: std::path::PathBuf,
    pub symbol: Uuid,
    #[arg(long, default_value_t = 0)]
    pub start: u32,
    #[arg(long, default_value_t = 1)]
    pub duration: u32,
    /// Where the registration point goes.
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    pub x: f64,
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    pub y: f64,
    /// Turned about the registration point, clockwise, in degrees.
    #[arg(long, default_value_t = 0.0, allow_negative_numbers = true)]
    pub rotate: f64,
    /// Scaled about the registration point, before the turn; a negative scale mirrors.
    #[arg(long, default_value_t = 1.0, allow_negative_numbers = true)]
    pub scale_x: f64,
    #[arg(long, default_value_t = 1.0, allow_negative_numbers = true)]
    pub scale_y: f64,
    /// Tween linearly to the next adjacent span of the same symbol.
    #[arg(long)]
    pub tween: bool,
    /// Layer id from `add-layer` or `info` (default: the first layer).
    #[arg(long)]
    pub layer: Option<Uuid>,
}

/// Places an instance on a layer of the first scene: its registration point at `x`, `y`, turned
/// and scaled about it.
pub fn place(args: &PlaceArgs) -> Result<Value> {
    let mut session = open(&args.path)?;
    let (scene, layer) = target_layer(&session, args.layer)?;
    let transform = ops::placement(args.x, args.y, args.rotate, args.scale_x, args.scale_y)
        .map_err(|e| anyhow::anyhow!("cannot place: {e}"))?;
    let instance = Instance::new(
        InstanceId::new(),
        SymbolId::from_uuid(args.symbol),
        transform,
    );
    let duration = NonZeroU32::new(args.duration).context("duration must be at least 1")?;
    let mut span = FrameSpan::new(args.start, duration, Content::Instance(instance.clone()));
    if args.tween {
        span = span.with_tween(Tween::Linear);
    }
    session.apply([Command::InsertSpan { scene, layer, span }])?;
    commit(&session, &args.path)?;
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

/// Renames the first scene; a blank or over-long name, or a project with no scene, writes nothing.
pub fn rename_scene(path: &Path, name: &str) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .map(|s| s.id())
        .context("the project has no scene")?;
    session.apply([Command::RenameScene {
        scene,
        name: name.to_owned(),
    }])?;
    commit(&session, path)?;
    let name = session.document().scenes()[0].name().to_owned();
    Ok(json!({ "scene": name }))
}

/// Replaces any of the stage's width, height and background.
pub fn set_stage(
    path: &Path,
    width: Option<u32>,
    height: Option<u32>,
    background: Option<Rgba8>,
    fps: Option<f64>,
) -> Result<Value> {
    let mut session = open(path)?;
    let current = session.document().stage();
    let stage = Stage::new(
        width.unwrap_or(current.width()),
        height.unwrap_or(current.height()),
        background.unwrap_or(current.background()),
    )?;
    let mut commands = vec![Command::SetStage(stage)];
    if let Some(fps) = fps {
        commands.push(Command::SetFrameRate(
            flare_core::FrameRate::from_per_second(fps)?,
        ));
    }
    session.apply(commands)?;
    commit(&session, path)?;
    let rate = session.document().frame_rate();
    Ok(json!({ "width": stage.width(), "height": stage.height(), "fps": rate.per_second() }))
}
