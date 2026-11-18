//! A layer's effect graph on disk: its nodes by number, kind name, inputs, settings by name and
//! place in the view. Rebuilt through `EffectGraph::from_nodes`, so a damaged file cannot load
//! a graph with a loop or a setting out of range. A setting the file lacks takes its default
//! (a file from before the setting existed); one Flare does not know is refused.

use std::collections::BTreeMap;

use flare_core::Rgba8;
use flare_fx::{EffectGraph, EffectKind, Node, NodeId, Value};
use serde::{Deserialize, Serialize};

use crate::StorageError;

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq)]
#[serde(untagged)]
pub(super) enum ValueWire {
    Number(f32),
    Color([u8; 4]),
}

#[derive(Serialize, Deserialize)]
pub(super) struct NodeWire {
    id: u32,
    kind: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    inputs: Vec<Option<u32>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    params: BTreeMap<String, ValueWire>,
    at: [f32; 2],
}

#[derive(Serialize, Deserialize)]
pub(super) struct EffectsWire {
    nodes: Vec<NodeWire>,
}

fn corrupt(message: impl Into<String>) -> StorageError {
    StorageError::Corrupt(message.into())
}

impl EffectsWire {
    pub(super) fn of(graph: &EffectGraph) -> Self {
        let nodes = graph
            .nodes()
            .iter()
            .map(|node| NodeWire {
                id: node.id().get(),
                kind: node.kind().name().to_owned(),
                inputs: node.inputs().iter().map(|i| i.map(NodeId::get)).collect(),
                params: node
                    .kind()
                    .params()
                    .iter()
                    .zip(node.values())
                    .map(|(spec, value)| {
                        let wire = match value {
                            Value::Number(n) => ValueWire::Number(*n),
                            Value::Color(c) => ValueWire::Color(c.to_array()),
                        };
                        (spec.name.to_owned(), wire)
                    })
                    .collect(),
                at: node.at(),
            })
            .collect();
        Self { nodes }
    }

    pub(super) fn build(self) -> Result<EffectGraph, StorageError> {
        if self.nodes.len() > flare_fx::MAX_NODES {
            return Err(corrupt("a layer's effect graph has too many nodes"));
        }
        let mut nodes = Vec::with_capacity(self.nodes.len());
        for wire in self.nodes {
            let id = NodeId::new(wire.id).ok_or_else(|| corrupt("an effect node numbered 0"))?;
            let kind = EffectKind::parse(&wire.kind)
                .ok_or_else(|| corrupt(format!("unknown effect {:?}", wire.kind)))?;
            if let Some(name) = wire.params.keys().find(|n| kind.param(n).is_none()) {
                return Err(corrupt(format!(
                    "a {} effect has no setting {name:?}",
                    kind.name()
                )));
            }
            let values = kind
                .params()
                .iter()
                .map(|spec| match wire.params.get(spec.name) {
                    None => Value::default_for(spec),
                    Some(ValueWire::Number(n)) => Value::Number(*n),
                    Some(ValueWire::Color([r, g, b, a])) => {
                        Value::Color(Rgba8::new(*r, *g, *b, *a))
                    }
                })
                .collect();
            let inputs = wire
                .inputs
                .into_iter()
                .map(|i| {
                    i.map(|raw| {
                        NodeId::new(raw).ok_or_else(|| corrupt("an effect input from node 0"))
                    })
                    .transpose()
                })
                .collect::<Result<Vec<_>, _>>()?;
            nodes.push(Node::from_parts(id, kind, inputs, values, wire.at));
        }
        EffectGraph::from_nodes(nodes).map_err(|e| corrupt(format!("a layer's effects: {e}")))
    }
}

#[cfg(test)]
#[path = "effects_tests.rs"]
mod tests;
