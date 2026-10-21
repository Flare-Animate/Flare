//! Port of stdfx/igs_motion_wind.cpp: the scanline walk of the motion-wind effect.
//!
//! The per-pixel decision lives in `igs_motion_wind_pixel` (already Rust); this file is
//! the loop that feeds it: pop a pixel to normalised doubles, optionally look up a
//! reference value, call `MotionWindPixel::change`, push the pixel back when it changed.
//!
//! Quirks of the C++ kept on purpose:
//! * the vertical walk (`direction` 1/3) loops x outermost and resets the wind key on
//!   `xx <= 0` (not on `yy <= 0`), so only the first column restarts the key;
//! * both walks sample the reference at `(xx, yy + refprev)` (the horizontal walk does
//!   not use `xx + refprev`), wrapping around the reference image;
//! * `length_max <= 0 && length_min <= 0` returns before the `bits` check, so a bad
//!   `bits` is silently accepted in that case;
//! * an unknown `direction` leaves the image untouched.
//!
//! Deviations (only where the C++ was undefined behaviour or could not finish):
//! * `channels` outside 1..=4 is refused (the C++ overran a `double[4]` on the stack);
//! * a reference image with `rw`/`rh`/`rc` < 1 that is actually sampled is refused (the
//!   C++ `while (xx < 0) xx += ww` loops forever for `ww == 0`);
//! * a wind-table size above 2^26 doubles is refused as `ERR_TOO_LARGE`;
//! * the push back `static_cast<T>(double)` saturates instead of being UB when a value
//!   leaves `[0, 1]`.
//!
//! C ABI: `flare_igs_motion_wind_change`; the header `igs_motion_wind.h` keeps the old
//! C++ signature as an inline forwarder and rethrows `std::domain_error` for status 2.

use crate::igs_levels::{ref_value, RefPx};
use crate::igs_motion_wind_pixel::MotionWindPixel;
use crate::igs_motion_wind_table;
use std::os::raw::{c_int, c_ulong};

pub const OK: i32 = 0;
/// `std::domain_error("Bad bits,Not uchar/ushort")`
pub const BAD_BITS: i32 = 2;
/// `std::invalid_argument`
pub const BAD_ARG: i32 = 3;
/// `std::bad_alloc`
pub const TOO_LARGE: i32 = 4;

const MAX_TABLE: i32 = 1 << 26;

/// Image sample types of the in-place walk.
pub(crate) trait Sample: Copy {
    const MAX: u32;
    fn to_f64(self) -> f64;
    /// `static_cast<T>(double)` (saturating here)
    fn from_f64(v: f64) -> Self;
}
impl Sample for u8 {
    const MAX: u32 = u8::MAX as u32;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> u8 {
        v as u8
    }
}
impl Sample for u16 {
    const MAX: u32 = u16::MAX as u32;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> u16 {
        v as u16
    }
}

/// Reference image: samples + `(rw, rh, rc)`.
#[derive(Clone, Copy)]
pub(crate) struct RefImage<'a, R> {
    pub data: &'a [R],
    pub rw: i32,
    pub rh: i32,
    pub rc: i32,
}

/// Start index of reference pixel `(xx, yy)` wrapped into the image (`ref_pixel_`).
fn ref_pixel_index(rh: i32, rw: i32, rc: i32, xx: i32, yy: i32) -> usize {
    let x = i64::from(xx).rem_euclid(i64::from(rw));
    let y = i64::from(yy).rem_euclid(i64::from(rh));
    (i64::from(rc) * i64::from(rw) * y + i64::from(rc) * x) as usize
}

fn lookup_ref<R: RefPx>(refimg: &Option<RefImage<R>>, ref_mode: i32, xx: i32, yy: i32) -> f64 {
    let mut ref_val = -1.0;
    if let Some(r) = refimg {
        if (0..=4).contains(&ref_mode) {
            let i = ref_pixel_index(r.rh, r.rw, r.rc, xx, yy);
            ref_val = ref_value(&r.data[i..i + r.rc as usize], r.rc as usize, R::MAX, ref_mode);
        }
    }
    ref_val
}

