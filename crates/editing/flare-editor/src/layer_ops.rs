//! What Animate's layer menu does to several layers at once: Show All, Lock Others, Hide Others
//! and Duplicate Layers. Each returns the commands for one undo step, so the window's menu, the
//! command line and the MCP server do the same thing.
//!
//! "Others" are every layer but the one picked and its family: the folders it sits in (hiding or
//! locking one would hide or lock it too) and, for a folder, what it holds.

use std::collections::HashSet;

use flare_commands::{Command, LayerChange};
use flare_core::{LayerId, SceneId};
use flare_document::{Content, Document, Layer};
use thiserror::Error;

use crate::frames::duplicate_content;

/// Why a layer operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LayerOpRefusal {
    #[error("there is no such layer")]
    NoSuchLayer,
    #[error("a scene has only one camera, so its layer cannot be duplicated")]
    Camera,
    #[error("duplicating a layer of bones is not supported yet")]
    Armature,
    #[error("the layer could not be copied")]
    Invalid,
}

fn layers_of(document: &Document, scene: SceneId) -> &[Layer] {
    document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .map(|s| s.timeline().layers())
        .unwrap_or_default()
}

/// `layer`, the folders it is in and, for a folder, everything in it at any depth.
fn family(layers: &[Layer], layer: LayerId) -> Result<HashSet<LayerId>, LayerOpRefusal> {
    let picked = layers
        .iter()
        .find(|l| l.id() == layer)
        .ok_or(LayerOpRefusal::NoSuchLayer)?;
    let mut family = HashSet::from([layer]);
    let mut up = picked.parent();
    // A parent is always above its layer, so this ends; the bound guards a damaged list.
    for _ in 0..layers.len() {
        let Some(parent) = up else { break };
        family.insert(parent);
        up = layers
            .iter()
            .find(|l| l.id() == parent)
            .and_then(Layer::parent);
    }
    family.extend(descendants(layers, layer));
    Ok(family)
}

/// Every layer inside `folder`, at any depth, top first. A parent always comes before what it
/// holds, so one pass finds them all.
fn descendants(layers: &[Layer], folder: LayerId) -> Vec<LayerId> {
    let mut inside: HashSet<LayerId> = HashSet::from([folder]);
    let mut found = Vec::new();
    for l in layers {
        if let Some(parent) = l.parent()
            && inside.contains(&parent)
        {
            inside.insert(l.id());
            found.push(l.id());
        }
    }
    found
}

fn change(scene: SceneId, layer: LayerId, change: LayerChange) -> Command {
    Command::UpdateLayer {
        scene,
        layer,
        change,
    }
}

/// Shows every hidden layer. Empty when none is hidden.
pub fn show_all(document: &Document, scene: SceneId) -> Vec<Command> {
    layers_of(document, scene)
        .iter()
        .filter(|l| !l.is_visible())
        .map(|l| change(scene, l.id(), LayerChange::Visible(true)))
        .collect()
}

/// Locks every other layer and unlocks `layer` and the folders it is in, so it is the one that
/// can be drawn on. Empty when that is how things already are.
pub fn lock_others(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerOpRefusal> {
    let layers = layers_of(document, scene);
    let family = family(layers, layer)?;
    Ok(layers
        .iter()
        .filter_map(|l| {
            let locked = !family.contains(&l.id());
            (l.is_locked() != locked).then(|| change(scene, l.id(), LayerChange::Locked(locked)))
        })
        .collect())
}

/// Hides every other layer and shows `layer` and the folders it is in. Empty when that is how
/// things already are.
pub fn hide_others(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, LayerOpRefusal> {
    let layers = layers_of(document, scene);
    let family = family(layers, layer)?;
    Ok(layers
        .iter()
        .filter_map(|l| {
            let visible = family.contains(&l.id());
            (l.is_visible() != visible)
                .then(|| change(scene, l.id(), LayerChange::Visible(visible)))
        })
        .collect())
}

/// A copy of `layer` just above it, named as Animate names one ("Layer 1 copy"), with its frames,
/// tweens, sounds and settings and every element its own. A folder is copied with everything in
/// it. Returns the commands and the copy's id.
pub fn duplicate(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<(Vec<Command>, LayerId), LayerOpRefusal> {
    let layers = layers_of(document, scene);
    let index = layers
        .iter()
        .position(|l| l.id() == layer)
        .ok_or(LayerOpRefusal::NoSuchLayer)?;
    let mut copied: Vec<&Layer> = vec![&layers[index]];
    for id in descendants(layers, layer) {
        copied.extend(layers.iter().find(|l| l.id() == id));
    }
    let names: HashSet<&str> = layers.iter().map(Layer::name).collect();
    let mut ids = std::collections::HashMap::new();
    let mut commands = Vec::new();
    for (offset, original) in copied.iter().enumerate() {
        if original.is_camera() {
            return Err(LayerOpRefusal::Camera);
        }
        let spans = original
            .spans()
            .iter()
            .map(|span| {
                if span.elements().any(|e| matches!(e, Content::Armature(_))) {
                    return Err(LayerOpRefusal::Armature);
                }
                let elements = span
                    .elements()
                    .map(duplicate_content)
                    .collect::<Option<Vec<_>>>()
                    .ok_or(LayerOpRefusal::Invalid)?;
                span.with_elements(elements)
                    .ok()
                    .flatten()
                    .map_or(Ok(span.clone()), Ok)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let id = LayerId::new();
        ids.insert(original.id(), id);
        // The picked layer gets the new name and stays in its folder; what a folder holds keeps
        // its name and goes in the copied folder.
        let (name, parent) = if offset == 0 {
            (copy_name(original.name(), &names), original.parent())
        } else {
            let parent = original.parent().map(|p| ids.get(&p).copied().unwrap_or(p));
            (original.name().to_owned(), parent)
        };
        let copy = Layer::new(id, name, spans)
            .map_err(|_| LayerOpRefusal::Invalid)?
            .with_visible(original.is_visible())
            .with_locked(original.is_locked())
            .with_mask(original.is_mask())
            .with_guide(original.is_guide())
            .with_folder(original.is_folder())
            .with_outline(original.outline())
            .with_view_opacity(original.view_opacity())
            .with_parent(parent)
            .with_cues(original.cues().to_vec())
            .map_err(|_| LayerOpRefusal::Invalid)?;
        commands.push(Command::AddLayer { scene, layer: copy });
        commands.push(Command::MoveLayer {
            scene,
            layer: id,
            index: index + offset,
        });
    }
    let id = ids.get(&layer).copied().ok_or(LayerOpRefusal::Invalid)?;
    Ok((commands, id))
}

/// `name copy`, or `name copy 2` and on when that is taken, as Animate numbers them.
fn copy_name(name: &str, taken: &HashSet<&str>) -> String {
    let first = format!("{name} copy");
    if !taken.contains(first.as_str()) {
        return first;
    }
    (2..=taken.len() + 2)
        .map(|n| format!("{name} copy {n}"))
        .find(|candidate| !taken.contains(candidate.as_str()))
        .unwrap_or(first)
}

#[cfg(test)]
#[path = "layer_ops_tests.rs"]
mod tests;
