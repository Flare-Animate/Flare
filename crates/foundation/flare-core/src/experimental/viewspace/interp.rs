//! Layer 3.5: the interpolation engine. `evaluate` turns a view, an expression and a clothing
//! state into a 2D picture:
//! 1. select and weight the nearest samples (`neighbours`), honouring feature pins;
//! 2. blend colours and fields per mesh vertex (never in image space), weighting each sample by how
//!    well it saw that vertex, so a turned-away sample does not smear;
//! 3. add expression deltas;
//! 4. project with depth and occlusion priority and rasterise.
//!
//! Sparse data degrades instead of failing: unseen vertices are flat grey with zero confidence.

use super::ViewSpaceError;
use super::clothing::{ClothingState, usable_samples};
use super::confidence::{heat_colour, mean, vertex_confidence};
use super::expression::{ExpressionDelta, ExpressionParams, MAX_EXPRESSIONS, apply_expression};
use super::fields::{FieldBlend, OCCLUSION, THICKNESS, blend_fields, field_default};
use super::keyframe::{
    Bitmap, KeyframeId, KeyframeSample, MeshAlignment, Registered, ViewParams, register,
};
use super::mesh::BaseMesh;
use super::neighbours::{Neighbour, NeighbourConfig, select};
use super::project::{Camera, Raster, rasterise};
use std::collections::BTreeMap;

pub const MAX_SAMPLES: usize = 64;

/// Output canvas and quality settings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderSettings {
    pub width: u32,
    pub height: u32,
    /// Pixels per model unit.
    pub scale: f32,
    pub neighbours: NeighbourConfig,
    pub depth_blend: FieldBlend,
    /// Also render the confidence heatmap.
    pub heatmap: bool,
}

impl Default for RenderSettings {
    fn default() -> Self {
        Self {
            width: 256,
            height: 256,
            scale: 100.0,
            neighbours: NeighbourConfig::default(),
            depth_blend: FieldBlend::Linear,
            heatmap: false,
        }
    }
}

/// A sample that contributed, with its weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Contribution {
    pub id: KeyframeId,
    pub weight: f32,
    pub distance: f32,
}

/// What `evaluate` returns: a picture ready for display plus the data behind it.
#[derive(Debug, Clone)]
pub struct Renderable2D {
    pub image: Bitmap,
    /// Depth per pixel (larger is closer); `f32::NEG_INFINITY` where empty.
    pub depth: Vec<f32>,
    pub vertex_confidence: Vec<f32>,
    pub mean_confidence: f32,
    pub heatmap: Option<Bitmap>,
    pub contributions: Vec<Contribution>,
}

pub struct ViewSpaceInterpolator {
    mesh: BaseMesh,
    samples: Vec<Registered>,
    pub settings: RenderSettings,
    pins: BTreeMap<u16, KeyframeId>,
    expressions: BTreeMap<String, ExpressionDelta>,
}

impl ViewSpaceInterpolator {
    pub fn new(mesh: BaseMesh, settings: RenderSettings) -> Result<Self, ViewSpaceError> {
        mesh.validate()?;
        Ok(Self {
            mesh,
            samples: Vec::new(),
            settings,
            pins: BTreeMap::new(),
            expressions: BTreeMap::new(),
        })
    }

    pub fn mesh(&self) -> &BaseMesh {
        &self.mesh
    }

    pub fn samples(&self) -> impl Iterator<Item = &KeyframeSample> {
        self.samples.iter().map(|r| &r.sample)
    }

    /// Register and add a sample. All-or-nothing.
    pub fn add_sample(&mut self, sample: KeyframeSample) -> Result<KeyframeId, ViewSpaceError> {
        if self.samples.len() >= MAX_SAMPLES {
            return Err(ViewSpaceError::Limit {
                what: "keyframe samples",
                max: MAX_SAMPLES,
            });
        }
        if self.samples.iter().any(|r| r.sample.id == sample.id) {
            return Err(ViewSpaceError::Invalid(
                "a sample with this id exists".into(),
            ));
        }
        let id = sample.id;
        self.samples.push(register(&self.mesh, sample)?);
        Ok(id)
    }

    pub fn remove_sample(&mut self, id: KeyframeId) -> bool {
        let before = self.samples.len();
        self.samples.retain(|r| r.sample.id != id);
        self.pins.retain(|_, v| *v != id);
        self.samples.len() != before
    }

    pub fn add_expression(&mut self, delta: ExpressionDelta) -> Result<(), ViewSpaceError> {
        if delta.offsets.len() != self.mesh.vertices.len()
            || delta.offsets.iter().flatten().any(|v| !v.is_finite())
        {
            return Err(ViewSpaceError::Invalid(
                "expression delta does not fit the mesh".into(),
            ));
        }
        if !self.expressions.contains_key(&delta.name) && self.expressions.len() >= MAX_EXPRESSIONS
        {
            return Err(ViewSpaceError::Limit {
                what: "expressions",
                max: MAX_EXPRESSIONS,
            });
        }
        self.expressions.insert(delta.name.clone(), delta);
        Ok(())
    }

    /// Pin a mesh region to one keyframe: its vertices take that sample alone whenever it is among
    /// the neighbours (eyes from one keyframe, hair from another).
    pub fn pin_region(&mut self, region: u16, sample: KeyframeId) -> Result<(), ViewSpaceError> {
        if !self.mesh.region_dict.contains_key(&region) {
            return Err(ViewSpaceError::Invalid(format!("unknown region {region}")));
        }
        if !self.samples.iter().any(|r| r.sample.id == sample) {
            return Err(ViewSpaceError::Invalid("unknown keyframe".into()));
        }
        self.pins.insert(region, sample);
        Ok(())
    }

    pub fn unpin_region(&mut self, region: u16) {
        self.pins.remove(&region);
    }

