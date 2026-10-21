//! Port of stdfx/igs_motion_blur.cpp: smooth motion blur along a vector.
//!
//! `set_smooth` builds the sparse kernel (pixel offsets + weights) from the vector by
//! sampling every pixel near the segment on a 16x16 sub-pixel grid; `convert_template`
//! then averages the image through that kernel (8/16-bit; with alpha: optional alpha
//! blur, RGB skipped where the output alpha is 0, increase-only masking when the alpha
//! is not blurred).
//!
//! Kept as in the C++: the dead `#if 0` DDA ("jaggy") path stays dead; `bure_decrement_`
//! uses the `pos` of the *last* sub-pixel it visited; the kernel centre pixel `(0, 0)`
//! is forced to weight 1 before the gamma/normalisation steps.
//!
//! Deviations (only where the C++ was undefined behaviour or could not finish):
//! * non-finite vector components (the `(int)` casts) are refused, and a vector whose
//!   bounding box exceeds 2^24 pixels is refused (`TOO_LARGE`, the C++ would run for
//!   hours / exhaust memory);
//! * negative sizes / channels < 1, a null buffer, a misaligned 16-bit buffer, or
//!   overlapping in/out buffers (unless identical and there is no effect) are refused
//!   instead of crashing;
//! * `static_cast<T>(double)` saturates (NaN -> 0) instead of being UB.
//!
//! Trigonometry (`atan`/`sin`/`cos`) and `pow` go through the platform libm exactly like
//! the C++, so weights can differ by a few ULP between platforms; the tests compare
//! them with a ULP tolerance and compare images exactly only when fed the same weights.
//!
//! C ABI: `flare_igs_motion_blur_convert`; `igs_motion_blur.h` keeps the C++ signature
//! as an inline forwarder and rethrows `std::domain_error` for status 2.

use std::f64::consts::PI;
use std::os::raw::c_int;

pub const OK: i32 = 0;
/// `std::domain_error("Bad bits,Not uchar/ushort")`
pub const BAD_BITS: i32 = 2;
/// `std::invalid_argument`
pub const BAD_ARG: i32 = 3;
/// `std::bad_alloc`
pub const TOO_LARGE: i32 = 4;

const MAX_BOX_PIXELS: i64 = 1 << 24;

/// Sparse blur kernel: parallel arrays of weight and pixel offset.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Kernel {
    pub ratio: Vec<f64>,
    pub xs: Vec<i32>,
    pub ys: Vec<i32>,
}

/// `vec_poi_to_len_pos_`: (distance from the vector to the point, position along it).
fn vec_poi_to_len_pos(xv: f64, yv: f64, xp: f64, yp: f64) -> (f64, f64) {
    let mut radian = 0.0f64;
    if xv == 0.0 && yv == 0.0 {
        return (0.0, 0.0);
    } else if 0.0 < xv && 0.0 <= yv {
        radian = (yv / xv).atan();
    } else if xv <= 0.0 && 0.0 < yv {
        radian = (-xv / yv).atan() + PI / 2.0;
    } else if xv < 0.0 && yv <= 0.0 {
        radian = (yv / xv).atan() + PI;
    } else if 0.0 <= xv && yv < 0.0 {
        radian = (xv / -yv).atan() + PI + PI / 2.0;
    }

    let xv_rot = xv * (-radian).cos() - yv * (-radian).sin();
    let xp_rot = xp * (-radian).cos() - yp * (-radian).sin();
    let yp_rot = xp * (-radian).sin() + yp * (-radian).cos();

    let len = if xp_rot < 0.0 {
        ((xp * xp) + (yp * yp)).sqrt()
    } else if xv_rot < xp_rot {
        ((xp - xv) * (xp - xv) + (yp - yv) * (yp - yv)).sqrt()
    } else {
        yp_rot.abs()
    };
    (len, xp_rot)
}

