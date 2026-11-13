//! Moving a layer to any place in the stacking order, as a drag in the timeline does: a folder
//! goes with everything in it, and the layer can go into a folder or out of one on the way.

use std::collections::HashSet;

use flare_core::{LayerId, SceneId};
use flare_document::Document;

use super::{LayerRefusal, layers_of};
use crate::{Command, LayerChange};

/// Moves `layer` to just above `before` (to the bottom with `None`) and puts it in `folder` (out
/// of any folder with `None`). A folder takes everything in it along, in the same order. Empty
/// when nothing would change.
///
/// Refused when `before` is the layer or inside it, or when `folder` is not a folder above the
/// place the layer goes, or is the layer itself or inside it.
pub fn move_to(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    before: Option<LayerId>,
    folder: Option<LayerId>,
) -> Result<Vec<Command>, LayerRefusal> {
    let layers = layers_of(document, scene);
    let from = layers
        .iter()
        .position(|l| l.id() == layer)
        .ok_or(LayerRefusal::NoSuchLayer)?;
    // The layer and everything in it, at any depth. A parent always comes before what it holds.
    let mut moving = HashSet::from([layer]);
    for l in &layers[from + 1..] {
        if l.parent().is_some_and(|p| moving.contains(&p)) {
            moving.insert(l.id());
        }
    }
    let (block, rest): (Vec<LayerId>, Vec<LayerId>) = layers
        .iter()
        .map(|l| l.id())
        .partition(|id| moving.contains(id));
    let at = match before {
        Some(b) if moving.contains(&b) => {
            return Err(LayerRefusal::Invalid("a layer cannot go inside itself"));
        }
        Some(b) => rest
            .iter()
            .position(|id| *id == b)
            .ok_or(LayerRefusal::NoSuchLayer)?,
        None => rest.len(),
    };
    if let Some(f) = folder {
        let above = rest[..at]
            .iter()
            .any(|id| *id == f && layers.iter().any(|l| l.id() == f && l.is_folder()));
        if !above {
            return Err(LayerRefusal::Invalid(
                "a layer can only go into a folder above it",
            ));
        }
    }
    let wanted: Vec<LayerId> = rest[..at]
        .iter()
        .chain(&block)
        .chain(&rest[at..])
        .copied()
        .collect();
    // Each layer in turn is put at its place, top first; the ones above it are already in theirs,
    // so it is always found further down, and each move is one MoveLayer as applying does it.
    let mut order: Vec<LayerId> = layers.iter().map(|l| l.id()).collect();
    let mut commands = Vec::new();
    for (index, id) in wanted.iter().enumerate() {
        if order[index] == *id {
            continue;
        }
        let Some(found) = order.iter().position(|o| o == id) else {
            continue;
        };
        let moved = order.remove(found);
        order.insert(index, moved);
        commands.push(Command::MoveLayer {
            scene,
            layer: *id,
            index,
        });
    }
    if layers[from].parent() != folder {
        commands.push(Command::UpdateLayer {
            scene,
            layer,
            change: LayerChange::Parent(folder),
        });
    }
    Ok(commands)
}
