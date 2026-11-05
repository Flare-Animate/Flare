//! Layer 3.4: depth and parameter fields ("Heightmap++"), per mesh vertex. They interpolate with
//! the view like textures.
//!
//! Field names stored in `KeyframeSample::other_param_fields`:
//! - [`THICKNESS`]: z extrusion multiplier (default 1) so a turn is never paper thin.
//! - [`OCCLUSION`]: layering priority added to depth for visibility (default 0; hair over collar).
//! - [`SOFTNESS`]: edge feather 0..=1 (default 0; style stage, see `style` stub).
//! - [`RIGIDITY`]: per-region deformation weight multiplier (default 1).
//!
//! Depth itself is `depth_override` (closeness 0..=1, lighter = closer) or `BaseMesh::default_depth`.

use super::ViewSpaceError;
use super::mesh::BaseMesh;

pub const THICKNESS: &str = "thickness";
pub const OCCLUSION: &str = "occlusion";
pub const SOFTNESS: &str = "softness";
pub const RIGIDITY: &str = "rigidity";

/// Default of a named field when no sample defines it.
pub fn field_default(name: &str) -> f32 {
    match name {
        THICKNESS | RIGIDITY => 1.0,
        _ => 0.0,
    }
}

/// How values are mixed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FieldBlend {
    Linear,
    /// Mix in `v^gamma` space (values clamped to >= 0), then undo. Gamma above 1 favours the
    /// larger value, which keeps a bump from being washed flat by a sample that lacks it.
    Gamma(f32),
}

/// Weighted blend of per-vertex fields. `weights` need not be normalised; they are normalised
/// here (all zero gives the first field). Every field must have the same length.
pub fn blend_fields(
    fields: &[&[f32]],
    weights: &[f32],
    mode: FieldBlend,
) -> Result<Vec<f32>, ViewSpaceError> {
    if fields.is_empty() || fields.len() != weights.len() {
        return Err(ViewSpaceError::Invalid(
            "blend needs one weight per field".into(),
        ));
    }
    let n = fields[0].len();
    if fields.iter().any(|f| f.len() != n) {
        return Err(ViewSpaceError::Invalid("fields differ in length".into()));
    }
    if weights.iter().any(|w| !w.is_finite() || *w < 0.0) {
        return Err(ViewSpaceError::Invalid(
            "weights must be finite and not negative".into(),
        ));
    }
    let sum: f32 = weights.iter().sum();
    let w: Vec<f32> = if sum > 0.0 {
        weights.iter().map(|x| x / sum).collect()
    } else {
        let mut v = vec![0.0; weights.len()];
        v[0] = 1.0;
        v
    };
    let g = match mode {
        FieldBlend::Linear => 1.0,
        FieldBlend::Gamma(g) if g.is_finite() && g > 0.0 => g,
        FieldBlend::Gamma(_) => {
            return Err(ViewSpaceError::Invalid("gamma must be positive".into()));
        }
    };
    Ok((0..n)
        .map(|i| {
            if g == 1.0 {
                fields.iter().zip(&w).map(|(f, w)| f[i] * w).sum()
            } else {
                fields
                    .iter()
                    .zip(&w)
                    .map(|(f, w)| f[i].max(0.0).powf(g) * w)
                    .sum::<f32>()
                    .powf(1.0 / g)
            }
        })
        .collect())
}

/// What a depth brush dab does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthPaintMode {
    Raise,
    Lower,
    Set,
}

/// Paint a field on the mesh with a round brush in the front view (x, y of the bind pose; radius
/// in model units; `hardness` 0..=1). Values are clamped to `0..=1`. `strength` is the added
/// amount for raise/lower, or the target for set. Manual depth paint of Phase 0.
pub fn paint_field(
    mesh: &BaseMesh,
    field: &mut [f32],
    centre: [f32; 2],
    radius: f32,
    hardness: f32,
    strength: f32,
    mode: DepthPaintMode,
) -> Result<usize, ViewSpaceError> {
    if field.len() != mesh.vertices.len() {
        return Err(ViewSpaceError::Invalid(
            "field length differs from the vertex count".into(),
        ));
    }
    if ![centre[0], centre[1], radius, hardness, strength]
        .iter()
        .all(|v| v.is_finite())
        || radius <= 0.0
    {
        return Err(ViewSpaceError::Invalid(
            "brush values must be finite with a positive radius".into(),
        ));
    }
    let hard = hardness.clamp(0.0, 1.0);
    let mut touched = 0;
    for (v, f) in mesh.vertices.iter().zip(field.iter_mut()) {
        let d = ((v[0] - centre[0]).powi(2) + (v[1] - centre[1]).powi(2)).sqrt() / radius;
        if d >= 1.0 {
            continue;
        }
        let fall = if d <= hard {
            1.0
        } else {
            1.0 - (d - hard) / (1.0 - hard).max(1e-3)
        };
        *f = match mode {
            DepthPaintMode::Raise => *f + strength * fall,
            DepthPaintMode::Lower => *f - strength * fall,
            DepthPaintMode::Set => *f + (strength - *f) * fall,
        }
        .clamp(0.0, 1.0);
        touched += 1;
    }
    Ok(touched)
}

/// Seed depth from the aligned mesh, the silhouette and the artist's shadows/highlights.
pub fn auto_depth(
    _mesh: &BaseMesh,
    _silhouette_and_shading: &super::keyframe::Bitmap,
) -> Result<Vec<f32>, ViewSpaceError> {
    // TODO(viewspace:depth-auto): 1. Start from `default_depth`. 2. Compute the silhouette distance
    // transform d(p) (flare_core::image_ops has one) and normalise: inflate(p) = sqrt(d / dmax), a
    // balloon extrusion, sampled at each vertex's projection. 3. Shading term: luminance L at each
    // vertex minus its local mean (blur radius ~ 8% of head width); lighter than the surround means
    // closer, so add k * (L - mean) with k = 0.15. 4. depth = clamp(0.6*default + 0.3*inflate +
    // 0.1*shade, 0, 1); then 2 passes of Laplacian smoothing over mesh edges. The artist paints over
    // the result with `paint_field`.
    Err(ViewSpaceError::NotImplemented(
        "automatic depth from silhouette and shading",
    ))
}

#[cfg(test)]
#[path = "fields_tests.rs"]
mod tests;
