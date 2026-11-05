//! Section 4: expressions as a parameter space orthogonal to the view. A delta is the per-vertex
//! offset that turns the neutral mesh into the expression; at runtime the view is interpolated
//! first, then the weighted deltas are added.

use super::ViewSpaceError;
use super::mesh::{BaseMesh, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_EXPRESSIONS: usize = 64;

/// Offsets relative to the neutral mesh, per vertex, in model space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpressionDelta {
    pub name: String,
    pub offsets: Vec<Vec3>,
}

impl ExpressionDelta {
    /// Build a delta from a neutral and a posed mesh of the same topology.
    pub fn from_meshes(
        name: &str,
        neutral: &BaseMesh,
        posed: &BaseMesh,
    ) -> Result<Self, ViewSpaceError> {
        if name.trim().is_empty() {
            return Err(ViewSpaceError::Invalid("an expression needs a name".into()));
        }
        neutral.validate()?;
        posed.validate()?;
        if neutral.vertices.len() != posed.vertices.len() || neutral.faces != posed.faces {
            return Err(ViewSpaceError::Invalid(
                "expression mesh has a different topology".into(),
            ));
        }
        let offsets = neutral
            .vertices
            .iter()
            .zip(&posed.vertices)
            .map(|(a, b)| [b[0] - a[0], b[1] - a[1], b[2] - a[2]])
            .collect();
        Ok(Self {
            name: name.trim().to_string(),
            offsets,
        })
    }
}

/// Current expression weights by name. Weight range is 0..=2 (above 1 exaggerates).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ExpressionParams {
    pub weights: BTreeMap<String, f32>,
}

impl ExpressionParams {
    pub fn with(mut self, name: &str, weight: f32) -> Self {
        self.weights.insert(name.to_string(), weight);
        self
    }
}

/// Add the weighted deltas to `positions` (in place). Unknown names and bad weights are errors so
/// a typo is not silently a neutral face.
pub fn apply_expression(
    positions: &mut [Vec3],
    deltas: &BTreeMap<String, ExpressionDelta>,
    params: &ExpressionParams,
) -> Result<(), ViewSpaceError> {
    for (name, w) in &params.weights {
        if !w.is_finite() || !(0.0..=2.0).contains(w) {
            return Err(ViewSpaceError::Invalid(format!(
                "expression weight for {name} must be 0..=2"
            )));
        }
        let d = deltas
            .get(name)
            .ok_or_else(|| ViewSpaceError::Invalid(format!("unknown expression {name}")))?;
        if d.offsets.len() != positions.len() {
            return Err(ViewSpaceError::Invalid(format!(
                "expression {name} does not fit the mesh"
            )));
        }
        for (p, o) in positions.iter_mut().zip(&d.offsets) {
            for k in 0..3 {
                p[k] += o[k] * w;
            }
        }
    }
    Ok(())
}

/// Build per-view expression deltas from expression keyframes (neutral and target at the same
/// angle) so the delta can differ by view.
pub fn deltas_from_keyframes(
    _neutral: &super::keyframe::KeyframeSample,
    _target: &super::keyframe::KeyframeSample,
) -> Result<ExpressionDelta, ViewSpaceError> {
    // TODO(viewspace:expr-from-keyframes): 1. Both samples must share a view (within 2 degrees).
    // 2. For every feature mapped in both, take the mask centroid in each keyframe and unproject to
    // mesh space through the keyframe's camera onto the feature's anchor surface (nearest vertex of
    // the feature's region). 3. The per-feature offset is target minus neutral in mesh space.
    // 4. Spread feature offsets to all vertices by inverse-distance weighting over the mesh
    // (radius = 25% of head width), falling to 0 outside; keep it a per-vertex Vec<Vec3>.
    // 5. Drivers: expose the mouth-corner offset as a scalar for a bone or shape-key controller.
    Err(ViewSpaceError::NotImplemented(
        "expression deltas built from expression keyframes",
    ))
}

/// Several deltas for one expression at different views (3/4 angry from front-angry + 3/4 neutral).
pub fn blend_view_deltas(
    _name: &str,
    _per_view: &[(super::keyframe::ViewParams, ExpressionDelta)],
) -> Result<ExpressionDelta, ViewSpaceError> {
    // TODO(viewspace:expr-view): At evaluation, run `neighbours::select` over the views of the
    // deltas that share a name and blend their offsets with the weights (add after the view blend);
    // with one view only use that delta unchanged (the spec's approximation), and report lowered
    // confidence proportional to the angular distance from that view.
    Err(ViewSpaceError::NotImplemented(
        "view-dependent expression delta blend",
    ))
}

#[cfg(test)]
#[path = "expression_tests.rs"]
mod tests;
