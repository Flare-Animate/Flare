//! `flare brush`: brush strokes laid on a layer from stage points, as the desktop's Brush lays
//! a drag when the pen lifts. One stroke comes from `--along`, many from a JSON file; either way
//! they are one undo step. What the settings mean and what is refused is
//! `flare_tools::brush`'s, the same the desktop app and the MCP server use.

use anyhow::{Context, Result, anyhow, bail};
use clap::Args;
use flare_tools::brush::{self, BrushRequest, Placement};
use flare_tools::ops::Target;
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::PathBuf;

use super::*;

/// The largest strokes file read.
const MAX_STROKES_FILE_BYTES: u64 = 64 * 1024 * 1024;

/// What `flare brush` takes.
#[derive(Args)]
pub struct BrushArgs {
    pub path: PathBuf,
    /// A point of the stroke on the stage, in the first scene; repeat for each point, in order.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, action = clap::ArgAction::Append, conflicts_with = "strokes")]
    pub along: Vec<f64>,
    /// The pen's pressure at each --along point, 0 to 1; without it every point is at full
    /// pressure, as a mouse is.
    #[arg(long, num_args = 1.., value_name = "P", requires = "along")]
    pub pressure: Vec<f32>,
    /// A JSON file of many strokes, laid in one go: `[[[x, y], [x, y, pressure], ...], ...]`,
    /// each stroke a list of points of two numbers, or three with the pressure.
    #[arg(long, value_name = "FILE")]
    pub strokes: Option<PathBuf>,
    /// Width at full pressure in stage units, 1 to 200.
    #[arg(long, default_value_t = 12.0)]
    pub size: f64,
    /// Paint color RRGGBB or RRGGBBAA.
    #[arg(long, default_value = "000000")]
    pub color: String,
    /// A ready-made brush: plain (the default), pencil, ink, grease-pencil, calligraphy, vector,
    /// animate, g-pen, mapping-pen, turnip-pen, brush-pen, dry-brush, bristle, charcoal, chalk
    /// or crayon.
    #[arg(long)]
    pub profile: Option<String>,
    /// How opaque the paint is, 0 to 100 (%); the profile's when omitted.
    #[arg(long)]
    pub flow: Option<f32>,
    /// Samples averaged to steady the hand, 0 (off) to 40; the profile's when omitted. It
    /// averages over the points given as it does over a pen's samples, so a stroke of a few
    /// far-apart points is pulled straight: give points a few units apart, or 0.
    #[arg(long)]
    pub stabilizer: Option<u8>,
    /// How long the start and the end of each stroke taper, in stage units.
    #[arg(long, num_args = 2, value_names = ["START", "END"])]
    pub taper: Option<Vec<f64>>,
    /// How far the centre line may be simplified, in stage units, 0 to 100 (the desktop's 2 at
    /// 100% zoom when omitted).
    #[arg(long)]
    pub smoothing: Option<f64>,
    /// The frame drawn on.
    #[arg(long, default_value_t = 0)]
    pub frame: u32,
    /// The length of a new keyframe; by default it runs to the end of the scene, as the
    /// desktop's does.
    #[arg(long)]
    pub duration: Option<u32>,
    /// The layer drawn on; by default the topmost that can take art, or a new "Layer 1".
    #[arg(long)]
    pub layer: Option<uuid::Uuid>,
    /// Merge Drawing: the strokes cut and join the layer's merged drawing, as with Object
    /// Drawing off; a stroke of the same colour over another becomes one with it.
    #[arg(long)]
    pub merge: bool,
}

/// The strokes `args` give: one from `--along`, or every one in the `--strokes` file.
fn strokes(args: &BrushArgs) -> Result<Vec<Vec<brush::InputSample>>> {
    let raw: Vec<Vec<Vec<f64>>> = match &args.strokes {
        Some(file) => {
            let size = std::fs::metadata(file)
                .with_context(|| format!("cannot read {}", file.display()))?
                .len();
            if size > MAX_STROKES_FILE_BYTES {
                bail!(
                    "{} is {size} bytes, more than the {MAX_STROKES_FILE_BYTES} a strokes file may be",
                    file.display()
                );
            }
            let text = std::fs::read_to_string(file)
                .with_context(|| format!("cannot read {}", file.display()))?;
            serde_json::from_str(&text).with_context(|| {
                format!(
                    "{} is not a list of strokes, each a list of [x, y] or [x, y, pressure]",
                    file.display()
                )
            })?
        }
        None => {
            let points = args.along.as_chunks::<2>().0;
            if points.is_empty() {
                bail!("give the stroke as --along X Y points, or many strokes with --strokes FILE");
            }
            if !args.pressure.is_empty() && args.pressure.len() != points.len() {
                bail!(
                    "give one --pressure for each of the {} points, not {}",
                    points.len(),
                    args.pressure.len()
                );
            }
            let stroke = points
                .iter()
                .enumerate()
                .map(|(i, [x, y])| match args.pressure.get(i) {
                    Some(p) => vec![*x, *y, f64::from(*p)],
                    None => vec![*x, *y],
                })
                .collect();
            vec![stroke]
        }
    };
    raw.iter()
        .enumerate()
        .map(|(i, points)| brush::stroke_samples(i, points).map_err(|e| anyhow!(e)))
        .collect()
}

/// Lays the strokes `args` describe on a layer of the first scene, as one undo step, and saves
/// the project.
pub fn brush_strokes(args: &BrushArgs) -> Result<Value> {
    let request = BrushRequest {
        profile: args.profile.clone(),
        size: args.size,
        color: parse_color(&args.color)?,
        flow: args.flow,
        stabilizer: args.stabilizer,
        taper: args.taper.as_ref().map(|t| (t[0], t[1])),
        smoothing: args.smoothing,
    };
    let (stroke, paint) = request.resolve().map_err(|e| anyhow!(e))?;
    let strokes = strokes(args)?;
    let mut session = open(&args.path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?;
    let target = match args.layer {
        Some(id) => {
            let layer = scene
                .timeline()
                .layers()
                .iter()
                .find(|candidate| *candidate.id().as_uuid() == id)
                .with_context(|| format!("no layer {id} in the first scene"))?;
            // On a raster layer the Brush paints pixels, which `raster-brush` does.
            if layer.is_raster() {
                anyhow::bail!(
                    "layer \"{}\" is a raster layer: paint on it with `flare raster-brush`",
                    layer.name()
                );
            }
            Target::Layer(layer.id())
        }
        None => Target::Default,
    };
    let duration = match args.duration {
        Some(frames) => Some(NonZeroU32::new(frames).context("duration must be at least 1")?),
        None => None,
    };
    let placement = Placement {
        scene: scene.id(),
        target,
        frame: args.frame,
        duration,
        merge: args.merge,
    };
    let plan = brush::lay_strokes(session.document(), placement, &stroke, paint, &strokes)
        .map_err(|e| anyhow!("Brush: {e}"))?;
    let elements: Vec<usize> = plan.draw.placed.iter().map(|at| at.element).collect();
    session.apply(plan.draw.commands)?;
    commit(&session, &args.path)?;
    Ok(json!({
        "drawn": elements.len(),
        "layer": plan.draw.layer.as_uuid(),
        "elements": elements,
        "skipped": plan.skipped,
    }))
}
