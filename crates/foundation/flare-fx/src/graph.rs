//! The graph: its nodes, the checks every graph passes, and what can be read from one.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};

use crate::{EffectKind, FxError, Node, NodeId, Value};

/// Most nodes a graph holds, the Layer and Output among them. Each running node may keep a
/// picture the size of the layer's, so this also bounds what a graph can cost.
pub const MAX_NODES: usize = 32;

/// A layer's effects. Every graph that exists passed [`EffectGraph::from_nodes`]: one Layer node
/// (1) and one Output node (2), inputs that fit each kind and name nodes that exist, no loops,
/// settings in range.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectGraph {
    nodes: Vec<Node>,
}

impl Default for EffectGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl EffectGraph {
    /// The graph that changes nothing: the Layer straight into the Output.
    pub fn new() -> Self {
        let layer = Node::new(NodeId::LAYER, EffectKind::Layer, [0.0, 0.0]);
        let mut output = Node::new(NodeId::OUTPUT, EffectKind::Output, [200.0, 0.0]);
        output.set_input(0, Some(NodeId::LAYER));
        Self {
            nodes: vec![layer, output],
        }
    }

    /// A graph of `nodes`, refused unless it passes every check.
    pub fn from_nodes(nodes: Vec<Node>) -> Result<Self, FxError> {
        if nodes.len() > MAX_NODES {
            return Err(FxError::TooManyNodes);
        }
        let mut ids = HashSet::with_capacity(nodes.len());
        for node in &nodes {
            if !ids.insert(node.id()) {
                return Err(FxError::DuplicateNode(node.id().get()));
            }
            node.check()?;
            let fixed = match node.id() {
                NodeId::LAYER => Some(EffectKind::Layer),
                NodeId::OUTPUT => Some(EffectKind::Output),
                _ => None,
            };
            if fixed.is_some_and(|kind| kind != node.kind())
                || (fixed.is_none() && node.kind().is_fixed())
            {
                return Err(FxError::FixedNodes);
            }
        }
        if !ids.contains(&NodeId::LAYER) || !ids.contains(&NodeId::OUTPUT) {
            return Err(FxError::FixedNodes);
        }
        for node in &nodes {
            for from in node.inputs().iter().flatten() {
                if *from == NodeId::OUTPUT {
                    return Err(FxError::FromOutput);
                }
                if !ids.contains(from) {
                    return Err(FxError::DanglingInput {
                        node: node.id().get(),
                        from: from.get(),
                    });
                }
            }
        }
        let graph = Self { nodes };
        graph.check_no_loops()?;
        Ok(graph)
    }

    /// A depth-first walk that meets a node already on its path has found a loop. Bounded by
    /// the node count, so a long chain cannot overflow the stack.
    fn check_no_loops(&self) -> Result<(), FxError> {
        #[derive(Clone, Copy, PartialEq)]
        enum Mark {
            Open,
            Done,
        }
        let mut marks: HashMap<NodeId, Mark> = HashMap::with_capacity(self.nodes.len());
        for start in &self.nodes {
            if marks.contains_key(&start.id()) {
                continue;
            }
            let mut stack = vec![(start.id(), 0usize)];
            marks.insert(start.id(), Mark::Open);
            while let Some((id, port)) = stack.pop() {
                let inputs = self.node(id).map(Node::inputs).unwrap_or_default();
                match inputs.get(port) {
                    Some(from) => {
                        stack.push((id, port + 1));
                        let Some(from) = from else { continue };
                        match marks.get(from) {
                            Some(Mark::Open) => return Err(FxError::Cycle),
                            Some(Mark::Done) => {}
                            None => {
                                marks.insert(*from, Mark::Open);
                                stack.push((*from, 0));
                            }
                        }
                    }
                    None => {
                        marks.insert(id, Mark::Done);
                    }
                }
            }
        }
        Ok(())
    }

    /// Every node, the Layer and Output first.
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id() == id)
    }

    pub(crate) fn into_nodes(self) -> Vec<Node> {
        self.nodes
    }

    /// Whether the graph draws the layer as it is: the Layer straight into the Output, whatever
    /// else is in the graph.
    pub fn is_identity(&self) -> bool {
        self.node(NodeId::OUTPUT)
            .and_then(|out| out.inputs().first().copied().flatten())
            == Some(NodeId::LAYER)
    }

    /// The nodes the Output depends on, each after its inputs, the Output last. Nodes nothing
    /// reaches are left out: they are drawn in the view but do not change the picture.
    pub fn order(&self) -> Vec<NodeId> {
        let mut seen = HashSet::with_capacity(self.nodes.len());
        let mut order = Vec::with_capacity(self.nodes.len());
        let mut stack = vec![(NodeId::OUTPUT, 0usize)];
        seen.insert(NodeId::OUTPUT);
        while let Some((id, port)) = stack.pop() {
            let inputs = self.node(id).map(Node::inputs).unwrap_or_default();
            match inputs.get(port) {
                Some(from) => {
                    stack.push((id, port + 1));
                    if let Some(from) = from
                        && seen.insert(*from)
                    {
                        stack.push((*from, 0));
                    }
                }
                None => order.push(id),
            }
        }
        order
    }

    /// Whether what the graph draws changes from frame to frame on its own (it has snow that
    /// reaches the Output), so a held drawing still needs a new picture each frame.
    pub fn is_animated(&self) -> bool {
        self.order()
            .iter()
            .filter_map(|id| self.node(*id))
            .any(|n| n.kind().is_animated())
    }

    /// How far, in stage units, the graph can spread a pixel of the layer: what a picture of
    /// part of the stage must take in beyond its edges for the effects to be right inside them.
    pub fn reach(&self) -> f64 {
        self.order()
            .iter()
            .filter_map(|id| self.node(*id))
            .map(|node| {
                let n = |i| f64::from(node.number(i));
                match node.kind() {
                    EffectKind::Blur => 1.5 * n(0) + 1.0,
                    EffectKind::DropShadow => n(0) + 1.5 * n(2) + 1.0,
                    EffectKind::Glow => 1.5 * n(0) + 1.0,
                    _ => 0.0,
                }
            })
            .sum()
    }

    /// A hash of what the graph draws: the nodes that reach the Output, their kinds, inputs and
    /// settings, and not where they sit in the view. Two graphs with the same fingerprint draw
    /// the same picture of the same layer (a collision aside), so it can key a picture cache
    /// within one document revision.
    pub fn fingerprint(&self) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for id in self.order() {
            let Some(node) = self.node(id) else { continue };
            id.hash(&mut hasher);
            node.kind().hash(&mut hasher);
            node.inputs().hash(&mut hasher);
            for value in node.values() {
                match value {
                    Value::Number(n) => n.to_bits().hash(&mut hasher),
                    Value::Color(c) => c.hash(&mut hasher),
                }
            }
        }
        hasher.finish()
    }

    /// Whether `other` draws exactly what this graph draws: the same nodes, connections and
    /// settings, wherever they sit in the view.
    pub fn same_picture(&self, other: &Self) -> bool {
        self.nodes.len() == other.nodes.len()
            && self.nodes.iter().zip(&other.nodes).all(|(a, b)| {
                a.id() == b.id()
                    && a.kind() == b.kind()
                    && a.inputs() == b.inputs()
                    && a.values() == b.values()
            })
    }
}
