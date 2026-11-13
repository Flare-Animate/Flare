//! Convert Layers to Symbol, as Animate's layer menu has it: the layers picked (a folder with
//! everything in it) move, frames, tweens and folders as they are, into a new symbol, and one
//! layer named after it takes their place with an instance of it from the first frame to their
//! last. Nothing moves on the stage: the symbol's origin is the stage's, so the instance sits
//! where the artwork was.
//!
//! Sounds are not something a symbol can play everywhere Flare publishes, so the layers' sound
//! cues stay in the scene, on the new layer, at the frames they had.

use std::collections::HashSet;
use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, SymbolId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, Layer, Symbol, SymbolKind, Timeline};
use thiserror::Error;

/// The longest symbol name, in characters.
pub const MAX_NAME_CHARS: usize = 256;

/// Why layers could not become a symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ConvertRefusal {
    #[error("pick at least one layer to convert")]
    NoLayers,
    #[error("there is no such layer")]
    NoSuchLayer,
    #[error("a symbol needs a name")]
    NameEmpty,
    #[error("a symbol's name must be at most {MAX_NAME_CHARS} characters")]
    NameTooLong,
    #[error("the library already has a symbol of that name")]
    NameTaken,
    #[error("the camera's layer belongs to the scene and cannot go into a symbol")]
    Camera,
    #[error("a layer of bones cannot go into a symbol yet")]
    Armature,
    #[error("the layers have no frames to put in a symbol")]
    NoFrames,
    #[error("the layers have more sounds than one layer can hold")]
    TooManySounds,
    #[error("the symbol could not be made")]
    Invalid,
}

/// What a conversion does: the commands for one undo step, the new symbol and the new layer.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub commands: Vec<Command>,
    pub symbol: SymbolId,
    pub layer: LayerId,
}

/// Moves `picked` (folders with everything in them) into a new symbol of `kind` called `name`,
/// and puts an instance of it, on one layer, where the topmost of them was.
pub fn convert_layers_to_symbol(
    document: &Document,
    scene: SceneId,
    picked: &[LayerId],
    name: &str,
    kind: SymbolKind,
) -> Result<Converted, ConvertRefusal> {
    let name = checked_name(document, name)?;
    let layers = document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .map(|s| s.timeline().layers())
        .unwrap_or_default();
    if picked.is_empty() {
        return Err(ConvertRefusal::NoLayers);
    }
    let mut moving: HashSet<LayerId> = HashSet::new();
    for id in picked {
        if !layers.iter().any(|l| l.id() == *id) {
            return Err(ConvertRefusal::NoSuchLayer);
        }
        moving.insert(*id);
    }
    // What a folder holds goes with it. A parent always comes before what it holds.
    for l in layers {
        if l.parent().is_some_and(|p| moving.contains(&p)) {
            moving.insert(l.id());
        }
    }
    let moved: Vec<&Layer> = layers.iter().filter(|l| moving.contains(&l.id())).collect();
    let mut inside = Vec::with_capacity(moved.len());
    let mut cues = Vec::new();
    for l in &moved {
        if l.is_camera() {
            return Err(ConvertRefusal::Camera);
        }
        if l.spans()
            .iter()
            .flat_map(FrameSpan::elements)
            .any(|e| matches!(e, Content::Armature(_)))
        {
            return Err(ConvertRefusal::Armature);
        }
        cues.extend(l.cues().iter().cloned());
        // A folder the picked layers sit in stays in the scene; inside, they are at the top.
        let parent = l.parent().filter(|p| moving.contains(p));
        let kept = Layer::clone(l)
            .with_parent(parent)
            .with_cues(Vec::new())
            .map_err(|_| ConvertRefusal::Invalid)?;
        inside.push(kept);
    }
    let frames = moved
        .iter()
        .flat_map(|l| l.spans())
        .map(FrameSpan::end)
        .max()
        .unwrap_or(0);
    let frames = u32::try_from(frames)
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or(ConvertRefusal::NoFrames)?;
    let symbol = SymbolId::new();
    let made = Symbol::new(symbol, name.clone(), Timeline::new(inside)).with_kind(kind);
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    let top = moved.first().ok_or(ConvertRefusal::NoLayers)?;
    let layer = LayerId::new();
    let replacement = Layer::new(
        layer,
        name,
        vec![FrameSpan::new(0, frames, Content::Instance(instance))],
    )
    .map_err(|_| ConvertRefusal::Invalid)?
    .with_parent(top.parent().filter(|p| !moving.contains(p)))
    .with_cues(cues)
    .map_err(|_| ConvertRefusal::TooManySounds)?;
    // Where the topmost layer was, counted among the layers that stay.
    let index = layers
        .iter()
        .take_while(|l| l.id() != top.id())
        .filter(|l| !moving.contains(&l.id()))
        .count();
    let mut commands: Vec<Command> = moved
        .iter()
        .map(|l| Command::RemoveLayer {
            scene,
            layer: l.id(),
        })
        .collect();
    commands.push(Command::AddSymbol(made));
    commands.push(Command::AddLayer {
        scene,
        layer: replacement,
    });
    commands.push(Command::MoveLayer {
        scene,
        layer,
        index,
    });
    Ok(Converted {
        commands,
        symbol,
        layer,
    })
}

/// `name` trimmed, when it is one a new symbol may have.
fn checked_name(document: &Document, name: &str) -> Result<String, ConvertRefusal> {
    let name = name.trim();
    if name.is_empty() {
        return Err(ConvertRefusal::NameEmpty);
    }
    if name.chars().count() > MAX_NAME_CHARS {
        return Err(ConvertRefusal::NameTooLong);
    }
    if document.symbols().iter().any(|s| s.name() == name) {
        return Err(ConvertRefusal::NameTaken);
    }
    Ok(name.to_owned())
}

/// `base` if no symbol has that name, else `base 2`, `base 3` and on: the name the window gives
/// a symbol made from a layer without asking.
pub fn free_name(document: &Document, base: &str) -> String {
    let base = base.trim();
    let base = if base.is_empty() { "Symbol" } else { base };
    let taken: HashSet<&str> = document.symbols().iter().map(Symbol::name).collect();
    if !taken.contains(base) {
        return base.to_owned();
    }
    (2..=taken.len() + 2)
        .map(|n| format!("{base} {n}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .unwrap_or_else(|| base.to_owned())
}

#[cfg(test)]
#[path = "layers_to_symbol_tests.rs"]
mod tests;
