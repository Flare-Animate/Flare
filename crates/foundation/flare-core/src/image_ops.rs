//! Raster image operations on RGBA8 buffers: gamma and levels, dilate/erode and box blur, and
//! - our own, not ports of the OpenToonz kernels (see PORTING_TRACKER.md) - a despeckle, a
//!   distance transform, and a step antialias. Buffers are row-major, 4 bytes/pixel.

/// Error for buffers whose length does not match `width * height * 4`.
#[derive(Debug, PartialEq, Eq)]
pub struct BadBuffer;

fn check(buf: &[u8], w: usize, h: usize) -> Result<(), BadBuffer> {
    if buf.len()
        == w.checked_mul(h)
            .and_then(|n| n.checked_mul(4))
            .ok_or(BadBuffer)?
    {
        Ok(())
    } else {
        Err(BadBuffer)
    }
}

/// Gamma-correct RGB in place (alpha untouched). `gamma` > 0; 1.0 is identity.
pub fn gamma_correct(buf: &mut [u8], gamma: f32) {
    let inv = 1.0 / gamma.max(f32::EPSILON);
    let lut: Vec<u8> = (0..256)
        .map(|i| {
            ((i as f32 / 255.0).powf(inv) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8
        })
        .collect();
    for px in buf.as_chunks_mut::<4>().0 {
        for c in &mut px[..3] {
            *c = lut[*c as usize];
        }
    }
}

/// Levels: map `[black, white]` to `[0, 255]` on RGB in place (alpha untouched).
pub fn levels(buf: &mut [u8], black: u8, white: u8) {
    let (b, w) = (black as f32, (white.max(black.saturating_add(1))) as f32);
    for px in buf.as_chunks_mut::<4>().0 {
        for c in &mut px[..3] {
            *c = (((*c as f32 - b) / (w - b)) * 255.0)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
}

fn morph(buf: &[u8], w: usize, h: usize, r: usize, grow: bool) -> Result<Vec<u8>, BadBuffer> {
    check(buf, w, h)?;
    let mut out = buf.to_vec();
    for y in 0..h {
        for x in 0..w {
            let (mut best, mut ch) = (buf[(y * w + x) * 4 + 3], [0u8; 4]);
            ch.copy_from_slice(&buf[(y * w + x) * 4..][..4]);
            for ny in y.saturating_sub(r)..=(y + r).min(h - 1) {
                for nx in x.saturating_sub(r)..=(x + r).min(w - 1) {
                    let p = &buf[(ny * w + nx) * 4..][..4];
                    if (grow && p[3] > best) || (!grow && p[3] < best) {
                        best = p[3];
                        ch.copy_from_slice(p);
                    }
                }
            }
            out[(y * w + x) * 4..][..4].copy_from_slice(&ch);
        }
    }
    Ok(out)
}

/// Dilate by alpha: each pixel takes the neighbour (radius `r`) with max alpha.
pub fn dilate(buf: &[u8], w: usize, h: usize, r: usize) -> Result<Vec<u8>, BadBuffer> {
    morph(buf, w, h, r, true)
}

/// Erode by alpha: each pixel takes the neighbour (radius `r`) with min alpha.
pub fn erode(buf: &[u8], w: usize, h: usize, r: usize) -> Result<Vec<u8>, BadBuffer> {
    morph(buf, w, h, r, false)
}

/// Connected runs of fully opaque pixels that share one RGB: the base of despeckle's removal.
fn same_colour(a: &[u8], b: &[u8]) -> bool {
    a[..3] == b[..3] && a[3] == b[3]
}

/// Removes colour specks: every connected (by the four directions) set of fully opaque pixels
/// that share one colour and cover at most `max_area` pixels is cleared to transparent. Big
/// strokes and fills are left as they are, whatever their shape.
pub fn despeckle(buf: &[u8], w: usize, h: usize, max_area: usize) -> Result<Vec<u8>, BadBuffer> {
    check(buf, w, h)?;
    let mut out = buf.to_vec();
    // One scan of every pixel, a stack no longer than a component: bounded memory.
    let mut seen = vec![false; w * h];
    let at = |i: usize| &buf[i * 4..i * 4 + 4];
    for start in 0..w * h {
        let base = at(start);
        if seen[start] || base[3] != 255 {
            continue;
        }
        let mut stack = vec![start];
        let mut component = Vec::new();
        seen[start] = true;
        while let Some(i) = stack.pop() {
            component.push(i);
            let x = i % w;
            let y = i / w;
            for (nx, ny) in [
                (x.saturating_sub(1), y),
                ((x + 1).min(w - 1), y),
                (x, y.saturating_sub(1)),
                (x, (y + 1).min(h - 1)),
            ] {
                let j = ny * w + nx;
                if !seen[j] && same_colour(base, at(j)) {
                    seen[j] = true;
                    stack.push(j);
                }
            }
        }
        if component.len() <= max_area {
            for i in component {
                out[i * 4 + 3] = 0;
            }
        }
    }
    Ok(out)
}

/// The squared distance, in one dimension, from every position to the nearest zero of `f`
/// (Felzenszwalb and Huttenlocher's exact transform). `f` holds 0 at the positions measured
/// from and one finite "too far to matter" value everywhere else, so every answer is finite:
/// callers pick the finite value from the size of their image.
fn squared_distance_to_zero(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    if n == 0 {
        return Vec::new();
    }
    // The lower envelope of the parabolas `f[i] + (x - i)²`: `v[k]` a position and `z[k]` the
    // front where it starts winning over `v[k - 1]` (`z[0]` is never read). The envelope has
    // one entry beyond the last placed position; reading past it means infinity, next.
    let intersect = |wide: f64, narrow: f64| {
        (f[wide as usize] + wide * wide - f[narrow as usize] - narrow * narrow)
            / (2.0 * (wide - narrow))
    };
    let mut v = vec![0.0];
    let mut z = vec![f64::NEG_INFINITY];
    let mut k = 0usize;
    for q in 1..n {
        let q = q as f64;
        let mut s = intersect(q, v[k]);
        // A new parabola whose front is before the last one's takes the last one's place,
        // all the way back until that is no longer so.
        while k > 0 && s <= z[k] {
            k -= 1;
            v.pop();
            z.pop();
            s = intersect(q, v[k]);
        }
        k += 1;
        v.push(q);
        z.push(s);
    }
    let mut d = vec![0f64; n];
    let mut k = 0usize;
    for (q, out) in d.iter_mut().enumerate() {
        let q = q as f64;
        // Beyond the placed envelope nothing wins anything: stay on the last position.
        while k + 1 < v.len() && z[k + 1] < q {
            k += 1;
        }
        let vk = v[k];
        *out = (q - vk) * (q - vk) + f[vk as usize];
    }
    d
}

/// The Euclidean distance, in pixels, from every pixel to the nearest fully transparent pixel
/// (alpha 0), written into the alpha channel and clamped at 255; the colour is left as it came
/// in. A mask operation, meant for images whose alphas are 0 and 255: the transparent pixels
/// measure 0 and stay 0, and a partial alpha is overwritten by the distance it measures. With
/// no transparent pixel at all, every alpha comes back 255.
pub fn distance_transform(buf: &[u8], w: usize, h: usize) -> Result<Vec<u8>, BadBuffer> {
    check(buf, w, h)?;
    // One squared pixel past the clamp (256²): the value a row without any transparency
    // reports, so that distant answers and "no transparency anywhere" both read opaque.
    let far = 65_536.0;
    // First pass along the rows, then along the columns over the row answers.
    let mut row_dist = vec![0f64; w * h];
    for y in 0..h {
        let row: Vec<f64> = (0..w)
            .map(|x| {
                if buf[(y * w + x) * 4 + 3] == 0 {
                    0.0
                } else {
                    far
                }
            })
            .collect();
        row_dist[y * w..y * w + w].copy_from_slice(&squared_distance_to_zero(&row));
    }
    let mut out = buf.to_vec();
    for x in 0..w {
        let column: Vec<f64> = (0..h).map(|y| row_dist[y * w + x]).collect();
        for (y, value) in squared_distance_to_zero(&column).into_iter().enumerate() {
            out[(y * w + x) * 4 + 3] = value.sqrt().round().clamp(0.0, 255.0) as u8;
        }
    }
    Ok(out)
}

/// Shades one step of a diagonal edge, told by a 2×2 block: `(a, b, c, d)` are its pixels
/// (top-left, top-right, bottom-left, bottom-right). It fires only when three are fully opaque
/// and the one across the diagonal is empty - otherwise the block is a straight edge, a
/// one-pixel line, or part of a bigger step, and it is left alone. The pattern is checked here
/// against what has already been written (`out`, not the reader's `buf`), so a pixel that an
/// earlier step has touched is never touched again.
fn smooth_step(out: &mut [u8], a: usize, b: usize, c: usize, d: usize) {
    // Colours read before anything is written: the faded corner blends with the colour the
    // empty pixel had, and the empty pixel is drawn a quarter over the solid neighbour's.
    let mut block = [[0u8; 4]; 4];
    for (got, i) in block.iter_mut().zip([a, b, c, d]) {
        got.copy_from_slice(&out[i * 4..i * 4 + 4]);
    }
    let [a4, b4, c4, d4] = block;
    let shade = |own: f32, over: f32| (own * 0.75 + over * 0.25).round() as u8;
    let draw = |own: f32, solid: f32| (solid * 0.75 + own * 0.25).round() as u8;
    if a4[3] == 255 && b4[3] == 0 && c4[3] == 255 && d4[3] == 255 {
        for ch in 0..3 {
            out[a * 4 + ch] = shade(f32::from(a4[ch]), f32::from(b4[ch]));
            out[b * 4 + ch] = draw(f32::from(b4[ch]), f32::from(c4[ch]));
        }
        out[a * 4 + 3] = 191;
        out[b * 4 + 3] = 64;
    } else if a4[3] == 255 && b4[3] == 255 && c4[3] == 255 && d4[3] == 0 {
        for ch in 0..3 {
            out[c * 4 + ch] = shade(f32::from(c4[ch]), f32::from(d4[ch]));
            out[d * 4 + ch] = draw(f32::from(d4[ch]), f32::from(c4[ch]));
        }
        out[c * 4 + 3] = 191;
        out[d * 4 + 3] = 64;
    }
}

/// Antialiases the staircase steps a raster leaves on diagonal edges: every 2×2 block whose
/// alpha is fully drawn on three pixels and empty on the one across the diagonal is a step of
/// a 45° edge, and [`smooth_step`] shades it. Straight edges and one-pixel lines come out
/// untouched; every block is read from the input and its writes land once, so nothing is
/// shaded twice.
pub fn antialias(buf: &[u8], w: usize, h: usize) -> Result<Vec<u8>, BadBuffer> {
    check(buf, w, h)?;
    let mut out = buf.to_vec();
    if w < 2 || h < 2 {
        return Ok(out);
    }
    let alpha = |i: usize| buf[i * 4 + 3];
    for y in 0..h - 1 {
        for x in 0..w - 1 {
            let p = y * w + x;
            let one_step = alpha(p) == 255
                && alpha(p + 1) == 0
                && alpha(p + w) == 255
                && alpha(p + w + 1) == 255;
            let other = alpha(p) == 255
                && alpha(p + 1) == 255
                && alpha(p + w) == 255
                && alpha(p + w + 1) == 0;
            if one_step || other {
                smooth_step(&mut out, p, p + 1, p + w, p + w + 1);
            }
        }
    }
    Ok(out)
}

/// Box blur with radius `r` (edge pixels clamp to the in-bounds window).
pub fn box_blur(buf: &[u8], w: usize, h: usize, r: usize) -> Result<Vec<u8>, BadBuffer> {
    check(buf, w, h)?;
    let mut out = vec![0u8; buf.len()];
    for y in 0..h {
        for x in 0..w {
            let mut sum = [0u32; 4];
            let mut n = 0u32;
            for ny in y.saturating_sub(r)..=(y + r).min(h - 1) {
                for nx in x.saturating_sub(r)..=(x + r).min(w - 1) {
                    for (s, v) in sum.iter_mut().zip(&buf[(ny * w + nx) * 4..][..4]) {
                        *s += *v as u32;
                    }
                    n += 1;
                }
            }
            for (o, s) in out[(y * w + x) * 4..][..4].iter_mut().zip(sum) {
                *o = ((s + n / 2) / n) as u8;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn dot() -> Vec<u8> {
        let mut b = vec![0u8; 5 * 5 * 4];
        b[(2 * 5 + 2) * 4..][..4].copy_from_slice(&[200, 100, 50, 255]);
        b
    }

    #[test]
    fn gamma_identity_and_alpha() {
        let mut b = vec![10, 128, 250, 77];
        gamma_correct(&mut b, 1.0);
        assert_eq!(b, [10, 128, 250, 77]);
        gamma_correct(&mut b, 2.0);
        assert!(b[1] > 128 && b[3] == 77);
    }

    #[test]
    fn levels_stretch() {
        let mut b = vec![50, 100, 150, 9];
        levels(&mut b, 50, 150);
        assert_eq!(b, [0, 128, 255, 9]);
    }

    #[test]
    fn dilate_then_erode() {
        let d = dilate(&dot(), 5, 5, 1).expect("dilate");
        let on = d.chunks(4).filter(|p| p[3] == 255).count();
        assert_eq!(on, 9);
        let e = erode(&d, 5, 5, 1).expect("erode");
        assert_eq!(e.chunks(4).filter(|p| p[3] == 255).count(), 1);
    }

    #[test]
    fn blur_spreads_and_rejects_bad_len() {
        let b = box_blur(&dot(), 5, 5, 1).expect("blur");
        assert_eq!(b[(2 * 5 + 1) * 4 + 3], 28);
        assert_eq!(box_blur(&[0; 3], 5, 5, 1), Err(BadBuffer));
    }

    /// An RGBA8 buffer with its alpha set by a picture given as `&["..", ...]` rows: `.` for
    /// alpha 0, `#` for alpha 255 (colour left black unless told otherwise below).
    fn picture(rows: &[&str]) -> Vec<u8> {
        let (w, h) = (rows[0].len(), rows.len());
        let mut b = vec![0u8; w * h * 4];
        for (y, row) in rows.iter().enumerate() {
            for (x, c) in row.bytes().enumerate() {
                if c == b'#' {
                    b[(y * w + x) * 4 + 3] = 255;
                }
            }
        }
        b
    }

    fn alpha_at(out: &[u8], w: usize, x: usize, y: usize) -> u8 {
        out[(y * w + x) * 4 + 3]
    }

    #[test]
    fn despeckle_removes_lone_dots_and_shapes_up_to_max_area() {
        let mut b = picture(&["#..", ".##", ".##"]);
        b[0] = 200; // the dot's colour, so it is a shape and not just an alpha accident
        let out = despeckle(&b, 3, 3, 3).unwrap();
        assert_eq!(alpha_at(&out, 3, 0, 0), 0, "the lone dot is cleared");
        assert_eq!(
            alpha_at(&out, 3, 2, 1),
            255,
            "a four-pixel shape above the cap stays"
        );
        let out = despeckle(&b, 3, 3, 4).unwrap();
        assert_eq!(
            alpha_at(&out, 3, 2, 1),
            0,
            "at the cap (inclusive) it goes too"
        );
        // The colours of kept shapes are untouched.
        assert_eq!(&out[(2 * 3 + 2) * 4..][..3], &b[(2 * 3 + 2) * 4..][..3]);
    }

    #[test]
    fn despeckle_takes_colours_apart_and_ignores_partial_alpha() {
        let mut b = picture(&["ab.", "...", "..."]);
        b[0] = 200;
        b[4] = 30;
        b[8] = 100;
        b[7 * 4 + 3] = 99;
        let out = despeckle(&b, 3, 3, 9).unwrap();
        assert_eq!(alpha_at(&out, 3, 0, 0), 0, "each colour is its own speck");
        assert_eq!(alpha_at(&out, 3, 1, 0), 0);
        assert_eq!(alpha_at(&out, 3, 1, 2), 99, "not fully opaque, not a speck");
    }

    #[test]
    fn despeckle_rejects_bad_len() {
        assert_eq!(despeckle(&[0; 3], 3, 3, 4), Err(BadBuffer));
    }

    #[test]
    fn the_distance_transform_measures_to_the_nearest_transparency() {
        let mut b = picture(&["#####", "#####", "##.##", "#####", "#####"]);
        b[0] = 7; // a colour to prove colours survive
        let out = distance_transform(&b, 5, 5).unwrap();
        let a = |x, y| alpha_at(&out, 5, x, y);
        assert_eq!(a(2, 2), 0, "the transparent pixel measures itself");
        assert_eq!(a(2, 0), 2);
        assert_eq!(a(1, 2), 1);
        assert_eq!(a(0, 0), 3, "the diagonal corner is sqrt(8), rounded");
        assert_eq!(&out[..3], &[7, 0, 0], "colours are left as they came in");
        let out = distance_transform(&picture(&["###", "###"]), 3, 2).unwrap();
        assert_eq!(
            out.chunks(4).filter(|p| p[3] == 255).count(),
            6,
            "no transparency anywhere: nothing is measured, nothing changes"
        );
    }

    #[test]
    fn the_distance_transform_rejects_bad_len() {
        assert_eq!(distance_transform(&[0; 5], 3, 3), Err(BadBuffer));
    }

    #[test]
    fn antialias_shades_the_steps_of_a_diagonal() {
        // A down-right staircase: each row one pixel wider than the one above.
        let b = picture(&["##..", "###.", "####", "...."]);
        let out = antialias(&b, 4, 4).unwrap();
        let a = |out: &[u8], x, y| alpha_at(out, 4, x, y);
        assert_eq!(
            a(&out, 1, 0),
            191,
            "the step's own pixel fades to three quarters"
        );
        assert_eq!(
            a(&out, 2, 0),
            64,
            "the pixel it cuts into is drawn a quarter"
        );
        assert_eq!(
            a(&out, 2, 1),
            191,
            "the next step down the edge is shaded too"
        );
        assert_eq!(a(&out, 3, 1), 64);
        assert_eq!(a(&out, 3, 0), 0, "past the steps nothing is touched");
        assert_eq!(a(&out, 3, 2), 255, "the solid end of the edge is untouched");
        // An up-right staircase gets the same treatment through the other pattern.
        let b = picture(&["###.", "##..", "....", "...."]);
        let out = antialias(&b, 4, 4).unwrap();
        assert_eq!(a(&out, 1, 1), 191);
        assert_eq!(a(&out, 2, 1), 64);
        assert_eq!(a(&out, 3, 0), 0, "past the steps nothing is touched");
        assert_eq!(out[1], 0, "colours of untouched pixels survive");
    }

    #[test]
    fn antialias_passes_straight_edges_and_lines() {
        // The bottom edge of a fill, a one-pixel line each way, and a full sheet.
        for sheet in [
            picture(&["###", "###", "..."]),
            picture(&["...", "###", "..."]),
            picture(&["#..", "#..", "#.."]),
            picture(&["###", "###", "###"]),
        ] {
            let out = antialias(&sheet, 3, 3).unwrap();
            assert_eq!(out, sheet, "straight work is left alone");
        }
        assert_eq!(antialias(&picture(&["#"]), 1, 1).unwrap(), picture(&["#"]));
        assert_eq!(antialias(&[0; 3], 3, 3), Err(BadBuffer));
    }
}
