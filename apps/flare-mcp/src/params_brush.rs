//! Parameters of the Brush tool.

use schemars::JsonSchema;
use serde::Deserialize;

/// The arguments of `brush_stroke`.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BrushStrokes {
    /// The strokes, laid in order, each above the one before: a stroke is its points on the
    /// stage of the first scene, each [x, y] at full pressure or [x, y, pressure] with the pen's
    /// pressure, 0 to 1. At most 4096 strokes, 16384 points in a stroke and 1000000 in all; a
    /// stroke that does not move is skipped and named in `skipped`.
    pub strokes: Vec<Vec<Vec<f64>>>,
    /// Width at full pressure in stage units, 1 to 200; defaults to 12, as the desktop starts.
    pub size: Option<f64>,
    /// Paint color, RRGGBB or RRGGBBAA hex. Defaults to black.
    pub color: Option<String>,
    /// A ready-made brush: plain (the default), pencil, ink, grease-pencil, calligraphy,
    /// vector, animate, g-pen, mapping-pen, turnip-pen, brush-pen, dry-brush, bristle, charcoal,
    /// chalk or crayon.
    pub profile: Option<String>,
    /// How opaque the paint is, 0 to 100 (%); the profile's when omitted.
    pub flow: Option<f32>,
    /// Samples averaged to steady the hand, 0 (off) to 40; the profile's when omitted. It
    /// averages over the points given, so a stroke of a few far-apart points is pulled
    /// straight: give points a few units apart, or 0.
    pub stabilizer: Option<u8>,
    /// How long the start of each stroke tapers in, in stage units; with `taper_end`.
    pub taper_start: Option<f64>,
    /// How long the end of each stroke tapers out, in stage units; with `taper_start`.
    pub taper_end: Option<f64>,
    /// How far the centre line may be simplified, in stage units, 0 to 100; defaults to 2, the
    /// desktop's at 100% zoom.
    pub smoothing: Option<f64>,
    /// Frame drawn on; defaults to 0.
    pub frame: Option<u32>,
    /// Length of a new keyframe in frames; by default it runs to the end of the scene.
    pub duration: Option<u32>,
    /// Layer id; by default the topmost that can take art, or a new "Layer 1".
    pub layer: Option<String>,
    /// Merge Drawing, as for `draw_rect`: strokes cut and join the layer's merged drawing.
    /// Defaults to false, drawing objects.
    pub merge: Option<bool>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}
