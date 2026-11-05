//! Layer 3.1: the base mesh (proxy). Low-poly, shared topology, the common space every keyframe is
//! registered into.
//!
//! Conventions (fixed so keyframes and meshes agree): right-handed, +x is to the viewer's right in
//! the front view, +y is up, +z points out of the face towards the camera at azimuth 0.
//! "Left" features (`L_*` regions) are the *character's* left, which is +x in the front view.
//! Triangles are counter-clockwise when seen from outside.

use super::ViewSpaceError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub type Vec3 = [f32; 3];
pub type Vec2 = [f32; 2];

/// Hard bound on mesh size; spec asks for hundreds to low thousands.
pub const MAX_VERTICES: usize = 50_000;
pub const MAX_BONES: usize = 512;

/// Region ids of the built-in head template. 0 is "unlabelled".
pub mod region {
    pub const NONE: u16 = 0;
    pub const FOREHEAD: u16 = 1;
    pub const L_CHEEK: u16 = 2;
    pub const R_CHEEK: u16 = 3;
    pub const NOSE_BRIDGE: u16 = 4;
    pub const UPPER_LIP: u16 = 5;
    pub const LOWER_LIP: u16 = 6;
    pub const CHIN: u16 = 7;
    pub const L_EYE_SOCKET: u16 = 8;
    pub const R_EYE_SOCKET: u16 = 9;
    pub const HAIR_ROOT: u16 = 10;
    pub const CLOTHING_ATTACH: u16 = 11;
}

/// Optional bone hierarchy (can drive or be driven, see the rig integration modes).
/// Parents must come before children, so a pass in order is a valid evaluation order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoneHierarchy {
    pub names: Vec<String>,
    pub parents: Vec<Option<u32>>,
    /// Bind-pose head position of each bone.
    pub bind_positions: Vec<Vec3>,
}

impl BoneHierarchy {
    pub fn validate(&self) -> Result<(), ViewSpaceError> {
        let n = self.names.len();
        if n > MAX_BONES {
            return Err(ViewSpaceError::Limit {
                what: "bones",
                max: MAX_BONES,
            });
        }
        if self.parents.len() != n || self.bind_positions.len() != n {
            return Err(ViewSpaceError::InvalidMesh(
                "bone arrays differ in length".into(),
            ));
        }
        for (i, p) in self.parents.iter().enumerate() {
            if let Some(p) = p.filter(|p| (*p as usize) >= i) {
                return Err(ViewSpaceError::InvalidMesh(format!(
                    "bone {i} has parent {p}; parents must come first"
                )));
            }
        }
        if self.bind_positions.iter().flatten().any(|c| !c.is_finite()) {
            return Err(ViewSpaceError::InvalidMesh(
                "non-finite bone position".into(),
            ));
        }
        Ok(())
    }
}

/// The base mesh data contract (spec section 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BaseMesh {
    /// Bind pose.
    pub vertices: Vec<Vec3>,
    pub faces: Vec<[u32; 3]>,
    pub uvs: Vec<Vec2>,
    /// Semantic region per vertex.
    pub region_ids: Vec<u16>,
    pub region_dict: BTreeMap<u16, String>,
    /// Per-vertex closeness in [0, 1] (1 = closest to the camera at azimuth 0).
    pub default_depth: Vec<f32>,
    /// Cage / handles for cage deformation.
    pub control_points: Vec<Vec3>,
    pub bones: Option<BoneHierarchy>,
    pub feature_anchors: BTreeMap<String, Vec3>,
}

impl BaseMesh {
    /// Reject anything a later stage would index out of range or choke on. Run on every mesh that
    /// comes from outside (a file, a user edit) before use.
    pub fn validate(&self) -> Result<(), ViewSpaceError> {
        let n = self.vertices.len();
        if n == 0 || self.faces.is_empty() {
            return Err(ViewSpaceError::InvalidMesh("empty mesh".into()));
        }
        if n > MAX_VERTICES {
            return Err(ViewSpaceError::Limit {
                what: "vertices",
                max: MAX_VERTICES,
            });
        }
        for (what, len) in [
            ("uvs", self.uvs.len()),
            ("region_ids", self.region_ids.len()),
            ("default_depth", self.default_depth.len()),
        ] {
            if len != n {
                return Err(ViewSpaceError::InvalidMesh(format!(
                    "{what} has {len} entries for {n} vertices"
                )));
            }
        }
        if self.faces.iter().flatten().any(|&i| (i as usize) >= n) {
            return Err(ViewSpaceError::InvalidMesh(
                "face index out of range".into(),
            ));
        }
        let finite3 = |v: &Vec3| v.iter().all(|c| c.is_finite());
        if !self.vertices.iter().all(finite3)
            || !self.control_points.iter().all(finite3)
            || !self.feature_anchors.values().all(finite3)
            || self.uvs.iter().flatten().any(|c| !c.is_finite())
            || self.default_depth.iter().any(|d| !d.is_finite())
        {
            return Err(ViewSpaceError::InvalidMesh("non-finite number".into()));
        }
        if let Some(bad) = self
            .region_ids
            .iter()
            .find(|r| **r != region::NONE && !self.region_dict.contains_key(r))
        {
            return Err(ViewSpaceError::InvalidMesh(format!(
                "region {bad} is not in the dictionary"
            )));
        }
        if let Some(b) = &self.bones {
            b.validate()?;
        }
        Ok(())
    }

