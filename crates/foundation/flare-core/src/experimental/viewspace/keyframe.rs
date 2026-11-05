//! Layer 3.2: keyframe samples. A keyframe is an image (or strokes) plus the exact view it shows
//! and how the base mesh is aligned to it, so it becomes an anchored sample in view-space and is
//! registered to the mesh (per-vertex here), never a free-floating picture.

use super::ViewSpaceError;
use super::features::{FeatureId, MaterialMap, RegionMask};
use super::mesh::{BaseMesh, dot};
use super::project::{Camera, ScreenVertex};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use uuid::Uuid;

/// Largest image side and pixel count accepted anywhere in this module.
pub const MAX_DIM: u32 = 8192;
pub const MAX_PIXELS: usize = 32 * 1024 * 1024;

/// Identity of a keyframe sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct KeyframeId(Uuid);

impl KeyframeId {
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}
impl Default for KeyframeId {
    fn default() -> Self {
        Self::new()
    }
}

/// Where in view-space a sample lives. Angles in radians. Azimuth 0 is the front view and turns
/// positive towards the character's left (+x); elevation positive looks down on the head.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct ViewParams {
    pub azimuth: f32,
    pub elevation: Option<f32>,
    /// Camera distance in model units; scales the apparent size by 1/distance. None = 1.
    pub distance: Option<f32>,
    /// Perspective strength 0 (orthographic) to below 1.
    pub perspective: Option<f32>,
}

impl ViewParams {
    pub fn from_degrees(azimuth: f32, elevation: Option<f32>) -> Self {
        Self {
            azimuth: azimuth.to_radians(),
            elevation: elevation.map(f32::to_radians),
            distance: None,
            perspective: None,
        }
    }

    pub fn validate(&self) -> Result<(), ViewSpaceError> {
        let bad = |o: Option<f32>| o.is_some_and(|v| !v.is_finite());
        if !self.azimuth.is_finite()
            || bad(self.elevation)
            || bad(self.distance)
            || bad(self.perspective)
        {
            return Err(ViewSpaceError::Invalid(
                "view parameters must be finite".into(),
            ));
        }
        if self
            .elevation
            .is_some_and(|e| e.abs() > std::f32::consts::FRAC_PI_2 + 1e-4)
        {
            return Err(ViewSpaceError::Invalid(
                "elevation must be within +-90 degrees".into(),
            ));
        }
        if self.distance.is_some_and(|d| d <= 0.0) {
            return Err(ViewSpaceError::Invalid("distance must be positive".into()));
        }
        if self.perspective.is_some_and(|p| !(0.0..1.0).contains(&p)) {
            return Err(ViewSpaceError::Invalid(
                "perspective must be in 0..1".into(),
            ));
        }
        Ok(())
    }
}

/// How the mesh sits on the keyframe's canvas: scale in pixels per model unit, a pixel offset
/// from the canvas centre, and a roll in radians.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct MeshAlignment {
    pub scale: f32,
    pub offset: [f32; 2],
    pub roll: f32,
}

impl Default for MeshAlignment {
    fn default() -> Self {
        Self {
            scale: 100.0,
            offset: [0.0; 2],
            roll: 0.0,
        }
    }
}

impl MeshAlignment {
    pub fn validate(&self) -> Result<(), ViewSpaceError> {
        if !self.scale.is_finite()
            || self.scale <= 0.0
            || self.offset.iter().any(|v| !v.is_finite())
            || !self.roll.is_finite()
        {
            return Err(ViewSpaceError::Invalid(
                "alignment must be finite with a positive scale".into(),
            ));
        }
        Ok(())
    }

    /// Rough automatic alignment from detected landmarks.
    pub fn auto_align(_mesh: &BaseMesh, _image: &Bitmap) -> Result<Self, ViewSpaceError> {
        // TODO(viewspace:align-auto): Needs a landmark source (eyes, nose, mouth, silhouette) from a
        // CV helper or model; none ships yet. Algorithm: take detected points P_i for the mesh's
        // feature anchors A_i; solve the 2D similarity (scale, roll, translation) minimising
        // sum |s R A_i.xy + t - P_i|^2 in closed form (Umeyama), after the view rotation of the
        // keyframe; reject when fewer than 3 landmarks or the residual exceeds 10% of the head width.
        Err(ViewSpaceError::NotImplemented(
            "automatic alignment by landmark detection",
        ))
    }
}

