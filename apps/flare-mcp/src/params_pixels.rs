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

/// The arguments of `blur` and `smudge`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct Blend {
    /// The stroke as points on the stage, [x, y] in order, in the first scene; the bitmap under
    /// the first is the one worked on. At most 4096.
    pub points: Vec<[f64; 2]>,
    /// The pen's pressure at each point, 0 to 1, one per point; without it every point is at full
    /// pressure.
    pub pressure: Option<Vec<f32>>,
    /// Frame; defaults to 0.
    pub frame: Option<u32>,
    /// Width of the stroke in stage units, 1 to 1000; Blur starts at 80, Smudge at 30.
    pub size: Option<f32>,
    /// How far the stroke goes at most, 0 to 100 (%); defaults to 100.
    pub strength: Option<f32>,
    /// How sharp the stroke's edge is, 0 to 100 (%); Blur starts at 0, Smudge at 50.
    pub hardness: Option<f32>,
    /// Pressure sets the size of each dab (default); false keeps every dab full size.
    pub pressure_size: Option<bool>,
    /// Smudge only: how far a picked-up color is carried, 0 to 100; defaults to 70.
    pub stretch: Option<u8>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
