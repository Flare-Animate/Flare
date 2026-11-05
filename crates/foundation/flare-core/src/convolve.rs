//! Convolutions on RGBA8 buffers: arbitrary square kernels with edge clamping. Buffers are
//! row-major, 4 bytes/pixel, like [`crate::image_ops`], and straight alpha throughout.

/// Why a convolution could not run.
#[derive(Debug, PartialEq, Eq)]
pub enum ConvolveError {
    /// The buffer's length does not match `width * height * 4`.
    BadBuffer,
    /// The kernel must be odd-sided and non-empty on both axes (it is given row by row,
    /// `width` wide).
    BadKernel,
}

use ConvolveError as Error;

/// Convolves the buffer with `kernel`, `kw` wide, summing over the whole window. The colour
/// channels are convolved **premultiplied by the alpha** and divided back afterwards, so a
/// half-covered red pixel spreads as red, not as the dark red of an RGB average; the alpha
/// itself spreads linearly. Past the edges the buffer is clamped (a constant image is a
/// fixed point of any kernel), and every answer is clamped at 255.
pub fn convolve(
    buf: &[u8],
    w: usize,
    h: usize,
    kernel: &[f32],
    kw: usize,
) -> Result<Vec<u8>, Error> {
    if buf.len()
        != w.checked_mul(h)
            .and_then(|n| n.checked_mul(4))
            .ok_or(Error::BadBuffer)?
    {
        return Err(Error::BadBuffer);
    }
    if kw == 0 || kw % 2 == 0 || kernel.len() % kw != 0 {
        return Err(Error::BadKernel);
    }
    let kh = kernel.len() / kw;
    if kh == 0 || kh % 2 == 0 {
        // An empty kernel cannot happen past the length divisibility above (a zero-length
        // one would divide), but the even check still needs the length to be there.
        return Err(Error::BadKernel);
    }
    let at_alpha = |x: i64, y: i64| buf[(y as usize * w + x as usize) * 4 + 3] as f32;
    let clamped = |x: i64, y: i64| (x.clamp(0, w as i64 - 1), y.clamp(0, h as i64 - 1));
    let half_w = kw as i64 / 2;
    let half_h = kh as i64 / 2;

    let mut out = vec![0u8; buf.len()];
    for y in 0..h {
        for x in 0..w {
            // The window's weighted sums: the colours premultiplied by their coverage, and
            // the alpha itself. The kernel's weights are applied by hand so that a zero
            // weight never touches a pixel's data in the first place.
            let (mut sum_a, mut sum_r, mut sum_g, mut sum_b) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);
            for dy in -half_h..=half_h {
                for dx in -half_w..=half_w {
                    let (cx, cy) = clamped(x as i64 + dx, y as i64 + dy);
                    let weight = kernel[((dy + half_h) as usize) * kw + (dx + half_w) as usize];
                    if weight == 0.0 {
                        continue;
                    }
                    let base = (cy as usize * w + cx as usize) * 4;
                    let coverage = at_alpha(cx, cy) / 255.0;
                    sum_r += weight * buf[base] as f32 * coverage;
                    sum_g += weight * buf[base + 1] as f32 * coverage;
                    sum_b += weight * buf[base + 2] as f32 * coverage;
                    sum_a += weight * at_alpha(cx, cy);
                }
            }
            let base = (y * w + x) * 4;
            out[base] = unpremultiply(sum_r, sum_a);
            out[base + 1] = unpremultiply(sum_g, sum_a);
            out[base + 2] = unpremultiply(sum_b, sum_a);
            out[base + 3] = sum_a.clamp(0.0, 255.0).round() as u8;
        }
    }
    Ok(out)
}

/// The premultiplied channel average taken back to straight alpha, going by the (not yet
/// rounded) accumulated alpha: with nothing behind it there is no colour to show.
fn unpremultiply(premultiplied: f32, alpha: f32) -> u8 {
    if alpha <= 0.0 {
        return 0;
    }
    (premultiplied * 255.0 / alpha).round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(out: &[u8], w: usize, x: usize, y: usize) -> [u8; 4] {
        let base = (y * w + x) * 4;
        [out[base], out[base + 1], out[base + 2], out[base + 3]]
    }

    #[test]
    fn the_one_by_one_identity_kernel_changes_nothing() {
        let dot = |c: u8| [c, 2u8.saturating_mul(c), 3u8.saturating_mul(c), 4u8.saturating_mul(c)];
        let buf: Vec<u8> = (0..u8::MAX).flat_map(dot).collect();
        assert_eq!(convolve(&buf, 255, 1, &[1.0], 1).unwrap(), buf);
    }

    #[test]
    fn a_kernel_that_sums_to_one_keeps_a_constant_sheet() {
        let sheet = vec![10u8, 20, 30, 200].repeat(9 * 7);
        let blur: Vec<f32> = vec![1.0 / 9.0; 9];
        assert_eq!(convolve(&sheet, 9, 7, &blur, 3).unwrap(), sheet);
        // A sharpening kernel with negatives: -4C + 5C is still C with clamped borders.
        let sharpen: Vec<f32> = [0.0, -1.0, 0.0, -1.0, 5.0, -1.0, 0.0, -1.0, 0.0].to_vec();
        assert_eq!(convolve(&sheet, 9, 7, &sharpen, 3).unwrap(), sheet);
    }

    #[test]
    fn a_half_covered_pixel_spreads_as_itself() {
        // A red pixel at half coverage among transparent black, under a 1/3-each kernel: the
        // colour stays pure red everywhere and the coverage is the pixel share.
        let mut buf = vec![0u8; 5 * 1 * 4];
        buf[0] = 255;
        buf[3] = 128;
        let out = convolve(&buf, 5, 1, &[1.0 / 3.0, 1.0 / 3.0, 1.0 / 3.0], 3).unwrap();
        // At the left edge the window's clamped copies of the pixel add up: 2/3 there, 1/3
        // one over, nothing past the spread.
        assert_eq!(pixels(&out, 5, 0, 0), [255, 0, 0, round(128.0 * 2.0 / 3.0)]);
        assert_eq!(pixels(&out, 5, 1, 0), [255, 0, 0, round(128.0 / 3.0)]);
        assert_eq!(pixels(&out, 5, 2, 0), [0, 0, 0, 0]);
        assert_eq!(pixels(&out, 5, 3, 0), [0, 0, 0, 0]);
        assert_eq!(pixels(&out, 5, 4, 0), [0, 0, 0, 0]);
    }

    /// The rounding the operation itself uses.
    fn round(v: f32) -> u8 {
        (v.round().clamp(0.0, 255.0)) as u8
    }

    #[test]
    fn bad_kernels_and_buffers_are_refused() {
        let kernel: Vec<f32> = vec![1.0; 4];
        assert_eq!(
            convolve(&[0; 8], 2, 1, &kernel, 2),
            Err(Error::BadKernel),
            "even width"
        );
        assert_eq!(
            convolve(&[0; 8], 2, 1, &kernel, 1),
            Err(Error::BadKernel),
            "four is two tall"
        );
        assert_eq!(
            convolve(&[0; 8], 2, 1, &[], 1),
            Err(Error::BadKernel),
            "empty"
        );
        assert_eq!(
            convolve(&[0; 9], 2, 1, &kernel, 2),
            Err(Error::BadBuffer),
            "the buffer comes first"
        );
    }
}
