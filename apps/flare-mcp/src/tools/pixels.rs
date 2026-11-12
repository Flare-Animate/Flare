//! Tools that work on the pixels of bitmaps: the Magic Wand.

use flare_document::{Content, Document};
use flare_tools::magic_wand::{self, Pos2, Smoothing, WandSelection, WandSettings};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene};
use crate::params_pixels::MagicWand;
use crate::server::{FlareServer, Outcome, State};

#[tool_router(router = pixel_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Magic Wand: pick the pixels of a bitmap whose color is like the one at each point and clear them (make them transparent), or with `keep` clear all the others, as one undo step: how a background is taken off a picture. The old picture leaves the library when nothing else shows it. `dry_run` only counts the pixels (needs write access otherwise)."
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
