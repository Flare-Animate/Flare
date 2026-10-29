//! Headless project commands. They use the same edit session and storage as any future UI.

use std::num::NonZeroU32;
use std::path::Path;

use anyhow::{Context, Result, bail};
use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, Scene, Symbol, Timeline, Tween,
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

pub fn new(path: &Path, numerator: u32, denominator: u32) -> Result<Value> {
    if path.exists() {
        bail!("{} already exists", path.display());
    }
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![])?;
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(numerator, denominator)?;
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .map_err(|errors| anyhow::anyhow!("{errors:?}"))?;
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
        "scenes": scenes,
        "symbols": symbols,
    }))
}

pub fn add_symbol(path: &Path, name: &str) -> Result<Value> {
    let mut session = open(path)?;
    let id = SymbolId::new();
    session.apply([Command::AddSymbol(Symbol::new(
        id,
        name,
        Timeline::default(),
    ))])?;
    commit(&session, path)?;
    Ok(json!({ "symbol": id.as_uuid() }))
}

/// Places an instance on the first layer of the first scene.
pub fn place(
    path: &Path,
    symbol: Uuid,
    start: u32,
    duration: u32,
    x: f64,
    y: f64,
    tween: bool,
) -> Result<Value> {
    let mut session = open(path)?;
    let (scene, layer) = {
        let scene = session
            .document()
            .scenes()
            .first()
            .context("document has no scene")?;
        let layer = scene
            .timeline()
            .layers()
            .first()
            .context("scene has no layer")?;
        (scene.id(), layer.id())
    };
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