/// Sub-pixel sample position: `x_tgt + (double)xx / x_div - (0.5 - 0.5 / x_div)`.
fn sub_pos(tgt: f64, i: i64, div: i64) -> f64 {
    tgt + i as f64 / div as f64 - (0.5 - 0.5 / div as f64)
}

/// `count_nearly_vector_`
fn count_nearly_vector(xv: f64, yv: f64, x_tgt: f64, y_tgt: f64, x_div: i64, y_div: i64, valid_len: f64) -> i32 {
    let mut count = 0;
    for yy in 0..y_div {
        for xx in 0..x_div {
            let xp = sub_pos(x_tgt, xx, x_div);
            let yp = sub_pos(y_tgt, yy, y_div);
            let (len, _pos) = vec_poi_to_len_pos(xv, yv, xp, yp);
            if len < valid_len {
                count += 1;
            }
        }
    }
    count
}

/// `liner_decrement_`
fn liner_decrement(xv: f64, yv: f64, x_tgt: f64, y_tgt: f64, x_div: i64, y_div: i64, valid_len: f64) -> f64 {
    let mut count = 0;
    let mut accum = 0.0f64;
    let line_len = (xv * xv + yv * yv).sqrt() + valid_len;
    for yy in 0..y_div {
        for xx in 0..x_div {
            let xp = sub_pos(x_tgt, xx, x_div);
            let yp = sub_pos(y_tgt, yy, y_div);
            let (len, pos) = vec_poi_to_len_pos(xv, yv, xp, yp);
            if len < valid_len {
                count += 1;
                accum += 1.0 - pos.abs() / line_len;
            }
        }
    }
    if count <= 0 {
        return 0.0;
    }
    accum / f64::from(count)
}

/// `bure_decrement_`
#[allow(clippy::too_many_arguments)]
fn bure_decrement(
    xv: f64,
    yv: f64,
    x_tgt: f64,
    y_tgt: f64,
    x_div: i64,
    y_div: i64,
    valid_len: f64,
    zanzo_length: i32,
) -> f64 {
    let mut count_in: i64 = 0;
    let mut count_out: i64 = 0;
    let mut pos = 0.0f64; // outlives the loops: the last visited sub-pixel's value is used below
    for yy in 0..y_div {
        for xx in 0..x_div {
            let xp = sub_pos(x_tgt, xx, x_div);
            let yp = sub_pos(y_tgt, yy, y_div);
            let (len, p) = vec_poi_to_len_pos(xv, yv, xp, yp);
            pos = p;
            if len < valid_len {
                // (int)pos % zanzo_length
                if 0 == (pos as i32) % zanzo_length {
                    count_in += 1;
                } else {
                    count_out += 1;
                }
            }
        }
    }
    if count_in + count_out <= 0 {
        return 0.0;
    }
    if pos < 0.0 {
        pos = -pos;
    }
    let line_len = (xv * xv + yv * yv).sqrt() + valid_len;
    if line_len < pos {
        pos = line_len;
    }
    count_in as f64 / (count_in + count_out) as f64 * (1.0 - pos / line_len)
}

