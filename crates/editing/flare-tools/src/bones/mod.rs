//! The Bone tool and posing: linking symbol instances with bones into an armature, and posing it.
//!
//! As in Animate, a drag with the Bone tool from one instance to another links them with a bone:
//! both leave their layers for an armature layer of their own (one keyframe per pose), and a drag
//! from an instance already in an armature adds a bone below it. Dragging a joint poses the
//! armature: a root moves it all, any other joint is pulled toward the pointer by inverse
//! kinematics, turning the joints above it within their limits. A pose made between two
//! keyframes is a new keyframe there, which the keyframe before tweens into.
//!
//! Every operation is a pure function from a document to commands, so the desktop app, the
//! command line and the MCP server share it.

use std::fmt;

use flare_core::{DocumentPoint, InstanceId, LayerId, SceneId};
use flare_document::{Armature, ArmatureError, Content, Document, FrameSpan, Layer};
use glamx::DVec2;

use crate::common::scene_timeline;

mod connect;
mod ik;
mod keys;
mod report;
mod rig;

pub use connect::{Bond, connect};
pub use ik::reach;
pub use keys::{pose_joint, shift_root, turn_joint};
pub use report::{ArmatureReport, JointReport, is_armature_layer, report};
pub use rig::{detach, release, set_limit};

/// One joint of one armature layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JointRef {
    pub layer: LayerId,
    pub joint: usize,
}

/// Why bones could not be made or posed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BoneRefusal {
    NoSuchScene,
    NoSuchLayer,
    /// No instance with that identity is shown at the frame.
    NoSuchInstance(InstanceId),
    /// A bone needs two different instances.
    SameInstance,
    /// The instance is on a locked or hidden layer.
    Locked,
    /// The instance's keyframe tweens; a tween and a bone cannot both move it.
    Tweening,
    /// The bone would join two armatures, or start from an instance outside one into an
    /// instance in one: drag from the armature out to the instance instead.
    OtherArmature,
    /// The layer has no armature at the frame.
    NoArmature,
    NoSuchJoint(usize),
    /// What the rig or a keyframe would become is not allowed (too many joints, a limit that
    /// is not one).
    Rig(String),
}

