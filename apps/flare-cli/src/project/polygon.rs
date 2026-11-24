//! `flare draw-polygon`: the Polygon tool on the command line. The corners given become one
//! closed shape (`flare_tools::polygon`), drawn into a layer of the first scene as `draw-rect`
//! draws a rectangle.

use anyhow::{Context, Result, bail};
use clap::Args;
use flare_document::Stroke;
use flare_tools::ops::{self, Target};
use serde_json::{Value, json};
use std::num::NonZeroU32;
use std::path::PathBuf;

use super::{commit, open, rect_style, target_layer};

/// Draw a filled shape with straight sides through the corners given, in order, as the Polygon
/// tool does: one undo step, joining the layer's keyframe at --start or starting one there.
#[derive(Args)]
pub struct DrawPolygonArgs {
    path: PathBuf,
    /// A corner, X Y; give at least three, in order around the outline.
    #[arg(long = "point", num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true)]
    points: Vec<f64>,
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
    /// Merge Drawing: the shape cuts and joins the layer's merged drawing, and its fill and
    /// outline become two elements, as with Object Drawing off. Without it, a drawing object.
    #[arg(long)]
    merge: bool,
}

pub fn draw_polygon(args: DrawPolygonArgs) -> Result<Value> {
    if !args.points.len().is_multiple_of(2) {
        bail!("each --point takes an X and a Y");
    }
    let corners: Vec<(f64, f64)> = args.points.chunks(2).map(|c| (c[0], c[1])).collect();
    let style = rect_style(
        &args.color,
        args.stroke.as_deref(),
        args.stroke_width,
        args.stroke_style.as_deref(),
    )?;
    let stroke = style
        .stroke
        .map(|(width, color)| Stroke::new(width, color).map(|s| s.with_style(style.stroke_style)))
        .transpose()?;
    let shape = flare_tools::polygon::polygon_shape(&corners, style.fill, stroke)?;
    let mut session = open(&args.path)?;
    let (scene, target) = match args.layer {
        Some(id) => {
            let (scene, layer) = target_layer(&session, Some(id))?;
            (scene, Target::Layer(layer))
        }
        None => {
            let scene = session
                .document()
                .scenes()
                .first()
                .context("document has no scene")?
                .id();
            (scene, Target::Default)
        }
    };
    let duration = NonZeroU32::new(args.duration).context("duration must be at least 1")?;
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
    let element = elements.first().copied().unwrap_or(0);
    session.apply(plan.commands)?;
    commit(&session, &args.path)?;
    Ok(json!({
        "drawn": true,
        "layer": plan.layer.as_uuid(),
        "element": element,
        "elements": elements,
        "corners": corners.len(),
    }))
}