    /// Area-weighted vertex normals (unit length; zero for an isolated vertex).
    pub fn vertex_normals(&self) -> Vec<Vec3> {
        let mut acc = vec![[0.0f32; 3]; self.vertices.len()];
        for f in &self.faces {
            let (Some(a), Some(b), Some(c)) = (
                self.vertices.get(f[0] as usize),
                self.vertices.get(f[1] as usize),
                self.vertices.get(f[2] as usize),
            ) else {
                continue;
            };
            let n = cross(sub(*b, *a), sub(*c, *a));
            for &i in f {
                if let Some(slot) = acc.get_mut(i as usize) {
                    *slot = add(*slot, n);
                }
            }
        }
        acc.into_iter().map(normalize).collect()
    }

    /// Vertices of one region.
    pub fn region_vertices(&self, id: u16) -> Vec<usize> {
        self.region_ids
            .iter()
            .enumerate()
            .filter(|(_, r)| **r == id)
            .map(|(i, _)| i)
            .collect()
    }

    /// Axis-aligned bounds `(min, max)`.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let mut lo = [f32::MAX; 3];
        let mut hi = [f32::MIN; 3];
        for v in &self.vertices {
            for k in 0..3 {
                lo[k] = lo[k].min(v[k]);
                hi[k] = hi[k].max(v[k]);
            }
        }
        (lo, hi)
    }

    /// Project the mesh outline onto a keyframe silhouette after rough alignment.
    pub fn snap_to_silhouette(
        &mut self,
        _silhouette: &super::features::RegionMask,
    ) -> Result<(), ViewSpaceError> {
        // TODO(viewspace:mesh-snap): 1. Project the mesh with the keyframe's view and alignment and
        // find its outline (edges used by one front-facing and one back-facing triangle). 2. Trace
        // the silhouette mask boundary (marching squares) and resample both to N equal-arc-length
        // points. 3. Move each outline vertex to the nearest silhouette point along its outward 2D
        // normal, capped at a fraction of the mesh width. 4. Diffuse the offsets to interior
        // vertices with a few Laplacian smoothing passes weighted by 1/distance to the outline,
        // keeping z. 5. Re-validate; revert on failure (all-or-nothing, like a command batch).
        Err(ViewSpaceError::NotImplemented(
            "snap mesh outline to keyframe silhouette",
        ))
    }

    /// Cage deformation of the bind pose by moved control points.
    pub fn apply_cage(&self, _moved: &[Vec3]) -> Result<BaseMesh, ViewSpaceError> {
        // TODO(viewspace:mesh-cage): Mean-value or Green coordinates of each vertex w.r.t. the cage
        // (control_points as a closed triangulated cage), computed once and cached; new vertex =
        // sum(w_i * moved_i). Linear blend skinning is acceptable for v1: weights from distance to
        // the nearest bones, normalised. Reject a `moved` of the wrong length.
        Err(ViewSpaceError::NotImplemented(
            "cage deformation of the base mesh",
        ))
    }

    /// Attach a body part mesh to this one without losing region mappings.
    pub fn attach(&self, _part: &BaseMesh, _anchor: &str) -> Result<BaseMesh, ViewSpaceError> {
        // TODO(viewspace:mesh-modular): Concatenate vertices/faces (offset the part's indices),
        // offset the part so its `anchor` feature anchor meets this mesh's anchor of the same name,
        // keep both region dictionaries (renumber the part's ids that collide and record the
        // mapping so stored FeatureMaps of either mesh remain valid), weld the seam vertices
        // within epsilon, and merge bones by parenting the part's root to the anchor's bone.
        Err(ViewSpaceError::NotImplemented(
            "modular attach of body parts",
        ))
    }
}

pub(crate) fn sub(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
pub(crate) fn add(a: Vec3, b: Vec3) -> Vec3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
pub(crate) fn dot(a: Vec3, b: Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
pub(crate) fn cross(a: Vec3, b: Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
pub(crate) fn normalize(a: Vec3) -> Vec3 {
    let l = dot(a, a).sqrt();
    if l > 1e-12 {
        [a[0] / l, a[1] / l, a[2] / l]
    } else {
        [0.0; 3]
    }
}

#[cfg(test)]
#[path = "mesh_tests.rs"]
mod tests;
