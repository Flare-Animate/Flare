//! `flare video-import`: a video brought into a project to draw over. What the import makes and
//! the checks are `flare_tools::video`'s, and the reading is `flare_import`'s through the
//! ffmpeg on the PATH; this reads the arguments, writes the project and prints.

use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use flare_tools::video::{Asked, plan, plan_json};
use serde_json::Value;

/// What `flare video-import` is asked.
#[derive(Args)]
pub struct VideoImportArgs {
    path: PathBuf,
    video: PathBuf,
    /// Pictures to take from each second of the video; the document's rate by default, which
    /// keeps the video's timing.
    #[arg(long)]
    fps: Option<f64>,
    /// The most of the video to read, from its start, in minutes (up to 60); as much of the
    /// first minute as fits by default.
    #[arg(long)]
    max_minutes: Option<f64>,
    /// The pictures' longest side in pixels, 16 to 3840 (960 by default); never enlarged.
    #[arg(long)]
    max_side: Option<u32>,
    /// How opaque the stage draws the layer while editing, 0 to 100 (50 by default).
    #[arg(long)]
    opacity: Option<u8>,
    /// The frame the video starts on (zero based).
    #[arg(long, default_value_t = 0)]
    frame: u32,
    /// The layer's and the symbol's name; the video's file name by default.
    #[arg(long)]
    name: Option<String>,
    /// Only print what the import would make; read nothing and change nothing.
    #[arg(long)]
    plan: bool,
}

pub fn run(args: VideoImportArgs) -> Result<Value> {
    let asked = Asked {
        fps: args.fps,
        max_minutes: args.max_minutes,
        max_side: args.max_side,
        opacity: args.opacity,
    };
    let mut session = crate::project::open_session(&args.path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("the document has no scene")?
        .id();
    let source = flare_import::probe_video(&args.video).map_err(|e| anyhow!(e))?;
    let settings = asked
        .settings(session.document(), &source)
        .map_err(|e| anyhow!(e))?;
    let plan = plan(session.document(), &source, &settings).map_err(|e| anyhow!(e))?;
    let mut report = plan_json(&plan);
    if args.plan {
        return Ok(report);
    }
    if let Some(why) = plan.too_large {
        bail!(
            "the video is too large to import: {why}; lower --max-minutes (at most {:.2} fit) or --max-side{}",
            plan.fitting_seconds / 60.0,
            plan.fitting_side
                .map_or_else(String::new, |side| format!(" (at most {side} fits)")),
        );
    }
    let name = args.name.unwrap_or_else(|| {
        args.video
            .file_stem()
            .map_or_else(|| "Video".to_owned(), |n| n.to_string_lossy().into_owned())
    });
    let placed = flare_import::import_video(
        session.document(),
        scene,
        args.frame,
        &name,
        &args.video,
        &plan,
    )
    .map_err(|e| anyhow!(e))?;
    session.apply(placed.commands)?;
    crate::project::commit_session(&session, &args.path)?;
    report["layer"] = serde_json::json!(placed.layer.as_uuid());
    report["symbol"] = serde_json::json!(placed.symbol.as_uuid());
    report["frame"] = serde_json::json!(args.frame);
    report["frames"] = serde_json::json!(placed.frames);
    report["distinct_pictures"] = serde_json::json!(placed.distinct);
    Ok(report)
}
