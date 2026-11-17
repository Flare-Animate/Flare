//! Drop shadow and glow: the picture's shape, softened, filled with one color and laid under the
//! picture. Only the alpha is blurred, a quarter of the work of blurring the colors.

use flare_core::Rgba8;

use super::blur::blur;
use super::composite::over_in_place;
use crate::Pixels;

/// The picture over a copy of its shape moved by `offset` pixels, blurred by `radius`, in
/// `color` at `opacity` (0 to 1, a glow's strength may go to 4).
pub(crate) fn shadow(
    input: &Pixels,
    offset: [f64; 2],
    radius: f64,
    color: Rgba8,
    opacity: f64,
) -> Pixels {
    let (w, h) = (input.width() as usize, input.height() as usize);
    let mut mask = shifted(&input.alpha(), w, h, offset);
    blur(&mut mask, w, h, 1, radius);
    let strength = opacity * f64::from(color.a) / 255.0;
    let mut out = Vec::with_capacity(w * h * 4);
    for a in mask {
        // The shape's coverage times the strength, which a glow lets go past 1.
        let a = (f64::from(a) * strength).round().clamp(0.0, 255.0);
        let mul = |c: u8| (f64::from(c) * a / 255.0).round() as u8;
        out.extend_from_slice(&[mul(color.r), mul(color.g), mul(color.b), a as u8]);
    }
    let mut under = Pixels::transparent(input.width(), input.height());
    under.data_mut().copy_from_slice(&out);
    over_in_place(&mut under, input);
    under
}

/// The mask moved by `offset` pixels, sampled between pixels so a shadow moves smoothly.
/// Outside the mask is empty.
fn shifted(mask: &[u8], w: usize, h: usize, offset: [f64; 2]) -> Vec<u8> {
    let [dx, dy] = offset;
    if dx == 0.0 && dy == 0.0 {
        return mask.to_vec();
    }
    let (fx, fy) = (dx.floor(), dy.floor());
    let (tx, ty) = (dx - fx, dy - fy);
    let (ix, iy) = (fx as i64, fy as i64);
    let at = |x: i64, y: i64| -> f64 {
        if x < 0 || y < 0 || x >= w as i64 || y >= h as i64 {
            0.0
        } else {
            f64::from(mask[y as usize * w + x as usize])
        }
    };
    let mut out = vec![0u8; mask.len()];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            // The source pixel is (x - dx, y - dy): between four pixels when the offset is not whole.
            let (sx, sy) = (x - ix, y - iy);
            let top = at(sx, sy) * (1.0 - tx) + at(sx - 1, sy) * tx;
            let bottom = at(sx, sy - 1) * (1.0 - tx) + at(sx - 1, sy - 1) * tx;
            let value = top * (1.0 - ty) + bottom * ty;
            out[y as usize * w + x as usize] = value.round().clamp(0.0, 255.0) as u8;
        }
    }
    out
}
