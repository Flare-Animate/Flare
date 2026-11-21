//! Tools that work on the pixels of bitmaps: the Magic Wand, Blur, Smudge and the Raster Brush.

use flare_core::Rgba8;
use flare_document::{Content, Document};
use flare_tools::blend::{self, BlendEdit, BlendKind, BlendSettings, InputSample, PointerKind};
use flare_tools::magic_wand::{self, Pos2, Smoothing, WandSelection, WandSettings};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, color_or, first_scene, target_layer};
use crate::params_pixels::{Blend, MagicWand, RasterBrush};
use crate::server::{FlareServer, Outcome, State};

#[tool_router(router = pixel_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Magic Wand: pick the pixels of a bitmap whose color is like the one at each point and clear them (make them transparent), or with `keep` clear all the others, as one undo step: how a background is taken off a picture. The old picture leaves the library when nothing else shows it. `dry_run` only counts the pixels and needs no write access; clearing does."
    )]
    fn magic_wand(&self, Parameters(args): Parameters<MagicWand>) -> Outcome {
        if args.dry_run.unwrap_or(false) {
            return self.read(|state| {
                let (selection, count) = pick(state, &args)?;
                Ok(report(&selection, count, false, state.session.document()))
            });
        }
        self.write(args.expected_revision, |state| {
            let (selection, count) = pick(state, &args)?;
            let scene = first_scene(state)?;
            let commands = magic_wand::clear(
                state.session.document(),
                scene,
                &selection,
                smoothing(&args)?,
            )
            .map_err(|e| format!("Magic Wand: {e}"))?;
            let revision = apply(state, commands)?;
            let mut out = report(&selection, count, true, state.session.document());
            out["revision"] = revision["revision"].clone();
            Ok(out)
        })
    }

    #[tool(
        description = "Blur: drag a stroke through `points` over a bitmap to soften it, as Clip Studio Paint's Blur does, as one undo step. Going over a place again in one stroke does not soften it further; call again for more. Pressure sets the width unless `pressure_size` is false. The element gets a new picture; the old one leaves the library when nothing else shows it."
    )]
    fn blur(&self, Parameters(args): Parameters<Blend>) -> Outcome {
        self.write(args.expected_revision, |state| {
            blend_stroke(state, BlendKind::Blur, &args)
        })
    }

    #[tool(
        description = "Smudge: drag a stroke through `points` over a bitmap to push its colors along, as a finger through wet paint (Clip Studio Paint's Finger tip), as one undo step. `stretch` sets how far a picked-up color is carried. Pressure sets the width unless `pressure_size` is false."
    )]
    fn smudge(&self, Parameters(args): Parameters<Blend>) -> Outcome {
        self.write(args.expected_revision, |state| {
            blend_stroke(state, BlendKind::Smudge, &args)
        })
    }

    #[tool(
        description = "Raster Brush: paint `color` along a stroke through `points` into a bitmap on a layer, as one undo step: the topmost bitmap of the layer's keyframe under the first point, or a new clear one the size of the stage. `mix` (0 to 100) blends in the colors already there, as wet paint does: 0 lays the paint as it is, 100 only smears what is there; clear pixels give no color. `stretch` sets how far a picked-up color is carried. Pressure sets the width unless `pressure_size` is false. A locked, hidden, folder, mask, guide or camera layer is refused."
    )]
    fn raster_brush(&self, Parameters(args): Parameters<RasterBrush>) -> Outcome {
        self.write(args.expected_revision, |state| paint_stroke(state, &args))
    }
}

/// Paints along the stroke `args` gives and applies it.
fn paint_stroke(state: &mut State, args: &RasterBrush) -> Result<Value, String> {
    let kind = BlendKind::RasterBrush;
    let samples = stroke_samples(kind, &args.points, args.pressure.as_deref())?;
    let start = BlendSettings::for_kind(kind);
    let settings = BlendSettings {
        size: args.size.unwrap_or(start.size),
        strength: args.opacity.map_or(start.strength, |s| s / 100.0),
        hardness: args.hardness.map_or(start.hardness, |h| h / 100.0),
        pressure_size: args.pressure_size.unwrap_or(true),
        stretch: args.stretch.unwrap_or(start.stretch),
        mix: args.mix.map_or(start.mix, |m| m / 100.0),
    };
    let label = |e: String| format!("Raster Brush: {e}");
    let layer = match args.layer.as_deref() {
        Some(text) => {
            flare_tools::ops::Target::Layer(target_layer(state, Some(text)).map_err(label)?)
        }
        None => flare_tools::ops::Target::Default,
    };
    let paint = blend::RasterPaint {
        color: color_or(args.color.as_deref(), Rgba8::opaque(0, 0, 0)).map_err(label)?,
        layer,
        new_layer: "Layer 1".into(),
        bitmap_name: "Painting".into(),
    };
    let scene = first_scene(state)?;
    let edit = blend::lay_paint(
        state.session.document(),
        scene,
        args.frame.unwrap_or(0),
        settings,
        paint,
        &samples,
    )
    .map_err(|e| label(e.to_string()))?;
    applied(state, kind, edit)
}

