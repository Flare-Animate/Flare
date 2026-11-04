//! Bones: an armature of joints, each carrying one symbol instance, posed by turning the joints.
//!
//! This is Animate's armature (the Bone tool's work). A bone runs from a joint to a child joint;
//! every joint holds the instance the bone was dragged from or to, and turning a joint turns the
//! bones below it and everything they carry. The joints, where they are and what they hold are
//! the *rig*; how far each one is turned is the *pose*. A layer of an armature holds one
//! keyframe per pose, all of one rig, and a motion tween moves from one pose to the next.
//!
//! Every joint's instance keeps the transform it had when it joined (its rest), and is drawn
//! through its joint's matrix: the turn of every joint above it and the shift of its root. A
//! joint with no parent is a root; an armature may have several, each moved by its own shift.

use std::sync::Arc;

use flare_core::{DocumentPoint, InstanceId};
use glamx::{DAffine2, DVec2};
use thiserror::Error;

use crate::Instance;

/// The most joints one armature may have.
pub const MAX_JOINTS: usize = 256;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArmatureError {
    #[error("an armature needs at least one joint")]
    Empty,
    #[error("an armature may have at most {MAX_JOINTS} joints, this one has {0}")]
    TooManyJoints(usize),
    #[error("joint {0}'s parent must be a joint before it")]
    ParentNotBefore(usize),
    #[error("joint {0}'s instance is in the armature twice")]
    DuplicateInstance(usize),
    #[error("a pose must turn each of the {0} joints exactly once")]
    PoseLength(usize),
    #[error("joint {0}'s turn or shift is not a finite number")]
    NotFinite(usize),
    #[error("joint {0} has a parent, so only its root's shift can move it")]
    ShiftOnChild(usize),
    #[error("a joint's limits must be finite, the least first, within a whole turn either way")]
    Limit,
}

/// One joint: the instance it carries, where it is at rest, the joint above it, and how far it
/// may turn from rest either way.
#[derive(Debug, Clone, PartialEq)]
pub struct Joint {
    instance: Instance,
    at: DocumentPoint,
    parent: Option<usize>,
    limit: Option<(f64, f64)>,
}

impl Joint {
    pub fn new(instance: Instance, at: DocumentPoint, parent: Option<usize>) -> Self {
        Self {
            instance,
            at,
            parent,
            limit: None,
        }
    }

    /// The same joint, free to turn only from `min` to `max` radians from rest (clockwise is
    /// positive), or freely with `None`.
    pub fn with_limit(mut self, limit: Option<(f64, f64)>) -> Result<Self, ArmatureError> {
        if let Some((min, max)) = limit {
            let tau = std::f64::consts::TAU;
            if !min.is_finite() || !max.is_finite() || min > max || min < -tau || max > tau {
                return Err(ArmatureError::Limit);
            }
        }
        self.limit = limit;
        Ok(self)
    }

    /// The same joint carrying another instance (the joint stays where it is).
    #[must_use]
    pub fn with_instance(mut self, instance: Instance) -> Self {
        self.instance = instance;
        self
    }

    pub fn instance(&self) -> &Instance {
        &self.instance
    }

    /// Where the joint is at rest, in the space of the layer.
    pub fn at(&self) -> DocumentPoint {
        self.at
    }

    pub fn parent(&self) -> Option<usize> {
        self.parent
    }

    pub fn limit(&self) -> Option<(f64, f64)> {
        self.limit
    }

    /// `turn` brought inside the joint's limits.
    pub fn limited(&self, turn: f64) -> f64 {
        match self.limit {
            Some((min, max)) => turn.clamp(min, max),
            None => turn,
        }
    }
}

/// How one joint is posed: turned by `turn` radians from rest (clockwise, about the joint, and
/// carrying the bones below it), and, for a root, moved by `shift`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct JointPose {
    pub turn: f64,
    pub shift: DVec2,
}

/// A rig and one pose of it. Both are shared, so keyframes of one rig copy neither.
#[derive(Debug, Clone, PartialEq)]
pub struct Armature {
    joints: Arc<[Joint]>,
    pose: Arc<[JointPose]>,
}

impl Armature {
    /// An armature at rest. Every parent comes before its child, so the joints are a tree (or
    /// several) and can be posed in one pass.
    pub fn new(joints: Vec<Joint>) -> Result<Self, ArmatureError> {
        if joints.is_empty() {
            return Err(ArmatureError::Empty);
        }
        if joints.len() > MAX_JOINTS {
            return Err(ArmatureError::TooManyJoints(joints.len()));
        }
        let mut seen: Vec<InstanceId> = Vec::with_capacity(joints.len());
        for (index, joint) in joints.iter().enumerate() {
            if joint.parent.is_some_and(|p| p >= index) {
                return Err(ArmatureError::ParentNotBefore(index));
            }
            if seen.contains(&joint.instance.id()) {
                return Err(ArmatureError::DuplicateInstance(index));
            }
            seen.push(joint.instance.id());
        }
        let pose = vec![JointPose::default(); joints.len()];
        Ok(Self {
            joints: joints.into(),
            pose: pose.into(),
        })
    }

