//! The vocabulary of document edits: [`Command`]s, and applying a batch of them to a document
//! as one validated step. History and sessions live in `flare-history`.
#![forbid(unsafe_code)]

use flare_core::{FrameRate, LayerId, SceneId, SoundId, SymbolId};
use flare_document::Timeline;
use flare_document::{
    Diagnostic, Document, FrameSpan, Layer, Scene, Sound, SoundCue, SoundError, Stage, Symbol,
    TimelineError,
};
use thiserror::Error;

/// A property change on a layer.
#[derive(Debug, Clone, PartialEq)]
pub enum LayerChange {
    Name(String),
    Visible(bool),
    Locked(bool),
    Mask(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    AddSymbol(Symbol),
    AddScene(Scene),
    RenameSymbol {
        symbol: SymbolId,
        name: String,
    },
    /// Removes a symbol from the library. The edit is refused while an instance still uses it.
    RemoveSymbol(SymbolId),
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
    SetFrameRate(FrameRate),
    /// Adds a sound to the library.
    AddSound(Sound),
    /// Removes a sound from the library. Fails if a cue still plays it.
    RemoveSound(SoundId),
    /// Replaces the sound cues of a layer.
    SetCues {
        scene: SceneId,
        layer: LayerId,
        cues: Vec<SoundCue>,
    },
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
    #[error(transparent)]
    Sound(#[from] SoundError),
    #[error("no such sound {0:?}")]
    NoSuchSound(SoundId),
    #[error("edit would make the document invalid: {0:?}")]
    Invalid(Vec<Diagnostic>),
}

impl From<Vec<Diagnostic>> for EditError {
    fn from(value: Vec<Diagnostic>) -> Self {
        Self::Invalid(value)
    }
}

/// Applies a batch of commands to a copy of `document` and validates the result once.
///
/// Intermediate states are not validated: only the document the batch produces has to be
/// valid, which is the same guarantee callers had, at a fraction of the cost (validation
/// walks the whole document, so checking after every command made an edit of `n` commands
/// cost `n` document walks). `None` means the batch was empty. The input is untouched.
pub fn apply_all(
    document: &Document,
    commands: impl IntoIterator<Item = Command>,
) -> Result<Option<Document>, EditError> {
    let mut commands = commands.into_iter().peekable();
    if commands.peek().is_none() {
        return Ok(None);
    }
    let mut scenes = document.scenes().to_vec();
    let mut symbols = document.symbols().to_vec();
    let mut sounds = document.sounds().to_vec();
    let mut stage = document.stage();
    let mut rate = document.frame_rate();
    for command in commands {
        step(
            &mut scenes,
            &mut symbols,
            &mut sounds,
            &mut stage,
            &mut rate,
            command,
        )?;
    }
    // The stage and source attachments are carried through every edit; attachments whose
    // anchor the edit removed (a deleted layer) are dropped with it.
    Ok(Some(
        Document::new(document.id(), rate, scenes, symbols)?
            .with_sounds(sounds)?
            .with_stage(stage)
            .with_attachments_retained(document.attachments()),
    ))
}

/// One command's effect on the working copy. Does not validate the whole document.
fn step(
    scenes: &mut Vec<Scene>,
    symbols: &mut Vec<Symbol>,
    sounds: &mut Vec<Sound>,
    stage: &mut Stage,
    rate: &mut FrameRate,
    command: Command,
) -> Result<(), EditError> {
    match command {
        Command::AddSymbol(symbol) => symbols.push(symbol),
        Command::AddScene(scene) => scenes.push(scene),
        Command::RenameSymbol { symbol, name } => {
            let slot = symbols
                .iter_mut()
                .find(|s| s.id() == symbol)
                .ok_or(EditError::NoSuchSymbol(symbol))?;
            *slot = slot.clone().with_name(name);
        }
        Command::RemoveSymbol(id) => {
            let before = symbols.len();
            symbols.retain(|s| s.id() != id);
            if symbols.len() == before {
                return Err(EditError::NoSuchSymbol(id));
            }
        }
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
        Command::SetStage(new_stage) => *stage = new_stage,
        Command::SetFrameRate(new_rate) => *rate = new_rate,
        Command::AddSound(sound) => sounds.push(sound),
        Command::RemoveSound(id) => {
            let before = sounds.len();
            sounds.retain(|s| s.id() != id);
            if sounds.len() == before {
                return Err(EditError::NoSuchSound(id));
            }
        }
        Command::SetCues { scene, layer, cues } => edit_layers(scenes, scene, |layers| {
            let slot = layers
                .iter_mut()
                .find(|l| l.id() == layer)
                .ok_or(EditError::NoSuchLayer(layer))?;
            *slot = slot.clone().with_cues(cues)?;
            Ok(())
        })?,
        Command::UpdateLayer {
            scene,
            layer,
            change,
        } => edit_layers(scenes, scene, |layers| {
            let slot = layers
                .iter_mut()
                .find(|l| l.id() == layer)
                .ok_or(EditError::NoSuchLayer(layer))?;
            *slot = match change {
                LayerChange::Name(name) => slot.clone().with_name(name),
                LayerChange::Visible(visible) => slot.clone().with_visible(visible),
                LayerChange::Locked(locked) => slot.clone().with_locked(locked),
                LayerChange::Mask(mask) => slot.clone().with_mask(mask),
            };
            Ok(())
        })?,
        Command::RemoveLayer { scene, layer } => edit_layers(scenes, scene, |layers| {
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
        } => edit_layers(scenes, scene, |layers| {
            let from = layers
                .iter()
                .position(|l| l.id() == layer)
                .ok_or(EditError::NoSuchLayer(layer))?;
            let moved = layers.remove(from);
            layers.insert(index.min(layers.len()), moved);
            Ok(())
        })?,
        Command::AddLayer { scene, layer } => {
            edit_layers(scenes, scene, |layers| {
                layers.push(layer);
                Ok(())
            })?;
        }
        Command::InsertSpan { scene, layer, span } => {
            edit_layer(scenes, scene, layer, |spans| {
                spans.push(span);
                Ok(())
            })?;
        }
        Command::RemoveSpan {
            scene,
            layer,
            start,
        } => {
            edit_layer(scenes, scene, layer, |spans| {
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
    Ok(())
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
    // Edited in place: the layer list is copied once if a snapshot shares it, and the layers
    // themselves (their names, spans and shapes) stay shared.
    edit(scene.timeline_mut().layers_mut())
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
        // with_spans keeps the layer's name, visibility, lock and mask.
        *slot = slot.with_spans(spans)?;
        Ok(())
    })
}
