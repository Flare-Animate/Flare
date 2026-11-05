//! Which pixels each picture shows, and the forward warp that blends the two at a time `t`.

use super::Raster;
use super::flow::{Flow, sample_flow};

/// Two pixels are the same surface when their offsets agree to within this many pixels (or
/// a tenth of the offset, for long motion).
const CONSISTENCY_PIXELS: f32 = 1.0;
const CONSISTENCY_FRACTION: f32 = 0.1;
/// Largest difference, in premultiplied channel units (0 to 255), between a pixel and the one
/// it is said to match. A vanishing or appearing shape has no match, so it is not shown.
const BRIGHTNESS_TOLERANCE: f32 = 40.0;
/// Below this much weight a target pixel is a hole and takes the nearer picture unwarped.
const HOLE: f32 = 1e-6;

/// Which pixels of `a` and of `b` have a match in the other picture. A pixel is visible when its
/// forward and backward offsets agree and the colours match; the rest are occluded, and do not
/// take part in the blend.
pub(super) fn visibility(
    a: Raster<'_>,
    b: Raster<'_>,
    forward: &Flow,
    backward: &Flow,
) -> (Vec<bool>, Vec<bool>) {
    (
        visible_side(a, b, forward, backward),
        visible_side(b, a, backward, forward),
    )
}

fn visible_side(from: Raster<'_>, to: Raster<'_>, forward: &Flow, backward: &Flow) -> Vec<bool> {
    let (w, h) = (from.width as usize, from.height as usize);
    let mut out = vec![false; w * h];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let [dx, dy] = forward[i];
            let (px, py) = (x as f32 + dx, y as f32 + dy);
            if px < 0.0 || py < 0.0 || px > (w - 1) as f32 || py > (h - 1) as f32 {
                continue;
            }
            let [bx, by] = sample_flow(backward, w, h, px, py);
            let (ex, ey) = (dx + bx, dy + by);
            let tolerance = CONSISTENCY_PIXELS.max(CONSISTENCY_FRACTION * dx.hypot(dy));
            if ex.hypot(ey) > tolerance {
                continue;
            }
            let here = premultiplied(from, x as f32, y as f32);
            let there = premultiplied(to, px, py);
            let mismatch = here
                .iter()
                .zip(there)
                .map(|(p, q)| (p - q).abs())
                .fold(0.0f32, f32::max);
            out[i] = mismatch <= BRIGHTNESS_TOLERANCE;
        }
    }
    out
}

/// The picture at time `t`, built from both sides and their visibility.
pub(super) struct Blend<'a> {
    pub(super) a: Raster<'a>,
    pub(super) b: Raster<'a>,
    pub(super) forward: &'a Flow,
    pub(super) backward: &'a Flow,
    pub(super) visible_a: &'a [bool],
    pub(super) visible_b: &'a [bool],
}

impl Blend<'_> {
    /// Each picture is splatted to its place at `t` with its own weight, `1 - t` for the first
    /// and `t` for the second. The accumulator holds premultiplied colour and weight, so the
    /// result is one weighted average, straight back to straight alpha.
    pub(super) fn frame(&self, t: f32) -> Vec<u8> {
        let (w, h) = (self.a.width as usize, self.a.height as usize);
        let mut acc = vec![0.0f32; w * h * 5];
        if t < 1.0 {
            splat(self.a, self.forward, self.visible_a, t, 1.0 - t, &mut acc);
        }
        if t > 0.0 {
            splat(self.b, self.backward, self.visible_b, 1.0 - t, t, &mut acc);
        }
        // A hole nobody covers shows the nearer picture where it stands.
        let nearer = if t < 0.5 { self.a } else { self.b };
        let mut out = vec![0u8; w * h * 4];
        for i in 0..w * h {
            let cell = &acc[i * 5..i * 5 + 5];
            let weight = cell[4];
            let pixel = &mut out[i * 4..i * 4 + 4];
            if weight < HOLE {
                pixel.copy_from_slice(&nearer.rgba[i * 4..i * 4 + 4]);
                continue;
            }
            let alpha = cell[3] / weight;
            if alpha < 0.5 {
                continue;
            }
            for k in 0..3 {
                let straight = cell[k] / weight * 255.0 / alpha;
                pixel[k] = to_byte(straight);
            }
            pixel[3] = to_byte(alpha);
        }
        out
    }
}

/// Moves each visible pixel of `src` by `scale * flow` and adds it, weighted by `coef` (bilinear
/// over the four pixels it lands between), to the accumulator.
fn splat(src: Raster<'_>, flow: &Flow, visible: &[bool], scale: f32, coef: f32, acc: &mut [f32]) {
    let (w, h) = (src.width as usize, src.height as usize);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !visible[i] {
                continue;
            }
            let [dx, dy] = flow[i];
            let (px, py) = (x as f32 + scale * dx, y as f32 + scale * dy);
            let premult = premultiplied(src, x as f32, y as f32);
            let (x0, y0) = (px.floor(), py.floor());
            let (fx, fy) = (px - x0, py - y0);
            for (ox, wx) in [(0isize, 1.0 - fx), (1, fx)] {
                for (oy, wy) in [(0isize, 1.0 - fy), (1, fy)] {
                    let weight = coef * wx * wy;
                    let (tx, ty) = (x0 as isize + ox, y0 as isize + oy);
                    if weight <= 0.0 || tx < 0 || ty < 0 || tx >= w as isize || ty >= h as isize {
                        continue;
                    }
                    let j = (ty as usize * w + tx as usize) * 5;
                    for k in 0..4 {
                        acc[j + k] += weight * premult[k];
                    }
                    acc[j + 4] += weight;
                }
            }
        }
    }
}

/// Premultiplied RGBA, 0 to 255, bilinear at a position. Used for matching and for nothing else:
/// the warp itself moves whole pixels.
fn premultiplied(src: Raster<'_>, x: f32, y: f32) -> [f32; 4] {
    let (w, h) = (src.width as usize, src.height as usize);
    let x = x.clamp(0.0, (w - 1) as f32);
    let y = y.clamp(0.0, (h - 1) as f32);
    let (x0, y0) = (x.floor(), y.floor());
    let (fx, fy) = (x - x0, y - y0);
    let (x0, y0) = (x0 as usize, y0 as usize);
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let mut out = [0.0f32; 4];
    for (k, value) in out.iter_mut().enumerate() {
        let at = |xx: usize, yy: usize| {
            let p = &src.rgba[(yy * w + xx) * 4..(yy * w + xx) * 4 + 4];
            let alpha = f32::from(p[3]);
            if k == 3 {
                alpha
            } else {
                f32::from(p[k]) * alpha / 255.0
            }
        };
        let top = at(x0, y0) + (at(x1, y0) - at(x0, y0)) * fx;
        let bottom = at(x0, y1) + (at(x1, y1) - at(x0, y1)) * fx;
        *value = top + (bottom - top) * fy;
    }
    out
}

fn to_byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}
