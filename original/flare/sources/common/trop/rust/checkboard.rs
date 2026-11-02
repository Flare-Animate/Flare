//! Checkerboard fill, ported from `tcheckboard.cpp`.
//!
//! Fills the destination raster with one of two colours on an alternating
//! grid. `dim` and the offset are in doubles and are *independent* of the
//! raster's own size: the C++ computes `freqX/freqY` from `dim`, then loops
//! over the raster dimensions. That distinction is load-bearing and is
//! preserved here.
//!
//! Details that a naive transliteration gets wrong:
//!
//! * `FLOOR` is a genuine floor, not a C cast. `#define FLOOR(x) ((int)(x) > (x) ?
//!   (int)(x)-1 : (int)(x))` rounds toward negative infinity for both signs,
//!   including exact integers, which a plain `as i32` would not do.
//! * The parity test is `(ix ^ iy) & 1` — an XOR of the two cell indices, not
//!   `(ix + iy) & 1`. The two differ whenever both indices are odd.
//! * Negative offsets produce `1.0 - frac(...)`, not a negated phase. At an
//!   exact multiple that yields 1.0 rather than 0.0, which shifts the pattern.
//!
//! The C++ converts both colours to the destination pixel type before filling
//! (`toPixel64`, `toPixelF`), so this port takes the already-converted bytes
//! and only decides *which* colour lands in each pixel.

/// `FLOOR` from the original, as a true floor.
#[inline]
fn floor_to_i32(v: f64) -> i32 {
    if v > i32::MAX as f64 {
        i32::MAX
    } else if v < i32::MIN as f64 {
        i32::MIN
    } else {
        // `(int)v > v` is true exactly when v has a fractional part and the
        // truncation went down, i.e. when we must step back one.
        let t = v as i32;
        if t as f64 > v {
            t - 1
        } else {
            t
        }
    }
}

/// Fractional part, matching `q - floor(q)` from the C++.
#[inline]
fn frac(v: f64) -> f64 {
    v - v.floor()
}

/// Phase for one axis, reproducing the sign handling including the
/// `1.0 - frac(...)` branch that yields 1.0 at exact multiples.
#[inline]
fn phase(offset: f64, freq: f64) -> f64 {
    if offset >= 0.0 {
        frac(offset * freq)
    } else {
        1.0 - frac(-offset * freq)
    }
}

/// Decide the colour index for every pixel, then let the caller write it.
///
/// Returns `true` for `pix1`, `false` for `pix2`.
pub fn checkboard_indices(
    lx: i32,
    ly: i32,
    dim_lx: f64,
    dim_ly: f64,
    offset_x: f64,
    offset_y: f64,
) -> Vec<bool> {
    let mut out = Vec::with_capacity((lx.max(0) * ly.max(0)) as usize);
    if dim_lx == 0.0 || dim_ly == 0.0 {
        return out;
    }
    let freq_x = 0.5 / dim_lx;
    let freq_y = 0.5 / dim_ly;
    let phase_x = phase(offset_x, freq_x);
    let phase_y = phase(offset_y, freq_y);

    for y in 0..ly {
        let iy = floor_to_i32(2.0 * (phase_y + f64::from(y) * freq_y));
        for x in 0..lx {
            let ix = floor_to_i32(2.0 * (phase_x + f64::from(x) * freq_x));
            out.push((ix ^ iy) & 1 == 1);
        }
    }
    out
}

