//! The stage's picture of a frame with transparent layers: each run of shapes and pictures that
//! share a view opacity is drawn on its own, faded as a whole and laid over what came before, so
//! a transparent layer is see-through as one sheet (its strokes do not show its fills through
//! them), as Animate draws a transparent layer. Exports never come here.

use flare_core::Rgba8;
use flare_eval::{DrawnBitmap, DrawnShape, Frame};

use crate::{Image, RenderError, Viewport, render_viewport};

/// Whether anything in `frame` is drawn less than fully opaque on the stage.
pub fn has_view_alpha(frame: &Frame<'_>) -> bool {
    frame.shapes.iter().any(|s| s.view_alpha < 1.0)
        || frame.bitmaps.iter().any(|b| b.view_alpha < 1.0)
}

/// [`render_viewport`] for the stage: what transparent layers drew is faded. With nothing
/// transparent it is the same picture.
pub fn render_stage_viewport(
    frame: &Frame<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
    background: Rgba8,
) -> Result<Image, RenderError> {
    if !has_view_alpha(frame) {
        return render_viewport(frame, viewport, width, height, background);
    }
    let mut image = render_viewport(&Frame::default(), viewport, width, height, background)?;
    for run in runs(frame) {
        let mut layer = render_viewport(&run.frame, viewport, width, height, Rgba8::TRANSPARENT)?;
        if run.alpha < 1.0 {
            layer.fade_in_place(run.alpha);
        }
        image.composite(&layer)?;
    }
    Ok(image)
}

struct Run<'a> {
    alpha: f32,
    frame: Frame<'a>,
}

/// One drawn thing, in paint order.
enum Item<'f, 'a> {
    Shape(&'f DrawnShape<'a>),
    Bitmap(&'f DrawnBitmap<'a>),
}

/// The frame cut into runs of things of one view opacity, in paint order. Every run keeps all
/// of the frame's mask regions, which its shapes' clips point into.
fn runs<'a>(frame: &Frame<'a>) -> Vec<Run<'a>> {
    let mut items: Vec<Item<'_, 'a>> = Vec::with_capacity(frame.shapes.len() + frame.bitmaps.len());
    let mut bitmaps = frame.bitmaps.iter().peekable();
    for (index, shape) in frame.shapes.iter().enumerate() {
        while let Some(bitmap) = bitmaps.next_if(|b| b.after_shapes <= index) {
            items.push(Item::Bitmap(bitmap));
        }
        items.push(Item::Shape(shape));
    }
    items.extend(bitmaps.map(Item::Bitmap));
    let mut runs: Vec<Run<'a>> = Vec::new();
    for item in items {
        let alpha = match &item {
            Item::Shape(s) => s.view_alpha,
            Item::Bitmap(b) => b.view_alpha,
        };
        let same = runs
            .last()
            .is_some_and(|r| r.alpha.to_bits() == alpha.to_bits());
        if !same {
            runs.push(Run {
                alpha,
                frame: Frame {
                    clips: frame.clips.clone(),
                    ..Frame::default()
                },
            });
        }
        let Some(run) = runs.last_mut() else { continue };
        match item {
            Item::Shape(s) => run.frame.shapes.push(s.clone()),
            Item::Bitmap(b) => {
                let mut b = b.clone();
                b.after_shapes = run.frame.shapes.len();
                run.frame.bitmaps.push(b);
            }
        }
    }
    runs
}
