//! `flare symbol` and `flare rig`: converting artwork to a symbol, a symbol's kind and playback,
//! layers made into a symbol, and skeleton rigs. The behaviour is in `flare-tools`,
//! `flare-commands`, `flare-editor` and `flare-rig`; this parses arguments and prints. A rig is a
//! JSON file
//! of its own for now (see docs/SURFACES.md): the project file does not hold rigs yet.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use flare_commands::Command;
use flare_core::SymbolId;
use flare_document::{PlayMode, Playback, SymbolKind};
use flare_rig::{AngleLimit, Bone, IkGoal, Interp, PoseKey, Rig, RigCommand, evaluate_rig};
use flare_tools::library::{self, Registration};
use flare_tools::ops::SpanRef;
use serde_json::{Value, json};
use uuid::Uuid;

/// `--kind` and `--mode` take the spellings every surface accepts (`movie-clip`, `movie clip`).
pub(crate) fn kind_arg(text: &str) -> Result<SymbolKind, String> {
    SymbolKind::parse(text)
        .ok_or_else(|| format!("unknown kind {text:?}: graphic, movie clip or button"))
}

fn registration_arg(text: &str) -> Result<Registration, String> {
    Registration::parse(text).ok_or_else(|| {
        format!("unknown registration {text:?}: top-left, top, top-right, left, center, right, bottom-left, bottom or bottom-right")
    })
}

fn mode_arg(text: &str) -> Result<PlayMode, String> {
    PlayMode::parse(text)
        .ok_or_else(|| format!("unknown mode {text:?}: loop, play once, single frame, loop reverse or play once reverse"))
}

#[derive(Subcommand)]
pub enum SymbolCommand {
    /// Convert to Symbol: the shape in the keyframe that starts at `--at` becomes a symbol, and an
    /// instance of it takes its place without moving on the stage.
    Convert {
        path: PathBuf,
        #[arg(long)]
        at: u32,
        #[arg(long)]
        layer: Option<Uuid>,
        /// Which of the keyframe's elements, counted from 0 at the bottom.
        #[arg(long, default_value_t = 0)]
        element: usize,
        /// The symbol's name; the first free "Symbol N" when left out. A name the library already
        /// has is refused.
        #[arg(long)]
        name: Option<String>,
        #[arg(long, value_parser = kind_arg, default_value = "graphic")]
        kind: SymbolKind,
        /// Where on the shape's box the symbol's own origin goes: top-left, top, top-right, left,
        /// center, right, bottom-left, bottom or bottom-right.
        #[arg(long, value_parser = registration_arg, default_value = "top-left")]
        registration: Registration,
    },
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
        /// loop, play once, single frame, loop reverse or play once reverse.
        #[arg(long, value_parser = mode_arg)]
        mode: PlayMode,
        #[arg(long, default_value_t = 0)]
        first_frame: u32,
        #[arg(long)]
        layer: Option<Uuid>,
    },
    /// Increment Instances: every instance of a symbol whose playback begins at `--from` or
    /// later begins `--by` frames further on (negative moves back).
    Increment {
        path: PathBuf,
        symbol: Uuid,
        #[arg(long, default_value_t = 0)]
        from: u32,
        #[arg(long, allow_hyphen_values = true)]
        by: i64,
    },
    /// Bake Transform: the transform of the instance at `--at` becomes part of its symbol's
    /// artwork and every instance of the symbol takes the reverse.
    Bake {
        path: PathBuf,
        #[arg(long)]
        at: u32,
        #[arg(long)]
        layer: Option<Uuid>,
        #[arg(long, default_value_t = 0)]
        element: usize,
    },
    /// Add Instance to Symbol: a copy of the element at `--at` becomes one frame at the end of
    /// the symbol's first layer.
    AddInstance {
        path: PathBuf,
        symbol: Uuid,
        #[arg(long)]
        at: u32,
        #[arg(long)]
        layer: Option<Uuid>,
        #[arg(long, default_value_t = 0)]
        element: usize,
    },
    /// Move layers (a folder with everything in it) into a new symbol, and put an instance of
    /// it where they were: Convert Layers to Symbol.
    FromLayers {
        path: PathBuf,
        /// A layer to convert; give `--layer` once for each.
        #[arg(long = "layer", required = true)]
        layers: Vec<Uuid>,
        #[arg(long)]
        name: String,
        #[arg(long, value_parser = kind_arg, default_value = "graphic")]
        kind: SymbolKind,
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
    /// The rigs stored in a project file: the same bones, poses and evaluation, but the rig is
    /// kept in the project and every change is one undo step in the apps.
    Project {
        #[command(subcommand)]
        op: crate::rig_project::ProjectRigCmd,
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
        SymbolCommand::Increment {
            path,
            symbol,
            from,
            by,
        } => crate::symbol_edit::increment(&path, symbol, from, by),
        SymbolCommand::Bake {
            path,
            at,
            layer,
            element,
        } => crate::symbol_edit::bake(&path, at, layer, element),
        SymbolCommand::AddInstance {
            path,
            symbol,
            at,
            layer,
            element,
        } => crate::symbol_edit::add_instance(&path, symbol, at, layer, element),
        SymbolCommand::Convert {
            path,
            at,
            layer,
            element,
            name,
            kind,
            registration,
        } => {
            let mut session = crate::project::open_session(&path)?;
            let (scene, layer) = crate::project::first_scene_layer(&session, layer)?;
            let document = session.document();
            let name = name
                .unwrap_or_else(|| library::free_symbol_name(document, |n| format!("Symbol {n}")));
            let at = SpanRef {
                layer,
                start: at,
                element,
            };
            let (commands, id) =
                library::convert_to_symbol(document, scene, at, &name, kind, registration)?;
            session.apply(commands)?;
            crate::project::commit_session(&session, &path)?;
            let name = name.trim();
            Ok(json!({
                "symbol": id.as_uuid(),
                "name": name,
                "kind": kind.name(),
                "registration": registration.name(),
            }))
        }
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
        SymbolCommand::FromLayers {
            path,
            layers,
            name,
            kind,
        } => {
            let mut session = crate::project::open_session(&path)?;
            let (scene, _) = crate::project::first_scene_layer(&session, None)?;
            let picked: Vec<flare_core::LayerId> = layers
                .into_iter()
                .map(flare_core::LayerId::from_uuid)
                .collect();
            let done = flare_editor::layers_to_symbol::convert_layers_to_symbol(
                session.document(),
                scene,
                &picked,
                &name,
                kind,
            )?;
            session.apply(done.commands)?;
            crate::project::commit_session(&session, &path)?;
            Ok(
                json!({ "symbol": done.symbol.as_uuid(), "layer": done.layer.as_uuid(),
                       "kind": kind.name() }),
            )
        }
    }
}

