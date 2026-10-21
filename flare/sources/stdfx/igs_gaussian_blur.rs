//! Port of stdfx/igs_gaussian_blur.cpp: separable gaussian blur of a float image that
//! carries a margin of `int_radius` pixels on every side (the output has no margin).
//!
//! Per channel: copy into a double plane, blur horizontally into a second plane, blur
//! vertically back into the first one, write the interior. A channel identical to the
//! previous one (compared sample by sample) reuses the previous result. With a
//! reference image the radius is `real_radius * ref` per pixel and the gaussian brush
//! is rebuilt whenever that radius changes (`igs_gauss_distribution`, also Rust).
//!
//! Quirks kept (they change every render, so they stay):
//! * the vertical pass starts its reference walk one column in (`ref_hori++` happens
//!   before the first read), so it samples the reference shifted by one pixel and the
//!   last column reads into the next row's first element (in bounds: the caller's
//!   reference has the *margined* size);
//! * the horizontal pass advances its reference row only for `r < yo < H - r`;
//! * both passes start with "previous radius" `-1.0` and share one brush buffer, so a
//!   pixel whose radius is exactly `-1.0` would keep the previous brush;
//! * `real_radius <= 0` returns before any other check (even a bad channel count).
//!
//! Deviations (only where the C++ was undefined behaviour or could not finish):
//! * a per-pixel brush with `int_radius(read_r)` larger than the margin (reference above
//!   1, or a reference radius > margin) wrote before the brush array; the radius handed
//!   to the brush builder is clamped to the margin instead;
//! * sizes `H, W < 1` (the C++ threw `std::out_of_range` from `vector::at(0)` or
//!   `length_error`) and `int_radius < 1` (the brush builder then wrote at index -1) are
//!   refused as `BAD_ARG`; null image / output / reference-less buffer pointers likewise;
//! * the work buffer is the caller's when it is 8-byte aligned, a private one otherwise;
//! * `int_radius(NaN)` is 0 (saturating cast) instead of UB.
//!
//! C ABI: `flare_igs_gaussian_blur_{int_radius,buffer_bytes,convert}`;
//! `igs_gaussian_blur.h` keeps the C++ signatures as inline forwarders and rethrows the
//! `std::domain_error`s (status 1, 5, 6) / `std::invalid_argument` (3).

use std::os::raw::c_int;

pub const OK: i32 = 0;
/// `std::domain_error("Bad channels,Not rgba/rgb/grayscale")`
pub const BAD_CHANNELS: i32 = 1;
/// `std::invalid_argument`
pub const BAD_ARG: i32 = 3;
/// `std::domain_error("buffer_inn is empty")`
pub const BUFFER_INN_EMPTY: i32 = 5;
/// `std::domain_error("buffer_out is empty")`
pub const BUFFER_OUT_EMPTY: i32 = 6;

fn diameter_from_radius(radius: i32) -> i32 {
    radius.wrapping_mul(2).wrapping_add(1)
}

/// `igs::gaussian_blur_hv::int_radius`: `static_cast<int>(ceil(real_radius))`.
pub fn int_radius(real_radius: f64) -> i32 {
    real_radius.ceil() as i32
}

/// `igs::gaussian_blur_hv::buffer_bytes`, with the C++ `int`/`size_t` arithmetic
/// (x86-64 wrap-around for absurd sizes).
pub fn buffer_bytes(height_with_margin: i32, width_with_margin: i32, int_radius: i32) -> i32 {
    let d = diameter_from_radius(int_radius);
    let dd = d.wrapping_mul(d) as i64 as u64;
    let hw = height_with_margin.wrapping_mul(width_with_margin) as i64 as u64;
    let total = dd.wrapping_mul(8).wrapping_add(hw.wrapping_mul(8)).wrapping_add(hw.wrapping_mul(8));
    total as u32 as i32
}

/// `gauss_distribution_1d_` into `seq` (`seq.len()` is the brush diameter, odd, >= 3),
/// with the radius clamped so the window stays inside `seq`.
fn gauss(seq: &mut [f64], int_radius: i32, real_radius: f64, sigma: f64) {
    let margin = ((seq.len() as i32) - 1) / 2;
    let eff = int_radius.min(margin);
    // SAFETY: with eff <= margin the window start_pos = (len - (2*max(eff,1)+1)) / 2 >= 0
    // and end_pos <= len because len >= 3.
    unsafe {
        crate::igs_gauss_distribution::gauss_distribution_1d(
            seq.as_mut_ptr(),
            seq.len() as i32,
            eff,
            real_radius,
            sigma,
        );
    }
}

