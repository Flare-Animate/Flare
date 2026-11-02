//! Damped-least-squares inverse kinematics for planar armatures, after
//! OpenToonz's IK engine (`toonzlib/ikjacobian.cpp`, `ikengine.cpp`, which
//! follow S. Buss's Jacobian methods; BSD-3-Clause, © DWANGO and OpenToonz
//! contributors; see THIRD-PARTY-NOTICES.md).
//!
//! Every joint is a rotation about its pivot; several effectors pull at once:
//! the dragged point towards the pointer, and pinned joints towards where
//! they were pinned. Each step solves `Δθ = Jᵀ (J Jᵀ + λ² I)⁻¹ e`, limits the
//! largest turn per step (OpenToonz's `MaxAngleDLS`), and applies the turns
//! from the root outwards so each joint turns about its current pivot.
//! Joint limits are applied by the armature when a turn is made.

use crate::geom::Pt;

/// A point on a part of the armature pulled towards a target.
#[derive(Clone, Copy, Debug)]
pub struct Effector {
    /// The part (joint index) the point belongs to.
    pub owner: usize,
    /// The point in the part's own coordinates (the armature knows how to place it).
    pub local: Pt,
    pub target: Pt,
    /// Relative importance (pins pull harder than the pointer).
    pub weight: f32,
}

/// What the solver needs from an armature.
pub trait Kinematic {
    /// The joints that may turn, in root-to-leaf order.
    fn joints(&self) -> Vec<usize>;
    /// A joint's pivot (current pose).
    fn pivot(&self, j: usize) -> Pt;
    /// Does turning joint `j` move part `owner`?
    fn moves(&self, j: usize, owner: usize) -> bool;
    /// Where an effector's point is now.
    fn world(&self, owner: usize, local: Pt) -> Pt;
    /// Turn joint `j` by `d` radians (the armature applies its limits).
    fn turn(&mut self, j: usize, d: f32);
}

/// Solve a 2n×2n system `a x = b` (Gaussian elimination with partial pivoting).
fn solve(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for c in 0..n {
        let p = (c..n).max_by(|&i, &j| a[i][c].abs().partial_cmp(&a[j][c].abs()).unwrap())?;
        if a[p][c].abs() < 1e-12 {
            return None;
        }
        a.swap(c, p);
        b.swap(c, p);
        for r in c + 1..n {
            let f = a[r][c] / a[c][c];
            if f == 0.0 {
                continue;
            }
            let (top, rest) = a.split_at_mut(r);
            for (x, y) in rest[0][c..n].iter_mut().zip(&top[c][c..n]) {
                *x -= f * y;
            }
            b[r] -= f * b[c];
        }
    }
    let mut x = vec![0.0; n];
    for r in (0..n).rev() {
        let s: f64 = (r + 1..n).map(|k| a[r][k] * x[k]).sum();
        x[r] = (b[r] - s) / a[r][r];
    }
    Some(x)
}

/// Damping (in px: about how far a target may be out of reach before the
/// armature stops straining towards it).
pub const LAMBDA: f64 = 6.0;
/// Largest turn of any joint per step (radians).
pub const MAX_STEP: f32 = 0.2;

/// Pull the effectors towards their targets. Returns the remaining weighted error (px).
pub fn solve_dls(k: &mut impl Kinematic, eff: &[Effector], iterations: usize) -> f32 {
    let joints = k.joints();
    let mut err = f32::MAX;
    if joints.is_empty() || eff.is_empty() {
        return err;
    }
    let m = eff.len() * 2;
    for _ in 0..iterations {
        // error vector and Jacobian (columns: joints; rows: effector x, y)
        let pos: Vec<Pt> = eff.iter().map(|e| k.world(e.owner, e.local)).collect();
        let e: Vec<f64> = eff.iter().zip(&pos).flat_map(|(f, p)| [((f.target.x - p.x) * f.weight) as f64, ((f.target.y - p.y) * f.weight) as f64]).collect();
        err = eff.iter().zip(&pos).map(|(f, p)| p.dist(f.target) * f.weight).fold(0.0, f32::max);
        if err < 0.05 {
            break;
        }
        let pivots: Vec<Pt> = joints.iter().map(|&j| k.pivot(j)).collect();
        let jac: Vec<Vec<f64>> = (0..m)
            .map(|r| {
                let (f, p) = (&eff[r / 2], pos[r / 2]);
                joints
                    .iter()
                    .zip(&pivots)
                    .map(|(&j, q)| {
                        if !k.moves(j, f.owner) {
                            return 0.0;
                        }
                        // d(point)/dθ for a turn about q: perpendicular of (p − q)
                        let d = if r % 2 == 0 { -(p.y - q.y) } else { p.x - q.x };
                        (d * f.weight) as f64
                    })
                    .collect()
            })
            .collect();
        // J Jᵀ + λ² I
        let n = joints.len();
        let mut a = vec![vec![0.0; m]; m];
        for r in 0..m {
            for c in 0..m {
                a[r][c] = (0..n).map(|x| jac[r][x] * jac[c][x]).sum::<f64>() + if r == c { LAMBDA * LAMBDA } else { 0.0 };
            }
        }
        let Some(y) = solve(a, e) else { break };
        let mut dth: Vec<f32> = (0..n).map(|x| (0..m).map(|r| jac[r][x] * y[r]).sum::<f64>() as f32).collect();
        let max = dth.iter().fold(0.0f32, |a, d| a.max(d.abs()));
        if max > MAX_STEP {
            for d in &mut dth {
                *d *= MAX_STEP / max;
            }
        }
        if max < 1e-6 {
            break;
        }
        for (&j, d) in joints.iter().zip(dth) {
            if d != 0.0 {
                k.turn(j, d);
            }
        }
    }
    err
}

