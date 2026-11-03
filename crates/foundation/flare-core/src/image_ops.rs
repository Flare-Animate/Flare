//! Raster image operations on RGBA8 buffers (ports of OpenToonz trop kernels:
//! gamma/levels, erode/dilate, box blur). Buffers are row-major, 4 bytes/pixel.

/// Error for buffers whose length does not match `width * height * 4`.
#[derive(Debug, PartialEq, Eq)]
pub struct BadBuffer;

fn check(buf: &[u8], w: usize, h: usize) -> Result<(), BadBuffer> {
    if buf.len() == w.checked_mul(h).and_then(|n| n.checked_mul(4)).ok_or(BadBuffer)? {
        Ok(())
    } else {
        Err(BadBuffer)
    }
}

/// Gamma-correct RGB in place (alpha untouched). `gamma` > 0; 1.0 is identity.
pub fn gamma_correct(buf: &mut [u8], gamma: f32) {
    let inv = 1.0 / gamma.max(f32::EPSILON);
    let lut: Vec<u8> = (0..256)
        .map(|i| ((i as f32 / 255.0).powf(inv) * 255.0).round().clamp(0.0, 255.0) as u8)
        .collect();
    for px in buf.chunks_exact_mut(4) {
        for c in &mut px[..3] {
            *c = lut[*c as usize];
        }
    }
}

/// Levels: map `[black, white]` to `[0, 255]` on RGB in place (alpha untouched).
pub fn levels(buf: &mut [u8], black: u8, white: u8) {
    let (b, w) = (black as f32, (white.max(black.saturating_add(1))) as f32);
    for px in buf.chunks_exact_mut(4) {
        for c in &mut px[..3] {
            *c = (((*c as f32 - b) / (w - b)) * 255.0).round().clamp(0.0, 255.0) as u8;
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
        let d = dilate(&dot(), 5, 5, 1).unwrap();
        let on = d.chunks(4).filter(|p| p[3] == 255).count();
        assert_eq!(on, 9);
        let e = erode(&d, 5, 5, 1).unwrap();
        assert_eq!(e.chunks(4).filter(|p| p[3] == 255).count(), 1);
    }

    #[test]
    fn blur_spreads_and_rejects_bad_len() {
        let b = box_blur(&dot(), 5, 5, 1).unwrap();
        assert_eq!(b[(2 * 5 + 1) * 4 + 3], 28);
        assert_eq!(box_blur(&[0; 3], 5, 5, 1), Err(BadBuffer));
    }
}
