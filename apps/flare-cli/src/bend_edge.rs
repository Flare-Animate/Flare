//! `flare bend-edge`: the arguments, turned into the `project::BendEdgeRequest` the reshape in
//! `project` runs.

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;
use serde_json::Value;

use crate::project;

/// What `flare bend-edge` is asked.
#[derive(Args)]
pub struct BendEdgeArgs {
    path: PathBuf,
    /// Where the edge is taken hold of, on the stage: the merged line or shape edge nearest it.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true)]
    from: Vec<f64>,
    /// Where the grabbed point is dragged to; the edge curves through it, its ends stay.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true)]
    to: Vec<f64>,
    /// How far from an edge a grab still takes hold of it, in stage units, 0 to 100.
    #[arg(long, default_value_t = flare_tools::reshape::DEFAULT_TOLERANCE)]
    tolerance: f64,
    #[arg(long, default_value_t = 0)]
    frame: u32,
    /// Only the shapes on this layer; without it, those on every unlocked layer.
    #[arg(long)]
    layer: Option<uuid::Uuid>,
    /// Say which edge would be taken and change nothing.
    #[arg(long)]
    dry_run: bool,
}

/// Bends what `args` asks for.
pub fn run(args: BendEdgeArgs) -> Result<Value> {
    let pair = |v: &[f64]| {
        (
            v.first().copied().unwrap_or(0.0),
            v.get(1).copied().unwrap_or(0.0),
        )
    };
    project::bend_edge(
        &args.path,
        &project::BendEdgeRequest {
            from: pair(&args.from),
            to: pair(&args.to),
            tolerance: args.tolerance,
            frame: args.frame,
            layer: args.layer,
            dry_run: args.dry_run,
        },
    )
}
