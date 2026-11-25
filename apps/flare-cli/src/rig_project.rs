//! `flare rig project`: the rigs a project file keeps. The behaviour is `flare_editor::rig_library`
//! (the same the Rigs panel and the MCP server use), so each change is one undo step; this parses
//! the arguments and prints.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use flare_core::SymbolId;
use flare_editor::rig_library;
use flare_rig::RigCommand;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::project;
use crate::symbol::{bone_of, eval_json, pose_key};

/// A rig file is small; refuse anything larger before parsing.
const MAX_RIG_FILE_BYTES: u64 = 4 << 20;

#[derive(Subcommand)]
pub enum ProjectRigCmd {
    /// List the project's rigs: the symbol, its bones and its pose keys.
    List { path: PathBuf },
    /// Give a symbol an empty rig, on the clock its kind gives it.
    Create {
        path: PathBuf,
        #[arg(long)]
        symbol: Uuid,
    },
    /// Add a bone to a symbol's rig. `--parent` is an earlier bone's index.
    AddBone {
        path: PathBuf,
        #[arg(long)]
        symbol: Uuid,
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
        path: PathBuf,
        #[arg(long)]
        symbol: Uuid,
        #[arg(long)]
        frame: u32,
        #[arg(long, allow_hyphen_values = true)]
        rotate: Vec<String>,
        #[arg(long, allow_hyphen_values = true)]
        ik: Vec<String>,
        #[arg(long, default_value = "linear")]
        interp: String,
    },
    /// Evaluate a symbol's bones at a frame.
    Eval {
        path: PathBuf,
        #[arg(long)]
        symbol: Uuid,
        #[arg(long, default_value_t = 0)]
        frame: u32,
    },
    /// Take a symbol's rig out of the project.
    Remove {
        path: PathBuf,
        #[arg(long)]
        symbol: Uuid,
    },
    /// Write a symbol's rig as a rig file (the form `flare rig` edits).
    Export {
        path: PathBuf,
        #[arg(long)]
        symbol: Uuid,
        out: PathBuf,
    },
    /// Store a rig file in the project, for the symbol it names, replacing that symbol's rig.
    Import { path: PathBuf, rig: PathBuf },
}

fn read_rig_file(path: &std::path::Path) -> Result<String> {
    let len = std::fs::metadata(path)
        .with_context(|| format!("cannot read {}", path.display()))?
        .len();
    if len > MAX_RIG_FILE_BYTES {
        bail!("{} is too large for a rig file", path.display());
    }
    std::fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))
}

pub fn run(op: ProjectRigCmd) -> Result<Value> {
    match op {
        ProjectRigCmd::List { path } => {
            let session = project::open_session(&path)?;
            let mut rigs = Vec::new();
            for asset in session.document().rig_assets() {
                let rig = rig_library::parse(asset.payload()).map_err(|e| anyhow!("{e}"))?;
                rigs.push(json!({
                    "symbol": asset.symbol().as_uuid(),
                    "bones": rig.bones.len(),
                    "keys": rig.keys.len(),
                    "clock": format!("{:?}", rig.clock),
                }));
            }
            Ok(json!({ "rigs": rigs }))
        }
        ProjectRigCmd::Create { path, symbol } => {
            change(&path, symbol, rig_library::create)?;
            Ok(json!({ "symbol": symbol }))
        }
        ProjectRigCmd::AddBone {
            path,
            symbol,
            name,
            parent,
            length,
            rest_angle,
            limit,
            target,
        } => {
            let bone = bone_of(name, parent, length, rest_angle, limit, target)?;
            let rig = change_edit(&path, symbol, &[RigCommand::AddBone(bone)])?;
            Ok(json!({ "bone": rig.bones.len() - 1 }))
        }
        ProjectRigCmd::SetPose {
            path,
            symbol,
            frame,
            rotate,
            ik,
            interp,
        } => {
            let key = pose_key(&interp, &rotate, &ik)?;
            let rig = change_edit(&path, symbol, &[RigCommand::SetRigPose { frame, key }])?;
            Ok(json!({ "frame": frame, "keys": rig.keys.len() }))
        }
        ProjectRigCmd::Eval {
            path,
            symbol,
            frame,
        } => {
            let session = project::open_session(&path)?;
            let rig = rig_library::find(session.document(), SymbolId::from_uuid(symbol))
                .map_err(|e| anyhow!("{e}"))?
                .ok_or_else(|| anyhow!("{}", rig_library::RigLibraryError::NoRig))?;
            eval_json(&rig, frame)
        }
        ProjectRigCmd::Remove { path, symbol } => {
            change(&path, symbol, rig_library::remove)?;
            Ok(json!({ "removed": symbol }))
        }
        ProjectRigCmd::Export { path, symbol, out } => {
            let session = project::open_session(&path)?;
            let rig = rig_library::find(session.document(), SymbolId::from_uuid(symbol))
                .map_err(|e| anyhow!("{e}"))?
                .ok_or_else(|| anyhow!("{}", rig_library::RigLibraryError::NoRig))?;
            std::fs::write(&out, serde_json::to_vec_pretty(&rig)?)
                .with_context(|| format!("cannot write {}", out.display()))?;
            Ok(json!({ "exported": out.to_string_lossy() }))
        }
        ProjectRigCmd::Import { path, rig } => {
            let rig = rig_library::parse(&read_rig_file(&rig)?).map_err(|e| anyhow!("{e}"))?;
            let mut session = project::open_session(&path)?;
            let command = rig_library::set(session.document(), &rig).map_err(|e| anyhow!("{e}"))?;
            session.apply([command])?;
            project::commit_session(&session, &path)?;
            Ok(json!({ "symbol": rig.symbol.as_uuid(), "bones": rig.bones.len() }))
        }
    }
}

/// Applies the one command `make` builds and writes the project back.
fn change(
    path: &std::path::Path,
    symbol: Uuid,
    make: impl FnOnce(
        &flare_document::Document,
        SymbolId,
    ) -> Result<flare_commands::Command, rig_library::RigLibraryError>,
) -> Result<()> {
    let mut session = project::open_session(path)?;
    let command =
        make(session.document(), SymbolId::from_uuid(symbol)).map_err(|e| anyhow!("{e}"))?;
    session.apply([command])?;
    project::commit_session(&session, path)
}

/// Edits a symbol's rig, writes the project back and returns the rig as it is now.
fn change_edit(
    path: &std::path::Path,
    symbol: Uuid,
    edits: &[RigCommand],
) -> Result<flare_rig::Rig> {
    let mut session = project::open_session(path)?;
    let id = SymbolId::from_uuid(symbol);
    let command = rig_library::edit(session.document(), id, edits).map_err(|e| anyhow!("{e}"))?;
    session.apply([command])?;
    project::commit_session(&session, path)?;
    rig_library::find(session.document(), id)
        .map_err(|e| anyhow!("{e}"))?
        .ok_or_else(|| anyhow!("{}", rig_library::RigLibraryError::NoRig))
}
