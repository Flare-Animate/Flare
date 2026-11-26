//! Parameters of Bend Edge, which reshapes one edge of merged drawing.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BendEdge {
    /// Where the edge is taken hold of, [x, y] on the stage of the first scene: the merged line
    /// or shape edge nearest it.
    pub from: [f64; 2],
    /// Where the grabbed point is dragged to; the edge curves through it and its ends stay.
    pub to: [f64; 2],
    /// How far from an edge a grab still takes hold of it, in stage units, 0 to 100; defaults to 6.
    pub tolerance: Option<f64>,
    /// Frame; defaults to 0.
    pub frame: Option<u32>,
    /// Only the shapes on this layer (its id); without it, those on every unlocked layer.
    pub layer: Option<String>,
    /// Only say which edge would be taken; changes nothing and needs no write access.
    pub dry_run: Option<bool>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