    /// The same rig in another pose. Turns are kept as given (limits are the poser's to keep,
    /// see [`Joint::limited`]); only a root may be shifted.
    pub fn with_pose(&self, pose: Vec<JointPose>) -> Result<Self, ArmatureError> {
        if pose.len() != self.joints.len() {
            return Err(ArmatureError::PoseLength(self.joints.len()));
        }
        for (index, (joint, posed)) in self.joints.iter().zip(&pose).enumerate() {
            if !posed.turn.is_finite() || !posed.shift.is_finite() {
                return Err(ArmatureError::NotFinite(index));
            }
            if joint.parent.is_some() && posed.shift != DVec2::ZERO {
                return Err(ArmatureError::ShiftOnChild(index));
            }
        }
        Ok(Self {
            joints: Arc::clone(&self.joints),
            pose: pose.into(),
        })
    }

    /// The same pose over another rig with as many joints (the joints' instances or limits
    /// changed, say). The pose is checked against the new rig.
    pub fn with_joints(&self, joints: Vec<Joint>) -> Result<Self, ArmatureError> {
        Self::new(joints)?.with_pose(self.pose.to_vec())
    }

    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }

    pub fn joint(&self, index: usize) -> Option<&Joint> {
        self.joints.get(index)
    }

    pub fn pose(&self) -> &[JointPose] {
        &self.pose
    }

    /// The joint carrying `instance`.
    pub fn joint_of(&self, instance: InstanceId) -> Option<usize> {
        self.joints.iter().position(|j| j.instance.id() == instance)
    }

    /// The joints whose parent is `index`.
    pub fn children(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        self.joints
            .iter()
            .enumerate()
            .filter(move |(_, j)| j.parent == Some(index))
            .map(|(i, _)| i)
    }

    /// `index` and every joint above it, up to its root.
    pub fn chain(&self, index: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut at = Some(index).filter(|i| *i < self.joints.len());
        // Parents come first, so this ends; the bound is only for the eye.
        while let Some(i) = at {
            chain.push(i);
            if chain.len() > MAX_JOINTS {
                break;
            }
            at = self.joints[i].parent;
        }
        chain
    }

    /// Whether `other` is the same rig: the same joints in the same places carrying the same
    /// instances, so a tween can run from this pose to its.
    pub fn same_rig(&self, other: &Armature) -> bool {
        Arc::ptr_eq(&self.joints, &other.joints) || self.joints == other.joints
    }

    /// Where every joint is and how far it is turned in all, in this pose: parents first.
    pub fn posed(&self) -> Vec<(DVec2, f64)> {
        let mut out: Vec<(DVec2, f64)> = Vec::with_capacity(self.joints.len());
        for (joint, posed) in self.joints.iter().zip(self.pose.iter()) {
            let here = joint.at.as_vec();
            let world = match joint.parent.and_then(|p| out.get(p).map(|w| (p, *w))) {
                Some((p, (parent_at, parent_turn))) => {
                    let offset = here - self.joints[p].at.as_vec();
                    (
                        parent_at + DVec2::from_angle(parent_turn).rotate(offset),
                        parent_turn + posed.turn,
                    )
                }
                None => (here + posed.shift, posed.turn),
            };
            out.push(world);
        }
        out
    }

    /// What each joint does to what it carries in this pose, in the order of the joints: its
    /// rest point goes to where it is posed, turned by its whole turn.
    pub fn joint_matrices(&self) -> Vec<DAffine2> {
        self.posed()
            .into_iter()
            .zip(self.joints.iter())
            .map(|((at, turn), joint)| {
                DAffine2::from_translation(at)
                    * DAffine2::from_angle(turn)
                    * DAffine2::from_translation(-joint.at.as_vec())
            })
            .collect()
    }

    /// Where each joint's instance is drawn in this pose, in the space of the layer.
    pub fn placements(&self) -> Vec<DAffine2> {
        self.joint_matrices()
            .into_iter()
            .zip(self.joints.iter())
            .map(|(matrix, joint)| matrix * joint.instance.transform().as_affine())
            .collect()
    }

    /// `t` of the way from this pose to `other`'s (0 to 1), when both are of one rig: every
    /// turn and shift moves in a straight line. `None` for two different rigs.
    pub fn lerp(&self, other: &Armature, t: f64) -> Option<Armature> {
        if !self.same_rig(other) {
            return None;
        }
        let t = t.clamp(0.0, 1.0);
        let pose = self
            .pose
            .iter()
            .zip(other.pose.iter())
            .map(|(a, b)| JointPose {
                turn: a.turn + (b.turn - a.turn) * t,
                shift: a.shift.lerp(b.shift, t),
            })
            .collect::<Vec<_>>();
        Some(Self {
            joints: Arc::clone(&self.joints),
            pose: pose.into(),
        })
    }
}

mod rig;

pub use rig::RigEdit;

#[cfg(test)]
mod tests;
