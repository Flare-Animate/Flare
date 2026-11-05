//! Rendering quality, style and rig-integration parts that are not built yet, plus the one
//! material-aware rule that is.

use super::ViewSpaceError;
use super::features::Material;
use super::interp::Renderable2D;
use super::mesh::BaseMesh;

/// Material-aware blend rule: sharpen weights as `w^p` and renormalise. Hair uses a low power
/// (soft blends), clothing a high one (a hard edge follows the nearest keyframe).
pub fn material_blend_power(m: Material) -> f32 {
    match m {
        Material::Hair => 0.75,
        Material::Skin | Material::Eyes | Material::Unset => 1.0,
        Material::Clothing | Material::Accessory => 3.0,
    }
}

/// Apply [`material_blend_power`] to normalised weights.
pub fn sharpen_weights(weights: &[f32], power: f32) -> Vec<f32> {
    let p = if power.is_finite() && power > 0.0 {
        power
    } else {
        1.0
    };
    let raw: Vec<f32> = weights.iter().map(|w| w.max(0.0).powf(p)).collect();
    let sum: f32 = raw.iter().sum();
    if sum > 0.0 && sum.is_finite() {
        raw.iter().map(|w| w / sum).collect()
    } else {
        weights.to_vec()
    }
}

/// Use the material map per vertex in the blend (interp calls `sharpen_weights` with the power of
/// the material under each vertex).
pub fn material_aware_weights(_interp_vertex_count: usize) -> Result<(), ViewSpaceError> {
    // TODO(viewspace:material-blend): In `ViewSpaceInterpolator::evaluate`, for each vertex read the
    // material under its projected pixel in each contributing sample's MaterialMap (majority vote
    // over the samples), then replace that vertex's weights by `sharpen_weights(weights,
    // material_blend_power(m))`. Hair also gets softer depth blending (FieldBlend::Linear) and
    // clothing the Gamma(2) mode; deformation uses the RIGIDITY field.
    Err(ViewSpaceError::NotImplemented(
        "material-aware blending inside evaluate",
    ))
}

/// Parameter-driven style: outline thickness scaling with foreshortening, cel-shade bands.
pub fn apply_style(_frame: &mut Renderable2D, _mesh: &BaseMesh) -> Result<(), ViewSpaceError> {
    // TODO(viewspace:style): Outline: width = base * (1 + k * (1 - cos(angle between the vertex
    // normal and the view))), drawn from the depth discontinuities of `Renderable2D::depth`
    // (a pixel is an edge when a neighbour's depth differs by more than a threshold). Cel bands:
    // quantise n.l (normal from the mesh and the softness field) into N bands. Parameters live in a
    // StyleParams struct; nothing is baked into keyframe textures.
    Err(ViewSpaceError::NotImplemented(
        "outline and cel-shade style rules",
    ))
}

/// Stroke-layer keyframes (vector sources) instead of rasters.
pub fn interpolate_strokes() -> Result<(), ViewSpaceError> {
    // TODO(viewspace:interp-strokes): Add `SourceLayer::Strokes(Vec<Stroke>)` with points registered
    // to mesh UV space and per-point width/softness; blend corresponding strokes (matched by feature
    // id and index) point by point with the neighbour weights, resampling to equal point counts.
    Err(ViewSpaceError::NotImplemented("stroke-layer keyframes"))
}

/// GPU compute path for 512-2048 px.
pub fn evaluate_gpu() -> Result<(), ViewSpaceError> {
    // TODO(viewspace:interp-gpu): Port `evaluate` to a wgpu compute shader: upload per-vertex colour
    // and field arrays of the k neighbours as storage buffers, blend in one dispatch per vertex,
    // rasterise with the existing render pipeline. The CPU path is the reference in tests. Lives in
    // a services-layer crate, since flare-core has no GPU dependency.
    Err(ViewSpaceError::NotImplemented("GPU compute evaluation"))
}

/// Low-resolution preview versus offline denser pass.
pub fn lod_settings(_level: u8) -> Result<(), ViewSpaceError> {
    // TODO(viewspace:lod): Preview: half-resolution canvas, k = 2, no heatmap, mesh decimated by
    // edge collapse to ~25% (keeping region borders); offline: full resolution, k = 4, mesh
    // subdivided once. Expose as RenderSettings presets and measure before choosing the numbers.
    Err(ViewSpaceError::NotImplemented("level-of-detail presets"))
}

/// Upstream rig provider: output deformed mesh as shape-key-like deltas into a 2D bone rig.
pub fn export_deltas_to_rig(_mesh: &BaseMesh) -> Result<(), ViewSpaceError> {
    // TODO(viewspace:rig-upstream): In a crate above flare-core that depends on flare-rig: for each
    // sample view compute per-vertex deltas against the bind pose and hand them to the rig as
    // pose-keyed shape keys (reading flare_rig types, not copying them); the rig adds secondary
    // motion and IK on top. Unidirectional first.
    Err(ViewSpaceError::NotImplemented("upstream rig provider"))
}

/// Downstream consumer: a bone rig drives the base mesh, the view system adds appearance.
pub fn drive_mesh_from_bones(_mesh: &BaseMesh) -> Result<BaseMesh, ViewSpaceError> {
    // TODO(viewspace:rig-downstream): Linear blend skinning of the bind pose by `mesh.bones` poses
    // (weights from the nearest bones, normalised), run before the view rotation in `evaluate`;
    // flare_rig supplies the evaluated pose.
    Err(ViewSpaceError::NotImplemented("downstream rig consumer"))
}

#[cfg(test)]
#[path = "quality_tests.rs"]
mod tests;
