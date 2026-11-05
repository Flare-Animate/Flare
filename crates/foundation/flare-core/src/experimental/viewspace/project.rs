//! Layer 3.5 step 4 (spec 8.3): view transform, projection and a depth-buffered triangle
//! rasteriser for per-vertex attributes. CPU reference implementation; the compute-shader version
//! is `interp-gpu`.

use super::ViewSpaceError;
use super::keyframe::{Bitmap, MeshAlignment, ViewParams, check_dims};
use super::mesh::Vec3;

/// A projected vertex: pixel position and depth (larger is closer to the camera).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenVertex {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

/// View rotation plus the 2D alignment onto a canvas.
#[derive(Debug, Clone, Copy)]
pub struct Camera {
    yaw: f32,
    pitch: f32,
    perspective: f32,
    size_mul: f32,
    align: MeshAlignment,
    center: [f32; 2],
}

impl Camera {
    pub fn new(view: ViewParams, align: MeshAlignment, width: u32, height: u32) -> Self {
        Self {
            // Turning the camera by +azimuth is turning the model by -azimuth.
            yaw: -view.azimuth,
            pitch: view.elevation.unwrap_or(0.0),
            perspective: view.perspective.unwrap_or(0.0).clamp(0.0, 0.95),
            size_mul: view.distance.map_or(1.0, |d| 1.0 / d.max(1e-3)),
            align,
            center: [width as f32 / 2.0, height as f32 / 2.0],
        }
    }

    /// Rotate a model-space vector into camera space (+z towards the camera).
    pub fn rotate(&self, v: Vec3) -> Vec3 {
        let (sy, cy) = self.yaw.sin_cos();
        let (x, z) = (v[0] * cy + v[2] * sy, -v[0] * sy + v[2] * cy);
        let (sp, cp) = self.pitch.sin_cos();
        [x, v[1] * cp - z * sp, v[1] * sp + z * cp]
    }

    pub fn project(&self, v: Vec3) -> ScreenVertex {
        let r = self.rotate(v);
        let (sr, cr) = self.align.roll.sin_cos();
        let (px, py) = (r[0] * cr - r[1] * sr, r[0] * sr + r[1] * cr);
        let f = 1.0 / (1.0 - self.perspective * r[2] * 0.5).max(0.1);
        let s = self.align.scale * self.size_mul * f;
        ScreenVertex {
            x: self.center[0] + self.align.offset[0] + px * s,
            y: self.center[1] + self.align.offset[1] - py * s,
            z: r[2],
        }
    }
}

/// Premultiplied RGBA float image plus depth.
#[derive(Debug, Clone)]
pub struct Raster {
    pub width: u32,
    pub height: u32,
    pub color: Vec<[f32; 4]>,
    /// `f32::NEG_INFINITY` where nothing was drawn.
    pub depth: Vec<f32>,
}

impl Raster {
    pub fn new(width: u32, height: u32) -> Result<Self, ViewSpaceError> {
        let n = check_dims(width, height)?;
        Ok(Self {
            width,
            height,
            color: vec![[0.0; 4]; n],
            depth: vec![f32::NEG_INFINITY; n],
        })
    }

    /// Straight-alpha RGBA8.
    pub fn to_bitmap(&self) -> Result<Bitmap, ViewSpaceError> {
        let mut data = Vec::with_capacity(self.color.len() * 4);
        for c in &self.color {
            let a = c[3].clamp(0.0, 1.0);
            let un = |v: f32| {
                if a > 0.0 {
                    (v / a).clamp(0.0, 1.0)
                } else {
                    0.0
                }
            };
            data.extend([un(c[0]), un(c[1]), un(c[2])].map(|v| (v * 255.0).round() as u8));
            data.push((a * 255.0).round() as u8);
        }
        Bitmap::new(self.width, self.height, data)
    }
}

/// Draw triangles with per-vertex attributes, depth-tested. Back faces are culled: a model
/// triangle that is counter-clockwise on the picture has a negative cross product in y-down pixel
/// coordinates, and that is the front.
/// `bias` is added to each vertex depth for the test only: the occlusion-priority hook (hair over
/// a collar). Invalid indices are skipped, not trusted.
pub fn rasterise(
    out: &mut Raster,
    screen: &[ScreenVertex],
    faces: &[[u32; 3]],
    attrs: &[[f32; 4]],
    bias: &[f32],
) -> Result<(), ViewSpaceError> {
    if attrs.len() != screen.len() || (!bias.is_empty() && bias.len() != screen.len()) {
        return Err(ViewSpaceError::Invalid(
            "attribute arrays do not match the vertices".into(),
        ));
    }
    let (w, h) = (out.width as i64, out.height as i64);
    let b = |i: usize| bias.get(i).copied().unwrap_or(0.0);
    for f in faces {
        let idx = [f[0] as usize, f[1] as usize, f[2] as usize];
        if idx.iter().any(|&i| i >= screen.len()) {
            continue;
        }
        let [a, bb, c] = [screen[idx[0]], screen[idx[1]], screen[idx[2]]];
        let area = (bb.x - a.x) * (c.y - a.y) - (bb.y - a.y) * (c.x - a.x);
        if !area.is_finite() || area >= -1e-9 {
            continue;
        }
        let x0 = a.x.min(bb.x).min(c.x).floor().max(0.0) as i64;
        let x1 = (a.x.max(bb.x).max(c.x).ceil() as i64).min(w - 1);
        let y0 = a.y.min(bb.y).min(c.y).floor().max(0.0) as i64;
        let y1 = (a.y.max(bb.y).max(c.y).ceil() as i64).min(h - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
                let w0 = ((bb.x - px) * (c.y - py) - (bb.y - py) * (c.x - px)) / area;
                let w1 = ((c.x - px) * (a.y - py) - (c.y - py) * (a.x - px)) / area;
                let w2 = 1.0 - w0 - w1;
                let eps = -1e-4;
                if w0 < eps || w1 < eps || w2 < eps {
                    continue;
                }
                let z = w0 * (a.z + b(idx[0])) + w1 * (bb.z + b(idx[1])) + w2 * (c.z + b(idx[2]));
                let i = (y * w + x) as usize;
                if z <= out.depth[i] {
                    continue;
                }
                out.depth[i] = z;
                let (a0, a1, a2) = (attrs[idx[0]], attrs[idx[1]], attrs[idx[2]]);
                for (k, o) in out.color[i].iter_mut().enumerate() {
                    *o = w0 * a0[k] + w1 * a1[k] + w2 * a2[k];
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "project_tests.rs"]
mod tests;
