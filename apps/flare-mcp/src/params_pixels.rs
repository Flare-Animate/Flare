//! Parameters of the tools that work on the pixels of bitmaps.

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct MagicWand {
    /// Points on the stage as [x, y], in the first scene: the first picks the pixels of a color
    /// like the one there, the others add to them. All must be on the same bitmap; at most 64.
    pub points: Vec<[f64; 2]>,
    /// Frame; defaults to 0.
    pub frame: Option<u32>,
    /// How far a color may be from the picked one, 0 to 200; defaults to 20.
    pub threshold: Option<u8>,
    /// Only pixels joined to the point (default); false picks every like pixel in the picture.
    pub contiguous: Option<bool>,
    /// How soft the edge left behind is: pixels, rough, normal (default) or smooth.
    pub smoothing: Option<String>,
    /// Clear every pixel except the picked ones, to keep only them.
    pub keep: Option<bool>,
    /// Only say how many pixels would be picked; changes nothing and needs no write access.
    pub dry_run: Option<bool>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
