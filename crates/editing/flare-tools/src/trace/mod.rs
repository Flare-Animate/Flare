//! Trace a picture into filled shapes: how a raster layer becomes a vector one. The picture's
//! pixels are gathered into a few colors, each color's pixels are outlined (the Paint Bucket's
//! tracer, `paint::area`) and the outline filled with the color's average.
//!
//! Neighbouring colors would each be outlined on their own, and the small straightening of each
//! outline leaves hairline seams between them. So the colors are stacked instead: the one covering
//! the most pixels is drawn first, outlining every colored pixel, the next outlines every pixel
//! but the first color's, and so on, each drawn over the ones before. Every border between two
//! colors is then inside the shape under it, and only the picture's outer edge is traced as an
//! edge.

mod keyframe;

use flare_core::Rgba8;
use flare_document::{Bitmap, BitmapInstance, Shape};
use thiserror::Error;

use crate::paint::area::{Outline, simplify, trace};
use crate::paint::{Fill, area_shape};

pub use keyframe::trace_keyframe;

/// Most colors a picture is traced in: past this the colors are gathered more coarsely.
pub const MAX_COLORS: usize = 48;
/// The color difference to start from: the sum of the differences of red, green, blue and alpha.
pub const DEFAULT_THRESHOLD: u32 = 60;
/// A pixel at least this opaque is part of the picture; one fainter is left clear, so an outline
/// runs through the middle of an anti-aliased edge.
const OPAQUE: u8 = 128;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TraceError {
    #[error("the picture is too detailed to trace")]
    TooDetailed,
    #[error("the keyframe is tweened, so what it shows changes from frame to frame")]
    Tweened,
    #[error("there is no such keyframe")]
    Missing,
    #[error("the traced shapes are more than a keyframe can hold")]
    TooManyShapes,
    #[error("a traced shape could not be made: {0}")]
    Shape(String),
}

/// One color of a picture and the pixels counted in it.
struct Swatch {
    /// The first pixel's color, which the others are measured against.
    seed: [u8; 4],
    sum: [u64; 4],
    count: u64,
}

impl Swatch {
    fn average(&self) -> Rgba8 {
        let c = |i: usize| (self.sum[i] / self.count.max(1)) as u8;
        Rgba8::new(c(0), c(1), c(2), c(3))
    }
}

fn distance(a: [u8; 4], b: [u8; 4]) -> u32 {
    a.iter().zip(b).map(|(x, y)| u32::from(x.abs_diff(y))).sum()
}

/// Each pixel's swatch (`u8::MAX` for clear), and the swatches, gathered within `threshold`;
/// `None` when that makes more than [`MAX_COLORS`].
fn gather(pixels: &[[u8; 4]], threshold: u32) -> Option<(Vec<u8>, Vec<Swatch>)> {
    let mut swatches: Vec<Swatch> = Vec::new();
    let mut labels = vec![u8::MAX; pixels.len()];
    let mut last = 0usize;
    for (pixel, label) in pixels.iter().zip(labels.iter_mut()) {
        if pixel[3] < OPAQUE {
            continue;
        }
        // Neighbouring pixels are mostly the same color: try the last swatch first.
        let found = if swatches
            .get(last)
            .is_some_and(|s| distance(s.seed, *pixel) <= threshold)
        {
            Some(last)
        } else {
            swatches
                .iter()
                .position(|s| distance(s.seed, *pixel) <= threshold)
        };
        let index = match found {
            Some(index) => index,
            None => {
                if swatches.len() >= MAX_COLORS {
                    return None;
                }
                swatches.push(Swatch {
                    seed: *pixel,
                    sum: [0; 4],
                    count: 0,
                });
                swatches.len() - 1
            }
        };
        let swatch = &mut swatches[index];
        for (s, c) in swatch.sum.iter_mut().zip(pixel) {
            *s += u64::from(*c);
        }
        swatch.count += 1;
        *label = index as u8;
        last = index;
    }
    Some((labels, swatches))
}