/// RGBA8 image, row-major, straight (non-premultiplied) alpha.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Bitmap {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

impl Bitmap {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, ViewSpaceError> {
        let px = check_dims(width, height)?;
        if data.len() != px * 4 {
            return Err(ViewSpaceError::Invalid(
                "bitmap data length does not match its size".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            data,
        })
    }

    pub fn filled(width: u32, height: u32, rgba: [u8; 4]) -> Result<Self, ViewSpaceError> {
        let px = check_dims(width, height)?;
        Ok(Self {
            width,
            height,
            data: rgba.repeat(px),
        })
    }

    /// Premultiplied float colour of one pixel; None outside the image.
    pub fn premul(&self, x: i64, y: i64) -> Option<[f32; 4]> {
        if x < 0 || y < 0 || x >= i64::from(self.width) || y >= i64::from(self.height) {
            return None;
        }
        let i = (y as usize * self.width as usize + x as usize) * 4;
        let p = self.data.get(i..i + 4)?;
        let a = f32::from(p[3]) / 255.0;
        Some([
            f32::from(p[0]) / 255.0 * a,
            f32::from(p[1]) / 255.0 * a,
            f32::from(p[2]) / 255.0 * a,
            a,
        ])
    }

    /// Bilinear premultiplied sample at a pixel position (pixel centres at +0.5). Outside pixels
    /// count as transparent only when every tap is outside.
    pub fn sample(&self, x: f32, y: f32) -> Option<[f32; 4]> {
        let (fx, fy) = (x - 0.5, y - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let taps = [
            (0, 0, (1.0 - tx) * (1.0 - ty)),
            (1, 0, tx * (1.0 - ty)),
            (0, 1, (1.0 - tx) * ty),
            (1, 1, tx * ty),
        ];
        let mut acc = [0.0f32; 4];
        let mut any = false;
        for (dx, dy, w) in taps {
            if let Some(c) = self.premul(x0 + dx, y0 + dy) {
                any = true;
                for k in 0..4 {
                    acc[k] += c[k] * w;
                }
            }
        }
        any.then_some(acc)
    }
}

pub(crate) fn check_dims(w: u32, h: u32) -> Result<usize, ViewSpaceError> {
    if w == 0 || h == 0 || w > MAX_DIM || h > MAX_DIM {
        return Err(ViewSpaceError::Limit {
            what: "image side",
            max: MAX_DIM as usize,
        });
    }
    let px = w as usize * h as usize;
    if px > MAX_PIXELS {
        return Err(ViewSpaceError::Limit {
            what: "image pixels",
            max: MAX_PIXELS,
        });
    }
    Ok(px)
}

/// Where a keyframe's pixels come from. Stroke layers are a stub (`interp-strokes`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SourceLayer {
    Raster(Bitmap),
}

/// The keyframe data contract (spec section 7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KeyframeSample {
    pub id: KeyframeId,
    pub source: SourceLayer,
    pub view: ViewParams,
    pub alignment: MeshAlignment,
    pub feature_map: BTreeMap<FeatureId, RegionMask>,
    pub material_ids: Option<MaterialMap>,
    /// Per-vertex closeness replacing the mesh default, when the artist painted one.
    pub depth_override: Option<Vec<f32>>,
    /// Named per-vertex fields: `thickness`, `occlusion`, `softness`, `rigidity` (see `fields`).
    pub other_param_fields: BTreeMap<String, Vec<f32>>,
    pub expression_tag: Option<String>,
    pub clothing_state: Option<String>,
    pub is_corrective: bool,
}

