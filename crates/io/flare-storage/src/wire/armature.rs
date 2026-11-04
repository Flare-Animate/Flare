//! An armature keyframe to and from its stored form: each joint with its instance, where it
//! rests, its parent and limits, and how this keyframe poses it.

use flare_core::DocumentPoint;
use flare_document::{Armature, Joint, JointPose};
use glamx::DVec2;
use serde::{Deserialize, Serialize};

use super::content::build_instance;
use crate::StorageError;

#[derive(Serialize, Deserialize)]
pub(super) struct JointWire {
    id: String,
    symbol: String,
    matrix: [f64; 6],
    #[serde(default)]
    play: Option<String>,
    #[serde(default)]
    first_frame: u32,
    at: [f64; 2],
    #[serde(default)]
    parent: Option<usize>,
    /// The least and the most turn from rest, in radians; absent for a joint that turns freely.
    #[serde(default)]
    limit: Option<[f64; 2]>,
    /// This keyframe's turn, in radians, clockwise.
    #[serde(default)]
    turn: f64,
    /// This keyframe's shift of a root.
    #[serde(default)]
    shift: [f64; 2],
}

pub(super) fn wire_joints(armature: &Armature) -> Vec<JointWire> {
    armature
        .joints()
        .iter()
        .zip(armature.pose())
        .map(|(joint, posed)| {
            let instance = joint.instance();
            JointWire {
                id: instance.id().as_uuid().to_string(),
                symbol: instance.symbol().as_uuid().to_string(),
                matrix: instance.transform().as_affine().to_cols_array(),
                play: Some(instance.playback().mode.name().to_owned()),
                first_frame: instance.playback().first_frame,
                at: joint.at().as_vec().to_array(),
                parent: joint.parent(),
                limit: joint.limit().map(|(min, max)| [min, max]),
                turn: posed.turn,
                shift: posed.shift.to_array(),
            }
        })
        .collect()
}

pub(super) fn build_armature(joints: Vec<JointWire>) -> Result<Armature, StorageError> {
    let corrupt = |e: flare_document::ArmatureError| StorageError::Corrupt(e.to_string());
    // Checked before anything is built for each one.
    if joints.len() > flare_document::MAX_JOINTS {
        return Err(corrupt(flare_document::ArmatureError::TooManyJoints(
            joints.len(),
        )));
    }
    let mut rig = Vec::with_capacity(joints.len());
    let mut pose = Vec::with_capacity(joints.len());
    for wire in joints {
        let instance = build_instance(
            &wire.id,
            &wire.symbol,
            wire.matrix,
            wire.play.as_deref(),
            wire.first_frame,
        )?;
        let at = DocumentPoint::new(wire.at[0], wire.at[1])
            .map_err(|_| StorageError::Corrupt("non-finite joint".into()))?;
        let joint = Joint::new(instance, at, wire.parent)
            .with_limit(wire.limit.map(|[min, max]| (min, max)))
            .map_err(corrupt)?;
        rig.push(joint);
        pose.push(JointPose {
            turn: wire.turn,
            shift: DVec2::from_array(wire.shift),
        });
    }
    Armature::new(rig)
        .and_then(|rest| rest.with_pose(pose))
        .map_err(corrupt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joint(parent: Option<usize>, limit: Option<[f64; 2]>, turn: f64) -> JointWire {
        JointWire {
            id: uuid::Uuid::now_v7().to_string(),
            symbol: uuid::Uuid::now_v7().to_string(),
            matrix: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            play: None,
            first_frame: 0,
            at: [0.0, 0.0],
            parent,
            limit,
            turn,
            shift: [0.0, 0.0],
        }
    }

    #[test]
    fn a_damaged_rig_is_refused_not_guessed_at() {
        let corrupt = |joints| matches!(build_armature(joints), Err(StorageError::Corrupt(_)));
        assert!(build_armature(vec![joint(None, None, 0.0)]).is_ok());
        assert!(corrupt(vec![joint(Some(0), None, 0.0)]), "its own parent");
        assert!(corrupt(vec![]), "no joints");
        assert!(
            corrupt(vec![joint(None, Some([1.0, -1.0]), 0.0)]),
            "a backwards limit"
        );
        assert!(
            corrupt(vec![joint(None, None, f64::NAN)]),
            "a turn that is not a number"
        );
        let mut shifted = joint(Some(0), None, 0.0);
        shifted.shift = [1.0, 0.0];
        assert!(
            corrupt(vec![joint(None, None, 0.0), shifted]),
            "a child that is shifted"
        );
        let many = (0..=flare_document::MAX_JOINTS)
            .map(|_| joint(None, None, 0.0))
            .collect();
        assert!(corrupt(many), "too many joints");
    }
}
