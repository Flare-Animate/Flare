//! A bone from one instance to another: what the Bone tool's drag makes.

use std::num::NonZeroU32;

use flare_commands::Command;
use flare_core::{DocumentPoint, InstanceId, LayerId, SceneId};
use flare_document::{
    Armature, Content, Document, FrameSpan, Instance, Joint, Layer, RigEdit, Tween,
};

use super::{BoneRefusal, find_layer};
use crate::common::scene_timeline;

/// One end of a bone: the instance there, and where on it the joint goes (its registration
/// point when not given).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bond {
    pub instance: InstanceId,
    pub at: Option<DocumentPoint>,
}

/// Where an instance is at a frame.
enum Found<'d> {
    /// An element of a plain keyframe.
    Plain {
        layer: &'d Layer,
        span: &'d FrameSpan,
        element: usize,
        instance: &'d Instance,
    },
    /// Carried by a joint of an armature.
    Joint { layer: &'d Layer, joint: usize },
}

fn find<'d>(
    document: &'d Document,
    scene: SceneId,
    frame: u32,
    instance: InstanceId,
) -> Result<Found<'d>, BoneRefusal> {
    let timeline = scene_timeline(document, scene).ok_or(BoneRefusal::NoSuchScene)?;
    for layer in timeline.layers() {
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        if let Content::Armature(armature) = span.content() {
            if let Some(joint) = armature.joint_of(instance) {
                return Ok(Found::Joint { layer, joint });
            }
            continue;
        }
        let hit = span
            .elements()
            .enumerate()
            .find_map(|(index, element)| match element {
                Content::Instance(i) if i.id() == instance => Some((index, i)),
                _ => None,
            });
        if let Some((element, instance)) = hit {
            return Ok(Found::Plain {
                layer,
                span,
                element,
                instance,
            });
        }
    }
    Err(BoneRefusal::NoSuchInstance(instance))
}

fn editable(layer: &Layer) -> Result<(), BoneRefusal> {
    if layer.is_locked() || !layer.is_visible() {
        return Err(BoneRefusal::Locked);
    }
    Ok(())
}

/// The commands that take `element` out of `span` on `layer`: the element alone, or the span
/// when it is all there is.
fn take_out(scene: SceneId, layer: LayerId, span: &FrameSpan, element: usize) -> Vec<Command> {
    let mut commands = vec![Command::RemoveSpan {
        scene,
        layer,
        start: span.start(),
    }];
    if let Some(rest) = span.without_element(element) {
        commands.push(Command::InsertSpan {
            scene,
            layer,
            span: rest,
        });
    }
    commands
}

fn joint_point(bond: Bond, instance: &Instance) -> DocumentPoint {
    bond.at.unwrap_or_else(|| {
        let at = instance.transform().as_affine().translation;
        DocumentPoint::new(at.x, at.y).unwrap_or(DocumentPoint::ORIGIN)
    })
}

