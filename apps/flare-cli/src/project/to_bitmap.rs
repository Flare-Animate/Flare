//! `flare convert-to-bitmap`: Modify > Convert to Bitmap from the command line. The elements of
//! one keyframe (all of them, or those named) become one bitmap of how they look, through
//! `flare_runtime::convert_to_bitmap`, the call the desktop's menu makes.

use anyhow::{Context, Result, anyhow};
use clap::Args;
use flare_tools::to_bitmap;
use serde_json::{Value, json};
use std::path::PathBuf;

use super::*;

/// What `flare convert-to-bitmap` takes.
#[derive(Args)]
pub struct ToBitmapArgs {
    pub path: PathBuf,
    /// The layer whose keyframe is converted, from `add-layer` or `info` (default: the first).
    #[arg(long)]
    pub layer: Option<uuid::Uuid>,
    /// A frame of the keyframe; what it shows there is drawn.
    #[arg(long, default_value_t = 0)]
    pub frame: u32,
    /// An element of the keyframe to convert, counting from the bottom one at 0; repeat for
    /// more. Without it every element of the keyframe is converted.
    #[arg(long)]
    pub element: Vec<usize>,
    /// Bitmap pixels to a stage unit along each side, 1 to 4.
    #[arg(long, default_value_t = 1)]
    pub scale: u32,
}

/// Converts the keyframe's elements to one bitmap, as one undo step, and saves the project.
pub fn convert_to_bitmap(args: &ToBitmapArgs) -> Result<Value> {
    let mut session = open(&args.path)?;
    let document = session.document();
    let scene = document.scenes().first().context("document has no scene")?;
    let layer = match args.layer {
        Some(id) => Some(
            scene
                .timeline()
                .layers()
                .iter()
                .find(|l| *l.id().as_uuid() == id)
                .with_context(|| format!("no layer {id} in the first scene"))?
                .id(),
        ),
        None => None,
    };
    let chosen = (!args.element.is_empty()).then_some(args.element.as_slice());
    let label = |e: to_bitmap::ToBitmapError| anyhow!("Convert to Bitmap: {e}");
    let selection = to_bitmap::keyframe_selection(document, scene.id(), layer, args.frame, chosen)
        .map_err(label)?;
    let edit = flare_runtime::convert_to_bitmap(
        document,
        scene.id(),
        args.frame,
        &selection,
        args.scale,
        "Bitmap",
    )
    .map_err(label)?;
    let at = edit.at;
    session.apply(edit.commands)?;
    commit(&session, &args.path)?;
    let (width, height) = session
        .document()
        .bitmap(edit.bitmap)
        .map_or((0, 0), |b| (b.width(), b.height()));
    Ok(json!({
        "converted": selection.len(),
        "layer": at.layer.as_uuid(),
        "start": at.start,
        "element": at.element,
        "bitmap": edit.bitmap.as_uuid().to_string(),
        "width": width,
        "height": height,
    }))
}
