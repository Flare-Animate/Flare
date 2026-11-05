//! `flare symbol` and `flare rig`: a symbol's kind and playback, and skeleton rigs. The behaviour
//! is in `flare-commands` and `flare-rig`; this parses arguments and prints. A rig is a JSON file
//! of its own for now (see docs/SURFACES.md): the project file does not hold rigs yet.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use flare_commands::Command;
use flare_core::SymbolId;
use flare_document::{PlayMode, Playback, SymbolKind};
use flare_rig::{AngleLimit, Bone, IkGoal, Interp, PoseKey, Rig, RigCommand, evaluate_rig};
use serde_json::{Value, json};
use uuid::Uuid;

/// `--kind` and `--mode` take the spellings every surface accepts (`movie-clip`, `movie clip`).
fn kind_arg(text: &str) -> Result<SymbolKind, String> {
    SymbolKind::parse(text)
        .ok_or_else(|| format!("unknown kind {text:?}: graphic, movie clip or button"))
}

fn mode_arg(text: &str) -> Result<PlayMode, String> {
    PlayMode::parse(text)
        .ok_or_else(|| format!("unknown mode {text:?}: loop, play once or single frame"))
}

#[derive(Subcommand)]
pub enum SymbolCommand {
    /// Make a symbol a Graphic, a Movie Clip or a Button.
    SetKind {
        path: PathBuf,
        symbol: Uuid,
        #[arg(long, value_parser = kind_arg)]
        kind: SymbolKind,
    },
    /// Set how the instance whose keyframe starts at `--at` plays its symbol.
    SetLoop {
        path: PathBuf,
        #[arg(long)]
        at: u32,
        #[arg(long, value_parser = mode_arg)]
        mode: PlayMode,
        #[arg(long, default_value_t = 0)]
        first_frame: u32,
        #[arg(long)]
        layer: Option<Uuid>,
    },
}

#[derive(Subcommand)]
pub enum RigCmd {
    /// Create a rig file for a symbol.
    Create {
        rig: PathBuf,
        #[arg(long)]
        symbol: Uuid,
        #[arg(long, value_parser = kind_arg, default_value = "graphic")]
        kind: SymbolKind,
    },
    /// Add a bone. `--parent` is an earlier bone's index.
    AddBone {
        rig: PathBuf,
        #[arg(long)]
        name: String,
        #[arg(long)]
        parent: Option<usize>,
        #[arg(long)]
        length: f64,
        #[arg(long, default_value_t = 0.0, allow_hyphen_values = true)]
        rest_angle: f64,
        /// Pose rotation limits in radians: min max.
        #[arg(long, num_args = 2, allow_hyphen_values = true)]
        limit: Option<Vec<f64>>,
        /// The child instance this bone moves.
        #[arg(long)]
        target: Option<Uuid>,
    },
    /// Key a pose at a frame: `--rotate bone=radians` and `--ik tip,chain,x,y`, repeatable.
    SetPose {
        rig: PathBuf,
        #[arg(long)]
        frame: u32,
        #[arg(long, allow_hyphen_values = true)]
        rotate: Vec<String>,
        #[arg(long, allow_hyphen_values = true)]
        ik: Vec<String>,
        #[arg(long, default_value = "linear")]
        interp: String,
    },
    /// Evaluate the bones at a frame.
    Eval {
        rig: PathBuf,
        #[arg(long, default_value_t = 0)]
        frame: u32,
    },
}

pub fn run_symbol(op: SymbolCommand) -> Result<Value> {
    match op {
        SymbolCommand::SetKind { path, symbol, kind } => {
            let mut session = crate::project::open_session(&path)?;
            session.apply([Command::SetSymbolKind {
                symbol: SymbolId::from_uuid(symbol),
                kind,
            }])?;
            crate::project::commit_session(&session, &path)?;
            Ok(json!({ "symbol": symbol, "kind": kind.name() }))
        }
        SymbolCommand::SetLoop {
            path,
            at,
            mode,
            first_frame,
            layer,
        } => {
            let mut session = crate::project::open_session(&path)?;
            let (scene, layer) = crate::project::first_scene_layer(&session, layer)?;
            session.apply([Command::SetInstancePlayback {
                scene,
                layer,
                start: at,
                playback: Playback { mode, first_frame },
            }])?;
            crate::project::commit_session(&session, &path)?;
            Ok(json!({ "frame": at, "mode": mode.name(), "first_frame": first_frame }))
        }
    }
}

