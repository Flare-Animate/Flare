//! Parameters of the Vector Eraser, which takes away whole shapes.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct VectorErase {
    /// `segment` (the default: each line a round tip along `points` touches loses the stretch out
    /// to where other lines cross it, or all of it when nothing does), `whole` (every shape the
    /// tip touches), `rectangular` (those wholly inside the box between two points) or
    /// `freehand` (those wholly inside the loop through three or more).
    #[serde(rename = "type")]
    pub kind: Option<String>,
    /// Points [x, y] on the stage of the first scene: the tip's path, the two corners, or the
    /// loop.
    pub points: Vec<[f64; 2]>,
    /// How wide the round tip is for `segment` and `whole`, in stage units, 1 to 400; defaults
    /// to 20.
    pub size: Option<f64>,
    /// Frame; defaults to 0.
    pub frame: Option<u32>,
    /// Only the shapes on this layer or folder (its id); without it, every shown, unlocked layer.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
