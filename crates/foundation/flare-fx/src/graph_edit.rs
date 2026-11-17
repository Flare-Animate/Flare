//! Edits: each makes a new graph from this one and checks it whole, so an edit that would leave
//! a bad graph (a loop, a missing node) is refused and the graph it started from is unchanged.

use crate::fields::value_from_text;
use crate::{EffectGraph, EffectKind, FxError, MAX_NODES, Node, NodeId, Value};

/// How far right of the node before it a node added to the chain is placed in the view.
const CHAIN_STEP: f32 = 170.0;
/// Most settings changed by one edit.
const MAX_FIELDS: usize = 32;

impl EffectGraph {
    fn edited(
        &self,
        edit: impl FnOnce(&mut Vec<Node>) -> Result<(), FxError>,
    ) -> Result<Self, FxError> {
        let mut nodes = self.clone().into_nodes();
        edit(&mut nodes)?;
        Self::from_nodes(nodes)
    }

    fn next_id(&self) -> Result<NodeId, FxError> {
        if self.nodes().len() >= MAX_NODES {
            return Err(FxError::TooManyNodes);
        }
        let highest = self.nodes().iter().map(|n| n.id().get()).max().unwrap_or(2);
        highest
            .checked_add(1)
            .and_then(NodeId::new)
            .ok_or(FxError::TooManyNodes)
    }

    fn known(&self, id: NodeId) -> Result<&Node, FxError> {
        self.node(id).ok_or(FxError::NoSuchNode(id.get()))
    }

    /// Adds a node of `kind` at `at` with nothing connected. Returns its number.
    pub fn add(&self, kind: EffectKind, at: [f32; 2]) -> Result<(Self, NodeId), FxError> {
        if kind.is_fixed() {
            return Err(FxError::FixedNode);
        }
        let id = self.next_id()?;
        let graph = self.edited(|nodes| {
            nodes.push(Node::new(id, kind, at));
            Ok(())
        })?;
        Ok((graph, id))
    }

    /// Adds a node of `kind` at the end of the chain: it takes what went into the Output (on
    /// its chain input) and the Output takes it, so the effect shows at once. The Output moves
    /// right to make room in the view.
    pub fn append(&self, kind: EffectKind) -> Result<(Self, NodeId), FxError> {
        if kind.is_fixed() {
            return Err(FxError::FixedNode);
        }
        let id = self.next_id()?;
        let output = self.known(NodeId::OUTPUT)?;
        let before = output.inputs().first().copied().flatten();
        let at = output.at();
        let graph = self.edited(|nodes| {
            let mut node = Node::new(id, kind, at);
            if let Some(port) = kind.chain_input() {
                node.set_input(port, before);
            }
            nodes.push(node);
            if let Some(out) = nodes.iter_mut().find(|n| n.id() == NodeId::OUTPUT) {
                out.set_input(0, Some(id));
                out.set_at([at[0] + CHAIN_STEP, at[1]]);
            }
            Ok(())
        })?;
        Ok((graph, id))
    }

    /// Connects the output of `from` to input `port` of `to`, replacing what was there.
    pub fn connect(&self, from: NodeId, to: NodeId, port: usize) -> Result<Self, FxError> {
        if from == NodeId::OUTPUT {
            return Err(FxError::FromOutput);
        }
        self.known(from)?;
        self.set_input(to, port, Some(from))
    }

    /// Leaves input `port` of `to` unconnected: it takes nothing, a transparent picture.
    pub fn disconnect(&self, to: NodeId, port: usize) -> Result<Self, FxError> {
        self.set_input(to, port, None)
    }

    fn set_input(&self, to: NodeId, port: usize, from: Option<NodeId>) -> Result<Self, FxError> {
        let node = self.known(to)?;
        let ports = node.kind().inputs().len();
        if port >= ports {
            return Err(FxError::NoSuchPort {
                node: to.get(),
                kind: node.kind().name(),
                port,
                ports,
            });
        }
        self.edited(|nodes| {
            if let Some(node) = nodes.iter_mut().find(|n| n.id() == to) {
                node.set_input(port, from);
            }
            Ok(())
        })
    }

    /// Removes a node. What took its picture takes what it took on its chain input instead, so
    /// taking an effect out of a chain leaves the chain joined. The Layer and Output stay.
    pub fn remove(&self, id: NodeId) -> Result<Self, FxError> {
        let node = self.known(id)?;
        if node.kind().is_fixed() {
            return Err(FxError::FixedNode);
        }
        let bypass = node
            .kind()
            .chain_input()
            .and_then(|port| node.inputs().get(port).copied().flatten());
        self.edited(|nodes| {
            nodes.retain(|n| n.id() != id);
            for node in nodes.iter_mut() {
                for port in 0..node.inputs().len() {
                    if node.inputs()[port] == Some(id) {
                        node.set_input(port, bypass);
                    }
                }
            }
            Ok(())
        })
    }

    /// Moves a node in the graph view. The picture does not change.
    pub fn move_node(&self, id: NodeId, at: [f32; 2]) -> Result<Self, FxError> {
        self.known(id)?;
        self.edited(|nodes| {
            if let Some(node) = nodes.iter_mut().find(|n| n.id() == id) {
                node.set_at(at);
            }
            Ok(())
        })
    }

    /// Sets the setting `name` of node `id`.
    pub fn set_value(&self, id: NodeId, name: &str, value: Value) -> Result<Self, FxError> {
        let kind = self.known(id)?.kind();
        let (index, spec) = kind.param(name).ok_or_else(|| FxError::UnknownParam {
            kind: kind.name(),
            name: name.to_owned(),
            list: kind.param_list(),
        })?;
        let value = value.checked(spec)?;
        self.edited(|nodes| {
            if let Some(node) = nodes.iter_mut().find(|n| n.id() == id) {
                node.set_value(index, value);
            }
            Ok(())
        })
    }

    /// Sets several settings of node `id` from `name=value` text, all or none.
    pub fn set_fields(&self, id: NodeId, fields: &[(String, String)]) -> Result<Self, FxError> {
        if fields.len() > MAX_FIELDS {
            return Err(FxError::TooManyFields(MAX_FIELDS));
        }
        let kind = self.known(id)?.kind();
        let mut graph = self.clone();
        for (name, text) in fields {
            let (_, spec) = kind.param(name).ok_or_else(|| FxError::UnknownParam {
                kind: kind.name(),
                name: name.clone(),
                list: kind.param_list(),
            })?;
            graph = graph.set_value(id, name, value_from_text(spec, text)?)?;
        }
        Ok(graph)
    }
}
