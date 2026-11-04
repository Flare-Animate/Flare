use std::collections::BTreeMap;

use flare_core::InstanceId;
use flare_document::{Playback, SymbolKind};
use flare_tween::{Easing, Interpolation, Params, ease};

use crate::ik::{Joint, solve_ccd, solve_two_bone, tip};
use crate::model::{Clock, IkGoal, Interp, Pose, Rig, RigError};

/// A bone's pose in rig space: where it starts, which way it points, and the instance it moves.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoneTransform {
    pub origin: [f64; 2],
    pub angle: f64,
    pub length: f64,
    pub target: Option<InstanceId>,
}

fn shaping(interp: Interp, t: f64) -> f64 {
    let (i, e) = match interp {
        Interp::Constant => (Interpolation::Constant, Easing::Auto),
        Interp::Linear => (Interpolation::Linear, Easing::Auto),
        Interp::EaseIn => (Interpolation::Quad, Easing::In),
        Interp::EaseOut => (Interpolation::Quad, Easing::Out),
        Interp::EaseInOut => (Interpolation::Quad, Easing::InOut),
    };
    ease(i, e, t, &Params::default()).unwrap_or(t.clamp(0.0, 1.0))
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn blend(a: &Pose, b: &Pose, t: f64) -> Pose {
    let mut rotations = BTreeMap::new();
    for bone in a.rotations.keys().chain(b.rotations.keys()) {
        let (x, y) = (
            a.rotations.get(bone).copied().unwrap_or(0.0),
            b.rotations.get(bone).copied().unwrap_or(0.0),
        );
        rotations.insert(*bone, lerp(x, y, t));
    }
    let ik = a
        .ik
        .iter()
        .map(|g| match b.ik.iter().find(|h| h.tip == g.tip && h.chain == g.chain) {
            Some(h) => IkGoal {
                target: [lerp(g.target[0], h.target[0], t), lerp(g.target[1], h.target[1], t)],
                ..*g
            },
            None => *g,
        })
        .collect();
    Pose {
        rotations,
        ik,
        root: [lerp(a.root[0], b.root[0], t), lerp(a.root[1], b.root[1], t)],
    }
}

fn pose_at(rig: &Rig, frame: u32) -> Pose {
    let before = rig.keys.range(..=frame).next_back();
    let after = rig.keys.range(frame.saturating_add(1)..).next();
    match (before, after) {
        (Some((f0, k0)), Some((f1, k1))) => {
            let t = f64::from(frame - f0) / f64::from(f1 - f0);
            blend(&k0.pose, &k1.pose, shaping(k0.interp, t))
        }
        (Some((_, k)), None) | (None, Some((_, k))) => k.pose.clone(),
        (None, None) => Pose::default(),
    }
}

/// The rig's bones at `frame` of the parent timeline: the keyed pose, forward kinematics with
/// every rotation inside its bone's limit, then each IK goal solved. Deterministic, bounded and
/// finite for any valid rig.
///
/// # Errors
/// The rig fails [`Rig::validate`].
pub fn evaluate_rig(rig: &Rig, frame: u32) -> Result<Vec<BoneTransform>, RigError> {
    rig.validate()?;
    let pose = pose_at(rig, frame);
    let mut local: Vec<f64> = rig
        .bones
        .iter()
        .enumerate()
        .map(|(i, b)| {
            let r = pose.rotations.get(&i).copied().unwrap_or(0.0);
            b.rest_angle + b.limit.map_or(r, |l| l.clamp(r))
        })
        .collect();
    for goal in &pose.ik {
        solve_goal(rig, &mut local, &pose, goal);
    }
    Ok(forward(rig, &local, pose.root))
}

fn forward(rig: &Rig, local: &[f64], root: [f64; 2]) -> Vec<BoneTransform> {
    let mut out: Vec<BoneTransform> = Vec::with_capacity(rig.bones.len());
    for (i, bone) in rig.bones.iter().enumerate() {
        let (origin, base) = match bone.parent.and_then(|p| out.get(p)) {
            Some(p) => (
                [
                    p.origin[0] + p.length * p.angle.cos(),
                    p.origin[1] + p.length * p.angle.sin(),
                ],
                p.angle,
            ),
            None => (root, 0.0),
        };
        out.push(BoneTransform {
            origin,
            angle: base + local[i],
            length: bone.length,
            target: bone.target,
        });
    }
    out
}

fn solve_goal(rig: &Rig, local: &mut [f64], pose: &Pose, goal: &IkGoal) {
    // Chain bones from the base down to the tip; validation guarantees the parents exist.
    let mut ids = Vec::with_capacity(goal.chain);
    let mut at = Some(goal.tip);
    for _ in 0..goal.chain {
        let Some(i) = at else { return };
        ids.push(i);
        at = rig.bones[i].parent;
    }
    ids.reverse();
    let world = forward(rig, local, pose.root);
    let (base, base_angle) = match at.and_then(|p| world.get(p)) {
        Some(p) => (
            [p.origin[0] + p.length * p.angle.cos(), p.origin[1] + p.length * p.angle.sin()],
            p.angle,
        ),
        None => (pose.root, 0.0),
    };
    let mut joints: Vec<Joint> = ids
        .iter()
        .map(|&i| {
            let b = &rig.bones[i];
            Joint {
                length: b.length,
                angle: local[i],
                // The limit is on the pose rotation, so offset it by the rest angle.
                limit: b.limit.map(|l| (b.rest_angle + l.min, b.rest_angle + l.max)),
            }
        })
        .collect();
    if let [a, b] = joints[..] {
        let mut pair = [a, b];
        let bend = if b.angle < 0.0 { -1.0 } else { 1.0 };
        solve_two_bone(&mut pair, base, base_angle, goal.target, bend);
        joints.copy_from_slice(&pair);
    }
    solve_ccd(&mut joints, base, base_angle, goal.target, 32, 1e-6);
    let _ = tip(&joints, base, base_angle);
    for (&i, j) in ids.iter().zip(&joints) {
        local[i] = j.angle;
    }
}

/// The frame of the rig's symbol shown `elapsed` frames after the instance's span began. A free
/// (Movie Clip) rig keeps its own clock whatever the pose does; a locked (Graphic) rig follows
/// the instance's playback, as any Graphic does.
pub fn rig_symbol_frame(rig: &Rig, playback: Playback, elapsed: u64, length: u64) -> u32 {
    let kind = match rig.clock {
        Clock::Free => SymbolKind::MovieClip,
        Clock::Locked => SymbolKind::Graphic,
    };
    flare_eval::symbol_frame(kind, playback, elapsed, length)
}
