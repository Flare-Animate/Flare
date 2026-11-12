//! The Magic Wand from the command line: pick the pixels of a bitmap by color from points on the
//! stage, and clear them (or every other pixel), as the desktop app's wand and Delete do.

use anyhow::{Context, Result};
use flare_tools::magic_wand::{self, Pos2, Smoothing, WandSettings};
use serde_json::{Value, json};
use std::path::Path;

use super::*;

/// What `flare magic-wand` was asked.
pub struct WandRequest {
    /// Stage points; the first picks and the others add to it, all on one bitmap.
    pub points: Vec<(f64, f64)>,
    pub frame: u32,
    pub threshold: u8,
    pub contiguous: bool,
    pub smoothing: String,
    /// Clear every pixel but the picked ones.
    pub keep: bool,
    /// Report what would be picked and change nothing.
    pub dry_run: bool,
}

pub fn magic_wand(path: &Path, request: &WandRequest) -> Result<Value> {
    let mut session = open(path)?;
    let scene = session
        .document()
        .scenes()
        .first()
        .context("document has no scene")?
        .id();
    let smoothing = Smoothing::parse(&request.smoothing).with_context(|| {
        format!(
            "unknown smoothing {:?}: use pixels, rough, normal or smooth",
            request.smoothing
        )
    })?;
    let settings = WandSettings {
        threshold: request.threshold,
        contiguous: request.contiguous,
        smoothing,
    };
    let points: Vec<Pos2> = request
        .points
        .iter()
        .map(|(x, y)| Pos2::new(*x as f32, *y as f32))
        .collect();
    let selection = magic_wand::pick_points(
        session.document(),
        scene,
        request.frame,
        &points,
        &settings,
        request.keep,
    )
    .map_err(|e| anyhow::anyhow!("Magic Wand: {e}"))?;
    // What the points picked, whichever side of it is cleared.
    let count = if request.keep {
        selection.mask.pixels().len() - selection.mask.count()
    } else {
        selection.mask.count()
    };
    let at = selection.at;
    let report = |cleared: bool, bitmap: Option<String>| {
        json!({
            "picked": count,
            "cleared": cleared,
            "layer": at.layer.as_uuid(),
            "start": at.start,
            "element": at.element,
            "bitmap": bitmap,
        })
    };
    if request.dry_run {
        return Ok(report(false, None));
    }
    let commands = magic_wand::clear(session.document(), scene, &selection, smoothing)
        .map_err(|e| anyhow::anyhow!("Magic Wand: {e}"))?;
    session.apply(commands)?;
    commit(&session, path)?;
    let now = flare_tools::ops::find_element(session.document(), scene, at).and_then(|c| match c {
        Content::Bitmap(b) => Some(b.bitmap().as_uuid().to_string()),
        _ => None,
    });
    Ok(report(true, now))
}