    /// Blend one named per-vertex field over the neighbours; pinned regions take the pinned value.
    fn blend_field(
        &self,
        nb: &[(usize, Neighbour)],
        name: &str,
    ) -> Result<Vec<f32>, ViewSpaceError> {
        let n = self.mesh.vertices.len();
        let default: Vec<f32> = if name == "depth" {
            self.mesh.default_depth.clone()
        } else {
            vec![field_default(name); n]
        };
        let of = |idx: usize| -> &[f32] {
            let s = &self.samples[idx].sample;
            let f = if name == "depth" {
                s.depth_override.as_ref()
            } else {
                s.other_param_fields.get(name)
            };
            f.map_or(default.as_slice(), Vec::as_slice)
        };
        let fields: Vec<&[f32]> = nb.iter().map(|(i, _)| of(*i)).collect();
        let weights: Vec<f32> = nb.iter().map(|(_, n)| n.weight).collect();
        let mode = if name == "depth" {
            self.settings.depth_blend
        } else {
            FieldBlend::Linear
        };
        let mut out = blend_fields(&fields, &weights, mode)?;
        for (v, o) in out.iter_mut().enumerate() {
            if let Some(pos) = self.pinned_position(nb, v) {
                *o = fields[pos][v];
            }
        }
        Ok(out)
    }

    /// Position in `nb` of the sample vertex `v` is pinned to, if that sample is a neighbour.
    fn pinned_position(&self, nb: &[(usize, Neighbour)], v: usize) -> Option<usize> {
        let id = self.pins.get(self.mesh.region_ids.get(v)?)?;
        nb.iter()
            .position(|(i, _)| self.samples[*i].sample.id == *id)
    }

    /// Spec section 7: view, expression and clothing in; a 2D picture out.
    pub fn evaluate(
        &self,
        view: &ViewParams,
        expr: &ExpressionParams,
        clothing: &ClothingState,
    ) -> Result<Renderable2D, ViewSpaceError> {
        view.validate()?;
        let usable = usable_samples(&self.samples, clothing);
        let views: Vec<ViewParams> = usable
            .iter()
            .map(|&i| self.samples[i].sample.view)
            .collect();
        let nb: Vec<(usize, Neighbour)> = select(view, &views, &self.settings.neighbours)?
            .into_iter()
            .map(|n| (usable[n.index], n))
            .collect();
        if nb.is_empty() {
            return Err(ViewSpaceError::NoSamples);
        }
        let n = self.mesh.vertices.len();

        // Steps 2: colour per vertex, weighted by weight x visibility.
        let mut colours = Vec::with_capacity(n);
        let mut coverage = Vec::with_capacity(n);
        for v in 0..n {
            let pinned = self.pinned_position(&nb, v);
            let (mut acc, mut wsum, mut cov) = ([0.0f32; 4], 0.0f32, 0.0f32);
            for (j, (idx, nbr)) in nb.iter().enumerate() {
                let w = match pinned {
                    Some(p) => f32::from(p == j),
                    None => nbr.weight,
                };
                let r = &self.samples[*idx];
                let eff = w * r.visibility[v];
                cov += eff;
                wsum += eff;
                for (a, c) in acc.iter_mut().zip(r.vertex_color[v]) {
                    *a += c * eff;
                }
            }
            colours.push(if wsum > 1e-6 {
                acc.map(|a| a / wsum)
            } else {
                [0.5, 0.5, 0.5, 1.0]
            });
            coverage.push(cov.min(1.0));
        }
        let vertex_conf: Vec<f32> = coverage
            .iter()
            .map(|c| vertex_confidence(&nb.iter().map(|x| x.1).collect::<Vec<_>>(), *c))
            .collect();

        // Steps 3 and 4: positions, expression, depth and thickness fields.
        let mut pos = self.mesh.vertices.clone();
        apply_expression(&mut pos, &self.expressions, expr)?;
        let depth = self.blend_field(&nb, "depth")?;
        let thickness = self.blend_field(&nb, THICKNESS)?;
        let occlusion = self.blend_field(&nb, OCCLUSION)?;
        let (lo, hi) = self.mesh.bounds();
        let range = (hi[2] - lo[2]).max(1e-6);
        for (v, p) in pos.iter_mut().enumerate() {
            p[2] = (p[2] + (depth[v] - self.mesh.default_depth[v]) * range) * thickness[v];
        }

        let s = &self.settings;
        let align = MeshAlignment {
            scale: s.scale,
            offset: [0.0; 2],
            roll: 0.0,
        };
        let cam = Camera::new(*view, align, s.width, s.height);
        let screen: Vec<_> = pos.iter().map(|p| cam.project(*p)).collect();
        let mut raster = Raster::new(s.width, s.height)?;
        rasterise(&mut raster, &screen, &self.mesh.faces, &colours, &occlusion)?;
        let heatmap = if s.heatmap {
            let heat: Vec<[f32; 4]> = vertex_conf.iter().map(|c| heat_colour(*c)).collect();
            let mut hr = Raster::new(s.width, s.height)?;
            rasterise(&mut hr, &screen, &self.mesh.faces, &heat, &occlusion)?;
            Some(hr.to_bitmap()?)
        } else {
            None
        };
        Ok(Renderable2D {
            image: raster.to_bitmap()?,
            depth: raster.depth,
            mean_confidence: mean(&vertex_conf),
            vertex_confidence: vertex_conf,
            heatmap,
            contributions: nb
                .iter()
                .map(|(i, x)| Contribution {
                    id: self.samples[*i].sample.id,
                    weight: x.weight,
                    distance: x.distance,
                })
                .collect(),
        })
    }
}

#[cfg(test)]
#[path = "interp_tests.rs"]
mod tests;