/// `blur_1st_hori_`: `inp` -> `out` (both `H*W` planes).
#[allow(clippy::too_many_arguments)]
fn blur_1st_hori(
    inp: &[f64],
    h: usize,
    w: usize,
    brush: &mut [f64],
    r: usize,
    out: &mut [f64],
    refp: Option<&[f32]>,
    real_radius: f64,
    sigma: f64,
) {
    let brush_diameter = 2 * r + 1;
    let w_no = w as isize - 2 * r as isize;
    let mut ref_vert: isize = 0;
    let mut ref_hori: isize = 0;
    let mut before = -1.0f64;
    for yo in 0..h {
        if refp.is_some() {
            if r < yo && yo < h - r {
                ref_vert += w_no;
            }
            ref_hori = ref_vert;
        }
        for xx in 0..w_no.max(0) as usize {
            let xo = r + xx;
            if let Some(rf) = refp {
                let read_r = real_radius * f64::from(rf[ref_hori as usize]);
                ref_hori += 1;
                if read_r != before {
                    gauss(brush, int_radius(read_r), read_r, sigma);
                    before = read_r;
                }
            }
            let mut accum = 0.0f64;
            let row = &inp[yo * w + xx..yo * w + xx + brush_diameter];
            for (a, b) in row.iter().zip(brush.iter()) {
                accum += *a * *b;
            }
            out[yo * w + xo] = accum;
        }
    }
}

/// `blur_2nd_vert_`: `inp` -> `out`.
#[allow(clippy::too_many_arguments)]
fn blur_2nd_vert(
    inp: &[f64],
    h: usize,
    w: usize,
    brush: &mut [f64],
    r: usize,
    out: &mut [f64],
    refp: Option<&[f32]>,
    real_radius: f64,
    sigma: f64,
) {
    let brush_diameter = 2 * r + 1;
    let h_no = h as isize - 2 * r as isize;
    let w_no = w as isize - 2 * r as isize;
    let mut ref_vert: isize = 0;
    let mut ref_hori: isize = 0;
    let mut before = -1.0f64;
    for xx in 0..w_no.max(0) as usize {
        let xo = r + xx;
        if refp.is_some() {
            ref_hori += 1;
            ref_vert = ref_hori;
        }
        for yy in 0..h_no.max(0) as usize {
            let yo = r + yy;
            if let Some(rf) = refp {
                let read_r = real_radius * f64::from(rf[ref_vert as usize]);
                ref_vert += w_no;
                if read_r != before {
                    gauss(brush, int_radius(read_r), read_r, sigma);
                    before = read_r;
                }
            }
            let mut accum = 0.0f64;
            for k in 0..brush_diameter {
                accum += inp[(yy + k) * w + xo] * brush[k];
            }
            out[yo * w + xo] = accum;
        }
    }
}

/// `diff_between_channel_`
fn diff_between_channel(inp: &[f32], h: usize, w: usize, ch: usize, ch1: usize, ch2: usize) -> bool {
    for i in 0..h * w {
        if inp[i * ch + ch1] != inp[i * ch + ch2] {
            return true;
        }
    }
    false
}

/// Everything after the argument checks (`convert_hv_`).
#[allow(clippy::too_many_arguments)]
fn convert_hv(
    inp: &[f32],
    out: &mut [f32],
    h: usize,
    w: usize,
    ch: usize,
    filter: &mut [f64],
    r: usize,
    plane_a: &mut [f64],
    plane_b: &mut [f64],
    refp: Option<&[f32]>,
    real_radius: f64,
    sigma: f64,
) {
    let h_no = h as isize - 2 * r as isize;
    let w_no = w as isize - 2 * r as isize;
    let mut diff_sw = true;
    for cc in 0..ch {
        if 0 < cc {
            diff_sw = diff_between_channel(inp, h, w, ch, cc - 1, cc);
        }
        if diff_sw {
            // get_
            for i in 0..h * w {
                plane_a[i] = f64::from(inp[i * ch + cc]);
            }
            blur_1st_hori(plane_a, h, w, filter, r, plane_b, refp, real_radius, sigma);
            blur_2nd_vert(plane_b, h, w, filter, r, plane_a, refp, real_radius, sigma);
        }
        // put_margin_
        if h_no > 0 && w_no > 0 {
            let mut o = cc;
            for yy in r..h - r {
                for xx in r..w - r {
                    out[o] = plane_a[yy * w + xx] as f32;
                    o += ch;
                }
            }
        }
    }
}

