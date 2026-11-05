//! Symbol kind and playback, and skeleton rigs: the same behaviour as `flare symbol` and
//! `flare rig`, from `flare_commands` and `flare_rig`. A rig is passed in and returned as JSON
//! because the project does not hold rigs yet (docs/SURFACES.md).

use flare_commands::Command;
use flare_core::{InstanceId, SymbolId};
use flare_document::{PlayMode, Playback, SymbolKind};
use flare_rig::{AngleLimit, Bone, IkGoal, Interp, PoseKey, Rig, RigCommand, evaluate_rig};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, parse_id, target_layer};
use crate::server::{FlareServer, Outcome};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetSymbolKind {
    pub symbol: String,
    /// "graphic", "movie clip" or "button".
    pub kind: String,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct SetInstanceLoop {
    /// Frame the instance's keyframe starts at.
    pub at: u32,
    /// "loop", "play once" or "single frame".
    pub mode: String,
    #[serde(default)]
    pub first_frame: u32,
    pub layer: Option<String>,
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RigCreate {
    pub symbol: String,
    /// "graphic" (frame-locked) or "movie clip" (its own clock).
    pub kind: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RigAddBone {
    /// The rig as returned by rig_create.
    pub rig: Value,
    pub name: String,
    /// Index of an earlier bone.
    pub parent: Option<usize>,
    pub length: f64,
    pub rest_angle: Option<f64>,
    /// Pose rotation limits in radians.
    pub limit: Option<[f64; 2]>,
    /// The child instance this bone moves.
    pub target: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RigSetPose {
    pub rig: Value,
    pub frame: u32,
    /// [bone index, radians] pairs.
    #[serde(default)]
    pub rotations: Vec<(usize, f64)>,
    /// [tip bone, chain length, target x, target y] goals.
    #[serde(default)]
    pub ik: Vec<(usize, usize, f64, f64)>,
    /// constant, linear, ease-in, ease-out or ease-in-out.
    pub interp: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RigEval {
    pub rig: Value,
    pub frame: u32,
}

fn kind(name: &str) -> Result<SymbolKind, String> {
    SymbolKind::parse(name).ok_or_else(|| format!("unknown kind {name:?}"))
}

fn rig_in(value: Value) -> Result<Rig, String> {
    let rig: Rig = serde_json::from_value(value).map_err(|e| format!("not a rig: {e}"))?;
    rig.validate().map_err(|e| e.to_string())?;
    Ok(rig)
}

fn rig_out(rig: &Rig) -> Outcome {
    serde_json::to_string(rig).map_err(|e| e.to_string())
}

#[tool_router(router = symbol_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(description = "Make a symbol a Graphic, a Movie Clip or a Button (needs write access).")]
    fn set_symbol_kind(&self, Parameters(a): Parameters<SetSymbolKind>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let symbol = SymbolId::from_uuid(parse_id(&a.symbol, "symbol")?);
            let kind = kind(&a.kind)?;
            apply(state, vec![Command::SetSymbolKind { symbol, kind }])
        })
    }

    #[tool(
        description = "Set how the instance whose keyframe starts at `at` plays its symbol: loop, play once or single frame, from first_frame (needs write access)."
    )]
    fn set_instance_loop(&self, Parameters(a): Parameters<SetInstanceLoop>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let mode =
                PlayMode::parse(&a.mode).ok_or_else(|| format!("unknown mode {:?}", a.mode))?;
            let scene = first_scene(state)?;
            let layer = target_layer(state, a.layer.as_deref())?;
            let playback = Playback {
                mode,
                first_frame: a.first_frame,
            };
            apply(
                state,
                vec![Command::SetInstancePlayback {
                    scene,
                    layer,
                    start: a.at,
                    playback,
                }],
            )
        })
    }

    #[tool(description = "Create an empty skeleton rig for a symbol; returns the rig as JSON.")]
    fn rig_create(&self, Parameters(a): Parameters<RigCreate>) -> Outcome {
        let symbol = SymbolId::from_uuid(parse_id(&a.symbol, "symbol")?);
        let k = match a.kind.as_deref() {
            Some(name) => kind(name)?,
            None => SymbolKind::Graphic,
        };
        rig_out(&Rig::new(symbol, k))
    }

    #[tool(
        description = "Add a bone to a rig; returns the new rig. A bad parent, length or limit is refused."
    )]
    fn rig_add_bone(&self, Parameters(a): Parameters<RigAddBone>) -> Outcome {
        let rig = rig_in(a.rig)?;
        let limit = a
            .limit
            .map(|[lo, hi]| AngleLimit::new(lo, hi))
            .transpose()
            .map_err(|e| e.to_string())?;
        let target = a
            .target
            .as_deref()
            .map(|t| parse_id(t, "target"))
            .transpose()?;
        let bone = Bone {
            name: a.name,
            parent: a.parent,
            length: a.length,
            rest_angle: a.rest_angle.unwrap_or(0.0),
            limit,
            target: target.map(InstanceId::from_uuid),
        };
        rig_out(
            &rig.apply(&[RigCommand::AddBone(bone)])
                .map_err(|e| e.to_string())?,
        )
    }

    #[tool(
        description = "Key a rig pose (rotations and IK goals) at a parent-timeline frame; returns the new rig."
    )]
    fn rig_set_pose(&self, Parameters(a): Parameters<RigSetPose>) -> Outcome {
        let rig = rig_in(a.rig)?;
        let interp = match a.interp.as_deref().unwrap_or("linear") {
            "constant" => Interp::Constant,
            "linear" => Interp::Linear,
            "ease-in" => Interp::EaseIn,
            "ease-out" => Interp::EaseOut,
            "ease-in-out" => Interp::EaseInOut,
            other => return Err(format!("unknown interpolation {other:?}")),
        };
        let mut key = PoseKey {
            interp,
            ..PoseKey::default()
        };
        key.pose.rotations.extend(a.rotations);
        key.pose.ik =
            a.ik.into_iter()
                .map(|(tip, chain, x, y)| IkGoal {
                    tip,
                    chain,
                    target: [x, y],
                })
                .collect();
        rig_out(
            &rig.apply(&[RigCommand::SetRigPose {
                frame: a.frame,
                key,
            }])
            .map_err(|e| e.to_string())?,
        )
    }

    #[tool(
        description = "Evaluate a rig's bones at a frame: origin, angle, length and the instance each moves."
    )]
    fn rig_eval(&self, Parameters(a): Parameters<RigEval>) -> Outcome {
        let rig = rig_in(a.rig)?;
        let bones: Vec<Value> = evaluate_rig(&rig, a.frame)
            .map_err(|e| e.to_string())?
            .iter()
            .zip(&rig.bones)
            .map(|(t, b)| json!({ "name": b.name, "origin": t.origin, "angle": t.angle, "length": t.length, "target": t.target.map(|i| *i.as_uuid()) }))
            .collect();
        Ok(json!({ "frame": a.frame, "bones": bones }).to_string())
    }
}
