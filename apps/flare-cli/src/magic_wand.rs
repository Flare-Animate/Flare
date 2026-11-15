//! `flare magic-wand`: the arguments, turned into the `project::WandRequest` the picking and
//! clearing in `project` run.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::project;

/// What `flare magic-wand` is asked.
#[derive(Args)]
pub struct MagicWandArgs {
    path: PathBuf,
    /// A point on the bitmap; repeat to add the pixels picked from more points.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true, action = clap::ArgAction::Append)]
    at: Vec<f64>,
    #[arg(long, default_value_t = 0)]
    frame: u32,
    /// How far a color may be from the clicked one, 0 to 200.
    #[arg(long, default_value_t = default_threshold(), value_parser = clap::value_parser!(u8).range(0..=200))]
    threshold: u8,
    /// Pick every pixel of a like color in the picture, not only those joined to the point.
    #[arg(long)]
    global: bool,
    /// How soft the edge left behind is: pixels, rough, normal or smooth.
    #[arg(long, default_value = "normal")]
    smoothing: String,
    /// Clear every pixel except the picked ones.
    #[arg(long)]
    keep: bool,
    /// Say how many pixels would be picked and change nothing.
    #[arg(long)]
    dry_run: bool,
}

const fn default_threshold() -> u8 {
    flare_tools::magic_wand::DEFAULT_THRESHOLD
}

/// Picks and clears the pixels `args` asks for.
pub fn run(args: MagicWandArgs) -> Result<Value> {
    project::magic_wand(
        &args.path,
        &project::WandRequest {
            points: args
                .at
                .as_chunks::<2>()
                .0
                .iter()
                .map(|[x, y]| (*x, *y))
                .collect(),
            frame: args.frame,
            threshold: args.threshold,
            contiguous: !args.global,
            smoothing: args.smoothing,
            keep: args.keep,
            dry_run: args.dry_run,
        },
    )
}
