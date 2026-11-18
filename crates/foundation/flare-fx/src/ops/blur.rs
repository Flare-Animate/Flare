//! A Gaussian blur, made of three box blurs along each axis. Each box is a running sum, so the
//! cost is the same whatever the radius. Outside the picture is transparent, so an edge fades
//! out instead of smearing its last pixels.

/// Largest box half-width used, in pixels: well past any picture's side, so a huge radius on a
/// zoomed stage costs no more than a large one.
const MAX_BOX: usize = 8192;

/// The half-widths of three boxes whose repeated blur comes close to a Gaussian of `sigma`
/// pixels (the usual sizing: two boxes of one width and one of the next odd width).
fn boxes(sigma: f64) -> [usize; 3] {
    let n = 3.0;
    let ideal = (12.0 * sigma * sigma / n + 1.0).sqrt();
    let mut lower = ideal.floor() as i64;
    if lower % 2 == 0 {
        lower -= 1;
    }
    let lower = lower.max(1);
    let upper = lower + 2;
    let l = lower as f64;
    let m = ((12.0 * sigma * sigma - n * l * l - 4.0 * n * l - 3.0 * n) / (-4.0 * l - 4.0))
        .round()
        .clamp(0.0, 3.0) as usize;
    let half = |width: i64| (((width - 1) / 2).max(0) as usize).min(MAX_BOX);
    let mut out = [half(upper); 3];
    for slot in out.iter_mut().take(m) {
        *slot = half(lower);
    }
    out
}

/// Blurs `data`, `channels` values a pixel (1 for an alpha mask, 4 for RGBA), by a Gaussian
/// `radius` pixels wide (sigma half of it). Below half a pixel nothing changes.
pub(crate) fn blur(data: &mut [u8], width: usize, height: usize, channels: usize, radius: f64) {
    if !(radius.is_finite() && radius >= 0.5) || width == 0 || height == 0 {
        return;
    }
    let mut scratch = vec![0u8; data.len()];
    for half in boxes(radius / 2.0) {
        if half == 0 {
            continue;
        }
        box_pass(data, &mut scratch, width, height, channels, half, true);
        box_pass(&scratch, data, width, height, channels, half, false);
    }
}

/// One box blur of half-width `half` along rows (`horizontal`) or columns, from `src` into `dst`.
fn box_pass(
    src: &[u8],
    dst: &mut [u8],
    width: usize,
    height: usize,
    channels: usize,
    half: usize,
    horizontal: bool,
) {
    let (lines, length) = if horizontal {
        (height, width)
    } else {
        (width, height)
    };
    let at = |line: usize, i: usize| {
        if horizontal {
            (line * width + i) * channels
        } else {
            (i * width + line) * channels
        }
    };
    let window = (2 * half + 1) as u32;
    let mut sums = [0u32; 4];
    for line in 0..lines {
        sums.iter_mut().for_each(|s| *s = 0);
        // The window before the first pixel holds the first `half` pixels (the rest is outside).
        for i in 0..half.min(length) {
            let p = at(line, i);
            for c in 0..channels {
                sums[c] += u32::from(src[p + c]);
            }
        }
        for i in 0..length {
            if i + half < length {
                let p = at(line, i + half);
                for c in 0..channels {
                    sums[c] += u32::from(src[p + c]);
                }
            }
            let out = at(line, i);
            for c in 0..channels {
                dst[out + c] = ((sums[c] + window / 2) / window) as u8;
            }
            if i >= half {
                let p = at(line, i - half);
                for c in 0..channels {
                    sums[c] -= u32::from(src[p + c]);
                }
            }
        }
    }
}
