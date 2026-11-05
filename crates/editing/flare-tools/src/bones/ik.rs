//! Inverse kinematics: turning the joints above one so it reaches a point.
//!
//! Cyclic coordinate descent: from the joint's parent up to its root, each joint in turn is
//! turned so the line from it to the dragged joint points at the target, kept within its
//! limits, and the pass is repeated until the joint is there or no pass helps. It never moves
//! a root, so the armature stays where it is and only bends, as Animate's does.

use flare_document::{Armature, JointPose};
use glamx::DVec2;

/// How many passes up the chain are made at most; each costs a pose of the armature per joint
/// of the chain, so this bounds a drag at 64 × 256 × 256 small steps.
pub const MAX_PASSES: usize = 64;

/// Close enough, in document units.
const REACHED: f64 = 1e-3;

/// `armature` posed so `joint` is as near `target` as its chain and limits let it be. A root, or
/// a joint that is not there, gives the armature back as it is.
pub fn reach(armature: &Armature, joint: usize, target: DVec2) -> Armature {
    let chain = armature.chain(joint);
    if chain.len() < 2 || !target.is_finite() {
        return armature.clone();
    }
    let mut pose: Vec<JointPose> = armature.pose().to_vec();
    let mut current = armature.clone();
    let mut last = f64::INFINITY;
    for _ in 0..MAX_PASSES {
        for &pivot in &chain[1..] {
            let posed = current.posed();
            let (end, at) = (posed[joint].0, posed[pivot].0);
            let (now, wanted) = (end - at, target - at);
            if now.length_squared() <= f64::EPSILON || wanted.length_squared() <= f64::EPSILON {
                continue;
            }
            let delta = now.angle_to(wanted);
            let Some(rig) = armature.joint(pivot) else {
                continue;
            };
            let turned = pose[pivot].turn + delta;
            pose[pivot].turn = match rig.limit() {
                Some(_) => rig.limited(turned),
                None => wrapped(turned),
            };
            if let Ok(next) = armature.with_pose(pose.clone()) {
                current = next;
            }
        }
        let miss = current.posed()[joint].0.distance(target);
        // Stop once there, or once a whole pass no longer brings it nearer.
        if miss <= REACHED || last - miss <= REACHED * 1e-3 {
            break;
        }
        last = miss;
    }
    current
}

/// `turn` brought within half a turn either way, so a joint that is dragged round and round
/// does not wind up.
fn wrapped(turn: f64) -> f64 {
    let tau = std::f64::consts::TAU;
    let mut t = turn % tau;
    if t > std::f64::consts::PI {
        t -= tau;
    } else if t <= -std::f64::consts::PI {
        t += tau;
    }
    t
}
