//! The stage's picture of a frame with transparent layers: each run of shapes and pictures that
//! share a view opacity is drawn on its own, faded as a whole and laid over what came before, so
//! a transparent layer is see-through as one sheet (its strokes do not show its fills through
//! them), as Animate draws a transparent layer. Exports never come here.

use flare_core::Rgba8;
use std::ops::Range;

use flare_eval::{DrawnBitmap, DrawnShape, EffectRun, Frame};

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

/// One drawn thing, or one layer with effects (all it drew, run through its graph as one), in
/// paint order.
enum Item<'f, 'a> {
    Shape(&'f DrawnShape<'a>),
    Bitmap(&'f DrawnBitmap<'a>),
    Effects(&'f EffectRun<'a>),
}

/// The frame's shapes and bitmaps in `shapes` and `bitmaps`, in paint order.
fn items_in<'f, 'a>(
    frame: &'f Frame<'a>,
    shapes: Range<usize>,
    bitmaps: Range<usize>,
    out: &mut Vec<Item<'f, 'a>>,
) {
    let mut bitmaps = frame
        .bitmaps
        .get(bitmaps)
        .unwrap_or_default()
        .iter()
        .peekable();
    for (index, shape) in frame
        .shapes
        .get(shapes.clone())
        .unwrap_or_default()
        .iter()
        .enumerate()
    {
        while let Some(bitmap) = bitmaps.next_if(|b| b.after_shapes <= shapes.start + index) {
            out.push(Item::Bitmap(bitmap));
        }
        out.push(Item::Shape(shape));
    }
    out.extend(bitmaps.map(Item::Bitmap));
}

/// The frame cut into runs of things of one view opacity, in paint order. Every run keeps all
/// of the frame's mask regions, which its shapes' clips point into. A layer with effects goes
/// into one run whole, at the opacity of the first thing it drew.
fn runs<'a>(frame: &Frame<'a>) -> Vec<Run<'a>> {
    let mut items: Vec<Item<'_, 'a>> = Vec::with_capacity(frame.shapes.len() + frame.bitmaps.len());
    let (mut shapes, mut bitmaps) = (0, 0);
    for run in crate::effects::runs(frame) {
        items_in(
            frame,
            shapes..run.shapes.start,
            bitmaps..run.bitmaps.start,
            &mut items,
        );
        items.push(Item::Effects(run));
        (shapes, bitmaps) = (run.shapes.end, run.bitmaps.end);
    }
    items_in(
        frame,
        shapes..frame.shapes.len(),
        bitmaps..frame.bitmaps.len(),
        &mut items,
    );
    let mut runs: Vec<Run<'a>> = Vec::new();
    for item in items {
        let alpha = match &item {
            Item::Shape(s) => s.view_alpha,
            Item::Bitmap(b) => b.view_alpha,
            Item::Effects(run) => frame
                .shapes
                .get(run.shapes.start)
                .filter(|_| !run.shapes.is_empty())
                .map(|s| s.view_alpha)
                .or_else(|| frame.bitmaps.get(run.bitmaps.start).map(|b| b.view_alpha))
                .unwrap_or(1.0),
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
        let push_bitmap = |run: &mut Run<'a>, b: &DrawnBitmap<'a>| {
            let mut b = b.clone();
            b.after_shapes = run.frame.shapes.len();
            run.frame.bitmaps.push(b);
        };
        match item {
            Item::Shape(s) => run.frame.shapes.push(s.clone()),
            Item::Bitmap(b) => push_bitmap(run, b),
            Item::Effects(effects) => {
                let (s0, b0) = (run.frame.shapes.len(), run.frame.bitmaps.len());
                let mut inner = Vec::new();
                items_in(
                    frame,
                    effects.shapes.clone(),
                    effects.bitmaps.clone(),
                    &mut inner,
                );
                for item in inner {
                    match item {
                        Item::Shape(s) => run.frame.shapes.push(s.clone()),
                        Item::Bitmap(b) => push_bitmap(run, b),
                        Item::Effects(_) => {}
                    }
                }
                run.frame.effects.push(EffectRun {
                    shapes: s0..run.frame.shapes.len(),
                    bitmaps: b0..run.frame.bitmaps.len(),
                    ..effects.clone()
                });
            }
        }
    }
    runs
}