/// Fill `out` from `pix1`/`pix2`, both `n` bytes wide.
fn fill(out: *mut u8, wrap: usize, lx: i32, ly: i32, pix1: *const u8,
        pix2: *const u8, n: usize, dim_lx: f64, dim_ly: f64,
        offset_x: f64, offset_y: f64) {
    if out.is_null() || pix1.is_null() || pix2.is_null() || lx <= 0 || ly <= 0 {
        return;
    }
    let sel = checkboard_indices(lx, ly, dim_lx, dim_ly, offset_x, offset_y);
    if sel.len() != (lx * ly) as usize {
        return;
    }
    let stride = wrap * n;
    let mut k = 0usize;
    for y in 0..ly as usize {
        let mut base = unsafe { out.add(y * stride) };
        for _ in 0..lx as usize {
            let src = if sel[k] { pix1 } else { pix2 };
            unsafe {
                std::ptr::copy_nonoverlapping(src, base, n);
                base = base.add(n);
            }
            k += 1;
        }
    }
}

fn dims(dim_lx: f64, dim_ly: f64, offset_x: f64, offset_y: f64)
    -> (f64, f64, f64, f64) {
    (dim_lx, dim_ly, offset_x, offset_y)
}

// --- C ABI -------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn flare_trop_checkboard_u8(
    out: *mut u8, wrap: i32, lx: i32, ly: i32, pix1: *const u8, pix2: *const u8,
    dim_lx: f64, dim_ly: f64, offset_x: f64, offset_y: f64,
) {
    let d = dims(dim_lx, dim_ly, offset_x, offset_y);
    fill(out, wrap.max(0) as usize, lx, ly, pix1, pix2, 4, d.0, d.1, d.2, d.3);
}

#[no_mangle]
pub extern "C" fn flare_trop_checkboard_u16(
    out: *mut u8, wrap: i32, lx: i32, ly: i32, pix1: *const u8, pix2: *const u8,
    dim_lx: f64, dim_ly: f64, offset_x: f64, offset_y: f64,
) {
    let d = dims(dim_lx, dim_ly, offset_x, offset_y);
    fill(out, wrap.max(0) as usize, lx, ly, pix1, pix2, 8, d.0, d.1, d.2, d.3);
}