/// `set_smooth_`. `Err` = refused (non-finite vector or oversized bounding box); an
/// empty kernel (`Ok`) means "no effect".
pub fn set_smooth(
    x_vector: f64,
    y_vector: f64,
    vector_scale: f64,
    curve: f64,
    zanzo_length: i32,
    zanzo_power: f64,
) -> Result<Kernel, i32> {
    let mut k = Kernel::default();
    if curve <= 0.0 {
        return Ok(k);
    }
    let xv = x_vector * vector_scale;
    let yv = y_vector * vector_scale;
    if xv == 0.0 && yv == 0.0 {
        return Ok(k);
    }
    if !xv.is_finite() || !yv.is_finite() || xv.abs() > 1.0e9 || yv.abs() > 1.0e9 {
        return Err(BAD_ARG);
    }

    let (x1, x2) = if 0.0 <= xv { (0, xv as i32 + 2) } else { (xv as i32 - 2, 0) };
    let (y1, y2) = if 0.0 <= yv { (0, yv as i32 + 2) } else { (yv as i32 - 2, 0) };
    let area = (i64::from(x2) - i64::from(x1) + 1) * (i64::from(y2) - i64::from(y1) + 1);
    if area > MAX_BOX_PIXELS {
        return Err(TOO_LARGE);
    }

    // The C++ counts the covered pixels once to size the arrays and once more to fill
    // them; the count is a pure function, so one pass gives the same result.
    for yy in y1..=y2 {
        for xx in x1..=x2 {
            let count = count_nearly_vector(xv, yv, f64::from(xx), f64::from(yy), 16, 16, 0.5);
            if 0 < count {
                k.ratio.push(f64::from(count) / (16 * 16) as f64);
                k.xs.push(xx);
                k.ys.push(yy);
            }
        }
    }
    if k.ratio.is_empty() {
        return Ok(k);
    }

    // linear decay
    for ii in 0..k.ratio.len() {
        k.ratio[ii] *= liner_decrement(xv, yv, f64::from(k.xs[ii]), f64::from(k.ys[ii]), 16, 16, 0.5);
    }

    // after-image steps
    if 0 < zanzo_length {
        for ii in 0..k.ratio.len() {
            k.ratio[ii] = (1.0 - zanzo_power) * k.ratio[ii]
                + zanzo_power
                    * bure_decrement(xv, yv, f64::from(k.xs[ii]), f64::from(k.ys[ii]), 16, 16, 0.5, zanzo_length);
        }
    }

    // the vector origin is 1
    for ii in 0..k.ratio.len() {
        if 0 == k.xs[ii] && 0 == k.ys[ii] {
            k.ratio[ii] = 1.0;
            break;
        }
    }

    // gamma
    if 1.0 != curve {
        for r in k.ratio.iter_mut() {
            *r = r.powf(1.0 / curve);
        }
    }
    // normalise
    let mut d_accum = 0.0f64;
    for r in &k.ratio {
        d_accum += *r;
    }
    for r in k.ratio.iter_mut() {
        *r /= d_accum;
    }
    Ok(k)
}

/// Image sample types.
pub(crate) trait Sample: Copy + PartialEq + PartialOrd {
    const MAX: u32;
    fn to_f64(self) -> f64;
    fn to_u32(self) -> u32;
    /// `static_cast<T>(double)` (saturating here)
    fn from_f64(v: f64) -> Self;
    fn from_u32(v: u32) -> Self;
    const ZERO: Self;
}
impl Sample for u8 {
    const MAX: u32 = u8::MAX as u32;
    const ZERO: u8 = 0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn to_u32(self) -> u32 {
        u32::from(self)
    }
    fn from_f64(v: f64) -> u8 {
        v as u8
    }
    fn from_u32(v: u32) -> u8 {
        v as u8
    }
}
impl Sample for u16 {
    const MAX: u32 = u16::MAX as u32;
    const ZERO: u16 = 0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn to_u32(self) -> u32 {
        u32::from(self)
    }
    fn from_f64(v: f64) -> u16 {
        v as u16
    }
    fn from_u32(v: u32) -> u16 {
        v as u16
    }
}

// Toonz6 channel order (igs_ifx_common.h): blu = 0, gre, red, alp
const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// `pixel_value`
#[allow(clippy::too_many_arguments)]
fn pixel_value<T: Sample>(
    image: &[T],
    height: i32,
    width: i32,
    channels: usize,
    xx: i32,
    yy: i32,
    zz: usize,
    k: &Kernel,
) -> T {
    let mut ratio_accum = 0.0f64;
    let mut accum = 0.0f64;
    for ii in 0..k.ratio.len() {
        let xp = xx + k.xs[ii];
        let yp = yy + k.ys[ii];
        if xp < 0 || yp < 0 || width <= xp || height <= yp {
            continue;
        }
        ratio_accum += k.ratio[ii];
        accum += k.ratio[ii]
            * image[channels * width as usize * yp as usize + channels * xp as usize + zz].to_f64();
    }
    if 0.0 == ratio_accum {
        return T::ZERO;
    }
    T::from_f64(accum / ratio_accum + 0.5)
}

