//! Layer effects as a node graph, the way OpenToonz and Tahoma2D build them in their FX
//! schematic: the layer's own picture flows from a Layer node through effect nodes (blur, drop
//! shadow, glow, color adjustments, snow, a merge) into one Output node, and the Output is what
//! is drawn in the layer's place.
//!
//! This crate is the graph as data (built only through checks, so a graph that exists is a valid
//! one), the edits that make a new graph from an old one, and the pixel work that runs a graph
//! over a picture. It draws nothing itself and has no I/O: `flare-render` hands it the layer's
//! pixels and lays the result back over the frame.
//!
//! Lengths are in stage units and are scaled to the picture's pixels when a graph runs (with
//! `flare_core::fx_params`), so an effect looks the same on a zoomed stage and in an export.
#![forbid(unsafe_code)]

mod error;
mod fields;
mod graph;
mod graph_edit;
mod kind;
mod node;
mod ops;
mod pixels;
mod run;

pub use error::FxError;
pub use fields::{parse_color, parse_fields};
pub use graph::{EffectGraph, MAX_NODES};
pub use kind::{EffectKind, ParamKind, ParamSpec, Unit};
pub use node::{Node, NodeId, Value};
pub use pixels::{Pixels, Space};
pub use run::{MAX_REACH_PIXELS, run};

#[cfg(test)]
mod tests;