fn read(path: &Path) -> Result<Rig> {
    let bytes = flare_core_read(path)?;
    let rig: Rig = serde_json::from_slice(&bytes).context("not a rig file")?;
    rig.validate()?;
    Ok(rig)
}

/// A rig file is small; refuse anything larger than 4 MiB before parsing.
fn flare_core_read(path: &Path) -> Result<Vec<u8>> {
    let len = std::fs::metadata(path)
        .with_context(|| format!("cannot read {}", path.display()))?
        .len();
    if len > 4 << 20 {
        bail!("{} is too large for a rig file", path.display());
    }
    Ok(std::fs::read(path)?)
}

fn write(path: &Path, rig: &Rig) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(rig)?)
        .with_context(|| format!("cannot write {}", path.display()))
}

fn nums(text: &str, count: usize) -> Result<Vec<f64>> {
    let parts: Vec<f64> = text
        .split(['=', ','])
        .map(|p| p.trim().parse::<f64>())
        .collect::<Result<_, _>>()
        .map_err(|_| anyhow!("{text:?} has a number that is not one"))?;
    if parts.len() != count {
        bail!("{text:?} needs {count} numbers");
    }
    Ok(parts)
}

fn index(value: f64) -> Result<usize> {
    if value >= 0.0 && value.fract() == 0.0 && value < 1e6 {
        Ok(value as usize)
    } else {
        bail!("{value} is not a bone index")
    }
}

fn interp(name: &str) -> Result<Interp> {
    Ok(match name {
        "constant" => Interp::Constant,
        "linear" => Interp::Linear,
        "ease-in" => Interp::EaseIn,
        "ease-out" => Interp::EaseOut,
        "ease-in-out" => Interp::EaseInOut,
        other => bail!("unknown interpolation {other:?}"),
    })
}

pub fn run_rig(op: RigCmd) -> Result<Value> {
    match op {
        RigCmd::Create { rig, symbol, kind } => {
            if rig.exists() {
                bail!("{} already exists", rig.display());
            }
            write(&rig, &Rig::new(SymbolId::from_uuid(symbol), kind))?;
            Ok(json!({ "created": rig.to_string_lossy() }))
        }
        RigCmd::AddBone {
            rig: path,
            name,
            parent,
            length,
            rest_angle,
            limit,
            target,
        } => {
            let rig = read(&path)?;
            let limit = match limit.as_deref() {
                Some(&[min, max]) => Some(AngleLimit::new(min, max)?),
                _ => None,
            };
            let bone = Bone {
                name,
                parent,
                length,
                rest_angle,
                limit,
                target: target.map(flare_core::InstanceId::from_uuid),
            };
            let rig = rig.apply(&[RigCommand::AddBone(bone)])?;
            write(&path, &rig)?;
            Ok(json!({ "bone": rig.bones.len() - 1 }))
        }
        RigCmd::SetPose {
            rig: path,
            frame,
            rotate,
            ik,
            interp: name,
        } => {
            let rig = read(&path)?;
            let mut key = PoseKey {
                interp: interp(&name)?,
                ..PoseKey::default()
            };
            for r in &rotate {
                let v = nums(r, 2)?;
                key.pose.rotations.insert(index(v[0])?, v[1]);
            }
            for g in &ik {
                let v = nums(g, 4)?;
                key.pose.ik.push(IkGoal {
                    tip: index(v[0])?,
                    chain: 0,
                    target: [v[2], v[3]],
                });
                if let Some(goal) = key.pose.ik.last_mut() {
                    goal.chain = index(v[1])?;
                }
            }
            let rig = rig.apply(&[RigCommand::SetRigPose { frame, key }])?;
            write(&path, &rig)?;
            Ok(json!({ "frame": frame, "keys": rig.keys.len() }))
        }
        RigCmd::Eval { rig, frame } => {
            let rig = read(&rig)?;
            let bones: Vec<Value> = evaluate_rig(&rig, frame)?
                .iter()
                .zip(&rig.bones)
                .map(|(t, b)| {
                    json!({ "name": b.name, "origin": t.origin, "angle": t.angle,
                        "length": t.length, "target": t.target.map(|i| *i.as_uuid()) })
                })
                .collect();
            Ok(json!({ "frame": frame, "clock": format!("{:?}", rig.clock), "bones": bones }))
        }
    }
}