/// `convert_template_`: `out` receives the blurred image.
pub(crate) fn convert_template<T: Sample>(
    inn: &[T],
    out: &mut [T],
    hh: i32,
    ww: i32,
    cc: usize,
    k: &Kernel,
    alpha_rend_sw: bool,
) {
    if k.ratio.is_empty() {
        let n = hh as usize * ww as usize * cc;
        out[..n].copy_from_slice(&inn[..n]);
        return;
    }
    let pv = |xx: i32, yy: i32, zz: usize| pixel_value(inn, hh, ww, cc, xx, yy, zz, k);
    if cc == 4 {
        let val_max = T::MAX;
        for yy in 0..hh {
            for xx in 0..ww {
                let base = (yy as usize * ww as usize + xx as usize) * cc;
                if alpha_rend_sw {
                    out[base + ALP] = pv(xx, yy, ALP);
                    if out[base + ALP] == T::ZERO {
                        out[base + RED] = inn[base + RED];
                        out[base + GRE] = inn[base + GRE];
                        out[base + BLU] = inn[base + BLU];
                    } else {
                        out[base + RED] = pv(xx, yy, RED);
                        out[base + GRE] = pv(xx, yy, GRE);
                        out[base + BLU] = pv(xx, yy, BLU);
                    }
                } else {
                    out[base + ALP] = inn[base + ALP];
                    if out[base + ALP] == T::ZERO {
                        out[base + RED] = inn[base + RED];
                        out[base + GRE] = inn[base + GRE];
                        out[base + BLU] = inn[base + BLU];
                    } else {
                        out[base + RED] = pv(xx, yy, RED);
                        out[base + GRE] = pv(xx, yy, GRE);
                        out[base + BLU] = pv(xx, yy, BLU);
                        // alpha above 0 and below max: mask the increase
                        if inn[base + ALP].to_u32() < val_max {
                            let aa = inn[base + ALP].to_u32();
                            for ch in [RED, GRE, BLU] {
                                if inn[base + ch] < out[base + ch] {
                                    let dif = out[base + ch].to_u32() - inn[base + ch].to_u32();
                                    out[base + ch] = T::from_u32(inn[base + ch].to_u32() + dif * aa / val_max);
                                }
                            }
                        }
                    }
                }
            }
        }
    } else {
        for yy in 0..hh {
            for xx in 0..ww {
                let base = (yy as usize * ww as usize + xx as usize) * cc;
                for zz in 0..cc {
                    out[base + zz] = pv(xx, yy, zz);
                }
            }
        }
    }
}

fn ranges_overlap(a: usize, a_len: usize, b: usize, b_len: usize) -> bool {
    a < b + b_len && b < a + a_len
}

/// `igs::motion_blur::convert` on raw byte buffers.
///
/// # Safety
/// `image_in`/`image_out` must be valid for `height*width*channels` samples of `bits`
/// bits (aligned for 16-bit); `image_out` is written.
#[allow(clippy::too_many_arguments)]
pub unsafe fn convert_raw(
    image_in: *const u8,
    image_out: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    x_vector: f64,
    y_vector: f64,
    vector_scale: f64,
    curve: f64,
    zanzo_length: i32,
    zanzo_power: f64,
    alpha_rend_sw: bool,
) -> i32 {
    if bits != 8 && bits != 16 {
        return BAD_BITS;
    }
    if height < 0 || width < 0 || channels < 1 {
        return BAD_ARG;
    }
    let kernel = match set_smooth(x_vector, y_vector, vector_scale, curve, zanzo_length, zanzo_power) {
        Ok(k) => k,
        Err(e) => return e,
    };
    let n = match (height as usize).checked_mul(width as usize).and_then(|v| v.checked_mul(channels as usize)) {
        Some(n) => n,
        None => return BAD_ARG,
    };
    if n == 0 {
        return OK;
    }
    if image_in.is_null() || image_out.is_null() {
        return BAD_ARG;
    }
    let elem = (bits / 8) as usize;
    if bits == 16 && ((image_in as usize) % 2 != 0 || (image_out as usize) % 2 != 0) {
        return BAD_ARG;
    }
    let bytes = n * elem;
    if ranges_overlap(image_in as usize, bytes, image_out as usize, bytes)
        && !(image_in as usize == image_out as usize && kernel.ratio.is_empty())
    {
        return BAD_ARG;
    }
    let cc = channels as usize;
    if bits == 8 {
        let inn = std::slice::from_raw_parts(image_in, n);
        let out = std::slice::from_raw_parts_mut(image_out, n);
        convert_template::<u8>(inn, out, height, width, cc, &kernel, alpha_rend_sw);
    } else {
        let inn = std::slice::from_raw_parts(image_in as *const u16, n);
        let out = std::slice::from_raw_parts_mut(image_out as *mut u16, n);
        convert_template::<u16>(inn, out, height, width, cc, &kernel, alpha_rend_sw);
    }
    OK
}

