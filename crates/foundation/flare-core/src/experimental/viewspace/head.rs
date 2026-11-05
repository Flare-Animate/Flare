//! The built-in parametric head template (Phase 0): an ellipsoid with a narrowing jaw, a
//! forehead stretch and eye spacing, labelled with semantic regions and feature anchors.

use super::ViewSpaceError;
use super::mesh::{BaseMesh, Vec3, region};
use std::collections::BTreeMap;

/// Mesh Setup parameters. Defaults give a neutral stylised head about 2 units tall.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeadParams {
    pub width: f32,
    pub height: f32,
    pub depth: f32,
    /// Multiplier of the width at the chin (1 = no taper).
    pub jaw_width: f32,
    /// Multiplier for the part of the head above the centre.
    pub forehead_height: f32,
    /// Distance of each eye from the centre line, as a fraction of half the width.
    pub eye_spacing: f32,
    /// Latitude rings (>= 3) and segments around (>= 4); the vertex count is
    /// `(rings - 1) * segments + 2`.
    pub rings: u32,
    pub segments: u32,
}

impl Default for HeadParams {
    fn default() -> Self {
        Self {
            width: 1.6,
            height: 2.0,
            depth: 1.8,
            jaw_width: 0.7,
            forehead_height: 1.0,
            eye_spacing: 0.45,
            rings: 12,
            segments: 24,
        }
    }
}

const MAX_RINGS: u32 = 128;

impl HeadParams {
    fn check(&self) -> Result<(), ViewSpaceError> {
        let positive = [
            self.width,
            self.height,
            self.depth,
            self.jaw_width,
            self.forehead_height,
        ];
        if positive.iter().any(|v| !v.is_finite() || *v <= 0.0)
            || !self.eye_spacing.is_finite()
            || !(0.0..=1.0).contains(&self.eye_spacing)
        {
            return Err(ViewSpaceError::Invalid(
                "head parameters must be finite and positive".into(),
            ));
        }
        if self.rings < 3
            || self.segments < 4
            || self.rings > MAX_RINGS
            || self.segments > MAX_RINGS
        {
            return Err(ViewSpaceError::Invalid(
                "rings 3..=128 and segments 4..=128 required".into(),
            ));
        }
        Ok(())
    }
}

fn region_dict() -> BTreeMap<u16, String> {
    [
        (region::FOREHEAD, "forehead"),
        (region::L_CHEEK, "L cheek"),
        (region::R_CHEEK, "R cheek"),
        (region::NOSE_BRIDGE, "nose bridge"),
        (region::UPPER_LIP, "upper lip"),
        (region::LOWER_LIP, "lower lip"),
        (region::CHIN, "chin"),
        (region::L_EYE_SOCKET, "L eye socket"),
        (region::R_EYE_SOCKET, "R eye socket"),
        (region::HAIR_ROOT, "hair root zone"),
        (region::CLOTHING_ATTACH, "clothing attachment (neck)"),
    ]
    .into_iter()
    .map(|(k, v)| (k, v.to_string()))
    .collect()
}

/// `(nx, ny, nz)` are the unit-sphere coordinates before scaling.
fn region_at(nx: f32, ny: f32, nz: f32) -> u16 {
    let ax = nx.abs();
    let left = nx > 0.0;
    if ny > 0.6 || (nz < -0.2 && ny > 0.1) {
        region::HAIR_ROOT
    } else if ny < -0.85 {
        region::CLOTHING_ATTACH
    } else if nz > 0.3 && (ny - 0.15).abs() < 0.17 && (0.25..0.7).contains(&ax) {
        if left {
            region::L_EYE_SOCKET
        } else {
            region::R_EYE_SOCKET
        }
    } else if nz > 0.4 && ax < 0.2 && (-0.2..0.35).contains(&ny) {
        region::NOSE_BRIDGE
    } else if nz > 0.45 && ax < 0.35 && (-0.5..-0.4).contains(&ny) {
        region::LOWER_LIP
    } else if nz > 0.45 && ax < 0.35 && (-0.4..-0.3).contains(&ny) {
        region::UPPER_LIP
    } else if nz > 0.0 && ny < -0.5 {
        region::CHIN
    } else if nz > 0.15 && ax > 0.3 && (-0.45..0.0).contains(&ny) {
        if left {
            region::L_CHEEK
        } else {
            region::R_CHEEK
        }
    } else if nz > 0.2 && ny > 0.3 {
        region::FOREHEAD
    } else {
        region::NONE
    }
}

