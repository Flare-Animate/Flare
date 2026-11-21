//! Convert to Bitmap with a renderer: `flare_tools::to_bitmap` says what to draw and makes the
//! edit, and this draws it between the two. Every surface calls [`convert_to_bitmap`]; the
//! window runs it on the pixels worker, since drawing and hashing a large picture takes too long
//! for a click (see PERFORMANCE.md).
//!
//! The selection is drawn alone, in a copy of the document whose scene holds nothing else, as the
//! playhead shows it but without the camera, the layer's effects or its mask: those belong to the
//! layer, which the bitmap stays on, so they still apply to it and are not drawn into it twice.

use std::num::NonZeroU32;

use flare_core::{DocumentId, LayerId, Rgba8, SceneId};
use flare_document::{Document, FrameSpan, Layer, Scene, Timeline};
use flare_render::Viewport;
use flare_tools::ops::SpanRef;
use flare_tools::to_bitmap::{self, ToBitmapEdit, ToBitmapError, ToBitmapPlan};

/// The edit that turns `selection` (one keyframe's elements) into one bitmap named `name`, drawn
/// `scale` pixels to a stage unit as it shows at `frame`.
pub fn convert_to_bitmap(
    document: &Document,
    scene: SceneId,
    frame: u32,
    selection: &[SpanRef],
    scale: u32,
    name: &str,
) -> Result<ToBitmapEdit, ToBitmapError> {
    let plan = to_bitmap::plan(document, scene, selection, scale)?;
    let pixels = draw(document, scene, frame, &plan)
        .map_err(|e| ToBitmapError::Bitmap(format!("drawing it failed: {e}")))?;
    to_bitmap::place(document, scene, &plan, &pixels, name)
}

/// The plan's contents drawn over a clear background, straight alpha.
fn draw(
    document: &Document,
    scene: SceneId,
    frame: u32,
    plan: &ToBitmapPlan,
) -> Result<Vec<u8>, String> {
    let (first, rest) = plan.contents.split_first().ok_or("nothing to draw")?;
    // An instance plays its symbol from the start of its keyframe, so the copy's keyframe starts
    // as far before the frame drawn as the original's did.
    let into = frame.saturating_sub(plan.at.start);
    let duration = NonZeroU32::new(into.saturating_add(1)).unwrap_or(NonZeroU32::MIN);
    let span = FrameSpan::new(0, duration, first.clone())
        .with_extras(rest.to_vec())
        .map_err(|e| e.to_string())?;
    let layer = Layer::new(LayerId::new(), "", vec![span]).map_err(|e| e.to_string())?;
    let alone = Document::new(
        DocumentId::new(),
        document.frame_rate(),
        vec![Scene::new(scene, "", Timeline::new(vec![layer]))],
        document.symbols().to_vec(),
    )
    .map_err(|_| "the selection could not be drawn on its own".to_owned())?
    .with_bitmaps(document.bitmaps().to_vec())
    .map_err(|_| "the selection's pictures could not be drawn".to_owned())?
    .with_stage(document.stage());
    let evaluated = flare_eval::evaluate_frame_without_camera(&alone, scene, into)
        .map_err(|e| e.to_string())?;
    let scale = f64::from(plan.scale);
    let viewport = Viewport {
        x: plan.x,
        y: plan.y,
        width: f64::from(plan.width) / scale,
        height: f64::from(plan.height) / scale,
    };
    let image = flare_render::render_viewport(
        &evaluated,
        &viewport,
        plan.width,
        plan.height,
        Rgba8::TRANSPARENT,
    )
    .map_err(|e| e.to_string())?;
    Ok(image.rgba().to_vec())
}

#[cfg(test)]
#[path = "to_bitmap_tests.rs"]
mod tests;
