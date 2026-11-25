//! The rigs a project keeps: the same behaviour as `flare rig project`, from
//! `flare_editor::rig_library`, so each change is one undo step.

use flare_core::{InstanceId, SymbolId};
use flare_editor::rig_library;
use flare_rig::{AngleLimit, Bone, IkGoal, Interp, PoseKey, Rig, RigCommand, evaluate_rig};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, parse_id};
use crate::params_rigs::{ProjectRigAddBone, ProjectRigEval, ProjectRigSetPose, ProjectRigSymbol};
use crate::server::{FlareServer, Outcome, State};

/// A named interpolation, as `rig_set_pose` and `project_rig_set_pose` take it.
pub(super) fn interp_named(name: &str) -> Result<Interp, String> {
    Interp::from_name(name).ok_or_else(|| format!("unknown interpolation {name:?}"))
}

fn symbol_of(text: &str) -> Result<SymbolId, String> {
    parse_id(text, "symbol").map(SymbolId::from_uuid)
}

fn rig_of(state: &State, symbol: SymbolId) -> Result<Rig, String> {
    rig_library::find(state.session.document(), symbol)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| rig_library::RigLibraryError::NoRig.to_string())
}

#[tool_router(router = project_rig_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "List the rigs the project keeps: each symbol with its bone and pose-key counts."
    )]
    fn project_rigs(&self) -> Outcome {
        self.read(|state| {
            let mut rigs = Vec::new();
            for asset in state.session.document().rig_assets() {
                let rig = rig_library::parse(asset.payload()).map_err(|e| e.to_string())?;
                rigs.push(json!({
                    "symbol": asset.symbol().as_uuid(),
                    "bones": rig.bones.len(),
                    "keys": rig.keys.len(),
                    "clock": format!("{:?}", rig.clock),
                }));
            }
            Ok(json!({ "rigs": rigs }))
        })
    }

    #[tool(
        description = "Give a symbol an empty rig kept in the project, on the clock its kind gives it; one undo step. Refused when the symbol is missing or already has a rig."
    )]
    fn project_rig_create(&self, Parameters(a): Parameters<ProjectRigSymbol>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let command = rig_library::create(state.session.document(), symbol_of(&a.symbol)?)
                .map_err(|e| e.to_string())?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Take a symbol's rig out of the project; one undo step. Refused when it has none."
    )]
    fn project_rig_remove(&self, Parameters(a): Parameters<ProjectRigSymbol>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let command = rig_library::remove(state.session.document(), symbol_of(&a.symbol)?)
                .map_err(|e| e.to_string())?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Add a bone to a symbol's rig kept in the project; one undo step. A bad parent, length or limit is refused and changes nothing."
    )]
    fn project_rig_add_bone(&self, Parameters(a): Parameters<ProjectRigAddBone>) -> Outcome {
        self.write(a.expected_revision, |state| {
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
                name: a.name.clone(),
                parent: a.parent,
                length: a.length,
                rest_angle: a.rest_angle.unwrap_or(0.0),
                limit,
                target: target.map(InstanceId::from_uuid),
            };
            let command = rig_library::edit(
                state.session.document(),
                symbol_of(&a.symbol)?,
                &[RigCommand::AddBone(bone)],
            )
            .map_err(|e| e.to_string())?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Key a pose (rotations and IK goals) at a parent-timeline frame on a symbol's rig kept in the project; one undo step."
    )]
    fn project_rig_set_pose(&self, Parameters(a): Parameters<ProjectRigSetPose>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let mut key = PoseKey {
                interp: interp_named(a.interp.as_deref().unwrap_or("linear"))?,
                ..PoseKey::default()
            };
            key.pose.rotations.extend(a.rotations.iter().copied());
            key.pose.ik =
                a.ik.iter()
                    .map(|&(tip, chain, x, y)| IkGoal {
                        tip,
                        chain,
                        target: [x, y],
                    })
                    .collect();
            let command = rig_library::edit(
                state.session.document(),
                symbol_of(&a.symbol)?,
                &[RigCommand::SetRigPose {
                    frame: a.frame,
                    key,
                }],
            )
            .map_err(|e| e.to_string())?;
            apply(state, vec![command])
        })
    }

    #[tool(
        description = "Evaluate the bones of a symbol's rig kept in the project at a frame: origin, angle, length and the instance each moves."
    )]
    fn project_rig_eval(&self, Parameters(a): Parameters<ProjectRigEval>) -> Outcome {
        self.read(|state| {
            let rig = rig_of(state, symbol_of(&a.symbol)?)?;
            let bones: Vec<Value> = evaluate_rig(&rig, a.frame)
                .map_err(|e| e.to_string())?
                .iter()
                .zip(&rig.bones)
                .map(|(t, b)| json!({ "name": b.name, "origin": t.origin, "angle": t.angle, "length": t.length, "target": t.target.map(|i| *i.as_uuid()) }))
                .collect();
            Ok(json!({ "frame": a.frame, "clock": format!("{:?}", rig.clock), "bones": bones }))
        })
    }
}
