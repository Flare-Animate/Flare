//! `flare magnet`: the arguments, turned into the `project::MagnetRequest` the pull in `project`
//! runs.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::project;

/// What `flare magnet` is asked.
#[derive(Args)]
pub struct MagnetArgs {
    path: PathBuf,
    /// Where the pull starts, on the stage: the lines and shapes near it bend.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true)]
    from: Vec<f64>,
    /// Where the pull ends: the point at `--from` moves there, the rest of the circle less.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true)]
    to: Vec<f64>,
    /// How far the pull reaches, in stage units, 2 to 1000.
    #[arg(long, default_value_t = flare_tools::magnet::DEFAULT_MAGNET_RADIUS)]
    radius: f64,
    #[arg(long, default_value_t = 0)]
    frame: u32,
    /// Only the shapes on this layer; without it, those on every unlocked layer.
    #[arg(long)]
    layer: Option<uuid::Uuid>,
    /// Say which shapes the pull reaches and change nothing.
    #[arg(long)]
    dry_run: bool,
}

/// Pulls what `args` asks for.
pub fn run(args: MagnetArgs) -> Result<Value> {
    let pair = |v: &[f64]| {
        (
            v.first().copied().unwrap_or(0.0),
            v.get(1).copied().unwrap_or(0.0),
        )
    };
    project::magnet(
        &args.path,
        &project::MagnetRequest {
            from: pair(&args.from),
            to: pair(&args.to),
            radius: args.radius,
            frame: args.frame,
            layer: args.layer,
            dry_run: args.dry_run,
        },
    )
}
