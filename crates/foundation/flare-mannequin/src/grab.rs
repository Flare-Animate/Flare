//! Posing by dragging any part, as a wooden figure is posed by hand: the part taken hold of
//! follows the pointer and the body gives where a body would. A hand, a forearm, a foot or a shin
//! pulls the hand or foot with the arm or leg bending to follow; an upper arm, a thigh, the
//! waist, the chest, the neck or the head swings about its joint so its far end stays under the
//! pointer; the hips move the whole body. Hands and feet that are held stay where they were, so
//! hips dragged down over held feet make a crouch.

use crate::figure::Mannequin;
use crate::math::{Rot3, Vec3};
use crate::skeleton::Joint;
use crate::view::{Placement, Projector};

/// The hands and feet, which can be held in place.
pub(crate) const ENDS: [Joint; 4] = [
    Joint::LeftWrist,
    Joint::RightWrist,
    Joint::LeftAnkle,
    Joint::RightAnkle,
];

#[derive(Debug, Clone, Copy, PartialEq)]
enum How {
    /// The whole body moves.
    Move,
    /// A hand or a foot is pulled, its limb following.
    Reach(Joint),
    /// These joints, nearest the body first, swing so `handle` (a point in a joint's frame)
    /// follows.
    Aim {
        chain: [Option<Joint>; 2],
        handle: (Joint, Vec3),
    },
}

/// A hold on one part of the figure, from the press that took it to the release.
#[derive(Debug, Clone, Copy)]
pub struct Grab {
    joint: Joint,
    how: How,
    /// The view when the part was taken, so a body that moves is not chased by its own camera.
    projector: Projector,
    /// How near the viewer the point that follows the pointer was.
    depth: f64,
    /// From the press to that point, in the picture.
    offset: [f64; 2],
    /// Where that point and the figure were.
    from: Vec3,
    lift: Vec3,
    held: [Option<(Joint, Vec3)>; 4],
}

impl Grab {
    /// The joint whose part was taken hold of.
    pub fn joint(&self) -> Joint {
        self.joint
    }
}

fn how(joint: Joint, top: Vec3) -> How {
    use Joint::*;
    let aim = |chain: [Option<Joint>; 2], end: Joint| How::Aim {
        chain,
        handle: (end, Vec3::ZERO),
    };
    match joint {
        Pelvis => How::Move,
        Spine => aim([Some(Spine), None], Chest),
        Chest => aim([Some(Spine), Some(Chest)], Neck),
        Neck => aim([Some(Neck), None], Head),
        Head => How::Aim {
            chain: [Some(Neck), Some(Head)],
            handle: (Head, top),
        },
        LeftShoulder => aim([Some(LeftShoulder), None], LeftElbow),
        RightShoulder => aim([Some(RightShoulder), None], RightElbow),
        LeftHip => aim([Some(LeftHip), None], LeftKnee),
        RightHip => aim([Some(RightHip), None], RightKnee),
        LeftElbow | LeftWrist => How::Reach(LeftWrist),
        RightElbow | RightWrist => How::Reach(RightWrist),
        LeftKnee | LeftAnkle => How::Reach(LeftAnkle),
        RightKnee | RightAnkle => How::Reach(RightAnkle),
    }
}

impl Mannequin {
    /// Takes hold of `joint`'s part at `press` in the picture at `placement`. The hands and feet
    /// in `held` stay where they are while it is dragged, but for one on the limb taken hold of,
    /// which has to move.
    pub fn grab(
        &self,
        joint: Joint,
        placement: Placement,
        press: [f64; 2],
        held: &[Joint],
    ) -> Grab {
        let posed = self.posed();
        // The top of the head, which a drag of the head moves.
        let crown = Vec3::new(0.0, 0.115 * self.build().head, 0.0);
        let how = how(joint, crown);
        let from = match how {
            How::Move => posed.at(Joint::Pelvis),
            How::Reach(end) => posed.at(end),
            How::Aim { handle, .. } => posed.point(handle.0, handle.1),
        };
        let projector = self.projector(placement);
        let seen = projector.project(from);
        let mut kept = [None; 4];
        for (slot, end) in kept.iter_mut().zip(ENDS) {
            let on_limb = end == joint || end.reach_chain().is_some_and(|c| c.contains(&joint));
            if held.contains(&end) && !on_limb {
                *slot = Some((end, posed.at(end)));
            }
        }
        Grab {
            joint,
            how,
            projector,
            depth: seen.depth,
            offset: [seen.at[0] - press[0], seen.at[1] - press[1]],
            from,
            lift: self.pose.lift(),
            held: kept,
        }
    }

    /// Moves the part held by `grab` so it follows the pointer, now at `to` in the picture the
    /// part was taken in. Every joint stays inside its limits, so a part stops where a body does.
    pub fn drag(&mut self, grab: &Grab, to: [f64; 2]) {
        let target = [to[0] + grab.offset[0], to[1] + grab.offset[1]];
        if !(target[0].is_finite() && target[1].is_finite()) {
            return;
        }
        let goal = grab.projector.unproject(target, grab.depth);
        match grab.how {
            How::Move => self.pose.set_lift(grab.lift + (goal - grab.from)),
            How::Reach(end) => {
                self.reach_point(end, goal);
            }
            How::Aim { chain, handle } => {
                let toward = grab.projector.unproject(target, grab.depth + 1.0) - goal;
                self.aim(chain, handle, goal, toward);
            }
        }
        for (end, at) in grab.held.into_iter().flatten() {
            self.reach_point(end, at);
        }
    }

    /// Swings `chain` so `handle` lands on the line through `on` along `toward` (the line of
    /// sight under the pointer): at the point of it as far from the first joint as the handle
    /// is, the nearer to where the handle is now; past the line's reach, flat toward it.
    fn aim(&mut self, chain: [Option<Joint>; 2], handle: (Joint, Vec3), on: Vec3, toward: Vec3) {
        let (Some(root), Some(toward)) = (chain[0], toward.normalized()) else {
            return;
        };
        let posed = self.posed();
        let now = posed.point(handle.0, handle.1);
        let center = posed.at(root);
        let radius = (now - center).length();
        let b = (on - center).dot(toward);
        let c = (on - center).dot(on - center) - radius * radius;
        let spread = b * b - c;
        let t = if spread <= 0.0 {
            -b
        } else {
            let (front, back) = (-b + spread.sqrt(), -b - spread.sqrt());
            let was = (now - on).dot(toward);
            // On a tie (a limb lying flat in the picture) it comes toward the viewer.
            if (front - was).abs() <= (back - was).abs() + 1e-9 {
                front
            } else {
                back
            }
        };
        let goal = on + toward.scale(t);
        let joints: Vec<Joint> = chain.into_iter().flatten().collect();
        // Once sharing the turn between the joints, then once more for what a limit refused.
        for pass in 0..2 {
            for (i, joint) in joints.iter().enumerate() {
                let posed = self.posed();
                let at = posed.at(*joint);
                let here = posed.point(handle.0, handle.1) - at;
                let share = if pass == 0 {
                    1.0 / (joints.len() - i) as f64
                } else {
                    1.0
                };
                if let (Some(here), Some(there)) = (here.normalized(), (goal - at).normalized()) {
                    self.turn_frame(*joint, Rot3::toward(here, there, share));
                }
            }
        }
    }
}
