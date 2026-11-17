//! A layer's effects, edited the same way from every surface: each edit is the command batch for
//! one undo step, built from the layer's graph as it is now, or refused with the reason. The
//! Effects panel, `flare fx` and the MCP effect tools all come here.

use flare_commands::{Command, LayerChange};
use flare_core::{LayerId, SceneId};
use flare_document::{Document, Layer};
use flare_fx::{EffectGraph, EffectKind, FxError, NodeId};
use thiserror::Error;

mod describe;

pub use describe::{describe, kinds};

#[derive(Debug, Clone, PartialEq, Error)]
pub enum EffectsRefusal {
    #[error("there is no such layer in the scene")]
    NoSuchLayer,
    #[error(
        "layer \"{0}\" is a mask, a guide, a camera or a folder; effects go on a layer that is drawn"
    )]
    NotDrawn(String),
    #[error("{0} is not a node number")]
    BadNode(u32),
    #[error("a {kind} node has no input {port:?}; its inputs are {list}")]
    BadPort {
        kind: &'static str,
        port: String,
        list: String,
    },
    #[error(transparent)]
    Graph(#[from] FxError),
}

/// The layer `layer` of `scene`, if effects can go on it.
pub fn layer(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<&Layer, EffectsRefusal> {
    let found = document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer))
        .ok_or(EffectsRefusal::NoSuchLayer)?;
    if found.is_mask() || found.is_guide() || found.is_camera() || found.is_folder() {
        return Err(EffectsRefusal::NotDrawn(found.name().to_owned()));
    }
    Ok(found)
}

/// The layer's graph: its own, or the plain one when it has none, to build on.
pub fn graph_of(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
) -> Result<EffectGraph, EffectsRefusal> {
    Ok(layer(document, scene, layer_id)?
        .effects()
        .cloned()
        .unwrap_or_default())
}

/// The batch that gives the layer `graph` (`None` takes its effects away); empty when it has
/// that already, so asking twice is not a second undo step.
pub fn set(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    graph: Option<EffectGraph>,
) -> Result<Vec<Command>, EffectsRefusal> {
    let current = layer(document, scene, layer_id)?;
    // What the layer would hold, as `with_effects` keeps it.
    let kept = current.clone().with_effects(graph.clone());
    if kept.effects() == current.effects() {
        return Ok(Vec::new());
    }
    Ok(vec![Command::UpdateLayer {
        scene,
        layer: layer_id,
        change: LayerChange::Effects(graph),
    }])
}

/// The batch that makes `edit` of the layer's graph, or the reason it is refused.
fn edit(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    edit: impl FnOnce(&EffectGraph) -> Result<EffectGraph, FxError>,
) -> Result<Vec<Command>, EffectsRefusal> {
    let graph = edit(&graph_of(document, scene, layer_id)?)?;
    set(document, scene, layer_id, Some(graph))
}

/// A kind of node by name, for the ones a person can add.
pub fn kind(name: &str) -> Result<EffectKind, EffectsRefusal> {
    EffectKind::parse(name)
        .filter(|k| !k.is_fixed())
        .ok_or_else(|| FxError::UnknownKind(name.to_owned()).into())
}

/// A node number as given.
pub fn node(raw: u32) -> Result<NodeId, EffectsRefusal> {
    NodeId::new(raw).ok_or(EffectsRefusal::BadNode(raw))
}

/// Input `port` of the node `to` in the layer's graph, by number or by name (`under`).
pub fn port(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    to: NodeId,
    port: &str,
) -> Result<usize, EffectsRefusal> {
    let graph = graph_of(document, scene, layer_id)?;
    let kind = graph.node(to).ok_or(FxError::NoSuchNode(to.get()))?.kind();
    let names = kind.inputs();
    let wanted = port.trim().to_ascii_lowercase();
    wanted
        .parse::<usize>()
        .ok()
        .filter(|i| *i < names.len())
        .or_else(|| names.iter().position(|n| *n == wanted))
        .ok_or_else(|| EffectsRefusal::BadPort {
            kind: kind.name(),
            port: port.to_owned(),
            list: if names.is_empty() {
                "none".to_owned()
            } else {
                names.join(", ")
            },
        })
}

/// Adds a node of `kind` at the end of the layer's chain, so it shows at once, with `fields`
/// (`name=value`) set. Returns the batch and the new node's number.
pub fn add(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    kind: EffectKind,
    fields: &[(String, String)],
) -> Result<(Vec<Command>, NodeId), EffectsRefusal> {
    let (graph, id) = graph_of(document, scene, layer_id)?.append(kind)?;
    let graph = graph.set_fields(id, fields)?;
    Ok((set(document, scene, layer_id, Some(graph))?, id))
}

/// Sets settings of node `id` from `name=value` text, all or none.
pub fn set_fields(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    id: NodeId,
    fields: &[(String, String)],
) -> Result<Vec<Command>, EffectsRefusal> {
    edit(document, scene, layer_id, |g| g.set_fields(id, fields))
}

/// Connects the output of `from` to input `port` of `to`.
pub fn connect(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    from: NodeId,
    to: NodeId,
    port: usize,
) -> Result<Vec<Command>, EffectsRefusal> {
    edit(document, scene, layer_id, |g| g.connect(from, to, port))
}

/// Leaves input `port` of `to` unconnected.
pub fn disconnect(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    to: NodeId,
    port: usize,
) -> Result<Vec<Command>, EffectsRefusal> {
    edit(document, scene, layer_id, |g| g.disconnect(to, port))
}

/// Removes node `id`; the chain through it is joined again.
pub fn remove(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    id: NodeId,
) -> Result<Vec<Command>, EffectsRefusal> {
    edit(document, scene, layer_id, |g| g.remove(id))
}

/// Moves node `id` in the graph view.
pub fn move_node(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
    id: NodeId,
    at: [f32; 2],
) -> Result<Vec<Command>, EffectsRefusal> {
    edit(document, scene, layer_id, |g| g.move_node(id, at))
}

/// Takes all of the layer's effects away.
pub fn clear(
    document: &Document,
    scene: SceneId,
    layer_id: LayerId,
) -> Result<Vec<Command>, EffectsRefusal> {
    set(document, scene, layer_id, None)
}

#[cfg(test)]
mod tests;