/// `igs::motion_blur::convert`. Returns 0, 2 (bad bits), 3 (bad argument), 4 (too large).
///
/// # Safety
/// See [`convert_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_motion_blur_convert(
    image_in: *const u8,
    image_out: *mut u8,
    height: c_int,
    width: c_int,
    channels: c_int,
    bits: c_int,
    x_vector: f64,
    y_vector: f64,
    vector_scale: f64,
    curve: f64,
    zanzo_length: c_int,
    zanzo_power: f64,
    alpha_rend_sw: bool,
) -> c_int {
    crate::guard(BAD_ARG, || {
        convert_raw(
            image_in,
            image_out,
            height,
            width,
            channels,
            bits,
            x_vector,
            y_vector,
            vector_scale,
            curve,
            zanzo_length,
            zanzo_power,
            alpha_rend_sw,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kernel(xv: f64, yv: f64) -> Kernel {
        set_smooth(xv, yv, 1.0, 1.0, 0, 1.0).unwrap()
    }

    #[test]
    fn kernel_is_normalised_and_contains_the_origin() {
        let k = kernel(5.0, 0.0);
        assert!(!k.ratio.is_empty());
        let sum: f64 = k.ratio.iter().sum();
        assert!((sum - 1.0).abs() < 1e-12);
        assert!(k.xs.iter().zip(&k.ys).any(|(&x, &y)| x == 0 && y == 0));
        // origin carries the largest weight
        let (i0, _) = k.xs.iter().zip(&k.ys).enumerate().find(|(_, (&x, &y))| x == 0 && y == 0).unwrap();
        assert!(k.ratio.iter().all(|&r| r <= k.ratio[i0] + 1e-15));
    }

    #[test]
    fn no_effect_cases_give_an_empty_kernel() {
        assert!(set_smooth(3.0, 3.0, 1.0, 0.0, 0, 1.0).unwrap().ratio.is_empty());
        assert!(set_smooth(3.0, 3.0, 1.0, -1.0, 0, 1.0).unwrap().ratio.is_empty());
        assert!(set_smooth(0.0, 0.0, 1.0, 1.0, 0, 1.0).unwrap().ratio.is_empty());
        assert!(set_smooth(3.0, 3.0, 0.0, 1.0, 0, 1.0).unwrap().ratio.is_empty());
    }

    #[test]
    fn refuses_non_finite_and_oversized_vectors() {
        assert_eq!(set_smooth(f64::NAN, 1.0, 1.0, 1.0, 0, 1.0), Err(BAD_ARG));
        assert_eq!(set_smooth(f64::INFINITY, 1.0, 1.0, 1.0, 0, 1.0), Err(BAD_ARG));
        assert_eq!(set_smooth(1.0e5, 1.0e5, 1.0, 1.0, 0, 1.0), Err(TOO_LARGE));
    }

    #[test]
    fn horizontal_blur_of_an_impulse_spreads_one_way() {
        // vector points +x: pixel p averages pixels p, p+1, ... (offsets are >= 0)
        let k = kernel(3.0, 0.0);
        assert!(k.xs.iter().all(|&x| x >= 0));
        let mut inn = vec![0u8; 9];
        inn[2] = 255;
        let mut out = vec![0u8; 9];
        convert_template::<u8>(&inn, &mut out, 1, 9, 1, &k, true);
        assert!(out[1] > 0 || out[0] > 0, "{out:?}");
        assert_eq!(out[8], 0);
    }

    #[test]
    fn empty_kernel_copies() {
        let k = Kernel::default();
        let inn: Vec<u8> = (0..24).collect();
        let mut out = vec![0u8; 24];
        convert_template::<u8>(&inn, &mut out, 2, 3, 4, &k, true);
        assert_eq!(inn, out);
    }

    #[test]
    fn rgba_without_alpha_blur_masks_increase_only() {
        // a transparent-ish bright pixel next to a dark opaque one
        let k = kernel(2.0, 0.0);
        let mut inn = vec![0u8; 4 * 4];
        // pixel 1: bright with alpha 128; others dark with alpha 128
        for p in 0..4 {
            inn[p * 4 + 3] = 128;
        }
        inn[1 * 4..1 * 4 + 3].copy_from_slice(&[200, 200, 200]);
        let mut out = vec![0u8; 16];
        convert_template::<u8>(&inn, &mut out, 1, 4, 4, &k, false);
        // alpha is kept
        for p in 0..4 {
            assert_eq!(out[p * 4 + 3], 128);
        }
        // masked increase: never above in + (blurred - in) * 128/255
        for p in 0..4 {
            assert!(out[p * 4] as u32 <= inn[p * 4] as u32 + (200 - inn[p * 4] as u32) * 128 / 255 + 1);
        }
    }

    fn call8(inn: &[u8], out: &mut [u8], h: i32, w: i32, c: i32, bits: i32, v: (f64, f64)) -> i32 {
        unsafe {
            convert_raw(inn.as_ptr(), out.as_mut_ptr(), h, w, c, bits, v.0, v.1, 1.0, 1.0, 0, 1.0, true)
        }
    }

    #[test]
    fn argument_checks() {
        let inn = vec![1u8; 16];
        let mut out = vec![0u8; 16];
        assert_eq!(call8(&inn, &mut out, 2, 2, 4, 12, (1.0, 0.0)), BAD_BITS);
        assert_eq!(call8(&inn, &mut out, -1, 2, 4, 8, (1.0, 0.0)), BAD_ARG);
        assert_eq!(call8(&inn, &mut out, 2, 2, 0, 8, (1.0, 0.0)), BAD_ARG);
        assert_eq!(call8(&inn, &mut out, 0, 2, 4, 8, (1.0, 0.0)), OK);
        assert_eq!(call8(&inn, &mut out, 2, 2, 4, 8, (f64::NAN, 0.0)), BAD_ARG);
        let st = unsafe {
            convert_raw(std::ptr::null(), out.as_mut_ptr(), 2, 2, 4, 8, 1.0, 0.0, 1.0, 1.0, 0, 1.0, true)
        };
        assert_eq!(st, BAD_ARG);
        // overlapping buffers
        let mut buf = vec![7u8; 32];
        let st = unsafe {
            convert_raw(buf.as_ptr(), buf.as_mut_ptr().add(4), 2, 2, 4, 8, 1.0, 0.0, 1.0, 1.0, 0, 1.0, true)
        };
        assert_eq!(st, BAD_ARG);
        // identical buffers are fine when there is no effect (memmove onto itself)
        let st = unsafe {
            convert_raw(buf.as_ptr(), buf.as_mut_ptr(), 2, 2, 4, 8, 1.0, 0.0, 1.0, 0.0, 0, 1.0, true)
        };
        assert_eq!(st, OK);
    }

    #[test]
    fn sixteen_bit_matches_eight_bit_scaled() {
        let k = kernel(2.0, 1.0);
        let inn8: Vec<u8> = (0..60).map(|i| ((i * 37) % 256) as u8).collect();
        let inn16: Vec<u16> = inn8.iter().map(|&v| u16::from(v) * 257).collect();
        let mut o8 = vec![0u8; 60];
        let mut o16 = vec![0u16; 60];
        convert_template::<u8>(&inn8, &mut o8, 5, 4, 3, &k, true);
        convert_template::<u16>(&inn16, &mut o16, 5, 4, 3, &k, true);
        for (a, b) in o8.iter().zip(&o16) {
            assert!((i32::from(*a) * 257 - i32::from(*b)).abs() <= 257, "{a} vs {b}");
        }
    }

    // --- differential golden -------------------------------------------------------

    /// One kernel configuration of the golden: xv, yv, scale, curve, zanzo length/power.
    const CFGS: [(f64, f64, f64, f64, i32, f64); 16] = [
        (3.0, 0.0, 1.0, 1.0, 0, 1.0),
        (0.0, -4.0, 1.0, 1.0, 0, 1.0),
        (5.5, 3.2, 1.0, 1.0, 0, 1.0),
        (-6.0, 2.0, 1.0, 1.0, 0, 1.0),
        (-2.5, -7.5, 1.0, 1.0, 0, 1.0),
        (1.0, 1.0, 1.0, 1.0, 0, 1.0),
        (0.4, 0.0, 1.0, 1.0, 0, 1.0),
        (9.0, 9.0, 0.5, 1.0, 0, 1.0),
        (6.0, 1.5, 1.0, 2.0, 0, 1.0),
        (6.0, 1.5, 1.0, 0.5, 0, 1.0),
        (6.0, 1.5, 1.0, 0.0, 0, 1.0),
        (6.0, 1.5, 1.0, -1.0, 0, 1.0),
        (8.0, 0.0, 1.0, 1.0, 3, 0.5),
        (8.0, 3.0, 1.0, 1.0, 5, 1.0),
        (-7.0, 4.0, 1.0, 1.5, 4, 0.25),
        (0.0, 0.0, 1.0, 1.0, 0, 1.0),
    ];

    fn val(bits: i32, i: usize, y: usize) -> u32 {
        if bits == 8 {
            ((i * 37 + (i / 3) * 11 + y * 5) % 256) as u32
        } else {
            ((i * 2579 + y * 97 + 13) % 65536) as u32
        }
    }

    fn make<T: Sample>(bits: i32, cc: usize, w: usize, h: usize, conv: impl Fn(u32) -> T) -> Vec<T> {
        (0..w * h * cc)
            .map(|i| {
                let mut v = val(bits, i, i / (w * cc));
                if cc == 4 && i % 4 == 3 && (i / 4) % 5 == 0 {
                    v = 0;
                }
                conv(v)
            })
            .collect()
    }

    fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    fn ulps(a: f64, b: f64) -> u64 {
        let (x, y) = (a.to_bits() as i64, b.to_bits() as i64);
        // both are non-negative weights here
        (x - y).unsigned_abs()
    }

    fn run_image(bits: i32, cc: usize, alpha: bool, k: &Kernel) -> u64 {
        let (w, h) = (13usize, 9usize);
        if bits == 8 {
            let inn = make::<u8>(8, cc, w, h, |v| v as u8);
            let mut out = vec![0u8; inn.len()];
            convert_template(&inn, &mut out, h as i32, w as i32, cc, k, alpha);
            fnv(out.iter().copied())
        } else {
            let inn = make::<u16>(16, cc, w, h, |v| v as u16);
            let mut out = vec![0u16; inn.len()];
            convert_template(&inn, &mut out, h as i32, w as i32, cc, k, alpha);
            fnv(out.iter().flat_map(|v| v.to_le_bytes()))
        }
    }

    /// Replays `gen_igs_motion_blur_cases.py`: the original `igs_motion_blur.cpp`
    /// (from git) over 16 kernel configurations x 12 images (8/16-bit x 4/3/1 channels
    /// x alpha-blur on/off).
    ///
    /// * the weights Rust computes must equal the recorded ones within 4 ULP and pick
    ///   the same pixels (they go through the platform libm like the C++);
    /// * the Rust blur fed with the *recorded* kernel must reproduce the recorded image
    ///   hash exactly (`+ - * /` only: platform independent);
    /// * when Rust's own kernel is bit-identical to the recorded one (true on the host
    ///   libm the golden was cut on), the end-to-end hash must match too.
    #[test]
    fn matches_the_cpp_reference() {
        let golden = include_str!("igs_motion_blur_cases.txt");
        let mut lines = golden.lines().peekable();
        let mut e2e_checked = 0;
        for (ci, cfg) in CFGS.iter().enumerate() {
            let head = lines.next().unwrap_or_else(|| panic!("missing cfg {ci}"));
            assert_eq!(head, format!("cfg {ci}"));
            let size: usize = lines.next().unwrap().strip_prefix("size ").unwrap().parse().unwrap();
            let rec = Kernel {
                ratio: lines
                    .next()
                    .unwrap()
                    .strip_prefix('r')
                    .unwrap()
                    .split_whitespace()
                    .map(|t| f64::from_bits(u64::from_str_radix(t, 16).unwrap()))
                    .collect(),
                xs: lines.next().unwrap().strip_prefix('x').unwrap().split_whitespace().map(|t| t.parse().unwrap()).collect(),
                ys: lines.next().unwrap().strip_prefix('y').unwrap().split_whitespace().map(|t| t.parse().unwrap()).collect(),
            };
            assert_eq!(rec.ratio.len(), size, "cfg {ci}");

            let mine = set_smooth(cfg.0, cfg.1, cfg.2, cfg.3, cfg.4, cfg.5).unwrap();
            // pixel selection depends on libm comparisons: hard requirement only on glibc
            let strict = cfg!(all(target_os = "linux", target_env = "gnu"));
            let max_ulps = if strict { 4 } else { 64 }; // other libms drift a few ULP
            let structure = mine.xs == rec.xs && mine.ys == rec.ys && mine.ratio.len() == rec.ratio.len();
            if strict {
                assert_eq!(mine.xs, rec.xs, "cfg {ci}: pixels");
                assert_eq!(mine.ys, rec.ys, "cfg {ci}: pixels");
            } else if !structure {
                eprintln!("note: libm differs from the recording for cfg {ci}; kernel structure not compared");
            }
            if structure {
                for (i, (a, b)) in mine.ratio.iter().zip(&rec.ratio).enumerate() {
                    assert!(ulps(*a, *b) <= max_ulps, "cfg {ci} weight {i}: {a:e} vs {b:e}");
                }
            }
            let same = structure && mine.ratio.iter().zip(&rec.ratio).all(|(a, b)| a.to_bits() == b.to_bits());

            for &bits in &[8, 16] {
                for &cc in &[4usize, 3, 1] {
                    for alpha in [false, true] {
                        let l = lines.next().unwrap_or_else(|| panic!("cfg {ci}: missing image"));
                        assert_eq!(
                            l.rsplit_once(' ').unwrap().0,
                            format!("img {bits} {cc} {}", alpha as i32),
                            "cfg {ci}"
                        );
                        let want = u64::from_str_radix(l.rsplit_once(' ').unwrap().1, 16).unwrap();
                        assert_eq!(
                            run_image(bits, cc, alpha, &rec),
                            want,
                            "cfg {ci} bits {bits} cc {cc} alpha {alpha}: blur on the recorded kernel"
                        );
                        if same {
                            assert_eq!(run_image(bits, cc, alpha, &mine), want, "cfg {ci}: end to end");
                            e2e_checked += 1;
                        }
                    }
                }
            }
        }
        assert!(lines.next().is_none(), "golden has extra lines");
        assert!(e2e_checked > 0 || cfg!(not(target_os = "linux")), "no end-to-end check ran on Linux");
    }
}
