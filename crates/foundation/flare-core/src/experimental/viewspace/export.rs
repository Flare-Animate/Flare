//! Section 12: export targets. The library side that is real is [`render_sequence`], which produces
//! the frames a sprite sheet or image sequence is made of. Packing them is done by
//! `flare_runtime::sprite_sheet`, which lives in a higher layer than this crate, so the wiring
//! belongs in a crate above `flare-core` (a front-end adapter), not here.

use super::ViewSpaceError;
use super::clothing::ClothingState;
use super::expression::ExpressionParams;
use super::interp::{Renderable2D, ViewSpaceInterpolator};
use super::keyframe::ViewParams;

pub const MAX_SEQUENCE: usize = 4096;

/// Render one picture per view for a fixed expression and clothing state. Stops at the first error
/// (for example no samples near a view) and returns nothing partial.
pub fn render_sequence(
    interp: &ViewSpaceInterpolator,
    views: &[ViewParams],
    expr: &ExpressionParams,
    clothing: &ClothingState,
) -> Result<Vec<Renderable2D>, ViewSpaceError> {
    if views.len() > MAX_SEQUENCE {
        return Err(ViewSpaceError::Limit {
            what: "frames in a sequence",
            max: MAX_SEQUENCE,
        });
    }
    views
        .iter()
        .map(|v| interp.evaluate(v, expr, clothing))
        .collect()
}

/// Evenly spaced azimuths over a full turn at one elevation (degrees).
pub fn turntable(frames: usize, elevation_deg: Option<f32>) -> Vec<ViewParams> {
    (0..frames.min(MAX_SEQUENCE))
        .map(|i| {
            ViewParams::from_degrees(
                -180.0 + 360.0 * i as f32 / frames.max(1) as f32,
                elevation_deg,
            )
        })
        .collect()
}

/// Live2D-style parameter model.
pub fn export_live2d_params(_interp: &ViewSpaceInterpolator) -> Result<Vec<u8>, ViewSpaceError> {
    // TODO(viewspace:export-live2d): Map azimuth to ParamAngleX (-30..30), elevation to ParamAngleY,
    // roll to ParamAngleZ, and each expression to a 0..1 parameter. Emit one mesh-deformer keyform
    // per sample (vertex positions projected at the sample's view) with the parameter binding
    // ranges from the sample views, in the Cubism .model3/.moc3-compatible JSON description the host
    // can compile. Needs the format reference and a licence check before shipping.
    Err(ViewSpaceError::NotImplemented(
        "Live2D-style parameter model export",
    ))
}

/// Blender Grease Pencil object with shape keys.
pub fn export_grease_pencil(_interp: &ViewSpaceInterpolator) -> Result<Vec<u8>, ViewSpaceError> {
    // TODO(viewspace:export-gp): One shape key per major view/expression sample (vertex offsets from
    // the bind pose projected at that view), drivers on a camera-angle empty mapping azimuth to key
    // values using the `neighbours` weights; write a .blend-importable script (Python) rather than
    // a binary .blend.
    Err(ViewSpaceError::NotImplemented(
        "Grease Pencil shape-key export",
    ))
}

/// Custom runtime format preserving BaseMesh, samples and correspondence for hosts implementing evaluate().
pub fn export_runtime(_interp: &ViewSpaceInterpolator) -> Result<Vec<u8>, ViewSpaceError> {
    // TODO(viewspace:export-runtime): The types already derive serde. Choose the container in the
    // io layer (JSON header + PNG sample images, zstd), write mesh, per-sample view/alignment/fields/
    // feature masks, expressions, pins and settings with a format version; the host re-runs
    // `register` on load, so no baked vertex colours are stored.
    Err(ViewSpaceError::NotImplemented(
        "custom runtime format export",
    ))
}

/// Layered PSD with clear layer names.
pub fn export_psd(_interp: &ViewSpaceInterpolator) -> Result<Vec<u8>, ViewSpaceError> {
    // TODO(viewspace:export-psd): One layer per keyframe image named "view az=<deg> el=<deg>
    // [expr] [clothing]", one per feature mask named "feature/<name>", depth as a grayscale layer;
    // needs a PSD writer (not a dependency of flare-core; do it in the io layer).
    Err(ViewSpaceError::NotImplemented("layered PSD export"))
}

#[cfg(test)]
#[path = "export_tests.rs"]
mod tests;