fn ranges_overlap(a: usize, a_len: usize, b: usize, b_len: usize) -> bool {
    a < b + b_len && b < a + a_len
}

/// `igs::gaussian_blur_hv::convert` on raw pointers.
///
/// # Safety
/// `in_with_margin` must hold `H*W*channels` floats, `out_no_margin`
/// `(H-2r)*(W-2r)*channels` floats, `refp` (when non-null) `H*W` floats and `buffer`
/// `buffer_bytes` bytes.
#[allow(clippy::too_many_arguments)]
pub unsafe fn convert_raw(
    in_with_margin: *const f32,
    out_no_margin: *mut f32,
    height_with_margin: i32,
    width_with_margin: i32,
    channels: i32,
    refp: *const f32,
    buffer: *mut u8,
    buffer_bytes: i32,
    int_radius: i32,
    real_radius: f64,
    sigma: f64,
) -> i32 {
    if real_radius <= 0.0 {
        return OK;
    }
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if height_with_margin < 1 || width_with_margin < 1 || int_radius < 1 {
        return BAD_ARG;
    }
    let (h, w, r, ch) = (height_with_margin as usize, width_with_margin as usize, int_radius as usize, channels as usize);

    // The buffer bookkeeping of the C++, including its int/size_t wrap-around.
    let mut bb = buffer_bytes;
    let sub = |bb: i32| ((bb as i64 as u64).wrapping_sub((width_with_margin as i64 as u64).wrapping_mul(8))) as u32 as i32;
    for _ in 0..h {
        bb = sub(bb);
        if bb <= 0 {
            return BUFFER_INN_EMPTY;
        }
    }
    for _ in 0..h {
        bb = sub(bb);
        if bb <= 0 {
            return BUFFER_OUT_EMPTY;
        }
    }

    let plane = match h.checked_mul(w) {
        Some(p) => p,
        None => return BAD_ARG,
    };
    let plane2 = match plane.checked_mul(2) {
        Some(p) => p,
        None => return BAD_ARG,
    };
    // What the loops above should have guaranteed; checked for real in case they wrapped.
    if plane2.checked_mul(8).map_or(true, |need| need as u64 > buffer_bytes.max(0) as u64) {
        return BUFFER_OUT_EMPTY;
    }
    if in_with_margin.is_null() || buffer.is_null() {
        return BAD_ARG;
    }

    let h_no = h as isize - 2 * r as isize;
    let w_no = w as isize - 2 * r as isize;
    if h_no <= 0 || w_no <= 0 {
        return OK; // the C++ blurred into its scratch planes and wrote nothing
    }
    let n_in = match plane.checked_mul(ch) {
        Some(n) => n,
        None => return BAD_ARG,
    };
    let n_out = h_no as usize * w_no as usize * ch;
    if out_no_margin.is_null() {
        return BAD_ARG;
    }
    if in_with_margin as usize % std::mem::align_of::<f32>() != 0
        || out_no_margin as usize % std::mem::align_of::<f32>() != 0
        || (!refp.is_null() && refp as usize % std::mem::align_of::<f32>() != 0)
    {
        return BAD_ARG;
    }
    if ranges_overlap(in_with_margin as usize, n_in * 4, out_no_margin as usize, n_out * 4) {
        return BAD_ARG;
    }

    let inp = std::slice::from_raw_parts(in_with_margin, n_in);
    let out = std::slice::from_raw_parts_mut(out_no_margin, n_out);
    let refs = if refp.is_null() { None } else { Some(std::slice::from_raw_parts(refp, plane)) };

    let mut own: Vec<f64>;
    let buf: &mut [f64] = if buffer as usize % std::mem::align_of::<f64>() == 0 {
        std::slice::from_raw_parts_mut(buffer as *mut f64, plane2)
    } else {
        own = vec![0.0; plane2];
        &mut own
    };
    let (plane_a, plane_b) = buf.split_at_mut(plane);

    let mut filter = vec![0.0f64; diameter_from_radius(int_radius) as usize];
    gauss(&mut filter, int_radius, real_radius, sigma);

    convert_hv(inp, out, h, w, ch, &mut filter, r, plane_a, plane_b, refs, real_radius, sigma);
    OK
}