/// The commands that link `from` to `to` with a bone at `frame`, as the Bone tool's drag does.
///
/// Two instances in plain keyframes become the root and first joint of a new armature layer,
/// named `layer_name`, just above the layer `from` was on; its keyframe covers both of theirs and
/// tweens to the next pose. From an instance an armature carries, the bone goes to `to`, which
/// leaves its layer and joins every pose of that armature below `from`'s joint, drawn where it
/// is in this frame's pose. An instance on a locked or hidden layer, one that tweens, and a bone
/// that would join two armatures (or start outside one and end in one) are refused.
pub fn connect(
    document: &Document,
    scene: SceneId,
    frame: u32,
    from: Bond,
    to: Bond,
    layer_name: &str,
) -> Result<Vec<Command>, BoneRefusal> {
    if from.instance == to.instance {
        return Err(BoneRefusal::SameInstance);
    }
    let (start, end) = (
        find(document, scene, frame, from.instance)?,
        find(document, scene, frame, to.instance)?,
    );
    let Found::Plain {
        layer: to_layer,
        span: to_span,
        element: to_element,
        instance: to_instance,
    } = end
    else {
        return Err(BoneRefusal::OtherArmature);
    };
    editable(to_layer)?;
    if to_span.tween() != Tween::None {
        return Err(BoneRefusal::Tweening);
    }
    let to_at = joint_point(to, to_instance);
    match start {
        Found::Joint { layer, joint } => {
            editable(layer)?;
            let (posed, _) = super::armature_at(document, scene, layer.id(), frame)?;
            let (rest, at) = posed
                .rest_below(joint, to_instance.transform().as_affine(), to_at.as_vec())
                .ok_or(BoneRefusal::NoSuchJoint(joint))?;
            let carried = Instance::new(to_instance.id(), to_instance.symbol(), rest)
                .with_playback(to_instance.playback());
            let edit = RigEdit::Add(Joint::new(carried, at, Some(joint)));
            let mut commands = take_out(scene, to_layer.id(), to_span, to_element);
            commands.extend(super::rig::edit_every_key(
                document,
                scene,
                layer.id(),
                &edit,
            )?);
            Ok(commands)
        }
        Found::Plain {
            layer: from_layer,
            span: from_span,
            element: from_element,
            instance: from_instance,
        } => {
            editable(from_layer)?;
            if from_span.tween() != Tween::None {
                return Err(BoneRefusal::Tweening);
            }
            let armature = Armature::new(vec![
                Joint::new(
                    from_instance.clone(),
                    joint_point(from, from_instance),
                    None,
                ),
                Joint::new(to_instance.clone(), to_at, Some(0)),
            ])?;
            let first = from_span.start().min(to_span.start());
            let last = from_span.end().max(to_span.end());
            let duration = u32::try_from(last - u64::from(first))
                .ok()
                .and_then(NonZeroU32::new)
                .ok_or(BoneRefusal::NoArmature)?;
            let key = FrameSpan::new(first, duration, Content::Armature(armature))
                .with_tween(Tween::Linear);
            let id = LayerId::new();
            let new_layer = Layer::new(id, layer_name, vec![key])
                .map_err(|e| BoneRefusal::Rig(e.to_string()))?;
            let index = scene_timeline(document, scene)
                .and_then(|t| t.layers().iter().position(|l| l.id() == from_layer.id()))
                .unwrap_or(0);
            // Both instances leave their keyframes first, so neither is ever in two places.
            let mut commands = Vec::new();
            if from_layer.id() == to_layer.id() && from_span.start() == to_span.start() {
                let mut all: Vec<Content> = from_span.elements().cloned().collect();
                let (high, low) = (from_element.max(to_element), from_element.min(to_element));
                all.remove(high);
                all.remove(low);
                commands.push(Command::RemoveSpan {
                    scene,
                    layer: from_layer.id(),
                    start: from_span.start(),
                });
                if let Some(rest) = from_span
                    .with_elements(all)
                    .map_err(|e| BoneRefusal::Rig(e.to_string()))?
                {
                    commands.push(Command::InsertSpan {
                        scene,
                        layer: from_layer.id(),
                        span: rest,
                    });
                }
            } else {
                commands.extend(take_out(scene, from_layer.id(), from_span, from_element));
                commands.extend(take_out(scene, to_layer.id(), to_span, to_element));
            }
            commands.push(Command::AddLayer {
                scene,
                layer: new_layer,
            });
            commands.push(Command::MoveLayer {
                scene,
                layer: id,
                index,
            });
            Ok(commands)
        }
    }
}

/// The layer an armature lives on, checked for editing.
pub(super) fn armature_layer(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
) -> Result<&Layer, BoneRefusal> {
    let found = find_layer(document, scene, layer)?;
    editable(found)?;
    if !found
        .spans()
        .iter()
        .any(|s| matches!(s.content(), Content::Armature(_)))
    {
        return Err(BoneRefusal::NoArmature);
    }
    Ok(found)
}
