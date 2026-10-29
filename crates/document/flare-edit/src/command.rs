use flare_core::{LayerId, SceneId};
use flare_document::Timeline;
use flare_document::{Diagnostic, Document, FrameSpan, Layer, Scene, Stage, Symbol, TimelineError};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddSymbol(Symbol),
    AddScene(Scene),
    SetStage(Stage),
    AddLayer {
        scene: SceneId,
        layer: Layer,
    },
    InsertSpan {
        scene: SceneId,
        layer: LayerId,
        span: FrameSpan,
    },
    RemoveSpan {
        scene: SceneId,
        layer: LayerId,
        start: u32,
    },
}

#[derive(Debug, Error, PartialEq)]
pub enum EditError {
    #[error("no such scene {0:?}")]
    NoSuchScene(SceneId),
    #[error("no such layer {0:?}")]
    NoSuchLayer(LayerId),
    #[error("no span starts at frame {0}")]
    NoSuchSpan(u32),
    #[error(transparent)]
    Timeline(#[from] TimelineError),
    #[error("edit would make the document invalid: {0:?}")]
    Invalid(Vec<Diagnostic>),
}

impl From<Vec<Diagnostic>> for EditError {
    fn from(value: Vec<Diagnostic>) -> Self {
        Self::Invalid(value)
    }
}

/// Applies one command, returning a new validated document. The input is untouched.
pub(crate) fn apply(document: &Document, command: Command) -> Result<Document, EditError> {
    let mut scenes = document.scenes().to_vec();
    let mut symbols = document.symbols().to_vec();
    let mut stage = document.stage();
    match command {
        Command::AddSymbol(symbol) => symbols.push(symbol),
        Command::AddScene(scene) => scenes.push(scene),
        Command::SetStage(new_stage) => stage = new_stage,
        Command::AddLayer { scene, layer } => {
            edit_layers(&mut scenes, scene, |layers| {
                layers.push(layer);
                Ok(())
            })?;
        }
        Command::InsertSpan { scene, layer, span } => {
            edit_layer(&mut scenes, scene, layer, |spans| {
                spans.push(span);
                Ok(())
            })?;
        }
        Command::RemoveSpan {
            scene,
            layer,
            start,
        } => {
            edit_layer(&mut scenes, scene, layer, |spans| {
                let before = spans.len();
                spans.retain(|span| span.start() != start);
                if spans.len() == before {
                    Err(EditError::NoSuchSpan(start))
                } else {
                    Ok(())
                }
            })?;
        }
    }
    // The stage is carried through every edit; dropping it here would reset it silently.
    Ok(Document::new(document.id(), document.frame_rate(), scenes, symbols)?.with_stage(stage))
}

fn edit_layers(
    scenes: &mut [Scene],
    scene_id: SceneId,
    edit: impl FnOnce(&mut Vec<Layer>) -> Result<(), EditError>,
) -> Result<(), EditError> {
    let scene = scenes
        .iter_mut()
        .find(|scene| scene.id() == scene_id)
        .ok_or(EditError::NoSuchScene(scene_id))?;
    let mut layers = scene.timeline().layers().to_vec();
    edit(&mut layers)?;
    *scene = Scene::new(scene.id(), scene.name(), Timeline::new(layers));
    Ok(())
}

fn edit_layer(
    scenes: &mut [Scene],
    scene_id: SceneId,
    layer_id: LayerId,
    edit: impl FnOnce(&mut Vec<FrameSpan>) -> Result<(), EditError>,
) -> Result<(), EditError> {
    edit_layers(scenes, scene_id, |layers| {
        let slot = layers
            .iter_mut()
            .find(|layer| layer.id() == layer_id)
            .ok_or(EditError::NoSuchLayer(layer_id))?;
        let mut spans = slot.spans().to_vec();
        edit(&mut spans)?;
        // with_spans keeps the layer's name, visibility and lock.
        *slot = slot.with_spans(spans)?;
        Ok(())
    })
}
