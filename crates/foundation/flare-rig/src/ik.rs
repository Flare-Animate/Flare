//! Inverse kinematics on a planar chain, in local angles with per-joint limits. Every loop is
//! bounded and every output is finite.

use crate::model::MAX_IK_ITERATIONS;

/// One joint of a chain: a bone of `length` turning by `angle` (radians, local to its parent)
/// within `limit`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Joint {
    pub length: f64,
    pub angle: f64,
    pub limit: Option<(f64, f64)>,
}

impl Joint {
    fn clamp(&self, a: f64) -> f64 {
        self.limit.map_or(a, |(lo, hi)| a.clamp(lo, hi))
    }
}

fn wrap(a: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let r = (a + std::f64::consts::PI).rem_euclid(tau) - std::f64::consts::PI;
    if r.is_finite() { r } else { 0.0 }
}

/// Tip position of the chain whose base is at `base` pointing along `base_angle`.
pub fn tip(chain: &[Joint], base: [f64; 2], base_angle: f64) -> [f64; 2] {
    let (mut p, mut a) = (base, base_angle);
    for j in chain {
        a += j.angle;
        p = [p[0] + j.length * a.cos(), p[1] + j.length * a.sin()];
    }
    p
}

/// Closed-form two-bone solve. `bend` picks the elbow side (+1 or -1). A target out of reach is
/// clamped to the straight line toward it, one too near to the base is clamped to the fully
/// folded chain. Limits are applied afterwards, so with limits the tip may fall short; the
/// caller can follow with [`solve_ccd`].
pub fn solve_two_bone(
    chain: &mut [Joint; 2],
    base: [f64; 2],
    base_angle: f64,
    target: [f64; 2],
    bend: f64,
) {
    let (l1, l2) = (chain[0].length, chain[1].length);
    let (dx, dy) = (target[0] - base[0], target[1] - base[1]);
    let lo = (l1 - l2).abs();
    let d = dx.hypot(dy).clamp(lo, l1 + l2);
    if d < 1e-12 || l1 < 1e-12 || l2 < 1e-12 {
        return;
    }
    let side = if bend < 0.0 { -1.0 } else { 1.0 };
    // Law of cosines; the clamps keep acos inside its domain.
    let a1 = ((d * d + l1 * l1 - l2 * l2) / (2.0 * d * l1))
        .clamp(-1.0, 1.0)
        .acos();
    let a2 = ((l1 * l1 + l2 * l2 - d * d) / (2.0 * l1 * l2))
        .clamp(-1.0, 1.0)
        .acos();
    let world1 = dy.atan2(dx) - side * a1;
    chain[0].angle = chain[0].clamp(wrap(world1 - base_angle));
    chain[1].angle = chain[1].clamp(wrap(side * (std::f64::consts::PI - a2)));
}

/// Cyclic coordinate descent, at most `iterations` sweeps (capped at `MAX_IK_ITERATIONS`),
/// stopping once the tip is within `tolerance` of the target. Joint limits hold after every
/// step, so a target the limits cannot reach leaves the chain at its closest limited pose.
pub fn solve_ccd(
    chain: &mut [Joint],
    base: [f64; 2],
    base_angle: f64,
    target: [f64; 2],
    iterations: u32,
    tolerance: f64,
) {
    for j in chain.iter_mut() {
        j.angle = j.clamp(j.angle);
    }
    for _ in 0..iterations.min(MAX_IK_ITERATIONS) {
        if dist(tip(chain, base, base_angle), target) <= tolerance {
            return;
        }
        for i in (0..chain.len()).rev() {
            let pivot = tip(&chain[..i], base, base_angle);
            let end = tip(chain, base, base_angle);
            let cur = (end[1] - pivot[1]).atan2(end[0] - pivot[0]);
            let want = (target[1] - pivot[1]).atan2(target[0] - pivot[0]);
            chain[i].angle = chain[i].clamp(wrap(chain[i].angle + want - cur));
        }
    }
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