fn pop<T: Sample>(src: &[T], div: f64, out: &mut [f64]) {
    for (o, s) in out.iter_mut().zip(src) {
        *o = s.to_f64() / div;
    }
}

fn push<T: Sample>(src: &[f64], mul: f64, out: &mut [T]) {
    for (o, s) in out.iter_mut().zip(src) {
        *o = T::from_f64(*s * mul);
    }
}

/// `change_template_vert_`
#[allow(clippy::too_many_arguments)]
pub(crate) fn change_vert<T: Sample, R: RefPx>(
    image: &mut [T],
    ww: i32,
    hh: i32,
    cc: i32,
    refimg: Option<RefImage<R>>,
    ref_mode: i32,
    pix: &mut MotionWindPixel,
    reverse: bool,
) {
    let div = f64::from(T::MAX);
    let mul = div + 0.999999;
    let refprev = if reverse { 1 } else { -1 };
    let c = cc as usize;
    for xx in 0..ww {
        for yy in 0..hh {
            let row = if reverse { hh - 1 - yy } else { yy };
            let next = row as usize * ww as usize * c + xx as usize * c;
            let mut pixel_in = [0.0f64; 4];
            pop(&image[next..next + c], div, &mut pixel_in[..c]);
            let ref_val = lookup_ref(&refimg, ref_mode, xx, yy + refprev);
            if pix.change(xx <= 0, ref_val, cc, &mut pixel_in[..c]) {
                push(&pixel_in[..c], mul, &mut image[next..next + c]);
            }
        }
    }
}

