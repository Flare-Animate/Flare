//! `flare vector-erase`: the arguments, turned into the `project::VectorEraseRequest` that
//! `project` runs.

use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Args;
use flare_tools::eraser::vector::VectorEraserType;
use serde_json::Value;

use crate::project;

/// What `flare vector-erase` is asked.
#[derive(Args)]
pub struct VectorEraseArgs {
    path: PathBuf,
    /// segment (each line a round tip along the points touches loses the stretch out to where
    /// other lines cross it), whole (every shape the tip touches), rectangular (the shapes
    /// wholly inside the box between two points) or freehand (those wholly inside the loop
    /// through three or more).
    #[arg(long = "type", default_value = "segment")]
    kind: String,
    /// A point on the stage; repeat for the path, the two corners or the loop.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true, action = clap::ArgAction::Append)]
    point: Vec<f64>,
    /// How wide the round tip is for segment and whole, in stage units, 1 to 400.
    #[arg(long, default_value_t = 20.0)]
    size: f64,
    #[arg(long, default_value_t = 0)]
    frame: u32,
    /// Only the shapes on this layer (or in this folder); without it, every shown, unlocked one.
    #[arg(long)]
    layer: Option<uuid::Uuid>,
}

/// Takes away what `args` asks for.
pub fn run(args: VectorEraseArgs) -> Result<Value> {
    let Some(kind) = VectorEraserType::parse(&args.kind) else {
        bail!(
            "Vector Eraser: no type '{}'; segment, whole, rectangular or freehand",
            args.kind
        );
    };
    project::vector_erase(
        &args.path,
        &project::VectorEraseRequest {
            kind,
            points: args
                .point
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[x, y]| (x, y))
                .collect(),
            size: args.size,
            frame: args.frame,
            layer: args.layer,
        },
    )
}