/// A bone from the arguments `rig add-bone` takes, with the same checks for a rig file and one in
/// a project.
pub(crate) fn bone_of(
    name: String,
    parent: Option<usize>,
    length: f64,
    rest_angle: f64,
    limit: Option<Vec<f64>>,
    target: Option<Uuid>,
) -> Result<Bone> {
    let limit = match limit.as_deref() {
        Some(&[min, max]) => Some(AngleLimit::new(min, max)?),
        _ => None,
    };
    Ok(Bone {
        name,
        parent,
        length,
        rest_angle,
        limit,
        target: target.map(flare_core::InstanceId::from_uuid),
    })
}

/// A pose key from the arguments `rig set-pose` takes.
pub(crate) fn pose_key(interpolation: &str, rotate: &[String], ik: &[String]) -> Result<PoseKey> {
    let mut key = PoseKey {
        interp: interp(interpolation)?,
        ..PoseKey::default()
    };
    for r in rotate {
        let v = nums(r, 2)?;
        key.pose.rotations.insert(index(v[0])?, v[1]);
    }
    for g in ik {
        let v = nums(g, 4)?;
        key.pose.ik.push(IkGoal {
            tip: index(v[0])?,
            chain: index(v[1])?,
            target: [v[2], v[3]],
        });
    }
    Ok(key)
}

/// A rig's bones at a frame, as the command line and the MCP server print them.
pub(crate) fn eval_json(rig: &Rig, frame: u32) -> Result<Value> {
    let bones: Vec<Value> = evaluate_rig(rig, frame)?
        .iter()
        .zip(&rig.bones)
        .map(|(t, b)| {
            json!({ "name": b.name, "origin": t.origin, "angle": t.angle,
                "length": t.length, "target": t.target.map(|i| *i.as_uuid()) })
        })
        .collect();
    Ok(json!({ "frame": frame, "clock": format!("{:?}", rig.clock), "bones": bones }))
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
    Interp::from_name(name).ok_or_else(|| anyhow!("unknown interpolation {name:?}"))
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
            let bone = bone_of(name, parent, length, rest_angle, limit, target)?;
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
            let key = pose_key(&name, &rotate, &ik)?;
            let rig = rig.apply(&[RigCommand::SetRigPose { frame, key }])?;
            write(&path, &rig)?;
            Ok(json!({ "frame": frame, "keys": rig.keys.len() }))
        }
        RigCmd::Project { op } => crate::rig_project::run(op),
        RigCmd::Eval { rig, frame } => {
            let rig = read(&rig)?;
            eval_json(&rig, frame)
        }
    }
}