/// Build the template. Always passes [`BaseMesh::validate`] for valid parameters.
pub fn parametric_head(p: &HeadParams) -> Result<BaseMesh, ViewSpaceError> {
    p.check()?;
    let (rings, segs) = (p.rings as usize, p.segments as usize);
    let (a, b, c) = (p.width / 2.0, p.height / 2.0, p.depth / 2.0);
    let sphere = |theta: f32, phi: f32| {
        [
            theta.sin() * phi.sin(),
            theta.cos(),
            theta.sin() * phi.cos(),
        ]
    };
    let place = |n: Vec3| -> Vec3 {
        let mut x = n[0] * a;
        if n[1] < 0.0 {
            // The jaw narrows towards the chin.
            x *= 1.0 + (p.jaw_width - 1.0) * (-n[1]);
        }
        let y = if n[1] > 0.0 {
            n[1] * b * p.forehead_height
        } else {
            n[1] * b
        };
        [x, y, n[2] * c]
    };

    let mut units: Vec<Vec3> = vec![[0.0, 1.0, 0.0]];
    for r in 1..rings {
        let theta = std::f32::consts::PI * r as f32 / rings as f32;
        for s in 0..segs {
            let phi = std::f32::consts::TAU * s as f32 / segs as f32;
            units.push(sphere(theta, phi));
        }
    }
    units.push([0.0, -1.0, 0.0]);
    let bottom = (units.len() - 1) as u32;
    let ring_start = |r: usize| (1 + (r - 1) * segs) as u32;

    let mut faces = Vec::new();
    for s in 0..segs {
        let (s0, s1) = (s as u32, ((s + 1) % segs) as u32);
        // Top fan, outward CCW.
        faces.push([0, ring_start(1) + s0, ring_start(1) + s1]);
        for r in 1..rings - 1 {
            let (a0, a1) = (ring_start(r) + s0, ring_start(r) + s1);
            let (b0, b1) = (ring_start(r + 1) + s0, ring_start(r + 1) + s1);
            faces.push([a0, b0, a1]);
            faces.push([a1, b0, b1]);
        }
        faces.push([
            bottom,
            ring_start(rings - 1) + s1,
            ring_start(rings - 1) + s0,
        ]);
    }

    let vertices: Vec<Vec3> = units.iter().map(|u| place(*u)).collect();
    let region_ids = units.iter().map(|u| region_at(u[0], u[1], u[2])).collect();
    let uvs = units
        .iter()
        .map(|u| [0.5 + u[0] * 0.5, 0.5 + u[1] * 0.5])
        .collect();
    // Closeness: 1 at the front-most point, 0 at the back-most.
    let default_depth = units.iter().map(|u| 0.5 + 0.5 * u[2]).collect();

    let on_surface = |nx: f32, ny: f32| -> Vec3 {
        let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
        place([nx, ny, nz])
    };
    let eye_x = p.eye_spacing;
    let mut feature_anchors = BTreeMap::new();
    feature_anchors.insert("eye_l".to_string(), on_surface(eye_x, 0.15));
    feature_anchors.insert("eye_r".to_string(), on_surface(-eye_x, 0.15));
    feature_anchors.insert("nose_tip".to_string(), on_surface(0.0, -0.15));
    feature_anchors.insert("mouth_corner_l".to_string(), on_surface(0.3, -0.38));
    feature_anchors.insert("mouth_corner_r".to_string(), on_surface(-0.3, -0.38));
    feature_anchors.insert("ear_tip_l".to_string(), place([1.0, 0.1, 0.0]));
    feature_anchors.insert("ear_tip_r".to_string(), place([-1.0, 0.1, 0.0]));

    let control_points = vec![
        [-a, b * p.forehead_height, -c],
        [a, b * p.forehead_height, -c],
        [-a, b * p.forehead_height, c],
        [a, b * p.forehead_height, c],
        [-a * p.jaw_width, -b, -c],
        [a * p.jaw_width, -b, -c],
        [-a * p.jaw_width, -b, c],
        [a * p.jaw_width, -b, c],
    ];

    let mesh = BaseMesh {
        vertices,
        faces,
        uvs,
        region_ids,
        region_dict: region_dict(),
        default_depth,
        control_points,
        bones: None,
        feature_anchors,
    };
    mesh.validate()?;
    Ok(mesh)
}

#[cfg(test)]
#[path = "head_tests.rs"]
mod tests;
