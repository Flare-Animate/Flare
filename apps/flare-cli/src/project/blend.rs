//! Blur, Smudge, and the Brush and Eraser on a raster layer from the command line: a stroke given
//! as stage points (and, if wanted, a pressure for each), laid in one go and saved, as the desktop
//! app's drag does.

use anyhow::{Context, Result, bail};
use clap::Args;
use flare_tools::blend::{
    self, BlendEdit, BlendKind, BlendSettings, InputSample, PointerKind, RasterPaint, RasterPreset,
};
use flare_tools::ops::Target;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::*;

/// What `flare blur`, `flare smudge` and `flare raster-brush` take.
#[derive(Args)]
pub struct BlendArgs {
    pub path: PathBuf,
    /// A point of the stroke on the stage, in the first scene; repeat for each point, in order.
    /// Blur and Smudge work on the bitmap under the first one.
    #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true, required = true, action = clap::ArgAction::Append)]
    pub along: Vec<f64>,
    /// The pen's pressure at each point, 0 to 1, one per --along; without it every point is at
    /// full pressure, as a mouse is.
    #[arg(long, num_args = 1.., value_name = "P")]
    pub pressure: Vec<f32>,
    #[arg(long, default_value_t = 0)]
    pub frame: u32,
    /// Width of the stroke in stage units, 1 to 1000 (Blur starts at 80, Smudge at 30, the Raster
    /// Brush and the eraser at 20).
    #[arg(long)]
    pub size: Option<f32>,
    /// How far the stroke goes at most, 0 to 100 (%); for the Raster Brush, its opacity (100
    /// to start, or the preset's).
    #[arg(long)]
    pub strength: Option<f32>,
    /// How sharp the edge of the stroke is, 0 to 100 (%) (Blur starts at 0, Smudge at 50, the
    /// Raster Brush and the eraser at 100).
    #[arg(long)]
    pub hardness: Option<f32>,
    /// Keep every dab full size whatever the pressure.
    #[arg(long)]
    pub no_pressure_size: bool,
    /// Smudge and Raster Brush: how far a picked-up color is carried, 0 to 100 (Smudge starts at
    /// 70, the Raster Brush at 30).
    #[arg(long)]
    pub stretch: Option<u8>,
}

/// What `flare raster-brush` takes besides the stroke.
#[derive(Args)]
pub struct RasterBrushArgs {
    #[command(flatten)]
    pub stroke: BlendArgs,
    /// Paint color RRGGBB or RRGGBBAA.
    #[arg(long, default_value = "000000")]
    pub color: String,
    /// How much of the color under the brush mixes into the paint, 0 to 100 (%) (0 to start,
    /// or the preset's); 0 lays the paint as it is.
    #[arg(long)]
    pub mix: Option<f32>,
    /// Start from a preset: round, soft-round, pencil, ink, marker, airbrush, wet-paint or
    /// blender. The other options change it.
    #[arg(long)]
    pub preset: Option<String>,
    /// The layer painted on; by default the topmost that can take art, or a new "Layer 1".
    #[arg(long)]
    pub layer: Option<uuid::Uuid>,
}

