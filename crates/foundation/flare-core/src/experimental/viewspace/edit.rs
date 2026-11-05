//! Layer 3.6: in-engine correction (Grease Pencil style). Loop: generate, inspect, correct,
//! regenerate. A correction local to one view becomes a corrective keyframe (denser sampling
//! there); pushing it back into the shared maps is a stub.

use super::ViewSpaceError;
use super::interp::ViewSpaceInterpolator;
use super::keyframe::{Bitmap, KeyframeId, KeyframeSample, MeshAlignment, ViewParams};

/// Turn a repainted generated frame into a corrective keyframe at `view`. The frame must be the
/// size the interpolator renders at; it is registered with the alignment `evaluate` used, so the
/// strokes land where they were drawn.
pub fn corrective_keyframe(
    interp: &ViewSpaceInterpolator,
    view: ViewParams,
    repainted: Bitmap,
) -> Result<KeyframeSample, ViewSpaceError> {
    let s = &interp.settings;
    if (repainted.width, repainted.height) != (s.width, s.height) {
        return Err(ViewSpaceError::Invalid(
            "the corrected frame must have the render size".into(),
        ));
    }
    let align = MeshAlignment {
        scale: s.scale,
        offset: [0.0; 2],
        roll: 0.0,
    };
    let mut sample = KeyframeSample::new(repainted, view, align);
    sample.is_corrective = true;
    Ok(sample)
}

/// Add a corrective keyframe to the interpolator; the next `evaluate` near that view uses it.
pub fn commit_corrective(
    interp: &mut ViewSpaceInterpolator,
    sample: KeyframeSample,
) -> Result<KeyframeId, ViewSpaceError> {
    if !sample.is_corrective {
        return Err(ViewSpaceError::Invalid("not a corrective keyframe".into()));
    }
    interp.add_sample(sample)
}

/// Propagate a local correction back into the shared feature maps, depth fields and mesh.
pub fn push_back_to_shared(
    _interp: &mut ViewSpaceInterpolator,
    _corrective: KeyframeId,
) -> Result<(), ViewSpaceError> {
    // TODO(viewspace:edit-push): 1. For the corrective sample, diff its image against the render
    // without it (evaluate with it removed) to find changed pixels. 2. Un-project changed pixels to
    // mesh vertices through the sample's camera. 3. Copy the corrected per-vertex colour into the
    // two nearest non-corrective samples (by view distance), weighted by their `neighbours` weights,
    // and write changed feature masks and painted depth/occlusion into their fields. 4. For control
    // point edits, apply them to the BaseMesh as one validated batch (all or nothing) and re-register
    // every sample. 5. Remove the corrective sample; keep an undo record of the replaced fields.
    Err(ViewSpaceError::NotImplemented(
        "push a correction back into the shared maps",
    ))
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