impl fmt::Display for BoneRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSuchScene => f.write_str("no such scene"),
            Self::NoSuchLayer => f.write_str("no such layer"),
            Self::NoSuchInstance(id) => {
                write!(
                    f,
                    "no instance {} is on the stage at that frame",
                    id.as_uuid()
                )
            }
            Self::SameInstance => f.write_str("a bone joins two different instances"),
            Self::Locked => f.write_str("the instance is on a locked or hidden layer"),
            Self::Tweening => {
                f.write_str("the instance tweens; remove its tween before giving it bones")
            }
            Self::OtherArmature => f.write_str(
                "a bone goes from an armature out to an instance that is not in one; armatures \
                 cannot be joined",
            ),
            Self::NoArmature => f.write_str("the layer has no armature at that frame"),
            Self::NoSuchJoint(joint) => write!(f, "the armature has no joint {joint}"),
            Self::Rig(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for BoneRefusal {}

impl From<ArmatureError> for BoneRefusal {
    fn from(error: ArmatureError) -> Self {
        Self::Rig(error.to_string())
    }
}

/// One armature as it stands at a frame, for drawing its bones: each joint where it is posed
/// and the joint above it.
#[derive(Debug, Clone, PartialEq)]
pub struct Skeleton {
    pub layer: LayerId,
    pub joints: Vec<(DVec2, Option<usize>)>,
}

pub(crate) fn find_layer(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<&Layer, BoneRefusal> {
    scene_timeline(document, scene)
        .ok_or(BoneRefusal::NoSuchScene)?
        .layers()
        .iter()
        .find(|l| l.id() == layer)
        .ok_or(BoneRefusal::NoSuchLayer)
}

/// The armature `layer` shows at `frame` (tweened, when it tweens there) and the keyframe it
/// is in.
pub fn armature_at(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    frame: u32,
) -> Result<(Armature, &FrameSpan), BoneRefusal> {
    let found = find_layer(document, scene, layer)?;
    let span = found.span_at(frame).ok_or(BoneRefusal::NoArmature)?;
    let posed = flare_eval::armature_at(found, span, frame).ok_or(BoneRefusal::NoArmature)?;
    Ok((posed.into_owned(), span))
}

/// Every armature of the scene that shows at `frame` and can be edited (visible and unlocked),
/// top layer first.
pub fn skeletons(document: &Document, scene: SceneId, frame: u32) -> Vec<Skeleton> {
    let Some(timeline) = scene_timeline(document, scene) else {
        return Vec::new();
    };
    timeline
        .layers()
        .iter()
        .filter(|l| l.is_visible() && !l.is_locked())
        .filter_map(|layer| {
            let span = layer.span_at(frame)?;
            let posed = flare_eval::armature_at(layer, span, frame)?;
            let joints = posed
                .posed()
                .into_iter()
                .zip(posed.joints())
                .map(|((at, _), joint)| (at, joint.parent()))
                .collect();
            Some(Skeleton {
                layer: layer.id(),
                joints,
            })
        })
        .collect()
}

/// The joint whose instance is `instance` at `frame`, if an armature carries it.
pub fn joint_of(
    document: &Document,
    scene: SceneId,
    frame: u32,
    instance: InstanceId,
) -> Option<JointRef> {
    scene_timeline(document, scene)?
        .layers()
        .iter()
        .find_map(|layer| match layer.span_at(frame)?.content() {
            Content::Armature(armature) => armature.joint_of(instance).map(|joint| JointRef {
                layer: layer.id(),
                joint,
            }),
            _ => None,
        })
}

/// The joint whose circle is nearest `point` at `frame`, within `radius`.
pub fn joint_near(
    document: &Document,
    scene: SceneId,
    frame: u32,
    point: DocumentPoint,
    radius: f64,
) -> Option<JointRef> {
    let p = point.as_vec();
    nearest(
        skeletons(document, scene, frame)
            .iter()
            .flat_map(|skeleton| {
                skeleton
                    .joints
                    .iter()
                    .enumerate()
                    .map(move |(joint, (at, _))| {
                        let at_joint = JointRef {
                            layer: skeleton.layer,
                            joint,
                        };
                        (at.distance(p), at_joint)
                    })
            }),
        radius,
    )
}

/// The bone whose line is nearest `point` at `frame`, within `radius`, as the joint at its tip:
/// a bone is pulled by its tip.
pub fn bone_near(
    document: &Document,
    scene: SceneId,
    frame: u32,
    point: DocumentPoint,
    radius: f64,
) -> Option<JointRef> {
    let p = point.as_vec();
    nearest(
        skeletons(document, scene, frame)
            .iter()
            .flat_map(|skeleton| {
                skeleton
                    .joints
                    .iter()
                    .enumerate()
                    .filter_map(move |(joint, (tip, parent))| {
                        let base = skeleton.joints.get((*parent)?)?.0;
                        let at_joint = JointRef {
                            layer: skeleton.layer,
                            joint,
                        };
                        Some((distance_to_segment(p, base, *tip), at_joint))
                    })
            }),
        radius,
    )
}

fn nearest(found: impl Iterator<Item = (f64, JointRef)>, radius: f64) -> Option<JointRef> {
    found
        .filter(|(distance, _)| *distance <= radius)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, at)| at)
}

fn distance_to_segment(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let along = b - a;
    let length = along.length_squared();
    if length <= f64::EPSILON {
        return p.distance(a);
    }
    let t = ((p - a).dot(along) / length).clamp(0.0, 1.0);
    p.distance(a + along * t)
}

#[cfg(test)]
mod tests;