/// Turn of `d` radians from relative angle `rel` (degrees) allowed by limits
/// (min ≤ 0 ≤ max degrees from `rest`); joints that don't rotate return 0.
pub fn limited_turn(rotate: bool, limit: Option<(f32, f32)>, rest: f32, rel: f32, d: f32) -> f32 {
    if !rotate {
        return 0.0;
    }
    match limit {
        Some((lo, hi)) => {
            let off = crate::model::wrap_deg(rel - rest);
            let to = (off + d.to_degrees()).clamp(lo.min(0.0), hi.max(0.0));
            (to - off).to_radians()
        }
        None => d,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::pt;

    /// A planar chain of segments from the origin along +x.
    struct Arm {
        len: Vec<f32>,
        angle: Vec<f32>,
        limit: Vec<Option<(f32, f32)>>,
    }
    impl Arm {
        fn joint_pos(&self, j: usize) -> Pt {
            let (mut p, mut a) = (pt(0.0, 0.0), 0.0f32);
            for i in 0..j {
                a += self.angle[i];
                p = pt(p.x + a.cos() * self.len[i], p.y + a.sin() * self.len[i]);
            }
            p
        }
    }
    impl Kinematic for Arm {
        fn joints(&self) -> Vec<usize> {
            (0..self.len.len()).collect()
        }
        fn pivot(&self, j: usize) -> Pt {
            self.joint_pos(j)
        }
        fn moves(&self, j: usize, owner: usize) -> bool {
            j <= owner
        }
        fn world(&self, owner: usize, local: Pt) -> Pt {
            // local.x = distance along segment `owner`
            let a: f32 = self.angle[..=owner].iter().sum();
            let p = self.joint_pos(owner);
            pt(p.x + a.cos() * local.x, p.y + a.sin() * local.x)
        }
        fn turn(&mut self, j: usize, d: f32) {
            let rel = self.angle[j].to_degrees();
            self.angle[j] += limited_turn(true, self.limit[j], 0.0, rel, d);
        }
    }

    #[test]
    fn reaches_a_target_and_holds_a_pin() {
        let mut arm = Arm { len: vec![100.0; 3], angle: vec![0.0; 3], limit: vec![None; 3] };
        let err = solve_dls(&mut arm, &[Effector { owner: 2, local: pt(100.0, 0.0), target: pt(150.0, 150.0), weight: 1.0 }], 200);
        assert!(err < 1.0, "reached: {err}");
        // pin the elbow (end of segment 0) where it is, then pull the tip elsewhere
        let elbow = arm.world(0, pt(100.0, 0.0));
        let tip = pt(100.0, -120.0);
        solve_dls(&mut arm, &[Effector { owner: 2, local: pt(100.0, 0.0), target: tip, weight: 1.0 }, Effector { owner: 0, local: pt(100.0, 0.0), target: elbow, weight: 4.0 }], 300);
        assert!(arm.world(0, pt(100.0, 0.0)).dist(elbow) < 1.0, "pin held");
        assert!(arm.world(2, pt(100.0, 0.0)).dist(tip) < 2.0, "tip still reached: {:?}", arm.world(2, pt(100.0, 0.0)));
    }

    #[test]
    fn limits_hold_and_unreachable_targets_settle() {
        let mut arm = Arm { len: vec![100.0; 2], angle: vec![0.0; 2], limit: vec![None, Some((-20.0, 20.0))] };
        solve_dls(&mut arm, &[Effector { owner: 1, local: pt(100.0, 0.0), target: pt(0.0, 50.0), weight: 1.0 }], 300);
        assert!(arm.angle[1].to_degrees().abs() <= 20.001, "{}", arm.angle[1].to_degrees());
        // far out of reach: points at it, stays finite
        let mut arm = Arm { len: vec![100.0; 2], angle: vec![0.3, 0.3], limit: vec![None; 2] };
        solve_dls(&mut arm, &[Effector { owner: 1, local: pt(100.0, 0.0), target: pt(0.0, 1000.0), weight: 1.0 }], 300);
        let tip = arm.world(1, pt(100.0, 0.0));
        assert!(tip.x.is_finite() && (tip.x.atan2(tip.y)).abs() < 0.1, "points at the target: {tip:?}");
    }
}
