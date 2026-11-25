//! `flare stop-motion-project`: the stop-motion session a project file keeps. The behaviour is
//! `flare_editor::stop_motion_library` (the same the Stop Motion panel and the MCP server use), so
//! each change is one undo step. A picture is a PNG file added as the session's next frame; no
//! camera backend is built (see `stop-motion-capture`), so frames come from files, a watched
//! folder's output or any other tool that writes pictures.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Subcommand;
use flare_editor::stop_motion_library as library;
use serde_json::{Value, json};

use crate::project;

/// A picture file is refused past this size before it is read.
const MAX_PICTURE_BYTES: u64 = 64 << 20;

#[derive(Subcommand)]
pub enum ProjectCommands {
    /// Start the project's session: captures a second, and the hold each new frame gets.
    Start {
        path: PathBuf,
        #[arg(long, default_value_t = 12)]
        fps: u32,
        #[arg(long, default_value_t = 1)]
        hold: u32,
    },
    /// Add a PNG file as the session's next frame.
    Add {
        path: PathBuf,
        picture: PathBuf,
        /// Timeline frames it is held for; the session's hold without it.
        #[arg(long)]
        hold: Option<u32>,
    },
    /// List the session: its rate and each frame's picture and hold.
    List { path: PathBuf },
    /// Hold frame INDEX (from 0) for HOLD timeline frames.
    Hold {
        path: PathBuf,
        index: usize,
        hold: u32,
    },
    /// Take frame INDEX out of the session (its picture stays in the library).
    Remove { path: PathBuf, index: usize },
    /// Change the capture rate and the hold new frames get.
    Rate {
        path: PathBuf,
        #[arg(long)]
        fps: u32,
        #[arg(long)]
        hold: u32,
    },
    /// Lay the frames on a new layer of the first scene, each held for its hold.
    Lay {
        path: PathBuf,
        #[arg(long, default_value = "Stop motion")]
        layer_name: String,
    },
    /// End the session (the pictures stay in the library).
    End { path: PathBuf },
}

fn fail(error: library::StopMotionLibraryError) -> anyhow::Error {
    anyhow!("{error}")
}

pub fn run(op: ProjectCommands) -> Result<Value> {
    match op {
        ProjectCommands::Start { path, fps, hold } => {
            edit(&path, |d| Ok(vec![library::start(d, fps, hold)?]))?;
            Ok(json!({ "fps": fps, "hold": hold }))
        }
        ProjectCommands::Add {
            path,
            picture,
            hold,
        } => {
            if project::open_session(&path)?
                .document()
                .stop_motion()
                .is_none()
            {
                bail!("{}", library::StopMotionLibraryError::NotStarted);
            }
            let len = std::fs::metadata(&picture)
                .with_context(|| format!("cannot read {}", picture.display()))?
                .len();
            if len > MAX_PICTURE_BYTES {
                bail!("{} is too large for a frame", picture.display());
            }
            let bytes = std::fs::read(&picture)
                .with_context(|| format!("cannot read {}", picture.display()))?;
            let name = picture
                .file_name()
                .map_or_else(|| "frame".to_owned(), |n| n.to_string_lossy().into_owned());
            let bitmap = flare_import::import_bitmap(&name, &bytes)?;
            let session = edit(&path, |d| library::add_frame(d, bitmap, hold))?;
            Ok(json!({ "frames": session.map_or(0, |s| s.frames().len()) }))
        }
        ProjectCommands::List { path } => {
            let session = project::open_session(&path)?;
            let Some(s) = session.document().stop_motion() else {
                bail!("{}", library::StopMotionLibraryError::NotStarted);
            };
            let frames: Vec<Value> = s
                .frames()
                .iter()
                .enumerate()
                .map(|(i, f)| json!({ "index": i, "bitmap": f.bitmap().as_uuid(), "hold": f.hold() }))
                .collect();
            Ok(json!({
                "fps": s.fps(),
                "hold": s.default_hold(),
                "duration": s.duration(),
                "frames": frames,
            }))
        }
        ProjectCommands::Hold { path, index, hold } => {
            edit(&path, |d| Ok(vec![library::set_hold(d, index, hold)?]))?;
            Ok(json!({ "index": index, "hold": hold }))
        }
        ProjectCommands::Remove { path, index } => {
            let session = edit(&path, |d| Ok(vec![library::remove_frame(d, index)?]))?;
            Ok(json!({ "frames": session.map_or(0, |s| s.frames().len()) }))
        }
        ProjectCommands::Rate { path, fps, hold } => {
            edit(&path, |d| Ok(vec![library::set_rate(d, fps, hold)?]))?;
            Ok(json!({ "fps": fps, "hold": hold }))
        }
        ProjectCommands::Lay { path, layer_name } => {
            let mut session = project::open_session(&path)?;
            let (scene, _) = project::first_scene_layer(&session, None)?;
            let (layer, commands) =
                library::lay_on_timeline(session.document(), scene, &layer_name).map_err(fail)?;
            session.apply(commands)?;
            project::commit_session(&session, &path)?;
            Ok(json!({ "layer": layer.as_uuid() }))
        }
        ProjectCommands::End { path } => {
            edit(&path, |d| Ok(vec![library::end(d)?]))?;
            Ok(json!({ "ended": true }))
        }
    }
}

/// Applies the commands `make` builds and writes the project back; returns the session as it is
/// now, `None` once it has ended.
fn edit(
    path: &std::path::Path,
    make: impl FnOnce(
        &flare_document::Document,
    ) -> Result<Vec<flare_commands::Command>, library::StopMotionLibraryError>,
) -> Result<Option<flare_document::StopMotion>> {
    let mut session = project::open_session(path)?;
    let commands = make(session.document()).map_err(fail)?;
    session.apply(commands)?;
    project::commit_session(&session, path)?;
    Ok(session.document().stop_motion().cloned())
}
