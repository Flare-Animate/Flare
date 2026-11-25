//! `flare draw-rect`: the Rectangle tool on the command line.

use anyhow::{Context, Result, bail};
use clap::Args;
use flare_document::Content;
use flare_tools::ops::{self, Target};
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::PathBuf;

use super::{commit, open, rect_content, rect_style};

/// Draw a filled rectangle into a layer of the first scene, as the desktop app does.
#[derive(Args)]
pub struct DrawRectArgs {
    path: PathBuf,
    #[arg(long, num_args = 4, value_names = ["X", "Y", "W", "H"], allow_negative_numbers = true, required = true)]
    rect: Vec<f64>,
    #[arg(long, default_value = "000000")]
    color: String,
    /// Outline color RRGGBB or RRGGBBAA (no outline when omitted).
    #[arg(long)]
    stroke: Option<String>,
    /// Outline width in document units.
    #[arg(long, default_value_t = 2.0)]
    stroke_width: f64,
    /// Outline pattern: solid, dashed, dotted, stipple, hatched, ragged, herringbone, rope or zigzag.
    #[arg(long)]
    stroke_style: Option<String>,
    #[arg(long, default_value_t = 0)]
    start: u32,
    #[arg(long, default_value_t = 1)]
    duration: u32,
    #[arg(long)]
    layer: Option<uuid::Uuid>,
    /// Merge Drawing: the rectangle cuts and joins the layer's merged drawing, and its fill and
    /// outline become two elements, as with Object Drawing off. Without it, a drawing object.
    #[arg(long)]
    merge: bool,
}

/// Draws a filled rectangle into a layer of the first scene, as the desktop app does: it joins the
/// layer's keyframe at `start` as one more element, or starts a keyframe of `duration` frames
/// there. The layer is the one named, else the topmost that is not a folder or the camera; a
/// document with none gets "Layer 1". A locked, hidden, folder, mask, guide or camera layer, or a
/// keyframe that tweens, is refused.
pub fn draw_rect(args: DrawRectArgs) -> Result<Value> {
    let [x, y, w, h] = args.rect[..] else {
        bail!("--rect takes X Y W H");
    };
    let style = rect_style(
        &args.color,
        args.stroke.as_deref(),
        args.stroke_width,
        args.stroke_style.as_deref(),
    )?;
    let mut session = open(&args.path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let duration = NonZeroU32::new(args.duration).context("duration must be at least 1")?;
    let Content::Shape(shape) = rect_content([x, y, w, h], style)? else {
        bail!("a rectangle is a shape");
    };
    let target = match args.layer {
        Some(id) => Target::Layer(
            session.document().scenes()[0]
                .timeline()
                .layers()
                .iter()
                .find(|candidate| *candidate.id().as_uuid() == id)
                .with_context(|| format!("no layer {id} in the first scene"))?
                .id(),
        ),
        None => Target::Default,
    };
    let plan = ops::draw_shapes(
        session.document(),
        scene,
        target,
        args.start,
        Some(duration),
        vec![shape.merged(args.merge)],
        "Layer 1",
    )
    .context("cannot draw")?;
    let elements: Vec<usize> = plan.placed.iter().map(|at| at.element).collect();
    session.apply(plan.commands)?;
    commit(&session, &args.path)?;
    Ok(json!({
        "drawn": true,
        "layer": plan.layer.as_uuid(),
        "element": elements.first().copied().unwrap_or(0),
        "elements": elements,
    }))
}
