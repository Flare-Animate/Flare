//! Forward kinematics: where every joint of a posed figure is, and which way it faces.

use crate::body::Build;
use crate::math::{Rot3, Vec3};
use crate::pose::Pose;
use crate::skeleton::{JOINT_COUNT, Joint, Side};

/// The posed figure: each joint's place and its frame in the figure's space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Posed {
    pub(crate) at: [Vec3; JOINT_COUNT],
    pub(crate) frame: [Rot3; JOINT_COUNT],
}

impl Posed {
    pub fn at(&self, joint: Joint) -> Vec3 {
        self.at[joint.index()]
    }

    pub fn frame(&self, joint: Joint) -> Rot3 {
        self.frame[joint.index()]
    }

    /// The frame `joint` turns in: its parent's, or the figure's own for the pelvis.
    pub fn parent_frame(&self, joint: Joint) -> Rot3 {
        joint.parent().map_or(Rot3::IDENTITY, |p| self.frame(p))
    }

    /// A point given in `joint`'s frame, in the figure's space.
    pub fn point(&self, joint: Joint, local: Vec3) -> Vec3 {
        self.at(joint) + self.frame(joint).apply(local)
    }
}

/// The rotation a joint's angles make, mirrored on the figure's right.
pub(crate) fn local_rotation(joint: Joint, pose: &Pose) -> Rot3 {
    let angles = pose.angles(joint);
    Rot3::from_angles(match joint.side() {
        Side::Right => angles.mirrored(),
        Side::Left | Side::Center => angles,
    })
}

/// Poses the figure: each joint is turned in its parent's frame, parents first.
pub(crate) fn pose_figure(build: &Build, pose: &Pose) -> Posed {
    let mut at = [Vec3::ZERO; JOINT_COUNT];
    let mut frame = [Rot3::IDENTITY; JOINT_COUNT];
    for joint in Joint::ALL {
        let local = local_rotation(joint, pose);
        let i = joint.index();
        match joint.parent() {
            None => {
                at[i] = build.offset(joint) + pose.lift();
                frame[i] = local;
            }
            Some(parent) => {
                let p = parent.index();
                at[i] = at[p] + frame[p].apply(build.offset(joint));
                frame[i] = frame[p] * local;
            }
        }
    }
    Posed { at, frame }
}
