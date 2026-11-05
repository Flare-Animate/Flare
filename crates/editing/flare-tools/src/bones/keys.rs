//! Posing: a new pose at a frame is a keyframe of the armature layer.
//!
//! On a keyframe's first frame the pose replaces the one there. Inside a keyframe, the keyframe
//! is split: the first part keeps its pose and its tween, so it moves into the new pose, and the
//! new keyframe takes the rest of the frames and the motion the old one had.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Armature, Content, Document};
use glamx::DVec2;

use super::connect::armature_layer;
use super::{BoneRefusal, armature_at};

/// The commands that make `pose` the armature's at `frame`.
fn key_pose(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    frame: u32,
    pose: Armature,
) -> Result<Vec<Command>, BoneRefusal> {
    let found = armature_layer(document, scene, layer)?;
    let span = found.span_at(frame).ok_or(BoneRefusal::NoArmature)?;
    if !matches!(span.content(), Content::Armature(_)) {
        return Err(BoneRefusal::NoArmature);
    }
    let mut commands = vec![Command::RemoveSpan {
        scene,
        layer,
        start: span.start(),
    }];
    let posed = span
        .with_element(0, Content::Armature(pose))
        .ok_or(BoneRefusal::NoArmature)?;
    if frame == span.start() {
        commands.push(Command::InsertSpan {
            scene,
            layer,
            span: posed,
        });
        return Ok(commands);
    }
    let before = NonZeroU32::new(frame - span.start()).ok_or(BoneRefusal::NoArmature)?;
    let after = u32::try_from(span.end() - u64::from(frame))
        .ok()
        .and_then(NonZeroU32::new)
        .ok_or(BoneRefusal::NoArmature)?;
    commands.push(Command::InsertSpan {
        scene,
        layer,
        span: span.retimed(span.start(), before),
    });
    commands.push(Command::InsertSpan {
        scene,
        layer,
        span: posed.retimed(frame, after),
    });
    Ok(commands)
}

/// The commands that pull `joint` toward `target` at `frame` (see [`super::reach`]); a root is
/// moved there instead, and the whole armature with it.
pub fn pose_joint(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    frame: u32,
    joint: usize,
    target: DVec2,
) -> Result<Vec<Command>, BoneRefusal> {
    let (posed, _) = armature_at(document, scene, layer, frame)?;
    let rig = posed.joint(joint).ok_or(BoneRefusal::NoSuchJoint(joint))?;
    let next = if rig.parent().is_none() {
        let now = posed.posed()[joint].0;
        return shift_root(document, scene, layer, frame, joint, target - now);
    } else {
        super::reach(&posed, joint, target)
    };
    key_pose(document, scene, layer, frame, next)
}

/// The commands that turn `joint` to `turn` radians from rest at `frame` (kept within its
/// limits), carrying the bones below it.
pub fn turn_joint(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    frame: u32,
    joint: usize,
    turn: f64,
) -> Result<Vec<Command>, BoneRefusal> {
    let (posed, _) = armature_at(document, scene, layer, frame)?;
    let rig = posed.joint(joint).ok_or(BoneRefusal::NoSuchJoint(joint))?;
    let mut pose = posed.pose().to_vec();
    pose[joint].turn = rig.limited(turn);
    let next = posed.with_pose(pose)?;
    key_pose(document, scene, layer, frame, next)
}

/// The commands that move the root `joint`, and everything below it, by `by` at `frame`. A joint
/// with a parent is refused: it moves by turning.
pub fn shift_root(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    frame: u32,
    joint: usize,
    by: DVec2,
) -> Result<Vec<Command>, BoneRefusal> {
    let (posed, _) = armature_at(document, scene, layer, frame)?;
    posed.joint(joint).ok_or(BoneRefusal::NoSuchJoint(joint))?;
    let mut pose = posed.pose().to_vec();
    pose[joint].shift += by;
    let next = posed.with_pose(pose)?;
    key_pose(document, scene, layer, frame, next)
}
