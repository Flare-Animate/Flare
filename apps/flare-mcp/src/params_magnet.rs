//! Parameters of the Magnet tool, which bends lines and shapes.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MagnetPull {
    /// Where the pull starts, [x, y] on the stage of the first scene: the lines and shapes near
    /// it bend.
    pub from: [f64; 2],
    /// Where the pull ends: the point at `from` moves there, the rest of the circle less.
    pub to: [f64; 2],
    /// How far the pull reaches, in stage units, 2 to 1000; defaults to 40.
    pub radius: Option<f64>,
    /// Frame; defaults to 0.
    pub frame: Option<u32>,
    /// Only the shapes on this layer (its id); without it, those on every unlocked layer.
    pub layer: Option<String>,
    /// Only say which shapes the pull reaches; changes nothing and needs no write access.
    pub dry_run: Option<bool>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
