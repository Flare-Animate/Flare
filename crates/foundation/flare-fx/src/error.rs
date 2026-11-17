//! Why a graph could not be built, edited or run.

use thiserror::Error;

use crate::MAX_NODES;

#[derive(Debug, Clone, PartialEq, Error)]
pub enum FxError {
    #[error("an effect graph holds at most {MAX_NODES} nodes")]
    TooManyNodes,
    #[error("there is no node {0} in the effect graph")]
    NoSuchNode(u32),
    #[error("node {0} appears twice in the effect graph")]
    DuplicateNode(u32),
    #[error("an effect graph has exactly one Layer node (number 1) and one Output node (number 2)")]
    FixedNodes,
    #[error("the Layer and Output nodes are part of every graph and cannot be added or removed")]
    FixedNode,
    #[error("node {node} has {got} inputs; a {kind} node has {expected}")]
    WrongInputs {
        node: u32,
        kind: &'static str,
        expected: usize,
        got: usize,
    },
    #[error("node {node} has no input {port}; a {kind} node has {ports}")]
    NoSuchPort {
        node: u32,
        kind: &'static str,
        port: usize,
        ports: usize,
    },
    #[error("node {node} takes its input from node {from}, which is not in the graph")]
    DanglingInput { node: u32, from: u32 },
    #[error("the Output node has no output of its own to connect")]
    FromOutput,
    #[error("that connection would make a loop; an effect cannot feed itself")]
    Cycle,
    #[error("unknown effect {0:?}; the effects are {list}", list = crate::EffectKind::name_list())]
    UnknownKind(String),
    #[error("a {kind} node has no setting {name:?}; it has {list}")]
    UnknownParam {
        kind: &'static str,
        name: String,
        list: String,
    },
    #[error("{name} is {min} to {max}, not {value}")]
    OutOfRange {
        name: &'static str,
        min: f32,
        max: f32,
        value: f32,
    },
    #[error("{name} needs a number, not {value:?}")]
    NotANumber { name: &'static str, value: String },
    #[error("{name} needs a color as RRGGBB or RRGGBBAA, not {value:?}")]
    BadColor { name: &'static str, value: String },
    #[error("node {0}'s settings do not match its kind")]
    WrongValues(u32),
    #[error("a node's place in the graph view must be finite and within 100000 of the middle")]
    BadPosition,
    #[error("{0:?} is not a name=value setting")]
    BadField(String),
    #[error("at most {0} settings are given at once")]
    TooManyFields(usize),
    #[error("the picture's pixels do not match its size")]
    BadPixels,
    #[error("the scale from stage to picture must be finite and above zero")]
    BadScale,
}