/// The colors of `bitmap` and the outlines that fill each, in pixel coordinates of the bitmap,
/// in the order they are drawn: each covers its own pixels and those of every color after it.
/// Nothing for a clear picture.
pub fn trace_colors(bitmap: &Bitmap) -> Result<Vec<(Rgba8, Vec<Outline>)>, TraceError> {
    let (w, h) = (bitmap.width() as usize, bitmap.height() as usize);
    let all = bitmap.pixels().as_chunks::<4>().0;
    // Only the box holding the picture is worked on: a painting is mostly clear.
    let Some([left, top, right, bottom]) = opaque_box(all, w, h) else {
        return Ok(Vec::new());
    };
    let (bw, bh) = (right - left, bottom - top);
    let mut pixels = Vec::with_capacity(bw * bh);
    for row in top..bottom {
        pixels.extend_from_slice(&all[row * w + left..row * w + right]);
    }
    let mut threshold = DEFAULT_THRESHOLD;
    let (labels, swatches) = loop {
        if let Some(found) = gather(&pixels, threshold) {
            break found;
        }
        // The widest difference is 1020: by then every pixel is one color.
        threshold = (threshold * 2).min(1020);
    };
    let mut order: Vec<usize> = (0..swatches.len()).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(swatches[i].count));
    let mut rank = vec![0usize; swatches.len()];
    for (place, &index) in order.iter().enumerate() {
        rank[index] = place;
    }
    let mut traced = Vec::with_capacity(order.len());
    for (place, &index) in order.iter().enumerate() {
        let set: Vec<bool> = labels
            .iter()
            .map(|&l| l != u8::MAX && rank[usize::from(l)] >= place)
            .collect();
        let loops = trace(&set, bw, bh).ok_or(TraceError::TooDetailed)?;
        let outlines: Vec<Outline> = loops
            .iter()
            .filter_map(simplify)
            .map(|o| {
                o.into_iter()
                    .map(|(x, y)| (x + left as f64, y + top as f64))
                    .collect()
            })
            .collect();
        if !outlines.is_empty() {
            traced.push((swatches[index].average(), outlines));
        }
    }
    Ok(traced)
}

/// The shapes that draw the picture `placed` shows, on the stage, bottom first.
pub fn trace_bitmap(bitmap: &Bitmap, placed: &BitmapInstance) -> Result<Vec<Shape>, TraceError> {
    let matrix = placed.transform().as_affine();
    trace_colors(bitmap)?
        .into_iter()
        .map(|(color, outlines)| {
            let to_stage = |(x, y): (f64, f64)| {
                let p = matrix.transform_point2(glamx::DVec2::new(x, y));
                (p.x, p.y)
            };
            let faded = Rgba8::new(
                color.r,
                color.g,
                color.b,
                (f32::from(color.a) * placed.opacity()).round() as u8,
            );
            area_shape(&outlines, to_stage, Fill::Solid(faded))
                .ok_or_else(|| TraceError::Shape("a point is not on the stage".into()))
        })
        .collect()
}

/// The smallest box, `[left, top, right, bottom)`, holding every pixel opaque enough to trace.
fn opaque_box(pixels: &[[u8; 4]], w: usize, h: usize) -> Option<[usize; 4]> {
    let mut found: Option<[usize; 4]> = None;
    for row in 0..h {
        let line = &pixels[row * w..(row + 1) * w];
        let Some(first) = line.iter().position(|p| p[3] >= OPAQUE) else {
            continue;
        };
        let last = line.iter().rposition(|p| p[3] >= OPAQUE).unwrap_or(first);
        found = Some(match found {
            None => [first, row, last + 1, row + 1],
            Some([l, t, r, _]) => [l.min(first), t, r.max(last + 1), row + 1],
        });
    }
    found
}

#[cfg(test)]
mod tests;
