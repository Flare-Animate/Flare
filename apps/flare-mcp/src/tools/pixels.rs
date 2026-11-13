//! Tools that work on the pixels of bitmaps: the Magic Wand, Blur and Smudge.

use flare_document::{Content, Document};
use flare_tools::blend::{self, BlendKind, BlendSettings, InputSample, PointerKind};
use flare_tools::magic_wand::{self, Pos2, Smoothing, WandSelection, WandSettings};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene};
use crate::params_pixels::{Blend, MagicWand};
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
}

/// Blurs or smudges along the stroke `args` gives and applies it.
fn blend_stroke(state: &mut State, kind: BlendKind, args: &Blend) -> Result<Value, String> {
    let name = match kind {
        BlendKind::Blur => "Blur",
        BlendKind::Smudge => "Smudge",
    };
    if kind == BlendKind::Blur && args.stretch.is_some() {
        return Err("Blur: `stretch` is for smudge; blur carries no color".into());
    }
    if let Some(pressure) = &args.pressure
        && pressure.len() != args.points.len()
    {
        return Err(format!(
            "{name}: give one pressure for each of the {} points, not {}",
            args.points.len(),
            pressure.len()
        ));
    }
    let samples = args
        .points
        .iter()
        .enumerate()
        .map(|(i, [x, y])| {
            let pressure = args.pressure.as_ref().and_then(|p| p.get(i)).copied();
            let pressure = pressure.unwrap_or(1.0);
            if !(0.0..=1.0).contains(&pressure) {
                return Err(format!("{name}: a pressure is 0 to 1, not {pressure}"));
            }
            InputSample::new(*x, *y, pressure, PointerKind::Pen)
                .ok_or_else(|| format!("{name}: point {x}, {y} is not a number"))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let start = BlendSettings::for_kind(kind);
    let settings = BlendSettings {
        size: args.size.unwrap_or(start.size),
        strength: args.strength.map_or(start.strength, |s| s / 100.0),
        hardness: args.hardness.map_or(start.hardness, |h| h / 100.0),
        pressure_size: args.pressure_size.unwrap_or(true),
        stretch: args.stretch.unwrap_or(start.stretch),
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
