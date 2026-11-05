//! Section 5: clothing and variant states. A sample is shared (no state) or belongs to one named
//! state; evaluation uses the requested state's samples when it has any and falls back to the
//! shared ones, so a mesh with no clothing keyframes still renders.

use super::ViewSpaceError;
use super::keyframe::{KeyframeSample, Registered};

/// Requested clothing state; `None` is the shared/base look.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ClothingState(pub Option<String>);

impl ClothingState {
    pub fn named(name: &str) -> Self {
        Self(Some(name.to_string()))
    }
}

/// Indices of the samples usable for `state` (expression-tagged samples are never view samples).
pub fn usable_samples(samples: &[Registered], state: &ClothingState) -> Vec<usize> {
    let ok = |s: &KeyframeSample| s.expression_tag.is_none();
    if let Some(name) = &state.0 {
        let own: Vec<usize> = samples
            .iter()
            .enumerate()
            .filter(|(_, r)| ok(&r.sample) && r.sample.clothing_state.as_deref() == Some(name))
            .map(|(i, _)| i)
            .collect();
        if !own.is_empty() {
            return own;
        }
    }
    samples
        .iter()
        .enumerate()
        .filter(|(_, r)| ok(&r.sample) && r.sample.clothing_state.is_none())
        .map(|(i, _)| i)
        .collect()
}

/// Copy a body's feature mapping to a clothing state so only clothing regions are re-mapped.
pub fn duplicate_body_mapping(
    _from: &KeyframeSample,
    _state: &str,
) -> Result<KeyframeSample, ViewSpaceError> {
    // TODO(viewspace:cloth-dup): Clone the sample with a new id and clothing_state = Some(state),
    // keep feature_map entries for body features (eyes, nose, mouth, chin, ear, skin) and drop those
    // for clothing/hair features so only they are re-mapped; record which features are shared and
    // which unique so the UI can show shared vs unique per state; keep the image (the artist
    // replaces it with the clothed keyframe).
    Err(ViewSpaceError::NotImplemented(
        "duplicate body mapping for a clothing state",
    ))
}

/// Suggest occlusion priorities from semantic tags (hair in front of collar), overridable.
pub fn suggest_occlusion(
    _mesh: &super::mesh::BaseMesh,
    _sample: &KeyframeSample,
) -> Result<Vec<f32>, ViewSpaceError> {
    // TODO(viewspace:cloth-occlusion): Default priorities by feature: hair bangs +0.30, side locks
    // +0.20, accessories +0.15, collar +0.10, sleeve +0.05, skin 0. For each vertex take the highest
    // priority of the features whose masks cover its projected pixel in the sample. Return the
    // per-vertex field for `fields::OCCLUSION`; the artist's painted field always wins.
    Err(ViewSpaceError::NotImplemented(
        "clothing-aware occlusion suggestions",
    ))
}

#[cfg(test)]
#[path = "clothing_tests.rs"]
mod tests;
