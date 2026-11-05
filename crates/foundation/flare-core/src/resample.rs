//! Resampling RGBA8 buffers to a new width and height. Buffers are row-major, 4 bytes/pixel,
//! straight alpha, like [`crate::image_ops`].

/// Why a resample could not run.
#[derive(Debug, PartialEq, Eq)]
pub enum ResampleError {
    /// The buffer's length does not match `width * height * 4`.
    BadBuffer,
    /// A dimension is zero or over the practical bound ([`MAX_DIMENSION`]), which keeps the
    /// position maths in exact f32 across platforms.
    BadDimensions,
}

use ResampleError as Error;

/// The largest width or height a resample accepts: past it, accumulate error sneaks up on
/// the exact positions below.
pub const MAX_DIMENSION: u32 = 8_192;

/// Resamples the buffer to `w x h` by bilinear filtering of the premultiplied colour, so a
/// half-covered red pixel stays red as it shrinks, and the alpha (premultiplied the same
/// way) fades over the transparent neighbours there. Sampling positions are scaled in
/// f64 and converted with `floor` so neighbouring pixels share the same window on every
/// platform. A dimension matches exactly when the target repeats the source size.
pub fn resample(buf: &[u8], w: u32, h: u32, into_w: u32, into_h: u32) -> Result<Vec<u8>, Error> {
    // The shape is judged before the buffer it would need, so a refused size stays
    // refused no matter what buffer came with it.
    if w == 0 || h == 0 || into_w == 0 || into_h == 0 {
        return Err(Error::BadDimensions);
    }
    if w > MAX_DIMENSION || h > MAX_DIMENSION || into_w > MAX_DIMENSION || into_h > MAX_DIMENSION {
        return Err(Error::BadDimensions);
    }
    let (w64, h64) = (u64::from(w), u64::from(h));
    if buf.len() as u64
        != w64
            .checked_mul(h64)
            .and_then(|n| n.checked_mul(4))
            .ok_or(Error::BadBuffer)?
    {
        return Err(Error::BadBuffer);
    }
    let mut out = vec![0u8; into_w as usize * into_h as usize * 4];
    // The scale of source pixels per target one; the half-pixel offset puts samples at the
    // pixels' centres (a shrink averages around the centre, not one side).
    let scale = |target: u32, source: u32| f64::from(source) / f64::from(target);
    for (into_y, row) in out
        .as_chunks_mut::<4>()
        .0
        .chunks_mut(into_w as usize)
        .enumerate()
    {
        let sy = (f64::from(into_y as u32) + 0.5) * scale(into_h, h) - 0.5;
        for (into_x, pixel) in row.iter_mut().enumerate() {
            let sx = (f64::from(into_x as u32) + 0.5) * scale(into_w, w) - 0.5;
            let (mut acc_r, mut acc_g, mut acc_b, mut acc_a) = (0f64, 0f64, 0f64, 0f64);
            // The two source columns and rows that straddle the sample, corner weights
            // outside [0, 1) kept out by the clamps.
            for (dy, wy) in weights(sy, h as i64) {
                for (dx, wx) in weights(sx, w as i64) {
                    let factor = wx * wy;
                    if factor == 0.0 {
                        continue;
                    }
                    let base = ((dy as usize) * (w as usize) + dx as usize) * 4;
                    let coverage = f64::from(buf[base + 3]) / 255.0;
                    acc_r += factor * f64::from(buf[base]) * coverage;
                    acc_g += factor * f64::from(buf[base + 1]) * coverage;
                    acc_b += factor * f64::from(buf[base + 2]) * coverage;
                    acc_a += factor * f64::from(buf[base + 3]);
                }
            }
            let alpha = clamped_alpha(acc_a).round() as u8;
            *pixel = [
                unpremultiply(acc_r, acc_a),
                unpremultiply(acc_g, acc_a),
                unpremultiply(acc_b, acc_a),
                alpha,
            ];
        }
    }
    Ok(out)
}

/// Up to two (position, weight) pairs of the bilinear window at a fractional position,
/// positions clamped into the source; the edge weights went into the pairs themselves.
fn weights(to: f64, bound: i64) -> [(i64, f64); 2] {
    let top = to.floor();
    let over = to - top;
    let top = top.clamp(0.0, (bound - 1) as f64) as i64;
    let next = (top + 1).min(bound - 1).max(top);
    // At the edge the two positions coincide: the whole weight lands on that one source.
    if next == top {
        [(top, 1.0), (next, 0.0)]
    } else {
        [(top, 1.0 - over), (next, over)]
    }
}

/// Anything past 255 is 255 and nothing is below zero: the coverage sum never exceeds the
/// source's own coverage, but the f64 sums keep a little numeric room.
fn clamped_alpha(coverage_or_alpha: f64) -> f64 {
    coverage_or_alpha.clamp(0.0, 255.0)
}

/// The premultiplied colour average taken back to straight alpha by the accumulated alpha;
/// with nothing behind it there is no colour to show.
fn unpremultiply(premultiplied: f64, alpha: f64) -> u8 {
    if alpha <= 0.0 {
        return 0;
    }
    (premultiplied * 255.0 / alpha).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn pixels(out: &[u8], w: u32, x: u32, y: u32) -> [u8; 4] {
        let base = ((y * w + x) * 4) as usize;
        [out[base], out[base + 1], out[base + 2], out[base + 3]]
    }

    #[test]
    fn the_same_size_is_a_copy() {
        let sheet = [10u8, 20, 30, 200].repeat(7 * 5);
        assert_eq!(resample(&sheet, 7, 5, 7, 5).unwrap(), sheet);
    }

    #[test]
    fn a_constant_sheet_stays_constant_under_any_size() {
        let sheet = [10u8, 20, 30, 255].repeat(11 * 13);
        let out = resample(&sheet, 11, 13, 23, 3).unwrap();
        let mut all: Vec<u8> = Vec::new();
        for chunk in out.as_chunks::<4>().0 {
            all.extend_from_slice(chunk);
        }
        for (i, b) in all.iter().enumerate() {
            assert_eq!(*b, [10, 20, 30, 255][i % 4], "at {i}");
        }
    }

    #[test]
    fn a_shrink_averages_the_premultiplied_colours() {
        // A fully opaque red half-sheet over transparent black, halved horizontally: each
        // target column covers one red and one transparent source pixel evenly.
        let mut buf = vec![0u8; 4 * 4 * 4];
        for y in 0..4 {
            buf[(y * 4) * 4..(y * 4) * 4 + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
        let out = resample(&buf, 4, 4, 2, 4).unwrap();
        for y in 0..4 {
            let got = pixels(&out, 2, 0, y);
            assert_eq!(got[0], 255, "still red");
            assert_eq!(got[3], 128, "half the coverage");
            assert_eq!(pixels(&out, 2, 1, y), [0, 0, 0, 0]);
        }
    }

    #[test]
    fn broken_input_is_refused() {
        assert_eq!(resample(&[0; 9], 4, 4, 2, 2), Err(Error::BadBuffer));
        assert_eq!(resample(&[0; 16], 0, 4, 2, 2), Err(Error::BadDimensions));
        assert_eq!(resample(&[0; 16], 4, 4, 0, 2), Err(Error::BadDimensions));
        assert_eq!(
            resample(&[0; (16 * 16 * 4) as usize], 16, 16, 16, MAX_DIMENSION + 1),
            Err(Error::BadDimensions)
        );
    }
}
