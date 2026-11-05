//! Changing the rig of an armature: a joint added, a bone taken away, a joint's limits.
//!
//! A layer of an armature holds one keyframe per pose, all of one rig, so a change of rig is
//! made to every keyframe alike ([`Armature::edited`]); each keyframe keeps its own pose of
//! what stays, and nothing that stays moves.

use flare_core::{DocumentPoint, Transform2D};
use glamx::{DAffine2, DVec2};

use super::{Armature, ArmatureError, Joint, JointPose, MAX_JOINTS};

/// One change of rig, the same for every keyframe of the layer.
#[derive(Debug, Clone, PartialEq)]
pub enum RigEdit {
    /// A joint after the others. Its parent must be one of them; it starts unturned.
    Add(Joint),
    /// The bone into this joint goes: it and every joint below it lose their bones and each
    /// stands alone, a root of its own, drawn where it is in each pose. A limit is how far a
    /// joint may turn from its bone's parent, so it goes with the bone.
    Detach(usize),
    /// A joint may turn only within these limits (or freely); each pose is brought inside them.
    Limit(usize, Option<(f64, f64)>),
}

impl Armature {
    /// This keyframe after `edit`.
    pub fn edited(&self, edit: &RigEdit) -> Result<Armature, ArmatureError> {
        match edit {
            RigEdit::Add(joint) => {
                if self.joints.len() >= MAX_JOINTS {
                    return Err(ArmatureError::TooManyJoints(self.joints.len() + 1));
                }
                let mut joints = self.joints.to_vec();
                joints.push(joint.clone());
                let mut pose = self.pose.to_vec();
                pose.push(JointPose::default());
                Armature::new(joints)?.with_pose(pose)
            }
            RigEdit::Detach(index) => self.detached(*index),
            RigEdit::Limit(index, limit) => {
                let mut joints = self.joints.to_vec();
                let joint = joints
                    .get_mut(*index)
                    .ok_or(ArmatureError::ParentNotBefore(*index))?;
                *joint = joint.clone().with_limit(*limit)?;
                let mut pose = self.pose.to_vec();
                if let Some(posed) = pose.get_mut(*index) {
                    posed.turn = joints[*index].limited(posed.turn);
                }
                Armature::new(joints)?.with_pose(pose)
            }
        }
    }

    fn detached(&self, index: usize) -> Result<Armature, ArmatureError> {
        if index >= self.joints.len() {
            return Err(ArmatureError::ParentNotBefore(index));
        }
        // Parents come first, so one pass finds every joint below `index`.
        let mut below = vec![false; self.joints.len()];
        below[index] = true;
        for (i, joint) in self.joints.iter().enumerate().skip(index + 1) {
            below[i] = joint.parent.is_some_and(|p| below[p]);
        }
        let posed = self.posed();
        let mut joints = self.joints.to_vec();
        let mut pose = self.pose.to_vec();
        for (i, gone) in below.iter().enumerate() {
            if !*gone {
                continue;
            }
            joints[i].parent = None;
            joints[i].limit = None;
            // A root is drawn by its own turn and shift alone, so these keep it where it is.
            let (at, turn) = posed[i];
            pose[i] = JointPose {
                turn,
                shift: at - joints[i].at.as_vec(),
            };
        }
        Armature::new(joints)?.with_pose(pose)
    }

    /// The rest an instance needs to join this armature below `parent` and be drawn, in this
    /// pose, exactly where it is now: its transform `placed` and its joint's point `at`, both
    /// as drawn, taken back through the parent's matrix. `None` for a parent that is not here.
    pub fn rest_below(
        &self,
        parent: usize,
        placed: DAffine2,
        at: DVec2,
    ) -> Option<(Transform2D, DocumentPoint)> {
        let matrix = *self.joint_matrices().get(parent)?;
        let back = matrix.inverse();
        let rest = Transform2D::new(back * placed).ok()?;
        let point = back.transform_point2(at);
        Some((rest, DocumentPoint::new(point.x, point.y).ok()?))
    }
}
