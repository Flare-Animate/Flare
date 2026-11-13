//! Blur and Smudge from the command line: a stroke given as stage points (and, if wanted, a
//! pressure for each) over a bitmap, laid in one go and saved, as the desktop app's drag does.

use anyhow::{Context, Result, bail};
use clap::Args;
use flare_tools::blend::{self, BlendKind, BlendSettings, InputSample, PointerKind};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::*;

/// What `flare blur` and `flare smudge` take.
#[derive(Args)]
pub struct BlendArgs {
    pub path: PathBuf,
    /// A point of the stroke on the stage, in the first scene; repeat for each point, in order.
    /// The bitmap under the first one is the one worked on.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true, action = clap::ArgAction::Append)]
    pub along: Vec<f64>,
    /// The pen's pressure at each point, 0 to 1, one per --along; without it every point is at
    /// full pressure, as a mouse is.
    #[arg(long, num_args = 1.., value_name = "P")]
    pub pressure: Vec<f32>,
    #[arg(long, default_value_t = 0)]
    pub frame: u32,
    /// Width of the stroke in stage units, 1 to 1000 (Blur starts at 80, Smudge at 30).
    #[arg(long)]
    pub size: Option<f32>,
    /// How far the stroke goes at most, 0 to 100 (%).
    #[arg(long, default_value_t = 100.0)]
    pub strength: f32,
    /// How sharp the edge of the stroke is, 0 to 100 (%) (Blur starts at 0, Smudge at 50).
    #[arg(long)]
    pub hardness: Option<f32>,
    /// Keep every dab full size whatever the pressure.
    #[arg(long)]
    pub no_pressure_size: bool,
    /// Smudge only: how far a picked-up color is carried, 0 to 100 (70 to start).
    #[arg(long)]
    pub stretch: Option<u8>,
}

/// The stroke `args` describe, as samples on the stage.
fn samples(args: &BlendArgs) -> Result<Vec<InputSample>> {
    let points = args.along.as_chunks::<2>().0;
    if !args.pressure.is_empty() && args.pressure.len() != points.len() {
        bail!(
            "give one --pressure for each of the {} points, not {}",
            points.len(),
            args.pressure.len()
        );
    }
    points
        .iter()
        .enumerate()
        .map(|(i, [x, y])| {
            let pressure = args.pressure.get(i).copied().unwrap_or(1.0);
            if !(0.0..=1.0).contains(&pressure) {
                bail!("a pressure is 0 to 1, not {pressure}");
            }
            InputSample::new(*x, *y, pressure, PointerKind::Pen)
                .with_context(|| format!("point {x}, {y} is not a number"))
        })
        .collect()
}

fn settings(kind: BlendKind, args: &BlendArgs) -> Result<BlendSettings> {
    let start = BlendSettings::for_kind(kind);
    if kind == BlendKind::Blur && args.stretch.is_some() {
        bail!("--stretch is for smudge; blur carries no color");
    }
    Ok(BlendSettings {
        size: args.size.unwrap_or(start.size),
        strength: args.strength / 100.0,
        hardness: args.hardness.map_or(start.hardness, |h| h / 100.0),
        pressure_size: !args.no_pressure_size,
        stretch: args.stretch.unwrap_or(start.stretch),
    })
}

/// Blurs or smudges the bitmap under the first point along the stroke, as one undo step, and
/// saves the project.
pub fn blend_stroke(kind: BlendKind, args: &BlendArgs) -> Result<Value> {
    let path: &Path = &args.path;
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let settings = settings(kind, args)?;
    let samples = samples(args)?;
    let name = match kind {
        BlendKind::Blur => "Blur",
        BlendKind::Smudge => "Smudge",
    };
    let edit = blend::rub(
        session.document(),
        scene,
        args.frame,
        kind,
        settings,
        &samples,
    )
    .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
    let at = edit.at;
    session.apply(edit.commands)?;
    commit(&session, path)?;
    Ok(json!({
        "tool": kind.name(),
        "points": samples.len(),
        "layer": at.layer.as_uuid(),
        "start": at.start,
        "element": at.element,
        "bitmap": edit.bitmap.as_uuid().to_string(),
    }))
}