/// `change_template_hori_`
#[allow(clippy::too_many_arguments)]
pub(crate) fn change_hori<T: Sample, R: RefPx>(
    image: &mut [T],
    ww: i32,
    hh: i32,
    cc: i32,
    refimg: Option<RefImage<R>>,
    ref_mode: i32,
    pix: &mut MotionWindPixel,
    reverse: bool,
) {
    let div = f64::from(T::MAX);
    let mul = div + 0.999999;
    let refprev = if reverse { 1 } else { -1 };
    let c = cc as usize;
    for yy in 0..hh {
        let row0 = yy as usize * ww as usize * c;
        for xx in 0..ww {
            let col = if reverse { ww - 1 - xx } else { xx };
            let next = row0 + col as usize * c;
            let mut pixel_in = [0.0f64; 4];
            pop(&image[next..next + c], div, &mut pixel_in[..c]);
            let ref_val = lookup_ref(&refimg, ref_mode, xx, yy + refprev);
            if pix.change(xx <= 0, ref_val, cc, &mut pixel_in[..c]) {
                push(&pixel_in[..c], mul, &mut image[next..next + c]);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn walk<T: Sample, R: RefPx>(
    image: &mut [T],
    ww: i32,
    hh: i32,
    cc: i32,
    refimg: Option<RefImage<R>>,
    ref_mode: i32,
    pix: &mut MotionWindPixel,
    direction: i32,
) {
    if direction == 0 || direction == 2 {
        change_hori(image, ww, hh, cc, refimg, ref_mode, pix, direction != 0);
    } else if direction == 1 || direction == 3 {
        change_vert(image, ww, hh, cc, refimg, ref_mode, pix, direction != 1);
    }
}

/// `igs::motion_wind::change` argument bundle (everything after the buffers).
#[derive(Clone, Copy)]
pub struct Params {
    pub ref_mode: i32,
    pub direction: i32,
    pub blow_dark_sw: bool,
    pub blow_alpha_sw: bool,
    pub length_random_seed: u64,
    pub length_min: f64,
    pub length_max: f64,
    pub length_bias: f64,
    pub length_ref_sw: bool,
    pub force_random_seed: u64,
    pub force_min: f64,
    pub force_max: f64,
    pub force_bias: f64,
    pub force_ref_sw: bool,
    pub density_random_seed: u64,
    pub density_min: f64,
    pub density_max: f64,
    pub density_bias: f64,
    pub density_ref_sw: bool,
}

impl Params {
    fn pixel(&self) -> MotionWindPixel {
        MotionWindPixel::new(
            self.blow_dark_sw,
            self.blow_alpha_sw,
            self.length_random_seed,
            self.length_min,
            self.length_max,
            self.length_bias,
            self.length_ref_sw,
            self.force_random_seed,
            self.force_min,
            self.force_max,
            self.force_bias,
            self.force_ref_sw,
            self.density_random_seed,
            self.density_min,
            self.density_max,
            self.density_bias,
            self.density_ref_sw,
        )
    }
}

/// Typed core: `image` is `T`, the optional reference is `R`.
pub(crate) fn change_typed<T: Sample, R: RefPx>(
    image: &mut [T],
    hh: i32,
    ww: i32,
    cc: i32,
    refimg: Option<RefImage<R>>,
    p: &Params,
) -> i32 {
    if !(1..=4).contains(&cc) {
        return BAD_ARG;
    }
    if igs_motion_wind_table::table_size(p.length_min, p.length_max) > MAX_TABLE {
        return TOO_LARGE;
    }
    if let Some(r) = &refimg {
        if (0..=4).contains(&p.ref_mode) {
            let sampled = ww > 0 && hh > 0 && (0..=3).contains(&p.direction);
            if sampled && (r.rw < 1 || r.rh < 1 || r.rc < 1) {
                return BAD_ARG;
            }
        }
    }
    if ww <= 0 || hh <= 0 {
        return OK; // empty loops in the C++
    }
    let mut pix = p.pixel();
    walk(image, ww, hh, cc, refimg, p.ref_mode, &mut pix, p.direction);
    pix.clear();
    OK
}

/// Byte-buffer entry point (what the C ABI forwards to): dispatches on `bb`/`rb`
/// exactly like the C++ (`rb == 16` selects a 16-bit reference, anything else is read
/// as 8-bit, including a missing reference).
///
/// # Safety
/// `inp` must be valid for `hh*ww*cc` samples of `bb` bits (aligned for 16-bit) and
/// `refp`, when non-null, for `rh*rw*rc` samples of `rb` bits.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    inp: *mut u8,
    hh: i32,
    ww: i32,
    cc: i32,
    bb: i32,
    refp: *const u8,
    rh: i32,
    rw: i32,
    rc: i32,
    rb: i32,
    p: &Params,
) -> i32 {
    if p.length_max <= 0.0 && p.length_min <= 0.0 {
        return OK;
    }
    if bb != 8 && bb != 16 {
        return BAD_BITS;
    }
    let n = if ww > 0 && hh > 0 && cc > 0 {
        match (ww as usize).checked_mul(hh as usize).and_then(|v| v.checked_mul(cc as usize)) {
            Some(n) => n,
            None => return BAD_ARG,
        }
    } else {
        0
    };
    if n > 0 && inp.is_null() {
        return BAD_ARG;
    }
    if bb == 16 && n > 0 && (inp as usize) % std::mem::align_of::<u16>() != 0 {
        return BAD_ARG;
    }
    let rn = if !refp.is_null() && rw > 0 && rh > 0 && rc > 0 {
        match (rw as usize).checked_mul(rh as usize).and_then(|v| v.checked_mul(rc as usize)) {
            Some(n) => n,
            None => return BAD_ARG,
        }
    } else {
        0
    };
    let ref16 = rb == 16;
    if ref16 && rn > 0 && (refp as usize) % std::mem::align_of::<u16>() != 0 {
        return BAD_ARG;
    }
    // A non-null reference with a degenerate size is only an error when it is sampled
    // (checked in `change_typed`); give it an empty slice so it can be reported there.
    macro_rules! mk_ref {
        ($t:ty) => {
            if refp.is_null() {
                None
            } else {
                Some(RefImage {
                    data: if rn > 0 { std::slice::from_raw_parts(refp as *const $t, rn) } else { &[] },
                    rw,
                    rh,
                    rc,
                })
            }
        };
    }
    macro_rules! run {
        ($t:ty) => {{
            let img: &mut [$t] = if n > 0 {
                std::slice::from_raw_parts_mut(inp as *mut $t, n)
            } else {
                &mut []
            };
            if ref16 {
                let r: Option<RefImage<u16>> = mk_ref!(u16);
                change_typed(img, hh, ww, cc, r, p)
            } else {
                let r: Option<RefImage<u8>> = mk_ref!(u8);
                change_typed(img, hh, ww, cc, r, p)
            }
        }};
    }
    if bb == 8 {
        run!(u8)
    } else {
        run!(u16)
    }
}

/// `igs::motion_wind::change`. Returns 0, 2 (bad bits), 3 (bad argument), 4 (too large).
///
/// # Safety
/// See [`change_raw`]; `inp` is modified in place.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::useless_conversion)] // c_ulong is u64 on LP64, u32 on LLP64
pub unsafe extern "C" fn flare_igs_motion_wind_change(
    inp: *mut u8,
    hh: c_int,
    ww: c_int,
    cc: c_int,
    bb: c_int,
    refp: *const u8,
    rh: c_int,
    rw: c_int,
    rc: c_int,
    rb: c_int,
    ref_mode: c_int,
    direction: c_int,
    blow_dark_sw: bool,
    blow_alpha_sw: bool,
    length_random_seed: c_ulong,
    length_min: f64,
    length_max: f64,
    length_bias: f64,
    length_ref_sw: bool,
    force_random_seed: c_ulong,
    force_min: f64,
    force_max: f64,
    force_bias: f64,
    force_ref_sw: bool,
    density_random_seed: c_ulong,
    density_min: f64,
    density_max: f64,
    density_bias: f64,
    density_ref_sw: bool,
) -> c_int {
    let p = Params {
        ref_mode,
        direction,
        blow_dark_sw,
        blow_alpha_sw,
        length_random_seed: u64::from(length_random_seed),
        length_min,
        length_max,
        length_bias,
        length_ref_sw,
        force_random_seed: u64::from(force_random_seed),
        force_min,
        force_max,
        force_bias,
        force_ref_sw,
        density_random_seed: u64::from(density_random_seed),
        density_min,
        density_max,
        density_bias,
        density_ref_sw,
    };
    crate::guard(BAD_ARG, || change_raw(inp, hh, ww, cc, bb, refp, rh, rw, rc, rb, &p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> Params {
        Params {
            ref_mode: 5,
            direction: 0,
            blow_dark_sw: false,
            blow_alpha_sw: true,
            length_random_seed: 1,
            length_min: 2.0,
            length_max: 6.0,
            length_bias: 1.0,
            length_ref_sw: false,
            force_random_seed: 1,
            force_min: 1.0,
            force_max: 1.0,
            force_bias: 1.0,
            force_ref_sw: false,
            density_random_seed: 1,
            density_min: 1.0,
            density_max: 1.0,
            density_bias: 1.0,
            density_ref_sw: false,
        }
    }

    fn call(img: &mut [u8], hh: i32, ww: i32, cc: i32, bb: i32, p: &Params) -> i32 {
        unsafe { change_raw(img.as_mut_ptr(), hh, ww, cc, bb, std::ptr::null(), 0, 0, 0, 0, p) }
    }

    /// bright pixel, then dark ones: the dark run takes the bright key's colour
    #[test]
    fn wind_trails_a_bright_pixel() {
        let mut img = vec![255u8, 0, 0, 0, 0, 0];
        let st = call(&mut img, 1, 6, 1, 8, &params());
        assert_eq!(st, OK);
        assert_eq!(img[0], 255);
        assert!(img[1] > 0, "pixel behind the bright one is lit: {img:?}");
        assert!(img.windows(2).all(|w| w[0] >= w[1]), "monotone fade: {img:?}");
    }

    #[test]
    fn reverse_direction_mirrors() {
        let mut a = vec![255u8, 0, 0, 0, 0, 0, 0, 0];
        let mut b = vec![0u8, 0, 0, 0, 0, 0, 0, 255];
        call(&mut a, 1, 8, 1, 8, &params());
        let mut p = params();
        p.direction = 2;
        call(&mut b, 1, 8, 1, 8, &p);
        a.reverse();
        assert_eq!(a, b);
    }

    #[test]
    fn zero_length_is_a_noop_even_with_bad_bits() {
        let mut p = params();
        p.length_min = 0.0;
        p.length_max = 0.0;
        let mut img = vec![9u8; 4];
        assert_eq!(call(&mut img, 1, 4, 1, 12, &p), OK);
        assert_eq!(img, vec![9u8; 4]);
    }

    #[test]
    fn bad_bits_and_bad_arguments() {
        let mut img = vec![0u8; 8];
        assert_eq!(call(&mut img, 1, 8, 1, 12, &params()), BAD_BITS);
        assert_eq!(call(&mut img, 1, 8, 5, 8, &params()), BAD_ARG);
        assert_eq!(call(&mut img, 1, 8, 0, 8, &params()), BAD_ARG);
        // null image with a non-empty size
        let st = unsafe {
            change_raw(std::ptr::null_mut(), 1, 8, 1, 8, std::ptr::null(), 0, 0, 0, 0, &params())
        };
        assert_eq!(st, BAD_ARG);
        // empty image is a no-op
        assert_eq!(call(&mut [], 0, 0, 1, 8, &params()), OK);
        // huge wind table
        let mut p = params();
        p.length_max = 1e12;
        assert_eq!(call(&mut img, 1, 8, 1, 8, &p), TOO_LARGE);
    }

    #[test]
    fn unknown_direction_leaves_the_image() {
        let mut p = params();
        p.direction = 9;
        let mut img = vec![255u8, 0, 0, 0];
        assert_eq!(call(&mut img, 1, 4, 1, 8, &p), OK);
        assert_eq!(img, vec![255u8, 0, 0, 0]);
    }

    #[test]
    fn degenerate_reference_is_refused_not_looped() {
        let mut p = params();
        p.ref_mode = 0;
        let mut img = vec![255u8, 0, 0, 0];
        let r = [1u8];
        let st = unsafe { change_raw(img.as_mut_ptr(), 1, 4, 1, 8, r.as_ptr(), 0, 0, 1, 8, &p) };
        assert_eq!(st, BAD_ARG);
        // ref_mode 5 never samples the reference
        p.ref_mode = 5;
        let st = unsafe { change_raw(img.as_mut_ptr(), 1, 4, 1, 8, r.as_ptr(), 0, 0, 1, 8, &p) };
        assert_eq!(st, OK);
    }

    #[test]
    fn misaligned_16_bit_is_refused() {
        let mut buf = vec![0u8; 17];
        let st = unsafe {
            change_raw(buf.as_mut_ptr().add(1), 1, 4, 2, 16, std::ptr::null(), 0, 0, 0, 0, &params())
        };
        // either alignment happens to be fine (then OK) or it is refused; never a crash
        assert!(st == OK || st == BAD_ARG);
    }

    #[test]
    fn ref_pixel_wraps() {
        assert_eq!(ref_pixel_index(3, 4, 2, -1, -1), 2 * 4 * 2 + 2 * 3);
        assert_eq!(ref_pixel_index(3, 4, 2, 4, 3), 0);
    }

    // --- differential golden -------------------------------------------------------

    fn fnv(bytes: impl Iterator<Item = u8>) -> u64 {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in bytes {
            h ^= u64::from(b);
            h = h.wrapping_mul(0x100000001b3);
        }
        h
    }

    fn val(bits: i32, i: usize, y: usize) -> u32 {
        if bits == 8 {
            ((i * 37 + (i / 3) * 11 + y * 5) % 256) as u32
        } else {
            ((i * 2579 + y * 97 + 13) % 65536) as u32
        }
    }

    fn fill<T: Sample>(n: usize, per_row: usize, bits: i32, conv: impl Fn(u32) -> T) -> Vec<T> {
        (0..n).map(|i| conv(val(bits, i, i / per_row))).collect()
    }

    /// Replays `gen_igs_motion_wind_change_cases.py`: the original
    /// `igs_motion_wind.cpp` (+ pixel + table, from git) over a fixed matrix of cases
    /// (2 configs x 8/16-bit x 4/3/1 channels x 4 directions x dark x alpha x
    /// {no ref, 8-bit ref, 16-bit ref}), compared by FNV-1a of the output bytes.
    /// Every config is arithmetic-only (bias 1, force 1), so the hashes are exact on any
    /// platform; the golden for the host's `unsigned long` width is selected.
    #[test]
    fn matches_the_cpp_reference() {
        let golden: &str = if std::mem::size_of::<c_ulong>() == 8 {
            include_str!("igs_motion_wind_change_cases.txt")
        } else {
            include_str!("igs_motion_wind_change_cases_32.txt")
        };
        let mut lines = golden.lines();
        let (ww, hh, rw, rh) = (11i32, 7i32, 5i32, 4i32);
        let mut count = 0;
        for cfg in 0..2 {
            for &bits in &[8, 16] {
                for &cc in &[4, 3, 1] {
                    for dir in 0..4 {
                        for dark in [false, true] {
                            for alpha in [false, true] {
                                for refk in 0..3 {
                                    let mut p = Params {
                                        ref_mode: if cc == 4 { 4 } else { 0 },
                                        direction: dir,
                                        blow_dark_sw: dark,
                                        blow_alpha_sw: alpha,
                                        length_random_seed: 1,
                                        length_min: 2.0,
                                        length_max: 6.0,
                                        length_bias: 1.0,
                                        length_ref_sw: false,
                                        force_random_seed: 1,
                                        force_min: 1.0,
                                        force_max: 1.0,
                                        force_bias: 1.0,
                                        force_ref_sw: false,
                                        density_random_seed: 1,
                                        density_min: 1.0,
                                        density_max: 1.0,
                                        density_bias: 1.0,
                                        density_ref_sw: false,
                                    };
                                    if cfg == 1 {
                                        p.length_random_seed = 3;
                                        p.length_min = 3.0;
                                        p.length_max = 9.0;
                                        p.length_ref_sw = true;
                                        p.force_random_seed = 5;
                                        p.density_random_seed = 7;
                                        p.density_min = 0.8;
                                    }
                                    let n = (ww * hh * cc) as usize;
                                    let rn = (rw * rh * cc) as usize;
                                    let rbits = if refk == 1 { 8 } else { 16 };
                                    let got = if bits == 8 {
                                        let mut img = fill(n, (ww * cc) as usize, 8, |v| v as u8);
                                        let st = run_ref(&mut img, hh, ww, cc, refk, rbits, rh, rw, rn, &p);
                                        assert_eq!(st, OK);
                                        fnv(img.iter().copied())
                                    } else {
                                        let mut img = fill(n, (ww * cc) as usize, 16, |v| v as u16);
                                        let st = run_ref(&mut img, hh, ww, cc, refk, rbits, rh, rw, rn, &p);
                                        assert_eq!(st, OK);
                                        fnv(img.iter().flat_map(|v| v.to_le_bytes()))
                                    };
                                    let want = lines.next().unwrap_or_else(|| panic!("golden ends at case {count}"));
                                    assert_eq!(
                                        want,
                                        format!("case {count} {got:016x}"),
                                        "cfg {cfg} bits {bits} cc {cc} dir {dir} dark {dark} alpha {alpha} ref {refk}"
                                    );
                                    count += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(count, 576);
        assert!(lines.next().is_none(), "golden has extra lines");
    }

    #[allow(clippy::too_many_arguments)]
    fn run_ref<T: Sample>(
        img: &mut [T],
        hh: i32,
        ww: i32,
        cc: i32,
        refk: i32,
        rbits: i32,
        rh: i32,
        rw: i32,
        rn: usize,
        p: &Params,
    ) -> i32 {
        let bytes = std::mem::size_of::<T>() as i32 * 8;
        match refk {
            0 => unsafe { change_raw(img.as_mut_ptr() as *mut u8, hh, ww, cc, bytes, std::ptr::null(), 0, 0, 0, 0, p) },
            _ => {
                if rbits == 8 {
                    let r = fill(rn, (rw * cc) as usize, 8, |v| v as u8);
                    unsafe { change_raw(img.as_mut_ptr() as *mut u8, hh, ww, cc, bytes, r.as_ptr(), rh, rw, cc, 8, p) }
                } else {
                    let r = fill(rn, (rw * cc) as usize, 16, |v| v as u16);
                    unsafe {
                        change_raw(img.as_mut_ptr() as *mut u8, hh, ww, cc, bytes, r.as_ptr() as *const u8, rh, rw, cc, 16, p)
                    }
                }
            }
        }
    }
}
