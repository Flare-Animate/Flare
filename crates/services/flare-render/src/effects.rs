//! A layer's effects in a render: the layer's shapes are drawn on their own picture, a margin
//! wider than the view so a shadow cast from just outside it still falls inside, the layer's
//! graph is run over that picture, and the middle is laid over the frame where the layer sits.

use std::sync::Arc;

use flare_core::Rgba8;
use flare_eval::{EffectRun, Frame};
use flare_fx::{MAX_REACH_PIXELS, Pixels, Space};
use vello_cpu::kurbo::{Affine, Rect};
use vello_cpu::peniko::{Extend, ImageAlphaType, ImageBrush, ImageQuality, ImageSampler};
use vello_cpu::{ImageSource, Pixmap, RenderContext};

use crate::render::rasterize_part;
use crate::{MAX_DIMENSION, MAX_PIXELS, Viewport};

/// The frame's effect runs that can be drawn: in paint order, inside the frame, none overlapping
/// another. A frame is plain data, so one built by hand with runs out of order loses those runs
/// (their layers are drawn as they are) rather than being drawn twice.
pub(crate) fn runs<'f, 'a>(frame: &'f Frame<'a>) -> impl Iterator<Item = &'f EffectRun<'a>> {
    let (mut shapes, mut bitmaps) = (0, 0);
    frame.effects.iter().filter(move |run| {
        let fits = run.shapes.start >= shapes
            && run.bitmaps.start >= bitmaps
            && run.shapes.start <= run.shapes.end
            && run.bitmaps.start <= run.bitmaps.end
            && run.shapes.end <= frame.shapes.len()
            && run.bitmaps.end <= frame.bitmaps.len();
        if fits {
            (shapes, bitmaps) = (run.shapes.end, run.bitmaps.end);
        }
        fits
    })
}

/// How many pixels beyond each edge the layer's picture takes in: the graph's reach at this
/// zoom, held to [`MAX_REACH_PIXELS`] and to what a picture may be.
fn margin(graph: &flare_fx::EffectGraph, scale: f64, width: u32, height: u32) -> u32 {
    let wanted = (graph.reach() * scale)
        .min(MAX_REACH_PIXELS)
        .ceil()
        .max(0.0) as u32;
    let fits = |m: u32| {
        let (w, h) = (
            u64::from(width) + 2 * u64::from(m),
            u64::from(height) + 2 * u64::from(m),
        );
        w <= u64::from(MAX_DIMENSION) && h <= u64::from(MAX_DIMENSION) && w * h <= MAX_PIXELS
    };
    // Halve the margin until the picture fits; none at all always does (the view itself did).
    let mut m = wanted;
    while m > 0 && !fits(m) {
        m /= 2;
    }
    m
}

/// The layer `run` drew, with its effects, as a picture `width` by `height` of `viewport`.
/// `None` when the layer has no effects, or the graph cannot run (a view with no size), so the
/// layer is drawn plainly.
pub(crate) fn picture(
    frame: &Frame<'_>,
    run: &EffectRun<'_>,
    viewport: &Viewport,
    width: u32,
    height: u32,
) -> Option<Pixmap> {
    let (sx, sy) = (
        f64::from(width) / viewport.width,
        f64::from(height) / viewport.height,
    );
    let graph = run.graph?;
    let scale = (sx * sy).sqrt();
    let m = margin(graph, scale, width, height);
    let (pw, ph) = (width + 2 * m, height + 2 * m);
    let padded = Viewport {
        x: viewport.x - f64::from(m) / sx,
        y: viewport.y - f64::from(m) / sy,
        width: viewport.width + 2.0 * f64::from(m) / sx,
        height: viewport.height + 2.0 * f64::from(m) / sy,
    };
    let layer = rasterize_part(
        frame,
        Some((run.shapes.clone(), run.bitmaps.clone())),
        &padded,
        pw,
        ph,
        Rgba8::TRANSPARENT,
    )
    .ok()?;
    let source = Pixels::new(pw, ph, layer.take_rgba8(ImageAlphaType::AlphaPremultiplied)).ok()?;
    let space = Space {
        scale,
        origin: [
            f64::from(m) - viewport.x * sx,
            f64::from(m) - viewport.y * sy,
        ],
        stage: run.stage,
    };
    let done = flare_fx::run(graph, source, &space, run.frame).ok()?;
    crop(&done, m, width, height)
}

/// The middle `width` by `height` of `picture`, `m` pixels in from each edge.
fn crop(picture: &Pixels, m: u32, width: u32, height: u32) -> Option<Pixmap> {
    let (w16, h16) = (u16::try_from(width).ok()?, u16::try_from(height).ok()?);
    let mut out = Pixmap::new(w16, h16);
    let stride = picture.width() as usize * 4;
    let row = width as usize * 4;
    let data = picture.data();
    let target = out.data_as_u8_slice_mut();
    for y in 0..height as usize {
        let from = (y + m as usize) * stride + m as usize * 4;
        target
            .get_mut(y * row..(y + 1) * row)?
            .copy_from_slice(data.get(from..from + row)?);
    }
    out.recompute_may_have_transparency();
    Some(out)
}

/// Lays `picture` over what `ctx` has drawn so far, pixel for pixel.
pub(crate) fn lay(ctx: &mut RenderContext, picture: Pixmap, width: u32, height: u32) {
    ctx.set_transform(Affine::IDENTITY);
    ctx.set_paint(ImageBrush {
        image: ImageSource::Pixmap(Arc::new(picture)),
        sampler: ImageSampler {
            x_extend: Extend::Pad,
            y_extend: Extend::Pad,
            quality: ImageQuality::Low,
            alpha: 1.0,
        },
    });
    ctx.fill_rect(&Rect::new(0.0, 0.0, f64::from(width), f64::from(height)));
}
