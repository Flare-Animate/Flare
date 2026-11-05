//! Layer 3.5 step 1 (spec 8.1): view-space distance, k-nearest selection and blend weights.
//!
//! Each keyframe sits at `(azimuth, elevation)` on a unit sphere; distance is the great-circle
//! angle. Weights always sum to 1 and an exact hit gets weight 1.

use super::ViewSpaceError;
use super::keyframe::ViewParams;

/// Distance (radians) below which a sample counts as an exact hit.
pub const EXACT_HIT: f32 = 1e-4;

/// Unit direction of a view: azimuth 0 looks at the front (+z), positive azimuth turns towards +x.
pub fn view_direction(v: &ViewParams) -> [f32; 3] {
    let el = v.elevation.unwrap_or(0.0);
    [
        el.cos() * v.azimuth.sin(),
        el.sin(),
        el.cos() * v.azimuth.cos(),
    ]
}

/// Great-circle angle in radians, 0..=pi. Uses atan2 of cross/dot, which stays accurate for tiny
/// and near-antipodal angles where `acos(dot)` does not.
pub fn angular_distance(a: &ViewParams, b: &ViewParams) -> f32 {
    let (u, w) = (view_direction(a), view_direction(b));
    let cross = [
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ];
    let c = (cross[0] * cross[0] + cross[1] * cross[1] + cross[2] * cross[2]).sqrt();
    let d = u[0] * w[0] + u[1] * w[1] + u[2] * w[2];
    c.atan2(d)
}

/// How blend weights fall off with distance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeightFn {
    /// `1 / d^power`; power 1 to 3 is typical.
    InverseDistance { power: f32 },
    /// For two neighbours: the spherical-linear weights `sin((1-t)d)/sin(d)` of the angle between
    /// them. With more neighbours it falls back to inverse distance (power 2).
    Slerp,
    /// Gaussian radial basis, `exp(-(d / sigma)^2)`; sigma in radians.
    Gaussian { sigma: f32 },
}

/// One selected sample: its index in the caller's list, distance and normalised weight.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Neighbour {
    pub index: usize,
    pub distance: f32,
    pub weight: f32,
}

/// Selection settings. `k` is clamped to 1..=8; `max_distance` drops far samples (None keeps all).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NeighbourConfig {
    pub k: usize,
    pub max_distance: Option<f32>,
    pub weight_fn: WeightFn,
}

impl Default for NeighbourConfig {
    fn default() -> Self {
        Self {
            k: 3,
            max_distance: None,
            weight_fn: WeightFn::InverseDistance { power: 2.0 },
        }
    }
}

/// Pick the k nearest of `views` to `target` and weight them. Empty result only when `views` is
/// empty or all are beyond `max_distance`; the caller decides what that means (no data).
pub fn select(
    target: &ViewParams,
    views: &[ViewParams],
    cfg: &NeighbourConfig,
) -> Result<Vec<Neighbour>, ViewSpaceError> {
    target.validate()?;
    let mut all: Vec<(usize, f32)> = Vec::with_capacity(views.len());
    for (i, v) in views.iter().enumerate() {
        v.validate()?;
        all.push((i, angular_distance(target, v)));
    }
    // total_cmp: NaN cannot occur (validated) but the sort must not panic regardless.
    all.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let limit = cfg.max_distance.unwrap_or(f32::INFINITY);
    all.retain(|(_, d)| *d <= limit);
    all.truncate(cfg.k.clamp(1, 8));
    if all.is_empty() {
        return Ok(Vec::new());
    }
    if let Some(&(index, distance)) = all.first().filter(|(_, d)| *d < EXACT_HIT) {
        return Ok(vec![Neighbour {
            index,
            distance,
            weight: 1.0,
        }]);
    }
    let raw = raw_weights(&all, &cfg.weight_fn, views);
    let sum: f32 = raw.iter().sum();
    let n = all.len() as f32;
    Ok(all
        .iter()
        .zip(raw)
        .map(|(&(index, distance), w)| Neighbour {
            index,
            distance,
            weight: if sum > 0.0 && sum.is_finite() {
                w / sum
            } else {
                1.0 / n
            },
        })
        .collect())
}

fn raw_weights(all: &[(usize, f32)], f: &WeightFn, views: &[ViewParams]) -> Vec<f32> {
    let idw = |p: f32| {
        all.iter()
            .map(|(_, d)| 1.0 / d.max(EXACT_HIT).powf(p.clamp(0.5, 8.0)))
            .collect()
    };
    match *f {
        WeightFn::InverseDistance { power } => idw(power),
        WeightFn::Gaussian { sigma } => {
            let s = sigma.max(1e-3);
            all.iter()
                .map(|(_, d)| (-(d / s) * (d / s)).exp())
                .collect()
        }
        WeightFn::Slerp => match all {
            [(i0, d0), (i1, d1)] => {
                // Angle between the two samples; the target is assumed near their arc.
                let span = match (views.get(*i0), views.get(*i1)) {
                    (Some(a), Some(b)) => angular_distance(a, b),
                    _ => d0 + d1,
                };
                let span = span.max(d0.max(*d1)).max(EXACT_HIT);
                let t = (d0 / (d0 + d1)).clamp(0.0, 1.0);
                if span.sin().abs() < 1e-6 {
                    vec![1.0 - t, t]
                } else {
                    vec![
                        ((1.0 - t) * span).sin() / span.sin(),
                        (t * span).sin() / span.sin(),
                    ]
                }
            }
            _ => idw(2.0),
        },
    }
}

#[cfg(test)]
#[path = "neighbours_tests.rs"]
mod tests;