/// What `flare raster-erase` takes besides the stroke.
#[derive(Args)]
pub struct RasterEraseArgs {
    #[command(flatten)]
    pub stroke: BlendArgs,
    /// The layer rubbed out on; by default the topmost picture under the first point.
    #[arg(long)]
    pub layer: Option<uuid::Uuid>,
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

fn settings(
    kind: BlendKind,
    args: &BlendArgs,
    mix: Option<f32>,
    preset: Option<&str>,
) -> Result<BlendSettings> {
    let start = match preset {
        Some(name) => RasterPreset::parse(name)
            .with_context(|| {
                let known: Vec<&str> = RasterPreset::ALL.iter().map(|p| p.name()).collect();
                format!("unknown preset {name:?}; one of {}", known.join(", "))
            })?
            .settings(),
        None => BlendSettings::for_kind(kind),
    };
    if !kind.carries_color() && args.stretch.is_some() {
        bail!(
            "--stretch is for smudge and raster-brush; {} carries no color",
            kind.name()
        );
    }
    Ok(BlendSettings {
        size: args.size.unwrap_or(start.size),
        strength: args.strength.map_or(start.strength, |s| s / 100.0),
        hardness: args.hardness.map_or(start.hardness, |h| h / 100.0),
        pressure_size: !args.no_pressure_size,
        stretch: args.stretch.unwrap_or(start.stretch),
        mix: mix.map_or(start.mix, |m| m / 100.0),
    })
}

/// Blurs or smudges the bitmap under the first point along the stroke, as one undo step, and
/// saves the project.
pub fn blend_stroke(kind: BlendKind, args: &BlendArgs) -> Result<Value> {
    let settings = settings(kind, args, None, None)?;
    let samples = samples(args)?;
    lay(kind, args, samples.len(), |document, scene| {
        blend::rub(document, scene, args.frame, kind, settings, &samples)
    })
}

/// The layer `id` names in the first scene of `document`.
fn named_layer(
    document: &flare_document::Document,
    id: uuid::Uuid,
) -> Result<flare_core::LayerId, blend::BlendError> {
    document
        .scenes()
        .first()
        .and_then(|s| {
            s.timeline()
                .layers()
                .iter()
                .find(|l| *l.id().as_uuid() == id)
        })
        .map(|l| l.id())
        .ok_or(blend::BlendError::Refused(
            flare_tools::ops::DrawRefusal::NoSuchLayer,
        ))
}

/// Rubs pixels out along the stroke, as the Eraser does on a raster layer, as one undo step, and
/// saves the project.
pub fn raster_erase(args: &RasterEraseArgs) -> Result<Value> {
    let kind = BlendKind::Erase;
    let settings = settings(kind, &args.stroke, None, None)?;
    let samples = samples(&args.stroke)?;
    lay(kind, &args.stroke, samples.len(), |document, scene| {
        let layer = args.layer.map(|id| named_layer(document, id)).transpose()?;
        blend::rub_out(
            document,
            scene,
            args.stroke.frame,
            settings,
            layer,
            &samples,
        )
    })
}

/// Paints along the stroke with the Raster Brush, as one undo step, and saves the project.
pub fn raster_brush(args: &RasterBrushArgs) -> Result<Value> {
    let kind = BlendKind::RasterBrush;
    let settings = settings(kind, &args.stroke, args.mix, args.preset.as_deref())?;
    let samples = samples(&args.stroke)?;
    let color = parse_color(&args.color)?;
    lay(kind, &args.stroke, samples.len(), |document, scene| {
        let layer = match args.layer {
            Some(id) => Target::Layer(named_layer(document, id)?),
            None => Target::Default,
        };
        let paint = RasterPaint {
            color,
            layer,
            new_layer: "Layer 1".into(),
            bitmap_name: "Painting".into(),
        };
        blend::lay_paint(
            document,
            scene,
            args.stroke.frame,
            settings,
            paint,
            &samples,
        )
    })
}

/// Opens the project, makes the edit `make` gives for its first scene, applies and saves it.
fn lay(
    kind: BlendKind,
    args: &BlendArgs,
    points: usize,
    make: impl FnOnce(
        &flare_document::Document,
        flare_core::SceneId,
    ) -> Result<BlendEdit, blend::BlendError>,
) -> Result<Value> {
    let path: &Path = &args.path;
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let edit =
        make(session.document(), scene).map_err(|e| anyhow::anyhow!("{}: {e}", kind.label()))?;
    let at = edit.at;
    session.apply(edit.commands)?;
    commit(&session, path)?;
    Ok(json!({
        "tool": kind.name(),
        "points": points,
        "layer": at.layer.as_uuid(),
        "start": at.start,
        "element": at.element,
        "bitmap": edit.bitmap.as_uuid().to_string(),
    }))
}
