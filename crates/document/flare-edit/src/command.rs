use flare_core::{LayerId, SceneId, SymbolId};
use flare_document::Timeline;
use flare_document::{Diagnostic, Document, FrameSpan, Layer, Scene, Stage, Symbol, TimelineError};
use thiserror::Error;

/// A property change on a layer.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerChange {
    Name(String),
    Visible(bool),
    Locked(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddSymbol(Symbol),
    AddScene(Scene),
    /// Replaces a symbol's whole timeline.
    ReplaceSymbolTimeline {
        symbol: SymbolId,
        timeline: Timeline,
    },
    /// Leaving in-place symbol editing: replaces `symbol`'s timeline and adds the symbols
    /// created while editing, as one step. They must go together because artwork may have
    /// moved from the edited timeline into a new symbol, which is only valid as a whole.
    MergeSymbolEdit {
        symbol: SymbolId,
        timeline: Timeline,
        new_symbols: Vec<Symbol>,
    },
    SetStage(Stage),
    UpdateLayer {
        scene: SceneId,
        layer: LayerId,
        change: LayerChange,
    },
    RemoveLayer {
        scene: SceneId,
        layer: LayerId,
    },
    /// Moves a layer to `index` in the top-to-bottom order (clamped to the end).
    MoveLayer {
        scene: SceneId,
        layer: LayerId,
        index: usize,
    },
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
    #[error("no such symbol {0:?}")]
    NoSuchSymbol(SymbolId),
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
        Command::MergeSymbolEdit {
            symbol,
            timeline,
            new_symbols,
        } => {
            symbols.extend(new_symbols);
            let slot = symbols
                .iter_mut()
                .find(|s| s.id() == symbol)
                .ok_or(EditError::NoSuchSymbol(symbol))?;
            *slot = Symbol::new(slot.id(), slot.name(), timeline);
        }
        Command::ReplaceSymbolTimeline { symbol, timeline } => {
            let slot = symbols
                .iter_mut()
                .find(|s| s.id() == symbol)
                .ok_or(EditError::NoSuchSymbol(symbol))?;
            *slot = Symbol::new(slot.id(), slot.name(), timeline);
        }
        Command::SetStage(new_stage) => stage = new_stage,
        Command::UpdateLayer {
            scene,
            layer,
            change,
        } => edit_layers(&mut scenes, scene, |layers| {
            let slot = layers
                .iter_mut()
                .find(|l| l.id() == layer)
                .ok_or(EditError::NoSuchLayer(layer))?;
            *slot = match change {
                LayerChange::Name(name) => slot.clone().with_name(name),
                LayerChange::Visible(visible) => slot.clone().with_visible(visible),
                LayerChange::Locked(locked) => slot.clone().with_locked(locked),
            };
            Ok(())
        })?,
        Command::RemoveLayer { scene, layer } => edit_layers(&mut scenes, scene, |layers| {
            let before = layers.len();
            layers.retain(|l| l.id() != layer);
            if layers.len() == before {
                Err(EditError::NoSuchLayer(layer))
            } else {
                Ok(())
            }
        })?,
        Command::MoveLayer {
            scene,
            layer,
            index,
        } => edit_layers(&mut scenes, scene, |layers| {
            let from = layers
                .iter()
                .position(|l| l.id() == layer)
                .ok_or(EditError::NoSuchLayer(layer))?;
            let moved = layers.remove(from);
            layers.insert(index.min(layers.len()), moved);
            Ok(())
        })?,
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
