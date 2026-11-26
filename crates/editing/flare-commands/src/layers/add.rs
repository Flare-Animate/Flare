//! Where a new layer goes: just under the layer being worked on, in the same folder, so it is
//! found where the eye already is rather than at the bottom of a long list.

use std::collections::HashSet;

use flare_core::{LayerId, SceneId};
use flare_document::{Document, Layer};

use super::{LayerRefusal, layers_of};
use crate::Command;

/// Adds `layer` to `scene` just under `under`, in the folder `under` is in; at the bottom with
/// `None`. A folder's contents stay with it, so under a folder the layer goes after everything
/// in it. A mask or a motion guide acts on the layer right under it, so under one the layer
/// goes after that pair rather than between them.
pub fn add_under(
    document: &Document,
    scene: SceneId,
    layer: Layer,
    under: Option<LayerId>,
) -> Result<Vec<Command>, LayerRefusal> {
    let layers = layers_of(document, scene);
    let Some(under) = under else {
        return Ok(vec![Command::AddLayer { scene, layer }]);
    };
    let index = layers
        .iter()
        .position(|l| l.id() == under)
        .ok_or(LayerRefusal::NoSuchLayer)?;
    let current = &layers[index];
    let mut at = block_end(layers, index);
    if (current.is_mask() || current.is_guide()) && at < layers.len() {
        at = block_end(layers, at);
    }
    let id = layer.id();
    let mut commands = vec![Command::AddLayer {
        scene,
        layer: layer.with_parent(current.parent()),
    }];
    // Added, the layer is last; it only moves when it belongs further up.
    if at < layers.len() {
        commands.push(Command::MoveLayer {
            scene,
            layer: id,
            index: at,
        });
    }
    Ok(commands)
}

/// The index just past the layer at `index` and everything inside it, at any depth. A parent
/// always comes before what it holds, so its contents follow it.
fn block_end(layers: &[Layer], index: usize) -> usize {
    let mut inside = HashSet::from([layers[index].id()]);
    let mut end = index + 1;
    while let Some(next) = layers.get(end)
        && next.parent().is_some_and(|p| inside.contains(&p))
    {
        inside.insert(next.id());
        end += 1;
    }
    end
}
