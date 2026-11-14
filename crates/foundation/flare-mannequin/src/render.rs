//! The figure as a shaded picture, as a 3D drawing figure is shown: the body's triangles filled
//! into samples, the nearest surface winning at each (a depth buffer), lit from the upper left
//! in front and darkened where the surface turns away, with guide lines round and along the
//! body (the middle of the front and back in orange), an outline round the figure and its
//! shadow on the floor. Each pixel also remembers the joint whose part it shows, so a press
//! picks what is seen.
//!
//! Small pictures are worked out at twice the size and brought down, so edges are smooth;
//! everything is bounded by [`MAX_PICTURE_SIDE`].

use flare_core::Rgba8;

use crate::figure::{Mannequin, on_floor};
use crate::raster::{Canvas, Corner, Lines};
use crate::skeleton::Joint;
use crate::view::Placement;

/// The widest and tallest picture made, in pixels.
pub const MAX_PICTURE_SIDE: u32 = 4000;
/// Most samples a picture is worked out from; above it the edges are left unsmoothed.
const MAX_SMOOTH_SAMPLES: usize = 6_000_000;

/// How the picture is to look.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    /// The joint whose parts are tinted, and the tint.
    pub picked: Option<(Joint, Rgba8)>,
    /// The guide lines round and along the body.
    pub guides: bool,
    /// The shadow on the floor, seen from above.
    pub shadow: bool,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            picked: None,
            guides: true,
            shadow: true,
        }
    }
}

/// A shaded picture of the figure on nothing: straight (not premultiplied) RGBA, rows from the
/// top, see-through wherever the figure and its shadow are not.
#[derive(Debug, Clone, PartialEq)]
pub struct Picture {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: Vec<u8>,
    /// One more than the index of the joint shown at each pixel; zero where there is none.
    pub(crate) joints: Vec<u8>,
}

impl Picture {
    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn into_rgba(self) -> Vec<u8> {
        self.rgba
    }

    /// The joint whose part shows at `p`, in pixels from the picture's top left.
    pub fn joint_at(&self, p: [f64; 2]) -> Option<Joint> {
        if !(p[0] >= 0.0 && p[1] >= 0.0) {
            return None;
        }
        let (x, y) = (p[0] as u32, p[1] as u32);
        if x >= self.width || y >= self.height {
            return None;
        }
        let id = *self.joints.get((y * self.width + x) as usize)?;
        Joint::ALL.get(usize::from(id).checked_sub(1)?).copied()
    }
}

/// The figure's colour: a pale matte grey.
const SKIN: [f32; 3] = [228.0, 228.0, 231.0];
/// The light, toward it, in the camera's space (x right, y up, z toward the viewer).
const LIGHT: [f32; 3] = [-0.42, 0.58, 0.70];
/// Halfway between the light and the viewer, for the shine.
const SHINE: [f32; 3] = [-0.22, 0.31, 0.93];

fn lit(n: [f32; 3]) -> [f32; 3] {
    let dot = |l: [f32; 3]| n[0] * l[0] + n[1] * l[1] + n[2] * l[2];
    // Half the light wraps round to the dark side, as a matte figure under a soft lamp.
    let wrap = dot(LIGHT) * 0.5 + 0.5;
    let light = 0.3 + 0.7 * wrap * wrap;
    // A surface turning away from the viewer darkens, which rounds the form at its edges.
    let facing = (n[2].max(0.0) / 0.45).min(1.0);
    let edge = 0.62 + 0.38 * facing * facing * (3.0 - 2.0 * facing);
    let shine = dot(SHINE).max(0.0).powi(20) * 22.0;
    SKIN.map(|c| (c * light * edge + shine).clamp(0.0, 255.0))
}

impl Mannequin {
    /// The figure shaded into a picture `width` by `height` pixels, placed as `placement` says
    /// in the picture's own pixels. None for a picture with no size or one over
    /// [`MAX_PICTURE_SIDE`] a side.
    pub fn render(
        &self,
        placement: Placement,
        width: u32,
        height: u32,
        look: Look,
    ) -> Option<Picture> {
        let sides = 1..=MAX_PICTURE_SIDE;
        if !(sides.contains(&width) && sides.contains(&height)) {
            return None;
        }
        let (width, height) = (width as usize, height as usize);
        let n = if width * height * 4 <= MAX_SMOOTH_SAMPLES {
            2
        } else {
            1
        };
        let scale = n as f64;
        let placement = Placement::new(
            [placement.center[0] * scale, placement.center[1] * scale],
            placement.height * scale,
        )?;
        let (mesh, skinned) = self.surface();
        let projector = self.projector(placement);
        let turn = self.view.rotation();
        let shadow = look.shadow && self.view.pitch() > 0.0;
        let projected: Vec<_> = skinned
            .points
            .iter()
            .map(|p| projector.project(*p))
            .collect();
        let fallen: Vec<[f64; 2]> = if shadow {
            skinned
                .points
                .iter()
                .map(|p| projector.project(on_floor(*p)).at)
                .collect()
        } else {
            Vec::new()
        };
        // Only the part of the picture the figure and its shadow are in is worked on.
        let (mut low, mut high) = ([f64::MAX; 2], [f64::MIN; 2]);
        for at in projected.iter().map(|p| &p.at).chain(&fallen) {
            for k in 0..2 {
                low[k] = low[k].min(at[k]);
                high[k] = high[k].max(at[k]);
            }
        }
        let span = |k: usize, most: usize| {
            let from = (low[k] / scale).floor().clamp(0.0, most as f64) as usize;
            let to = (high[k] / scale).ceil().clamp(0.0, most as f64) as usize;
            (from, to.saturating_sub(from))
        };
        let ((left, across), (top, down)) = (span(0, width), span(1, height));
        let mut canvas = Canvas::new(across * n, down * n);
        let origin = [(left * n) as f32, (top * n) as f32];
        let mut corners: Vec<Corner> = projected
            .iter()
            .zip(&skinned.normals)
            .zip(&mesh.vertices)
            .map(|((p, normal), vertex)| {
                let n = turn.apply(*normal);
                let [r, g, b] = lit([n.x as f32, n.y as f32, n.z as f32]);
                let [around, along, middle] = vertex.guide;
                Corner {
                    x: p.at[0] as f32 - origin[0],
                    y: p.at[1] as f32 - origin[1],
                    v: [p.depth as f32, r, g, b, around, along, middle],
                }
            })
            .collect();
        if shadow {
            for triangle in &mesh.triangles {
                let [a, b, c] = triangle.map(|k| {
                    let at = fallen[k as usize];
                    [at[0] as f32 - origin[0], at[1] as f32 - origin[1]]
                });
                canvas.shade(a, b, c);
            }
        }
        let tint = look
            .picked
            .map(|(joint, color)| (joint, [color.r, color.g, color.b].map(f32::from)));
        let lines = Lines { shown: look.guides };
        for (triangle, owner) in mesh.triangles.iter().zip(&mesh.owners) {
            let tinted = tint.and_then(|(joint, color)| (joint == *owner).then_some(color));
            let [a, b, c] = triangle.map(|k| k as usize);
            let joint = owner.index() as u8 + 1;
            canvas.fill(
                [&corners[a], &corners[b], &corners[c]],
                joint,
                tinted,
                lines,
            );
        }
        corners.clear();
        Some(canvas.picture(n, [left, top], width, height))
    }
}