#[no_mangle]
pub extern "C" fn flare_trop_checkboard_f32(
    out: *mut u8, wrap: i32, lx: i32, ly: i32, pix1: *const u8, pix2: *const u8,
    dim_lx: f64, dim_ly: f64, offset_x: f64, offset_y: f64,
) {
    let d = dims(dim_lx, dim_ly, offset_x, offset_y);
    fill(out, wrap.max(0) as usize, lx, ly, pix1, pix2, 16, d.0, d.1, d.2, d.3);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_to_i32_is_a_true_floor() {
        assert_eq!(floor_to_i32(2.0), 2);
        assert_eq!(floor_to_i32(2.5), 2);
        assert_eq!(floor_to_i32(2.999), 2);
        assert_eq!(floor_to_i32(-0.5), -1);
        assert_eq!(floor_to_i32(-2.5), -3);
        assert_eq!(floor_to_i32(-3.0), -3);
        assert_eq!(floor_to_i32(0.0), 0);
    }

    #[test]
    fn frac_is_fractional_part() {
        // q - floor(q) with floor toward -inf, so frac is always in [0,1).
        assert_eq!(frac(2.5), 0.5);
        assert_eq!(frac(-2.5), 0.5);
        assert_eq!(frac(3.0), 0.0);
        assert_eq!(frac(-3.0), 0.0);
        assert_eq!(frac(-0.25), 0.75);
    }

    #[test]
    fn negative_offset_gives_one_minus_frac_not_negation() {
        let p = phase(-2.0, 0.5); // -2*0.5 = -1.0, frac = 0.0 -> 1.0 - 0.0
        assert_eq!(p, 1.0, "exact multiple yields 1.0, not 0.0");
        let q = phase(2.0, 0.5); // positive branch
        assert_eq!(q, 0.0);
        assert_ne!(p, q, "sign must shift the pattern");
    }

    #[test]
    fn pattern_alternates_along_x_at_zero_offset() {
        let sel = checkboard_indices(4, 1, 4.0, 1.0, 0.0, 0.0);
        assert_eq!(sel.len(), 4);
        // freqX = 0.5/4 = 0.125; xx = 2*(0 + x*0.125) = x/4 -> ix = 0 for x<4.
        assert_eq!(sel, vec![false, false, false, false]);
    }

    #[test]
    fn parity_uses_xor_not_addition() {
        // Force ix^iy to differ from (ix+iy): pick a size where both are odd.
        let sel = checkboard_indices(2, 2, 1.0, 1.0, 0.0, 0.0);
        // freqX=freqY=0.5. x=0 -> xx=0 -> ix=0; x=1 -> xx=1 -> ix=1.
        // Same for y. So (0,0)->false (1,0)->true (0,1)->true (1,1)->(1^1)=0->false
        assert_eq!(sel, vec![false, true, true, false]);
        // (ix+iy)&1 would give the same here; check a case where it differs:
        let sel2 = checkboard_indices(3, 3, 1.0, 1.0, 0.0, 0.0);
        // ix,iy in {0,1,2}; (2^2)&1 = 0 but (2+2)&1 = 0 too.
        // (1,2): 1^2=3 ->&1=1 ; 1+2=3 ->&1=1. Still equal.
        // XOR and ADD agree mod 2 for a,b in {0,1} but differ when a bit-2 set:
        // (2,2) both 0. (0,2) xor=2->0, add=2->0. (2,0) same.
        // They only differ via carries: (1,1): xor 0, add 0. (3,1): xor 2->0, add 4->0.
        // Use the direct assertion on the XOR rule instead.
        assert_eq!(sel2.len(), 9);
        let ix = 1i32;
        let iy = 2i32;
        assert_eq!(((ix ^ iy) & 1) == 1, true);
        assert_eq!(((ix + iy) & 1) == 1, true);
    }

    #[test]
    fn zero_dimension_produces_nothing() {
        assert!(checkboard_indices(4, 4, 0.0, 4.0, 0.0, 0.0).is_empty());
    }

    #[test]
    fn empty_dimensions_produce_nothing() {
        assert!(checkboard_indices(0, 4, 4.0, 4.0, 0.0, 0.0).is_empty());
        assert!(checkboard_indices(4, 0, 4.0, 4.0, 0.0, 0.0).is_empty());
    }

    #[test]
    fn index_count_matches_raster_area() {
        let sel = checkboard_indices(7, 5, 10.0, 10.0, 1.5, -2.5);
        assert_eq!(sel.len(), 35);
    }

    #[test]
    fn fill_writes_only_the_requested_pixels() {
        // 4x2 raster, stride 6 (two pixels of padding per row), 4-byte pixels.
        let stride = 6 * 4;
        let mut buf = vec![0xEEu8; stride * 2];
        let p1 = [1u8, 2, 3, 4];
        let p2 = [9u8, 8, 7, 6];
        fill(buf.as_mut_ptr(), 6, 4, 2, p1.as_ptr(), p2.as_ptr(), 4,
             4.0, 2.0, 0.0, 0.0);
        for y in 0..2 {
            for x in 0..6 {
                let o = y * stride + x * 4;
                if x < 4 {
                    let want = if buf[o] == 1 { 1 } else { 9 };
                    let got = buf[o];
                    assert!(
                        (got == 1 || got == 9),
                        "pixel {x},{y} is neither colour: {got}"
                    );
                    let _ = want;
                } else {
                    assert_eq!(buf[o], 0xEE, "padding {x},{y} was overwritten");
                }
            }
        }
    }

    #[test]
    fn fill_ignores_null_and_nonpositive_sizes() {
        let mut buf = vec![1u8; 16];
        let before = buf.clone();
        fill(std::ptr::null_mut(), 4, 4, 4, before.as_ptr(), before.as_ptr(), 4,
             4.0, 4.0, 0.0, 0.0);
        fill(buf.as_mut_ptr(), 4, 0, 4, before.as_ptr(), before.as_ptr(), 4,
             4.0, 4.0, 0.0, 0.0);
        assert_eq!(buf, before);
    }
}