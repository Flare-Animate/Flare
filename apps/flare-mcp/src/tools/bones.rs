//! Bones: linking instances into an armature and posing it, as Animate's Bone tool does. The
//! same library calls as the desktop app and the command line.

use flare_core::{DocumentPoint, InstanceId, LayerId};
use flare_tools::bones::{self, Bond};
use glamx::DVec2;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, parse_id};
use crate::params::{AddBone, ArmatureLayer, BoneJoint, FrameArgs, PoseBone};
use crate::server::{FlareServer, Outcome, State};

/// The armature layer named, or the first scene's only one.
fn armature_layer(state: &State, layer: Option<&str>) -> Result<LayerId, String> {
    let layers = state
        .session
        .document()
        .scenes()
        .first()
        .ok_or("the document has no scene")?
        .timeline()
        .layers();
    match layer {
        Some(text) => {
            let id = parse_id(text, "layer")?;
            layers
                .iter()
                .find(|l| *l.id().as_uuid() == id && bones::is_armature_layer(l))
                .map(|l| l.id())
                .ok_or_else(|| format!("bones: no armature layer {id} in the first scene"))
        }
        None => {
            let mut found = layers.iter().filter(|l| bones::is_armature_layer(l));
            let first = found
                .next()
                .ok_or("bones: the first scene has no armature; make one with add_bone")?;
            if found.next().is_some() {
                return Err("bones: the first scene has several armatures; name one".to_owned());
            }
            Ok(first.id())
        }
    }
}

fn point(x: Option<f64>, y: Option<f64>, what: &str) -> Result<Option<DVec2>, String> {
    match (x, y) {
        (None, None) => Ok(None),
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => Ok(Some(DVec2::new(x, y))),
        _ => Err(format!(
            "bones: {what} needs both x and y, as finite numbers"
        )),
    }
}

/// Applies a bone edit and reports the armatures at `frame` after it.
fn finish(
    state: &mut State,
    made: Result<Vec<flare_commands::Command>, bones::BoneRefusal>,
    doing: &str,
    frame: u32,
) -> Result<Value, String> {
    let commands = made.map_err(|e| format!("bones: cannot {doing}: {e}"))?;
    let mut out = apply(state, commands)?;
    let scene = first_scene(state)?;
    out["armatures"] = json!(bones::report(state.session.document(), scene, frame));
    Ok(out)
}

#[tool_router(router = bone_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Link two instances with a bone, as a drag of Animate's Bone tool from one to the other does. Two instances on plain layers become a new armature layer (one keyframe per pose, tweening to the next); from an instance an armature carries, the other joins it below that joint. Instance ids are the paths evaluate_frame lists (needs write access)."
    )]
    fn add_bone(&self, Parameters(args): Parameters<AddBone>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let frame = args.frame.unwrap_or(0);
            let at = |p: Option<DVec2>| p.and_then(|p| DocumentPoint::new(p.x, p.y).ok());
            let from = Bond {
                instance: InstanceId::from_uuid(parse_id(&args.from, "from")?),
                at: at(point(args.from_x, args.from_y, "from_x and from_y")?),
            };
            let to = Bond {
                instance: InstanceId::from_uuid(parse_id(&args.to, "to")?),
                at: at(point(args.to_x, args.to_y, "to_x and to_y")?),
            };
            let document = state.session.document();
            let count = document.scenes()[0]
                .timeline()
                .layers()
                .iter()
                .filter(|l| bones::is_armature_layer(l))
                .count();
            let made = bones::connect(
                document,
                scene,
                frame,
                from,
                to,
                &format!("Armature_{}", count + 1),
            );
            finish(state, made, "add the bone", frame)
        })
    }

    #[tool(
        description = "Pose an armature at a frame: pull a joint toward to_x, to_y (inverse kinematics; a root moves the whole armature), turn it to `turn` degrees from rest, or move a root by by_x, by_y. Between two poses this makes a pose of its own, which the one before tweens into (needs write access)."
    )]
    fn pose_bone(&self, Parameters(args): Parameters<PoseBone>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = armature_layer(state, args.layer.as_deref())?;
            let frame = args.frame.unwrap_or(0);
            let document = state.session.document();
            let target = point(args.to_x, args.to_y, "to_x and to_y")?;
            let by = point(args.by_x, args.by_y, "by_x and by_y")?;
            let made = match (target, args.turn, by) {
                (Some(target), None, None) => {
                    bones::pose_joint(document, scene, layer, frame, args.joint, target)
                }
                (None, Some(turn), None) if turn.is_finite() => {
                    bones::turn_joint(document, scene, layer, frame, args.joint, turn.to_radians())
                }
                (None, None, Some(by)) => {
                    bones::shift_root(document, scene, layer, frame, args.joint, by)
                }
                _ => return Err("bones: give one of to_x/to_y, turn or by_x/by_y".to_owned()),
            };
            finish(state, made, "pose the armature", frame)
        })
    }

    #[tool(
        description = "Let a joint turn only between min and max degrees from rest in every pose, or freely with neither (needs write access)."
    )]
    fn limit_joint(&self, Parameters(args): Parameters<BoneJoint>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = armature_layer(state, args.layer.as_deref())?;
            let limit = match (args.min, args.max) {
                (None, None) => None,
                (Some(min), Some(max)) => Some((min.to_radians(), max.to_radians())),
                _ => return Err("bones: give both min and max, or neither".to_owned()),
            };
            let made = bones::set_limit(state.session.document(), scene, layer, args.joint, limit);
            finish(state, made, "limit the joint", 0)
        })
    }

    #[tool(
        description = "Take away the bone into a joint and every bone below it, in every pose; the instances stay where they are (needs write access)."
    )]
    fn detach_bone(&self, Parameters(args): Parameters<BoneJoint>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = armature_layer(state, args.layer.as_deref())?;
            let made = bones::detach(state.session.document(), scene, layer, args.joint);
            finish(state, made, "detach the bone", 0)
        })
    }

    #[tool(
        description = "Turn an armature back into plain keyframes of its instances, each pose where it puts them (needs write access)."
    )]
    fn release_armature(&self, Parameters(args): Parameters<ArmatureLayer>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = armature_layer(state, args.layer.as_deref())?;
            let made = bones::release(state.session.document(), scene, layer);
            finish(state, made, "release the armature", 0)
        })
    }

    #[tool(
        description = "List the armatures of the first scene at a frame: each joint, where it is posed, how far it is turned (degrees), its limits and the instance it carries."
    )]
    fn list_bones(&self, Parameters(args): Parameters<FrameArgs>) -> Outcome {
        self.read(|state| {
            let scene = first_scene(state)?;
            Ok(json!({
                "frame": args.frame,
                "armatures": bones::report(state.session.document(), scene, args.frame),
            }))
        })
    }
}
