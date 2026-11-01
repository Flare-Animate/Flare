//! Commands for what is done to layers: showing and locking them, renaming, deleting, moving and
//! putting them in folders. Each builder looks at the document and returns the commands that do
//! it, or says why not, so a window and a script ask the same thing and get the same answer.

use flare_core::{LayerId, SceneId};
use flare_document::{Document, Layer};
use thiserror::Error;

use crate::{Command, LayerChange};

/// Why a layer operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LayerRefusal {
    #[error("there is no such layer")]
    NoSuchLayer,
    #[error("A layer needs a name")]
    NeedsAName,
    #[error("A scene needs at least one layer")]
    OnlyLayer,
    #[error("The layer is already at the top")]
    AtTop,
    #[error("The layer is already at the bottom")]
    AtBottom,
    #[error("{0}")]
    Invalid(&'static str),
}

/// The layers of `scene`, top first.
fn layers_of(document: &Document, scene: SceneId) -> &[Layer] {
    document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .map(|s| s.timeline().layers())
        .unwrap_or_default()
}

fn layer_in(document: &Document, scene: SceneId, layer: LayerId) -> Result<&Layer, LayerRefusal> {
    layers_of(document, scene)
        .iter()
        .find(|l| l.id() == layer)
        .ok_or(LayerRefusal::NoSuchLayer)
}

fn update(scene: SceneId, layer: LayerId, change: LayerChange) -> Vec<Command> {
    vec![Command::UpdateLayer {
        scene,
        layer,
        change,
    }]
}

/// Shows the layer, or hides it.
pub fn set_visible(scene: SceneId, layer: LayerId, visible: bool) -> Vec<Command> {
    update(scene, layer, LayerChange::Visible(visible))
}

/// Locks the layer, or unlocks it.
pub fn set_locked(scene: SceneId, layer: LayerId, locked: bool) -> Vec<Command> {
    update(scene, layer, LayerChange::Locked(locked))
}

/// Shows the layer if it is hidden, hides it if it is shown.
pub fn toggle_visible(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerRefusal> {
    let shown = layer_in(document, scene, layer)?.is_visible();
    Ok(update(scene, layer, LayerChange::Visible(!shown)))
}

/// Locks the layer if it is unlocked, unlocks it if it is locked.
pub fn toggle_locked(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerRefusal> {
    let locked = layer_in(document, scene, layer)?.is_locked();
    Ok(update(scene, layer, LayerChange::Locked(!locked)))
}

/// Makes the layer a mask for the layer below it, or a normal layer again.
pub fn toggle_mask(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerRefusal> {
    let mask = layer_in(document, scene, layer)?.is_mask();
    Ok(update(scene, layer, LayerChange::Mask(!mask)))
}

/// Makes the layer a motion guide for the layer below it, or a normal layer again.
pub fn toggle_guide(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerRefusal> {
    let guide = layer_in(document, scene, layer)?.is_guide();
    Ok(update(scene, layer, LayerChange::Guide(!guide)))
}

/// Gives the layer a name; a blank one is refused.
pub fn rename(scene: SceneId, layer: LayerId, name: &str) -> Result<Vec<Command>, LayerRefusal> {
    let name = name.trim();
    if name.is_empty() {
        return Err(LayerRefusal::NeedsAName);
    }
    Ok(update(scene, layer, LayerChange::Name(name.to_owned())))
}

/// Deletes the layer. What a folder held is let out of it into the folder's own parent first, and
/// sounds only this layer played go with it, so nothing is left behind. A scene keeps at least
/// one layer.
pub fn delete(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerRefusal> {
    let layers = layers_of(document, scene);
    if layers.len() <= 1 {
        return Err(LayerRefusal::OnlyLayer);
    }
    let outer = layer_in(document, scene, layer)?.parent();
    let mut commands: Vec<Command> = layers
        .iter()
        .filter(|l| l.parent() == Some(layer))
        .map(|held| Command::UpdateLayer {
            scene,
            layer: held.id(),
            change: LayerChange::Parent(outer),
        })
        .collect();
    commands.push(Command::RemoveLayer { scene, layer });
    commands.extend(
        document
            .sounds_only_played_by(layer)
            .into_iter()
            .map(Command::RemoveSound),
    );
    Ok(commands)
}

/// Moves the layer up (`direction` below zero) or down one place in the stacking order.
pub fn move_by(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    direction: i32,
) -> Result<Vec<Command>, LayerRefusal> {
    let layers = layers_of(document, scene);
    let from = layers
        .iter()
        .position(|l| l.id() == layer)
        .ok_or(LayerRefusal::NoSuchLayer)?;
    let index = if direction < 0 {
        from.checked_sub(1).ok_or(LayerRefusal::AtTop)?
    } else {
        from + 1
    };
    if index >= layers.len() {
        return Err(LayerRefusal::AtBottom);
    }
    Ok(vec![Command::MoveLayer {
        scene,
        layer,
        index,
    }])
}

/// Adds a folder just above `held`, in the same folder if that layer is in one, and puts `held`
/// in it. With no layer to hold, the folder goes at the top, empty. Returns the commands and the
/// folder's id.
pub fn add_folder(
    document: &Document,
    scene: SceneId,
    held: Option<LayerId>,
) -> Result<(Vec<Command>, LayerId), LayerRefusal> {
    let layers = layers_of(document, scene);
    let held = held.and_then(|id| layers.iter().position(|l| l.id() == id));
    let folders = layers.iter().filter(|l| l.is_folder()).count();
    let id = LayerId::new();
    let outer = held.and_then(|i| layers[i].parent());
    let folder = Layer::new(id, format!("Folder {}", folders + 1), vec![])
        .map_err(|_| LayerRefusal::Invalid("the folder could not be made"))?
        .with_folder(true)
        .with_parent(outer);
    let mut commands = vec![
        Command::AddLayer {
            scene,
            layer: folder,
        },
        Command::MoveLayer {
            scene,
            layer: id,
            index: held.unwrap_or(0),
        },
    ];
    if let Some(i) = held {
        commands.push(Command::UpdateLayer {
            scene,
            layer: layers[i].id(),
            change: LayerChange::Parent(Some(id)),
        });
    }
    Ok((commands, id))
}

/// Puts the layer in `folder` (a folder above it), or takes it out of its folder with `None`.
pub fn set_folder(scene: SceneId, layer: LayerId, folder: Option<LayerId>) -> Vec<Command> {
    update(scene, layer, LayerChange::Parent(folder))
}

#[cfg(test)]
mod tests;
