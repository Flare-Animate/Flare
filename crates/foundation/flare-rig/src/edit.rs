use flare_core::InstanceId;

use crate::model::{
    AngleLimit, Bone, Clock, MAX_BONES, MAX_IK_CHAIN, MAX_POSE_KEYS, Pose, PoseKey, Rig, RigError,
    finite,
};

/// One validated edit. A batch is applied to a copy, so a failure changes nothing and the old
/// rig is the undo step.
#[derive(Debug, Clone, PartialEq)]
pub enum RigCommand {
    AddBone(Bone),
    SetBoneAngleLimit {
        bone: usize,
        limit: Option<AngleLimit>,
    },
    SetBoneTarget {
        bone: usize,
        target: Option<InstanceId>,
    },
    SetRigPose {
        frame: u32,
        key: PoseKey,
    },
    RemoveRigPose {
        frame: u32,
    },
    SetClock(Clock),
}

pub(crate) fn check_bone(bone: &Bone, index: usize) -> Result<(), RigError> {
    let name_len = bone.name.trim().chars().count();
    if name_len == 0 || bone.name.chars().count() > 64 {
        return Err(RigError::BoneName);
    }
    if bone.parent.is_some_and(|p| p >= index) {
        return Err(RigError::BadParent(bone.parent.unwrap_or(0)));
    }
    if finite(bone.length, "length")? < 0.0 {
        return Err(RigError::NotFinite("length"));
    }
    finite(bone.rest_angle, "rest angle")?;
    if let Some(limit) = bone.limit {
        AngleLimit::new(limit.min, limit.max)?;
    }
    Ok(())
}

pub(crate) fn check_pose(pose: &Pose, bones: usize) -> Result<(), RigError> {
    for (&bone, &angle) in &pose.rotations {
        if bone >= bones {
            return Err(RigError::NoSuchBone(bone));
        }
        finite(angle, "rotation")?;
    }
    for goal in &pose.ik {
        if goal.tip >= bones {
            return Err(RigError::NoSuchBone(goal.tip));
        }
        // The chain climbs `chain - 1` parents from the tip, so it must fit above it.
        if goal.chain == 0 || goal.chain > MAX_IK_CHAIN || goal.chain > chain_room(goal.tip) {
            return Err(RigError::BadChain);
        }
        finite(goal.target[0], "target")?;
        finite(goal.target[1], "target")?;
    }
    finite(pose.root[0], "root")?;
    finite(pose.root[1], "root")?;
    Ok(())
}

/// Bones from index 0 to the tip: an upper bound; `Rig::apply` checks real parents.
fn chain_room(tip: usize) -> usize {
    tip + 1
}

fn check_chains(rig: &Rig) -> Result<(), RigError> {
    for key in rig.keys.values() {
        for goal in &key.pose.ik {
            let mut at = Some(goal.tip);
            for _ in 0..goal.chain {
                at = match at {
                    Some(i) => rig.bones[i].parent,
                    None => return Err(RigError::BadChain),
                };
            }
        }
    }
    Ok(())
}

impl Rig {
    /// Applies `commands` in order to a copy; `Err` leaves `self` as it was.
    pub fn apply(&self, commands: &[RigCommand]) -> Result<Rig, RigError> {
        let mut rig = self.clone();
        for command in commands {
            match command {
                RigCommand::AddBone(bone) => {
                    if rig.bones.len() >= MAX_BONES {
                        return Err(RigError::TooManyBones);
                    }
                    check_bone(bone, rig.bones.len())?;
                    rig.bones.push(bone.clone());
                }
                RigCommand::SetBoneAngleLimit { bone, limit } => {
                    let limit = limit
                        .map(|l| AngleLimit::new(l.min, l.max))
                        .transpose()?;
                    rig.bones.get_mut(*bone).ok_or(RigError::NoSuchBone(*bone))?.limit = limit;
                }
                RigCommand::SetBoneTarget { bone, target } => {
                    rig.bones.get_mut(*bone).ok_or(RigError::NoSuchBone(*bone))?.target = *target;
                }
                RigCommand::SetRigPose { frame, key } => {
                    check_pose(&key.pose, rig.bones.len())?;
                    if !rig.keys.contains_key(frame) && rig.keys.len() >= MAX_POSE_KEYS {
                        return Err(RigError::TooManyKeys);
                    }
                    rig.keys.insert(*frame, key.clone());
                }
                RigCommand::RemoveRigPose { frame } => {
                    rig.keys.remove(frame);
                }
                RigCommand::SetClock(clock) => rig.clock = *clock,
            }
        }
        check_chains(&rig)?;
        Ok(rig)
    }
}
