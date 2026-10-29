//! Headless project commands. They use the same edit session and storage as any future UI.

use std::num::NonZeroU32;
use std::path::Path;

use anyhow::{Context, Result, bail};
use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Shape, Stage, Stroke, Symbol, Timeline,
    Tween,
};
use flare_edit::{Command, EditorSession};
use glamx::{DAffine2, DVec2};
use serde_json::{Value, json};
use uuid::Uuid;

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
fn target_layer(session: &EditorSession, layer: Option<Uuid>) -> Result<(SceneId, LayerId)> {
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

/// Renders one frame to a PNG file.
pub fn export(
    path: &Path,
    out: &Path,
    frame: u32,
    width: Option<u32>,
    height: Option<u32>,
    transparent: bool,
) -> Result<Value> {
    let session = open(path)?;
    let document = session.document();
    let scene = document
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let evaluated = flare_eval::evaluate_frame(document, scene, frame)?;
    // Size and background default to the document's stage.
    let stage = document.stage();
    let (width, height) = (
        width.unwrap_or(stage.width()),
        height.unwrap_or(stage.height()),
    );
    let background = if transparent {
        Rgba8::TRANSPARENT
    } else {
        stage.background()
    };
    let image = flare_render::render(&evaluated, width, height, background)?;
    // Write to a sibling temp file, then rename, so a failed export never leaves a partial PNG.
    let directory = out
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(directory)?;
    image.write_png(std::io::BufWriter::new(&mut staged))?;
    staged.persist(out).map_err(|error| error.error)?;
    Ok(json!({ "exported": out.to_string_lossy(), "width": width, "height": height }))
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

/// Builds a rectangle style from the command-line color strings.
pub fn rect_style(color: &str, stroke: Option<&str>, stroke_width: f64) -> Result<RectStyle> {
    Ok(RectStyle {
        fill: parse_color(color)?,
        stroke: stroke
            .map(|c| Ok::<_, anyhow::Error>((stroke_width, parse_color(c)?)))
            .transpose()?,
    })
}

/// Timeline frame operations, shared with the desktop UI through `flare_edit::frames`.
#[derive(Clone, Copy, Debug, clap::ValueEnum)]
pub enum FrameOperation {
    InsertFrame,
    RemoveFrame,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
}

pub fn frame_op(path: &Path, op: FrameOperation, at: u32, layer: Option<Uuid>) -> Result<Value> {
    use flare_edit::frames;
    let mut session = open(path)?;
    let (scene, layer_id) = target_layer(&session, layer)?;
    let layer = session
        .document()
        .scenes()
        .first()
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer_id))
        .context("layer disappeared")?;
    let commands = match op {
        FrameOperation::InsertFrame => frames::insert_frame(scene, layer, at),
        FrameOperation::RemoveFrame => frames::remove_frame(scene, layer, at),
        FrameOperation::InsertKeyframe => frames::insert_keyframe(scene, layer, at),
        FrameOperation::InsertBlankKeyframe => frames::insert_blank_keyframe(scene, layer, at),
        FrameOperation::ClearKeyframe => frames::clear_keyframe(scene, layer, at),
    }
    .with_context(|| format!("{op:?} does not apply at frame {at} of this layer"))?;
    session.apply(commands)?;
    commit(&session, path)?;
    Ok(json!({ "applied": format!("{op:?}"), "frame": at }))
}
