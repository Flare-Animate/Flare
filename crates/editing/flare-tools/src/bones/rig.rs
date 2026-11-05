//! Changing an armature's rig in every pose, and letting the instances go.

use flare_commands::Command;
use flare_core::{InstanceId, LayerId, SceneId, Transform2D};
use flare_document::{Content, Document, FrameSpan, Instance, RigEdit};

use super::BoneRefusal;
use super::connect::armature_layer;

/// The commands that make `edit` to every keyframe of the armature on `layer`, each keeping its
/// own pose.
pub(super) fn edit_every_key(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    edit: &RigEdit,
) -> Result<Vec<Command>, BoneRefusal> {
    let found = armature_layer(document, scene, layer)?;
    let mut commands = Vec::new();
    for span in found.spans() {
        let Content::Armature(armature) = span.content() else {
            continue;
        };
        let edited = armature.edited(edit)?;
        let rebuilt = span
            .with_element(0, Content::Armature(edited))
            .ok_or(BoneRefusal::NoArmature)?;
        commands.push(Command::RemoveSpan {
            scene,
            layer,
            start: span.start(),
        });
        commands.push(Command::InsertSpan {
            scene,
            layer,
            span: rebuilt,
        });
    }
    Ok(commands)
}

/// The commands that take away the bone into `joint` and every bone below it, in every pose: the
/// instances they carried stay where they are, each a joint on its own.
pub fn detach(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    joint: usize,
) -> Result<Vec<Command>, BoneRefusal> {
    check_joint(document, scene, layer, joint)?;
    edit_every_key(document, scene, layer, &RigEdit::Detach(joint))
}

/// The commands that let `joint` turn only from `min` to `max` radians from rest (or freely with
/// `None`), in every pose; a pose outside them is brought inside.
pub fn set_limit(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    joint: usize,
    limit: Option<(f64, f64)>,
) -> Result<Vec<Command>, BoneRefusal> {
    check_joint(document, scene, layer, joint)?;
    edit_every_key(document, scene, layer, &RigEdit::Limit(joint, limit))
}

/// The commands that turn the armature on `layer` back into plain keyframes: each pose becomes
/// its instances, where it puts them, over the same frames. The bones are gone; the first
/// keyframe keeps the instances' identities and the others get their own, as plain keyframes
/// must.
pub fn release(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<Vec<Command>, BoneRefusal> {
    let found = armature_layer(document, scene, layer)?;
    let mut commands = Vec::new();
    let mut first = true;
    for span in found.spans() {
        let Content::Armature(armature) = span.content() else {
            continue;
        };
        let elements = armature
            .joints()
            .iter()
            .zip(armature.placements())
            .map(|(joint, placed)| {
                let instance = joint.instance();
                let id = if first {
                    instance.id()
                } else {
                    InstanceId::new()
                };
                let transform = Transform2D::new(placed)
                    .map_err(|_| BoneRefusal::Rig("a pose went out of range".into()))?;
                Ok(Content::Instance(
                    Instance::new(id, instance.symbol(), transform)
                        .with_playback(instance.playback()),
                ))
            })
            .collect::<Result<Vec<_>, BoneRefusal>>()?;
        first = false;
        // The poses tweened as one; plain instances of several in a keyframe hold.
        let plain = FrameSpan::new(span.start(), span.duration(), Content::Empty);
        let Some(rebuilt) = plain
            .with_elements(elements)
            .map_err(|e| BoneRefusal::Rig(e.to_string()))?
        else {
            continue;
        };
        commands.push(Command::RemoveSpan {
            scene,
            layer,
            start: span.start(),
        });
        commands.push(Command::InsertSpan {
            scene,
            layer,
            span: rebuilt,
        });
    }
    Ok(commands)
}

fn check_joint(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    joint: usize,
) -> Result<(), BoneRefusal> {
    let found = armature_layer(document, scene, layer)?;
    let count = found
        .spans()
        .iter()
        .find_map(|s| match s.content() {
            Content::Armature(a) => Some(a.joints().len()),
            _ => None,
        })
        .unwrap_or(0);
    if joint >= count {
        return Err(BoneRefusal::NoSuchJoint(joint));
    }
    Ok(())
}
