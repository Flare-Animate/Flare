//! ViewSpace Keys (VS-Keys): hybrid multi-view 2D interpolation. **Experimental and unstable.**
//!
//! Automated tweening between sparse keyframes of a character drawn from different angles. Every
//! keyframe is registered to a common low-poly base mesh and a set of semantic features; novel
//! angles are produced by interpolating in that mesh/feature space (which carries the
//! correspondence), not in image space. Artists control every stage (no black-box ML), it extends
//! or runs without a bone rig, and the output stays 2D/2.5D.
//!
//! The full specification, status of every part, stub ids and open questions are in
//! `docs/experimental/VIEWSPACE.md`; this module doc is the short map. Each stubbed function
//! returns `Err(ViewSpaceError::NotImplemented(..))` and carries a `// TODO(viewspace:<id>)`
//! comment holding the algorithm to implement. [`STATUS`] lists every id and a test keeps that list,
//! the source comments and the document in step.
//!
//! # Layers (spec section 3) and modules
//! 1. base mesh: [`mesh`], [`head`] (parametric head template)
//! 2. keyframe capture and alignment: [`keyframe`]
//! 3. semantic mapping, the Magic Brush: [`features`] (data), [`magic`] (selection algorithms)
//! 4. depth and parameter fields: [`fields`]
//! 5. interpolation engine: [`neighbours`], [`interp`], [`project`], [`confidence`], [`quality`]
//! 6. in-engine correction: [`edit`]
//!
//! Also [`expression`] (section 4), [`clothing`] (5) and [`export`] (12). The UI tab (6) is not
//! built: `// TODO(viewspace:ui-tab)`: a custom tab with an asset browser, viewport with overlay,
//! onion skin and heatmap, right-hand tool tabs and a bottom view-sphere navigator; every mapping
//! or correction action takes one or two inputs. It is a debt row in `docs/SURFACES.md`.
//!
//! Conventions: right-handed, +x to the viewer's right in the front view, +y up, +z towards the
//! camera at azimuth 0; angles in radians; triangles counter-clockwise from outside; "L" regions
//! are the character's left (+x).

pub mod clothing;
pub mod confidence;
pub mod edit;
pub mod export;
pub mod expression;
pub mod features;
pub mod fields;
pub mod head;
pub mod interp;
pub mod keyframe;
pub mod magic;
pub mod mesh;
pub mod neighbours;
pub mod project;
pub mod quality;

pub use clothing::ClothingState;
pub use expression::{ExpressionDelta, ExpressionParams};
pub use features::{FeatureId, FeaturePreset, Material, MaterialMap, RegionMask};
pub use head::{HeadParams, parametric_head};
pub use interp::{RenderSettings, Renderable2D, ViewSpaceInterpolator};
pub use keyframe::{Bitmap, KeyframeId, KeyframeSample, MeshAlignment, ViewParams};
pub use mesh::BaseMesh;
pub use neighbours::{NeighbourConfig, WeightFn};

/// Errors of this module. Library errors are typed; nothing here panics on user data.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ViewSpaceError {
    /// A stub: the named part is specified but not built yet.
    #[error("not implemented: {0}")]
    NotImplemented(&'static str),
    #[error("invalid mesh: {0}")]
    InvalidMesh(String),
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("limit exceeded: {what} (at most {max})")]
    Limit { what: &'static str, max: usize },
    /// No keyframe sample can serve the requested view and clothing state.
    #[error("no keyframe samples for this view")]
    NoSamples,
}

/// One row of the status table. `done` false means a stub with a `TODO(viewspace:<id>)` comment.
#[derive(Debug, Clone, Copy)]
pub struct Status {
    pub id: &'static str,
    pub done: bool,
    pub what: &'static str,
}

const fn done(id: &'static str, what: &'static str) -> Status {
    Status {
        id,
        done: true,
        what,
    }
}
const fn stub(id: &'static str, what: &'static str) -> Status {
    Status {
        id,
        done: false,
        what,
    }
}

/// Every id, by phase order. Mirrors the table in `docs/experimental/VIEWSPACE.md`.
pub const STATUS: &[Status] = &[
    // Phase 0
    done(
        "mesh-head",
        "parametric head template with regions and anchors",
    ),
    done(
        "mesh-validate",
        "BaseMesh validation, normals, bounds, serde",
    ),
    done(
        "view-params",
        "ViewParams, MeshAlignment, Bitmap, KeyframeSample, registration",
    ),
    done("depth-paint", "manual depth/field paint on the mesh"),
    done("project", "camera, projection, depth-buffered rasteriser"),
    stub("mesh-snap", "snap mesh outline to a silhouette"),
    stub("mesh-cage", "cage deformation / skinning of the mesh"),
    stub("mesh-modular", "attach body parts without losing mappings"),
    stub("align-auto", "automatic alignment from landmarks"),
    stub("align-import", "PSD / sprite sheet / Grease Pencil import"),
    // Phase 1
    done(
        "feature-presets",
        "presets, custom features, hotkeys, materials, masks, assignment",
    ),
    done("magic-wand", "magic wand (contiguous or not, edge bias)"),
    done("magic-morph", "grow, shrink, border, smooth, combine"),
    done("magic-lasso", "lasso polygon and brush stamp"),
    done(
        "neighbours",
        "view-sphere distance, k-nearest, IDW/slerp/Gaussian weights",
    ),
    done("depth-blend", "field blending, linear and gamma"),
    done(
        "interp-evaluate",
        "ViewSpaceInterpolator::evaluate, pins, fallback",
    ),
    done("confidence", "per-vertex confidence and heatmap"),
    done("confidence-suggest", "suggest next keyframe view"),
    stub("magic-stroke-aware", "selection snapping to line art"),
    stub("magic-semantic-grow", "grow limited to a material/feature"),
    stub(
        "magic-consistency",
        "multi-keyframe feature consistency warning",
    ),
    stub("depth-auto", "auto depth from silhouette and shading"),
    // Phase 2
    done(
        "edit-corrective",
        "corrective keyframes from a repainted frame",
    ),
    done(
        "expr-apply",
        "expression deltas from meshes and their application",
    ),
    done(
        "material-sharpen",
        "material-aware weight sharpening function",
    ),
    stub("edit-push", "push a correction back into shared maps"),
    stub(
        "expr-from-keyframes",
        "expression deltas from expression keyframes",
    ),
    stub("expr-view", "view-dependent expression deltas"),
    stub("material-blend", "material rules wired into evaluate"),
    // Phase 3
    done(
        "cloth-select",
        "clothing state sample selection with fallback",
    ),
    done(
        "export-sequence",
        "render a sequence of views (sprite sheet input)",
    ),
    stub("cloth-dup", "duplicate body mapping for a clothing state"),
    stub("cloth-occlusion", "clothing-aware occlusion suggestions"),
    stub("export-live2d", "Live2D-style parameter model"),
    stub("export-gp", "Grease Pencil shape-key export"),
    stub("export-runtime", "custom runtime format"),
    stub("export-psd", "layered PSD export"),
    stub("style", "outline and cel-shade style rules"),
    stub("interp-strokes", "stroke-layer keyframes"),
    stub("interp-gpu", "wgpu compute evaluation"),
    stub("lod", "LOD presets"),
    stub("rig-upstream", "deltas into a bone rig"),
    stub("rig-downstream", "bone rig drives the mesh"),
    stub("ui-tab", "the custom tab (UI)"),
];

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