/// Blurs or smudges along the stroke `args` gives and applies it.
fn blend_stroke(state: &mut State, kind: BlendKind, args: &Blend) -> Result<Value, String> {
    let name = kind.label();
    if !kind.carries_color() && args.stretch.is_some() {
        return Err(format!(
            "{name}: `stretch` is for smudge; blur carries no color"
        ));
    }
    let samples = stroke_samples(kind, &args.points, args.pressure.as_deref())?;
    let start = BlendSettings::for_kind(kind);
    let settings = BlendSettings {
        size: args.size.unwrap_or(start.size),
        strength: args.strength.map_or(start.strength, |s| s / 100.0),
        hardness: args.hardness.map_or(start.hardness, |h| h / 100.0),
        pressure_size: args.pressure_size.unwrap_or(true),
        stretch: args.stretch.unwrap_or(start.stretch),
        mix: start.mix,
    };
    let scene = first_scene(state)?;
    let edit = blend::rub(
        state.session.document(),
        scene,
        args.frame.unwrap_or(0),
        kind,
        settings,
        &samples,
    )
    .map_err(|e| format!("{name}: {e}"))?;
    applied(state, kind, edit)
}

/// The stroke through `points`, with a pressure for each from `pressure` or full pressure.
fn stroke_samples(
    kind: BlendKind,
    points: &[[f64; 2]],
    pressure: Option<&[f32]>,
) -> Result<Vec<InputSample>, String> {
    let name = kind.label();
    if let Some(pressure) = pressure
        && pressure.len() != points.len()
    {
        return Err(format!(
            "{name}: give one pressure for each of the {} points, not {}",
            points.len(),
            pressure.len()
        ));
    }
    points
        .iter()
        .enumerate()
        .map(|(i, [x, y])| {
            let pressure = pressure.and_then(|p| p.get(i)).copied().unwrap_or(1.0);
            if !(0.0..=1.0).contains(&pressure) {
                return Err(format!("{name}: a pressure is 0 to 1, not {pressure}"));
            }
            InputSample::new(*x, *y, pressure, PointerKind::Pen)
                .ok_or_else(|| format!("{name}: point {x}, {y} is not a number"))
        })
        .collect()
}

/// Applies a stroke's edit and says where it went.
fn applied(state: &mut State, kind: BlendKind, edit: BlendEdit) -> Result<Value, String> {
    let (at, bitmap) = (edit.at, edit.bitmap);
    let revision = apply(state, edit.commands)?;
    Ok(json!({
        "tool": kind.name(),
        "layer": at.layer.as_uuid(),
        "start": at.start,
        "element": at.element,
        "bitmap": bitmap.as_uuid().to_string(),
        "revision": revision["revision"].clone(),
    }))
}

fn smoothing(args: &MagicWand) -> Result<Smoothing, String> {
    match args.smoothing.as_deref() {
        None => Ok(Smoothing::Normal),
        Some(word) => Smoothing::parse(word).ok_or_else(|| {
            format!("unknown smoothing {word:?}: use pixels, rough, normal or smooth")
        }),
    }
}

/// What the points pick (all the other pixels with `keep`), and how many pixels the points
/// themselves picked.
fn pick(state: &State, args: &MagicWand) -> Result<(WandSelection, usize), String> {
    let scene = first_scene(state)?;
    let settings = WandSettings {
        threshold: args.threshold.unwrap_or(magic_wand::DEFAULT_THRESHOLD),
        contiguous: args.contiguous.unwrap_or(true),
        smoothing: smoothing(args)?,
    };
    if args.points.len() > magic_wand::MAX_POINTS {
        return Err(format!(
            "Magic Wand: at most {} points",
            magic_wand::MAX_POINTS
        ));
    }
    let points: Vec<Pos2> = args
        .points
        .iter()
        .map(|[x, y]| Pos2::new(*x as f32, *y as f32))
        .collect();
    let keep = args.keep.unwrap_or(false);
    let selection = magic_wand::pick_points(
        state.session.document(),
        scene,
        args.frame.unwrap_or(0),
        &points,
        &settings,
        keep,
    )
    .map_err(|e| format!("Magic Wand: {e}"))?;
    let marked = selection.mask.count();
    let count = if keep {
        selection.mask.pixels().len() - marked
    } else {
        marked
    };
    Ok((selection, count))
}

fn report(selection: &WandSelection, count: usize, cleared: bool, document: &Document) -> Value {
    let at = selection.at;
    let bitmap = document
        .scenes()
        .first()
        .and_then(|scene| flare_tools::ops::find_element(document, scene.id(), at))
        .and_then(|content| match content {
            Content::Bitmap(placed) => Some(placed.bitmap().as_uuid().to_string()),
            _ => None,
        });
    json!({
        "picked": count,
        "cleared": cleared,
        "layer": at.layer.as_uuid(),
        "start": at.start,
        "element": at.element,
        "bitmap": bitmap,
    })
}