impl KeyframeSample {
    pub fn new(image: Bitmap, view: ViewParams, alignment: MeshAlignment) -> Self {
        Self {
            id: KeyframeId::new(),
            source: SourceLayer::Raster(image),
            view,
            alignment,
            feature_map: BTreeMap::new(),
            material_ids: None,
            depth_override: None,
            other_param_fields: BTreeMap::new(),
            expression_tag: None,
            clothing_state: None,
            is_corrective: false,
        }
    }

    pub fn image(&self) -> &Bitmap {
        let SourceLayer::Raster(b) = &self.source;
        b
    }

    /// Check the sample against the mesh it will be registered to.
    pub fn validate(&self, mesh: &BaseMesh) -> Result<(), ViewSpaceError> {
        self.view.validate()?;
        self.alignment.validate()?;
        let img = self.image();
        if img.data.len() != check_dims(img.width, img.height)? * 4 {
            return Err(ViewSpaceError::Invalid(
                "bitmap data length does not match its size".into(),
            ));
        }
        let n = mesh.vertices.len();
        let field_ok = |f: &[f32]| f.len() == n && f.iter().all(|v| v.is_finite());
        if self.depth_override.as_deref().is_some_and(|f| !field_ok(f))
            || self.other_param_fields.values().any(|f| !field_ok(f))
        {
            return Err(ViewSpaceError::Invalid(
                "a per-vertex field has the wrong length or a non-finite value".into(),
            ));
        }
        Ok(())
    }

    /// Camera that reproduces this keyframe's projection of the mesh.
    pub fn camera(&self) -> Camera {
        let img = self.image();
        Camera::new(self.view, self.alignment, img.width, img.height)
    }
}

/// Import PNG/PSD layers, sprite sheets or Grease Pencil objects as keyframe images.
pub fn import_layers(_bytes: &[u8]) -> Result<Vec<Bitmap>, ViewSpaceError> {
    // TODO(viewspace:align-import): Decoding belongs in the io layer (flare-import already reads PNG
    // and image sequences; PSD needs a reader). Contract: return one Bitmap per layer in
    // bottom-to-top order, each at most MAX_DIM per side and MAX_PIXELS in total, reject everything
    // else with `Limit`; a sprite sheet is cut by its atlas rectangles into one Bitmap per cell.
    Err(ViewSpaceError::NotImplemented(
        "import of PSD layers, sprite sheets and Grease Pencil objects",
    ))
}

/// A sample registered to the mesh: what each vertex looks like in this keyframe.
#[derive(Debug, Clone)]
pub struct Registered {
    pub sample: KeyframeSample,
    /// Premultiplied colour per vertex; transparent where the keyframe has nothing there.
    pub vertex_color: Vec<[f32; 4]>,
    /// 0 (back-facing or off-image) to 1 (seen head on).
    pub visibility: Vec<f32>,
}

/// Register a sample: project the bind-pose mesh with the keyframe's own view and read the image
/// at each vertex. A vertex facing away from that view, or off the image, has visibility 0
/// (this is also the occlusion test for convex-ish parts; see `interp-occlusion` in the doc).
pub fn register(mesh: &BaseMesh, sample: KeyframeSample) -> Result<Registered, ViewSpaceError> {
    mesh.validate()?;
    sample.validate(mesh)?;
    let cam = sample.camera();
    let normals = mesh.vertex_normals();
    let img = sample.image();
    let mut vertex_color = Vec::with_capacity(mesh.vertices.len());
    let mut visibility = Vec::with_capacity(mesh.vertices.len());
    for (v, n) in mesh.vertices.iter().zip(&normals) {
        let ScreenVertex { x, y, .. } = cam.project(*v);
        let facing = dot(cam.rotate(*n), [0.0, 0.0, 1.0]);
        match img.sample(x, y) {
            Some(c) if facing > 0.0 && c[3] > 0.0 => {
                vertex_color.push(c);
                visibility.push((facing / 0.3).min(1.0));
            }
            _ => {
                vertex_color.push([0.0; 4]);
                visibility.push(0.0);
            }
        }
    }
    Ok(Registered {
        sample,
        vertex_color,
        visibility,
    })
}

#[cfg(test)]
#[path = "keyframe_tests.rs"]
mod tests;