#[no_mangle]
pub extern "C" fn flare_igs_gaussian_blur_int_radius(real_radius: f64) -> c_int {
    crate::guard(0, || int_radius(real_radius))
}

#[no_mangle]
pub extern "C" fn flare_igs_gaussian_blur_buffer_bytes(
    height_with_margin: c_int,
    width_with_margin: c_int,
    int_radius: c_int,
) -> c_int {
    crate::guard(0, || buffer_bytes(height_with_margin, width_with_margin, int_radius))
}

/// `igs::gaussian_blur_hv::convert`. Returns 0, 1, 3, 5 or 6 (see the constants).
///
/// # Safety
/// See [`convert_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_gaussian_blur_convert(
    in_with_margin: *const f32,
    out_no_margin: *mut f32,
    height_with_margin: c_int,
    width_with_margin: c_int,
    channels: c_int,
    refp: *const f32,
    buffer: *mut u8,
    buffer_bytes: c_int,
    int_radius: c_int,
    real_radius: f64,
    sigma: f64,
) -> c_int {
    crate::guard(BAD_ARG, || {
        convert_raw(
            in_with_margin,
            out_no_margin,
            height_with_margin,
            width_with_margin,
            channels,
            refp,
            buffer,
            buffer_bytes,
            int_radius,
            real_radius,
            sigma,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(
        inp: &[f32],
        h: i32,
        w: i32,
        ch: i32,
        refp: Option<&[f32]>,
        r: i32,
        real: f64,
        sigma: f64,
    ) -> (i32, Vec<f32>) {
        let (h_no, w_no) = ((h - 2 * r).max(0) as usize, (w - 2 * r).max(0) as usize);
        let mut out = vec![-1.0f32; h_no * w_no * ch as usize];
        let bb = buffer_bytes(h, w, r);
        let mut buf = vec![0.0f64; (bb as usize + 7) / 8];
        let st = unsafe {
            convert_raw(
                inp.as_ptr(),
                out.as_mut_ptr(),
                h,
                w,
                ch,
                refp.map_or(std::ptr::null(), |r| r.as_ptr()),
                buf.as_mut_ptr() as *mut u8,
                bb,
                r,
                real,
                sigma,
            )
        };
        (st, out)
    }

    fn img(h: usize, w: usize, ch: usize) -> Vec<f32> {
        (0..h * w * ch).map(|i| ((i * 37 + (i / 3) * 11 + (i / (w * ch)) * 5) % 256) as f32 / 255.0).collect()
    }

    #[test]
    fn int_radius_and_buffer_bytes() {
        assert_eq!(int_radius(0.2), 1);
        assert_eq!(int_radius(2.0), 2);
        assert_eq!(int_radius(2.0000001), 3);
        assert_eq!(int_radius(-0.5), 0);
        assert_eq!(int_radius(f64::NAN), 0);
        // 5x5 brush + two planes of 7x9
        assert_eq!(buffer_bytes(7, 9, 2), 25 * 8 + 63 * 8 * 2);
    }

    #[test]
    fn constant_image_stays_constant() {
        let inp = vec![0.5f32; 9 * 11 * 3];
        let (st, out) = run(&inp, 9, 11, 3, None, 2, 2.0, 0.25);
        assert_eq!(st, OK);
        assert_eq!(out.len(), 5 * 7 * 3);
        for v in out {
            assert!((v - 0.5).abs() < 1e-6, "{v}");
        }
    }

    #[test]
    fn impulse_spreads_symmetrically() {
        let (h, w) = (11usize, 11usize);
        let mut inp = vec![0.0f32; h * w];
        inp[5 * w + 5] = 1.0;
        let (st, out) = run(&inp, 11, 11, 1, None, 3, 3.0, 0.25);
        assert_eq!(st, OK);
        let n = 5usize; // 11 - 2*3
        let sum: f32 = out.iter().sum();
        // the 5x5 interior misses the tails of the 7x7 kernel
        assert!(sum > 0.99 && sum < 1.0001, "{sum}");
        assert!((out[2 * n + 1] - out[2 * n + 3]).abs() < 1e-7);
        assert!((out[1 * n + 2] - out[3 * n + 2]).abs() < 1e-7);
        assert!(out[2 * n + 2] > out[2 * n + 3]);
    }

    #[test]
    fn identical_channels_reuse_the_first_result() {
        let base = img(9, 11, 1);
        let mut rgba = Vec::new();
        for &v in &base {
            rgba.extend_from_slice(&[v, v, v, 1.0]);
        }
        let (st, out) = run(&rgba, 9, 11, 4, None, 2, 2.0, 0.25);
        assert_eq!(st, OK);
        for p in out.chunks(4) {
            assert_eq!(p[0], p[1]);
            assert_eq!(p[1], p[2]);
            assert!((p[3] - 1.0).abs() < 1e-6);
        }
    }

    #[test]
    fn non_positive_radius_returns_before_any_check() {
        let inp = vec![1.0f32; 4];
        // bad channels and bad sizes are not looked at
        let st = unsafe {
            convert_raw(inp.as_ptr(), std::ptr::null_mut(), 0, 0, 7, std::ptr::null(), std::ptr::null_mut(), 0, 0, 0.0, 0.25)
        };
        assert_eq!(st, OK);
        let st = unsafe {
            convert_raw(inp.as_ptr(), std::ptr::null_mut(), 0, 0, 7, std::ptr::null(), std::ptr::null_mut(), 0, 0, -1.0, 0.25)
        };
        assert_eq!(st, OK);
    }

    #[test]
    fn errors() {
        let inp = img(9, 11, 2);
        let (st, _) = run(&inp, 9, 11, 2, None, 2, 2.0, 0.25);
        assert_eq!(st, BAD_CHANNELS);
        let inp = img(9, 11, 1);
        assert_eq!(run(&inp, 9, 11, 1, None, 0, 2.0, 0.25).0, BAD_ARG);
        assert_eq!(run(&inp, 0, 11, 1, None, 2, 2.0, 0.25).0, BAD_ARG);
        // too small a buffer: the first plane check trips
        let mut out = vec![0.0f32; 5 * 7];
        let mut buf = vec![0.0f64; 1024];
        let st = unsafe {
            convert_raw(inp.as_ptr(), out.as_mut_ptr(), 9, 11, 1, std::ptr::null(), buf.as_mut_ptr() as *mut u8, 9 * 11 * 8 + 1, 2, 2.0, 0.25)
        };
        assert_eq!(st, BUFFER_OUT_EMPTY);
        let st = unsafe {
            convert_raw(inp.as_ptr(), out.as_mut_ptr(), 9, 11, 1, std::ptr::null(), buf.as_mut_ptr() as *mut u8, 11 * 8, 2, 2.0, 0.25)
        };
        assert_eq!(st, BUFFER_INN_EMPTY);
        // null pointers
        let st = unsafe {
            convert_raw(std::ptr::null(), out.as_mut_ptr(), 9, 11, 1, std::ptr::null(), buf.as_mut_ptr() as *mut u8, buffer_bytes(9, 11, 2), 2, 2.0, 0.25)
        };
        assert_eq!(st, BAD_ARG);
        // margin swallows the image: nothing to write
        let (st, out) = run(&img(4, 4, 1), 4, 4, 1, None, 2, 2.0, 0.25);
        assert_eq!(st, OK);
        assert!(out.is_empty());
    }

    #[test]
    fn oversized_reference_radius_is_clamped_not_out_of_bounds() {
        // reference of 3.0 with real radius 2.0 -> per-pixel radius 6 > margin 2
        let inp = img(9, 11, 1);
        let refp = vec![3.0f32; 9 * 11];
        let (st, out) = run(&inp, 9, 11, 1, Some(&refp), 2, 2.0, 0.25);
        assert_eq!(st, OK);
        assert!(out.iter().all(|v| v.is_finite()));
        // negative / NaN references fall back to the minimum brush
        let mut refp = vec![0.5f32; 9 * 11];
        refp[10] = -2.0;
        refp[20] = f32::NAN;
        let (st, out) = run(&inp, 9, 11, 1, Some(&refp), 2, 2.0, 0.25);
        assert_eq!(st, OK);
        assert!(out.iter().all(|v| v.is_finite()));
    }

    #[test]
    fn reference_changes_the_result() {
        let inp = img(9, 11, 1);
        let (_, a) = run(&inp, 9, 11, 1, None, 2, 2.0, 0.25);
        let refp = vec![0.5f32; 9 * 11];
        let (_, b) = run(&inp, 9, 11, 1, Some(&refp), 2, 2.0, 0.25);
        assert_ne!(a, b);
        let ones = vec![1.0f32; 9 * 11];
        let (_, c) = run(&inp, 9, 11, 1, Some(&ones), 2, 2.0, 0.25);
        assert_eq!(a, c, "reference 1.0 is the plain blur");
    }

    // --- differential golden -------------------------------------------------------

    fn ref_img(h: usize, w: usize) -> Vec<f32> {
        (0..h * w).map(|i| ((i * 37) % 101) as f32 / 100.0).collect()
    }

    /// input styles of the golden: 0 = pattern, 1 = rgb channels equal (alpha differs)
    fn input(style: i32, h: usize, w: usize, ch: usize) -> Vec<f32> {
        let mut v = img(h, w, ch);
        if style == 1 {
            for p in v.chunks_mut(ch) {
                for c in 1..ch.min(3) {
                    p[c] = p[0];
                }
            }
        }
        v
    }

    fn f32_ulps(a: f32, b: f32) -> u32 {
        (a.to_bits() as i32 - b.to_bits() as i32).unsigned_abs()
    }

    /// Replays `gen_igs_gaussian_blur_cases.py`: the original `igs_gaussian_blur.cpp`
    /// (+ the original `igs_gauss_distribution.cpp`, both from git) over 14 cases. The
    /// cases carry their own parameters; the output floats are compared bit for bit and,
    /// when the host `exp` differs, within 4 float ULP.
    #[test]
    fn matches_the_cpp_reference() {
        let golden = include_str!("igs_gaussian_blur_cases.txt");
        let mut lines = golden.lines();
        let mut n = 0;
        let mut exact = 0;
        while let Some(head) = lines.next() {
            let t: Vec<&str> = head.split_whitespace().collect();
            assert_eq!(t[0], "case", "{head}");
            let (h, w, ch, r): (usize, usize, usize, i32) =
                (t[1].parse().unwrap(), t[2].parse().unwrap(), t[3].parse().unwrap(), t[4].parse().unwrap());
            let real = f64::from_bits(u64::from_str_radix(t[5], 16).unwrap());
            let sigma = f64::from_bits(u64::from_str_radix(t[6], 16).unwrap());
            let use_ref = t[7] == "1";
            let style: i32 = t[8].parse().unwrap();
            let want: Vec<u32> = lines
                .next()
                .unwrap()
                .strip_prefix("out")
                .unwrap()
                .split_whitespace()
                .map(|x| u32::from_str_radix(x, 16).unwrap())
                .collect();
            let inp = input(style, h, w, ch);
            let refp = ref_img(h, w);
            let (st, got) = run(&inp, h as i32, w as i32, ch as i32, use_ref.then_some(&refp[..]), r, real, sigma);
            assert_eq!(st, OK, "{head}");
            // untouched output buffer for the no-op case is -1.0
            assert_eq!(got.len(), want.len(), "{head}");
            let mut all_exact = true;
            for (i, (g, wt)) in got.iter().zip(&want).enumerate() {
                if g.to_bits() != *wt {
                    all_exact = false;
                    assert!(f32_ulps(*g, f32::from_bits(*wt)) <= 4, "{head}: sample {i}: {g} vs {}", f32::from_bits(*wt));
                }
            }
            exact += usize::from(all_exact);
            n += 1;
        }
        assert_eq!(n, 14);
        assert!(exact > 0);
    }
}
