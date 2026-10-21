//! Port of stdfx/igs_maxmin.cpp and of the two headers only it used
//! (`igs_maxmin_getput.h`, `igs_maxmin_multithread.h`): the max/min ("dilate / erode")
//! effect on 8-bit, 16-bit and float images with an optional per-pixel strength
//! reference.
//!
//! Per channel the image is walked scanline by scanline; for each scanline the
//! `odd_diameter` surrounding rows are loaded as `double` tracks (edges repeated),
//! `igs_maxmin_slrender::render` computes the max/min through the lens
//! (`igs_maxmin_lens_matrix`) and the row is written back. RGBA images render alpha
//! first, then B, G, R; the alpha value can scale the strength of the colour channels
//! (`add_blend_sw`) and the reference scales the lens radius per pixel.
//!
//! Threads: like the C++, the rows are split into `number_of_thread` bands (adjacent
//! bands share one row, which both compute and both write with identical values).
//!
//! Deviations (only where the C++ was undefined behaviour or could not finish):
//! * **data race removed**: the C++ threads shared one lens matrix, and with an alpha /
//!   reference radius (every RGBA image) each scanline pass *reshaped* it while other
//!   threads were rendering from it, so multi-threaded RGBA output could, rarely, use a
//!   half-written lens. Each Rust band owns a copy of the lens; with one thread the
//!   result is identical to the C++, with several it is the deterministic result;
//! * the shared overlap row is written through relaxed atomics (same values from both
//!   bands) so it is not a data race in the Rust memory model either;
//! * a float *reference* image: `numeric_limits<float>::max()` was stored in an `int`
//!   (out-of-range conversion, UB); `ref_value` is given 1, as the other ported igs
//!   effects (`igs_levels`, ...) do for float references;
//! * `static_cast<T>(double)` when storing saturates; NaN radius / roll, a lens of more
//!   than 1 GiB, polygons above 2^20 corners and |roll| above 1e7 degrees are refused
//!   (`std::length_error` / `bad_alloc` / endless loops in the C++); negative sizes,
//!   null, misaligned or overlapping buffers are refused instead of crashing.
//!
//! C ABI: `flare_igs_maxmin_convert`; `igs_maxmin.h` keeps the C++ signature as an
//! inline forwarder and rethrows `std::domain_error` (status 1, 2) /
//! `std::invalid_argument` (3) / `std::bad_alloc` (4).

use crate::igs_levels::{ref_value, RefPx};
use crate::igs_maxmin_lens_matrix::{alloc_and_shape_lens_matrix, Lens, LensError};
use crate::igs_maxmin_slrender::{render, Scanline};
use std::os::raw::c_int;
use std::sync::atomic::{AtomicU16, AtomicU32, AtomicU8, Ordering};

pub const OK: i32 = 0;
/// `std::domain_error("Bad channels,Not rgba/rgb/grayscale")`
pub const BAD_CHANNELS: i32 = 1;
/// `std::domain_error("Bad bits,Not uchar/ushort/float")`
pub const BAD_BITS: i32 = 2;
/// `std::invalid_argument`
pub const BAD_ARG: i32 = 3;
/// `std::bad_alloc`
pub const TOO_LARGE: i32 = 4;

const MAX_POLYGON: i32 = 1 << 20;
const MAX_ROLL: f64 = 1.0e7;

// Toonz6 channel order (igs_ifx_common.h): blu = 0, gre, red, alp
const BLU: i32 = 0;
const GRE: i32 = 1;
const RED: i32 = 2;
const ALP: i32 = 3;

/// Image sample types of the effect (`unsigned char`, `unsigned short`, `float`).
pub(crate) trait Px: Copy + Send + Sync + 'static {
    /// `div_val`: integer maximum, 1 for float
    const DIV: f64;
    fn to_f64(self) -> f64;
    /// `put`: `static_cast<T>(v * (max + 0.999999))` / `static_cast<float>(v)`
    fn from_result(v: f64) -> Self;
    /// # Safety
    /// `p` valid and aligned; concurrent access only through these atomics.
    unsafe fn load(p: *const Self) -> Self;
    /// # Safety
    /// as `load`
    unsafe fn store(p: *mut Self, v: Self);
}

impl Px for u8 {
    const DIV: f64 = u8::MAX as f64;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_result(v: f64) -> u8 {
        (v * (Self::DIV + 0.999999)) as u8
    }
    unsafe fn load(p: *const u8) -> u8 {
        AtomicU8::from_ptr(p as *mut u8).load(Ordering::Relaxed)
    }
    unsafe fn store(p: *mut u8, v: u8) {
        AtomicU8::from_ptr(p).store(v, Ordering::Relaxed)
    }
}
impl Px for u16 {
    const DIV: f64 = u16::MAX as f64;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_result(v: f64) -> u16 {
        (v * (Self::DIV + 0.999999)) as u16
    }
    unsafe fn load(p: *const u16) -> u16 {
        AtomicU16::from_ptr(p as *mut u16).load(Ordering::Relaxed)
    }
    unsafe fn store(p: *mut u16, v: u16) {
        AtomicU16::from_ptr(p).store(v, Ordering::Relaxed)
    }
}
impl Px for f32 {
    const DIV: f64 = 1.0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_result(v: f64) -> f32 {
        v as f32
    }
    unsafe fn load(p: *const f32) -> f32 {
        f32::from_bits(AtomicU32::from_ptr(p as *mut u32).load(Ordering::Relaxed))
    }
    unsafe fn store(p: *mut f32, v: f32) {
        AtomicU32::from_ptr(p as *mut u32).store(v.to_bits(), Ordering::Relaxed)
    }
}

/// Raw output pointer shared by the band threads (written through atomics only).
struct OutPtr<T>(*mut T);
impl<T> Clone for OutPtr<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for OutPtr<T> {}
// SAFETY: the pointee is only accessed through relaxed atomics (`Px::load/store`).
unsafe impl<T> Send for OutPtr<T> {}
unsafe impl<T> Sync for OutPtr<T> {}

/// Effect parameters (everything but the buffers).
#[derive(Clone, Copy, Debug)]
pub struct Params {
    pub ref_mode: i32,
    pub radius: f64,
    pub smooth_outer_range: f64,
    pub polygon_number: i32,
    pub roll_degree: f64,
    pub min_sw: bool,
    pub alpha_rendering_sw: bool,
    pub add_blend_sw: bool,
    pub number_of_thread: i32,
}

/// `maxmin::thread<IT, RT>`: one band of rows with its own scratch and lens.
struct Band<'a, IT: Px, RT: RefPx + Send + Sync> {
    inn: &'a [IT],
    out: OutPtr<IT>,
    height: i32,
    width: i32,
    channels: i32,
    refimg: Option<&'a [RT]>,
    y_begin: i32,
    y_end: i32,
    lens: Lens,
    p: Params,
    sl: Scanline,
}

impl<IT: Px, RT: RefPx + Send + Sync> Band<'_, IT, RT> {
    /// `csl_top_clamped_in_h_` / `sl_out_clamped_in_h_` as an element offset.
    fn row_base(&self, yy: i32) -> usize {
        let (h, w, c) = (self.height as i64, self.width as i64, self.channels as i64);
        let yy = i64::from(yy);
        (if h <= yy {
            c * w * (h - 1)
        } else if yy < 0 {
            0
        } else {
            c * w * yy
        }) as usize
    }

    /// `paint_margin_`
    fn paint_margin(margin: usize, track: &mut [f64]) {
        for xx in 0..margin {
            track[xx] = track[margin];
        }
        let n = track.len();
        for xx in 0..margin {
            track[n - 1 - xx] = track[n - 1 - margin];
        }
    }

    /// `inn_to_track_`
    fn inn_to_track(&mut self, track_i: usize, row: i32, zz: i32, margin: usize) {
        let base = self.row_base(row) + zz as usize;
        let c = self.channels as usize;
        let track = &mut self.sl.tracks[track_i];
        for xx in 0..self.width as usize {
            track[margin + xx] = self.inn[base + xx * c].to_f64() / IT::DIV;
        }
        Self::paint_margin(margin, track);
    }

    /// The part of `get_first` / `get_next` after the tracks: `inn_to_result_` and the
    /// per-pixel strength (`alpha_ref`).
    fn result_and_alpha(&mut self, yy: i32, zz: i32, add_blend_sw: bool) {
        let c = self.channels as usize;
        let base = self.row_base(yy) + zz as usize;
        for xx in 0..self.width as usize {
            self.sl.result[xx] = self.inn[base + xx * c].to_f64() / IT::DIV;
        }
        if self.sl.alpha_ref.is_empty() {
            return;
        }
        /* alpha_ref_init_one_ */
        for a in self.sl.alpha_ref.iter_mut() {
            *a = 1.0;
        }
        if let Some(refimg) = self.refimg {
            /* alpha_ref_mul_ref_ */
            let rbase = self.row_base(yy);
            for xx in 0..self.width as usize {
                let px = &refimg[rbase + xx * c..rbase + xx * c + c];
                let refv = ref_value(px, c, RT::MAX, self.p.ref_mode);
                /* clamp 0 to 1 in case using HDR raster: std::min(1., std::max(0., refv)) */
                let m = if 0.0 < refv { refv } else { 0.0 };
                let m = if m < 1.0 { m } else { 1.0 };
                self.sl.alpha_ref[xx] *= m;
            }
        }
        if self.channels < 4 {
            return; /* no alpha channel */
        }
        if add_blend_sw {
            /* alpha_ref_mul_alpha_ */
            let abase = self.row_base(yy) + 3;
            for xx in 0..self.width as usize {
                // SAFETY: in bounds of the output image; read through the atomic.
                let a = unsafe { IT::load(self.out.0.add(abase + xx * c)) };
                self.sl.alpha_ref[xx] *= a.to_f64() / IT::DIV;
            }
        }
    }

    /// `getput::get_first`
    fn get_first(&mut self, yy: i32, zz: i32, margin: usize, add_blend_sw: bool) {
        let m = margin as i32;
        let mut ii = margin * 2;
        for yp in (-m + yy)..=(m + yy) {
            self.inn_to_track(ii, yp, zz, margin);
            ii = ii.wrapping_sub(1);
        }
        self.result_and_alpha(yy, zz, add_blend_sw);
    }

    /// `getput::get_next`
    fn get_next(&mut self, yy: i32, zz: i32, margin: usize, add_blend_sw: bool) {
        self.inn_to_track(0, yy + margin as i32, zz, margin);
        self.result_and_alpha(yy, zz, add_blend_sw);
    }

    /// `getput::copy`
    fn copy(&self, yy: i32, zz: i32) {
        let c = self.channels as usize;
        let base = self.row_base(yy) + zz as usize;
        for xx in 0..self.width as usize {
            // SAFETY: in bounds of the output image.
            unsafe { IT::store(self.out.0.add(base + c * xx), self.inn[base + c * xx]) };
        }
    }

    /// `getput::put`
    fn put(&self, yy: i32, zz: i32) {
        let c = self.channels as usize;
        let base = self.row_base(yy) + zz as usize;
        for xx in 0..self.width as usize {
            // SAFETY: in bounds of the output image.
            unsafe { IT::store(self.out.0.add(base + c * xx), IT::from_result(self.sl.result[xx])) };
        }
    }

    /// `rendering_sl_ch_`
    fn rendering_sl_ch(&mut self, yy: i32, zz: i32, render_sw: bool, add_blend_sw: bool) {
        if !render_sw {
            self.copy(yy, zz);
            return;
        }
        let margin = self.sl.tracks.len() / 2;
        if yy == self.y_begin {
            self.get_first(yy, zz, margin, add_blend_sw);
        } else {
            self.sl.shift();
            self.get_next(yy, zz, margin, add_blend_sw);
        }
        render(
            self.p.radius,
            self.p.smooth_outer_range,
            self.p.polygon_number,
            self.p.roll_degree,
            self.p.min_sw,
            &mut self.lens,
            &self.sl.tracks,
            &self.sl.alpha_ref,
            &mut self.sl.result,
        );
        self.put(yy, zz);
    }

    /// `thread::run`
    fn run(&mut self) {
        let mut rgb_ren_sw = true;
        let mut alp_ren_sw = self.p.alpha_rendering_sw;
        let add_ble_sw = self.p.add_blend_sw;

        /* too small to process */
        if self.sl.tracks.len() <= 1 {
            rgb_ren_sw = false;
            alp_ren_sw = false;
        } // not render, then copy

        /* first scanline --> next scanline, so loop channel by channel */
        let (yb, ye) = (self.y_begin, self.y_end);
        if self.channels == 4 {
            for yy in yb..=ye {
                self.rendering_sl_ch(yy, ALP, alp_ren_sw, false);
            }
            for zz in [BLU, GRE, RED] {
                for yy in yb..=ye {
                    self.rendering_sl_ch(yy, zz, rgb_ren_sw, add_ble_sw);
                }
            }
        } else if self.channels == 3 {
            for zz in [BLU, GRE, RED] {
                for yy in yb..=ye {
                    self.rendering_sl_ch(yy, zz, rgb_ren_sw, add_ble_sw);
                }
            }
        } else if self.channels == 1 {
            for yy in yb..=ye {
                self.rendering_sl_ch(yy, 0, rgb_ren_sw, add_ble_sw);
            }
        }
    }
}

/// `maxmin::multithread` + `run`: split the rows into bands and render them.
fn run_bands<IT: Px, RT: RefPx + Send + Sync>(
    inn: &[IT],
    out: *mut IT,
    height: i32,
    width: i32,
    channels: i32,
    refimg: Option<&[RT]>,
    p: &Params,
) -> i32 {
    let mut lens = Lens::default();
    if let Err(e) = alloc_and_shape_lens_matrix(
        p.radius,
        p.radius + p.smooth_outer_range,
        p.polygon_number,
        p.roll_degree,
        &mut lens,
    ) {
        return match e {
            LensError::BadRadius => BAD_ARG,
            LensError::TooLarge => TOO_LARGE,
        };
    }

    let mut thread_num = p.number_of_thread;
    if thread_num < 1 {
        thread_num = 1;
    }
    if height < thread_num {
        thread_num = height;
    }

    let mut bands: Vec<Band<IT, RT>> = Vec::new();
    let mut yy = 0;
    for ii in 0..thread_num {
        let y_end = (f64::from(height) * f64::from(ii + 1) / f64::from(thread_num) + 0.999999) as i32 - 1;
        let mut sl = Scanline::default();
        sl.resize(lens.offsets.len(), width as usize, refimg.is_some() || 4 <= channels);
        bands.push(Band {
            inn,
            out: OutPtr(out),
            height,
            width,
            channels,
            refimg,
            y_begin: yy,
            y_end,
            lens: lens.clone(),
            p: *p,
            sl,
        });
        yy = y_end;
    }

    if bands.len() == 1 {
        bands[0].run();
    } else {
        std::thread::scope(|s| {
            for b in bands.iter_mut() {
                s.spawn(move || b.run());
            }
        });
    }
    OK
}

fn ranges_overlap(a: usize, a_len: usize, b: usize, b_len: usize) -> bool {
    a < b + b_len && b < a + a_len
}

/// One monomorphisation: validates the buffers and runs the bands.
#[allow(clippy::too_many_arguments)]
unsafe fn convert_typed<IT: Px, RT: RefPx + Send + Sync>(
    inn: *const u8,
    out: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    refp: *const u8,
    p: &Params,
) -> i32 {
    if height < 0 || width < 0 {
        return BAD_ARG;
    }
    if height == 0 || width == 0 {
        return OK; // no band / no pixel in the C++
    }
    if p.polygon_number > MAX_POLYGON || !p.roll_degree.is_finite() || p.roll_degree.abs() > MAX_ROLL {
        return if p.polygon_number > MAX_POLYGON { TOO_LARGE } else { BAD_ARG };
    }
    let n = match (height as usize).checked_mul(width as usize).and_then(|v| v.checked_mul(channels as usize)) {
        Some(n) => n,
        None => return BAD_ARG,
    };
    if inn.is_null() || out.is_null() {
        return BAD_ARG;
    }
    let (isz, rsz) = (std::mem::size_of::<IT>(), std::mem::size_of::<RT>());
    if inn as usize % std::mem::align_of::<IT>() != 0
        || out as usize % std::mem::align_of::<IT>() != 0
        || (!refp.is_null() && refp as usize % std::mem::align_of::<RT>() != 0)
    {
        return BAD_ARG;
    }
    if ranges_overlap(inn as usize, n * isz, out as usize, n * isz)
        || (!refp.is_null() && ranges_overlap(refp as usize, n * rsz, out as usize, n * isz))
    {
        return BAD_ARG;
    }
    let inn = std::slice::from_raw_parts(inn as *const IT, n);
    let refimg = if refp.is_null() { None } else { Some(std::slice::from_raw_parts(refp as *const RT, n)) };
    run_bands::<IT, RT>(inn, out as *mut IT, height, width, channels, refimg, p)
}

/// `igs::maxmin::convert` on raw byte buffers.
///
/// # Safety
/// `inn`/`out` must hold `height*width*channels` samples of `bits` bits (8, 16, or 24 =
/// `std::numeric_limits<float>::digits` for float), `refp` (when non-null) the same
/// number of `ref_bits` samples; `out` is written.
#[allow(clippy::too_many_arguments)]
pub unsafe fn convert_raw(
    inn: *const u8,
    out: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    refp: *const u8,
    ref_bits: i32,
    p: &Params,
) -> i32 {
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    /* same order of tests as the C++ */
    if bits == 8 && (ref_bits == 8 || ref_bits == 0) {
        convert_typed::<u8, u8>(inn, out, height, width, channels, refp, p)
    } else if bits == 16 && (ref_bits == 8 || ref_bits == 0) {
        convert_typed::<u16, u8>(inn, out, height, width, channels, refp, p)
    } else if bits == 16 && ref_bits == 16 {
        convert_typed::<u16, u16>(inn, out, height, width, channels, refp, p)
    } else if bits == 8 && ref_bits == 16 {
        convert_typed::<u8, u16>(inn, out, height, width, channels, refp, p)
    } else if bits == 24 && (ref_bits == 24 || ref_bits == 0) {
        convert_typed::<f32, f32>(inn, out, height, width, channels, refp, p)
    } else {
        BAD_BITS
    }
}

/// `igs::maxmin::convert`. Returns 0, 1/2 (domain errors), 3 (bad argument), 4 (too large).
///
/// # Safety
/// See [`convert_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_maxmin_convert(
    inn: *const u8,
    out: *mut u8,
    height: c_int,
    width: c_int,
    channels: c_int,
    bits: c_int,
    refp: *const u8,
    ref_bits: c_int,
    ref_mode: c_int,
    radius: f64,
    smooth_outer_range: f64,
    polygon_number: c_int,
    roll_degree: f64,
    min_sw: bool,
    alpha_rendering_sw: bool,
    add_blend_sw: bool,
    number_of_thread: c_int,
) -> c_int {
    let p = Params {
        ref_mode,
        radius,
        smooth_outer_range,
        polygon_number,
        roll_degree,
        min_sw,
        alpha_rendering_sw,
        add_blend_sw,
        number_of_thread,
    };
    crate::guard(BAD_ARG, || convert_raw(inn, out, height, width, channels, bits, refp, ref_bits, &p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(radius: f64, smooth: f64) -> Params {
        Params {
            ref_mode: 5,
            radius,
            smooth_outer_range: smooth,
            polygon_number: 2,
            roll_degree: 0.0,
            min_sw: false,
            alpha_rendering_sw: true,
            add_blend_sw: false,
            number_of_thread: 1,
        }
    }

    fn run8(inn: &[u8], h: i32, w: i32, c: i32, p: &Params) -> (i32, Vec<u8>) {
        let mut out = vec![0u8; inn.len()];
        let st = unsafe { convert_raw(inn.as_ptr(), out.as_mut_ptr(), h, w, c, 8, std::ptr::null(), 0, p) };
        (st, out)
    }

    fn patt(n: usize) -> Vec<u8> {
        (0..n).map(|i| ((i * 37 + (i / 3) * 11) % 256) as u8).collect()
    }

    #[test]
    fn zero_radius_copies() {
        for &c in &[1, 3, 4] {
            let inn = patt(7 * 5 * c as usize);
            let (st, out) = run8(&inn, 5, 7, c, &params(0.0, 1.0));
            assert_eq!(st, OK);
            assert_eq!(out, inn);
        }
    }

    #[test]
    fn dilate_a_bright_pixel_gray() {
        let (h, w) = (9usize, 9usize);
        let mut inn = vec![0u8; h * w];
        inn[4 * w + 4] = 200;
        let (st, out) = run8(&inn, h as i32, w as i32, 1, &params(1.0, 1.0));
        assert_eq!(st, OK);
        assert_eq!(out[4 * w + 4], 200);
        assert_eq!(out[4 * w + 5], 200, "distance 1 keeps the full value");
        assert_eq!(out[5 * w + 4], 200);
        assert_eq!(out[4 * w + 6], 0, "distance 2 is the outer edge: weight 0");
        assert!(out[5 * w + 5] > 0 && out[5 * w + 5] < 200, "diagonal sqrt2 is partial: {}", out[5 * w + 5]);
    }

    #[test]
    fn erode_a_dark_pixel_gray() {
        let (h, w) = (9usize, 9usize);
        let mut inn = vec![200u8; h * w];
        inn[4 * w + 4] = 0;
        let mut p = params(1.0, 1.0);
        p.min_sw = true;
        let (_, out) = run8(&inn, h as i32, w as i32, 1, &p);
        assert_eq!(out[4 * w + 5], 0);
        assert_eq!(out[4 * w + 6], 200);
    }

    #[test]
    fn rgba_alpha_is_copied_when_not_rendered_and_rendered_when_asked() {
        let (h, w) = (7usize, 7usize);
        let mut inn = vec![0u8; h * w * 4];
        inn[(3 * w + 3) * 4 + 3] = 255; // alpha of the centre pixel
        let mut p = params(1.0, 1.0);
        p.alpha_rendering_sw = false;
        let (_, kept) = run8(&inn, h as i32, w as i32, 4, &p);
        assert_eq!(kept[(3 * w + 4) * 4 + 3], 0);
        p.alpha_rendering_sw = true;
        let (_, grown) = run8(&inn, h as i32, w as i32, 4, &p);
        assert_eq!(grown[(3 * w + 4) * 4 + 3], 255);
        assert_eq!(grown[(3 * w + 3) * 4 + 3], 255);
    }

    #[test]
    fn threads_do_not_change_the_result() {
        for &c in &[1, 3, 4] {
            let (h, w) = (23usize, 17usize);
            let inn = patt(h * w * c as usize);
            let mut p = params(2.0, 1.5);
            let (_, one) = run8(&inn, h as i32, w as i32, c, &p);
            for nt in [2, 3, 7, 100, 0, -4] {
                p.number_of_thread = nt;
                let (st, many) = run8(&inn, h as i32, w as i32, c, &p);
                assert_eq!(st, OK);
                assert_eq!(many, one, "channels {c} threads {nt}");
            }
        }
    }

    #[test]
    fn sixteen_bit_and_float_agree_with_eight_bit() {
        let (h, w) = (11usize, 13usize);
        let i8: Vec<u8> = patt(h * w * 3);
        let i16: Vec<u16> = i8.iter().map(|&v| u16::from(v) * 257).collect();
        let f32s: Vec<f32> = i8.iter().map(|&v| f32::from(v) / 255.0).collect();
        let p = params(2.0, 1.0);
        let (_, o8) = run8(&i8, h as i32, w as i32, 3, &p);
        let mut o16 = vec![0u16; i16.len()];
        let st = unsafe {
            convert_raw(i16.as_ptr() as *const u8, o16.as_mut_ptr() as *mut u8, h as i32, w as i32, 3, 16, std::ptr::null(), 0, &p)
        };
        assert_eq!(st, OK);
        let mut of = vec![0f32; f32s.len()];
        let st = unsafe {
            convert_raw(f32s.as_ptr() as *const u8, of.as_mut_ptr() as *mut u8, h as i32, w as i32, 3, 24, std::ptr::null(), 0, &p)
        };
        assert_eq!(st, OK);
        for i in 0..o8.len() {
            assert!((i32::from(o8[i]) * 257 - i32::from(o16[i])).abs() <= 257, "{i}");
            assert!((f32::from(o8[i]) / 255.0 - of[i]).abs() <= 1.0 / 255.0 + 1e-6, "{i}");
        }
    }

    #[test]
    fn reference_scales_the_radius() {
        let (h, w) = (9usize, 9usize);
        let mut inn = vec![0u8; h * w * 4];
        inn[(4 * w + 4) * 4 + 2] = 255; // red of the centre
        let plain = {
            let mut p = params(2.0, 1.0);
            p.alpha_rendering_sw = false;
            run8(&inn, h as i32, w as i32, 4, &p).1
        };
        // reference all 0 (red channel, mode 0) -> radius 0 everywhere -> pure copy
        let zeros = vec![0u8; h * w * 4];
        let mut p = params(2.0, 1.0);
        p.alpha_rendering_sw = false;
        p.ref_mode = 0;
        let mut out = vec![0u8; inn.len()];
        let st = unsafe { convert_raw(inn.as_ptr(), out.as_mut_ptr(), 9, 9, 4, 8, zeros.as_ptr(), 8, &p) };
        assert_eq!(st, OK);
        assert_eq!(out, inn);
        // reference all 255 -> same as no reference
        let full = vec![255u8; h * w * 4];
        let mut out2 = vec![0u8; inn.len()];
        let st = unsafe { convert_raw(inn.as_ptr(), out2.as_mut_ptr(), 9, 9, 4, 8, full.as_ptr(), 8, &p) };
        assert_eq!(st, OK);
        assert_eq!(out2, plain);
        assert_ne!(plain, inn);
    }

    #[test]
    fn dispatch_matches_the_cpp_order_and_errors() {
        let inn = patt(4 * 4);
        let mut out = vec![0u8; 16];
        let p = params(1.0, 1.0);
        let go = |inn: &[u8], out: &mut [u8], c: i32, bits: i32, rb: i32, r: *const u8| unsafe {
            convert_raw(inn.as_ptr(), out.as_mut_ptr(), 4, 4, c, bits, r, rb, &p)
        };
        assert_eq!(go(&inn, &mut out, 2, 8, 0, std::ptr::null()), BAD_CHANNELS);
        assert_eq!(go(&inn, &mut out, 1, 12, 0, std::ptr::null()), BAD_BITS);
        assert_eq!(go(&inn, &mut out, 1, 32, 0, std::ptr::null()), BAD_BITS);
        assert_eq!(go(&inn, &mut out, 1, 24, 8, std::ptr::null()), BAD_BITS, "float image with an 8-bit reference");
        assert_eq!(go(&inn, &mut out, 1, 8, 24, std::ptr::null()), BAD_BITS);
        assert_eq!(go(&inn, &mut out, 1, 8, 0, std::ptr::null()), OK);
    }

    #[test]
    fn argument_checks() {
        let inn = patt(16);
        let mut out = vec![0u8; 16];
        let p = params(1.0, 1.0);
        unsafe {
            assert_eq!(convert_raw(inn.as_ptr(), out.as_mut_ptr(), -1, 4, 1, 8, std::ptr::null(), 0, &p), BAD_ARG);
            assert_eq!(convert_raw(inn.as_ptr(), out.as_mut_ptr(), 0, 4, 1, 8, std::ptr::null(), 0, &p), OK);
            assert_eq!(convert_raw(inn.as_ptr(), out.as_mut_ptr(), 4, 0, 1, 8, std::ptr::null(), 0, &p), OK);
            assert_eq!(convert_raw(std::ptr::null(), out.as_mut_ptr(), 4, 4, 1, 8, std::ptr::null(), 0, &p), BAD_ARG);
            assert_eq!(convert_raw(inn.as_ptr(), std::ptr::null_mut(), 4, 4, 1, 8, std::ptr::null(), 0, &p), BAD_ARG);
            // overlapping in/out
            let mut buf = patt(32);
            assert_eq!(convert_raw(buf.as_ptr(), buf.as_mut_ptr().add(8), 4, 4, 1, 8, std::ptr::null(), 0, &p), BAD_ARG);
            // reference overlapping the output
            assert_eq!(convert_raw(inn.as_ptr(), buf.as_mut_ptr(), 4, 4, 1, 8, buf.as_ptr().add(4), 8, &p), BAD_ARG);
            // misaligned 16-bit
            let raw = vec![0u8; 80];
            let mis = raw.as_ptr().add(1);
            let mut o2 = vec![0u16; 16];
            assert_eq!(convert_raw(mis, o2.as_mut_ptr() as *mut u8, 4, 4, 1, 16, std::ptr::null(), 0, &p), BAD_ARG);
        }
        let mut bad = params(f64::NAN, 1.0);
        assert_eq!(run8(&inn, 4, 4, 1, &bad).0, BAD_ARG);
        bad = params(1.0, 1.0);
        bad.roll_degree = f64::INFINITY;
        assert_eq!(run8(&inn, 4, 4, 1, &bad).0, BAD_ARG);
        bad.roll_degree = 0.0;
        bad.polygon_number = i32::MAX;
        assert_eq!(run8(&inn, 4, 4, 1, &bad).0, TOO_LARGE);
        assert_eq!(run8(&inn, 4, 4, 1, &params(1.0e7, 1.0)).0, TOO_LARGE);
    }

    // --- differential golden -------------------------------------------------------

    /// (radius, smooth, polygon, roll, min, alpha, threads)
    const CFGS: [(f64, f64, i32, f64, bool, bool, i32); 14] = [
        (1.0, 2.0, 2, 0.0, false, true, 1),
        (2.5, 1.0, 2, 0.0, false, true, 1),
        (2.5, 1.0, 2, 0.0, true, true, 2),
        (3.0, 0.0, 2, 0.0, false, false, 3),
        (0.5, 2.0, 2, 0.0, false, true, 1),
        (0.0, 1.0, 2, 0.0, false, true, 1),
        (2.0, 1.5, 4, 0.0, false, true, 1),
        (2.0, 1.5, 5, 30.0, true, true, 2),
        (3.0, 1.0, 6, 180.0, false, true, 1),
        (2.2, 2.0, 3, 45.0, false, true, 3),
        (1.5, 0.5, 8, 22.5, true, false, 1),
        (2.0, 1.0, 2, 0.0, false, true, 100),
        (4.0, 1.0, 2, 0.0, true, true, 4),
        (1.0, 0.0, 16, 0.0, false, true, 1),
    ];

    fn val(bits: i32, i: usize, y: usize) -> u32 {
        if bits == 16 {
            ((i * 2579 + y * 97 + 13) % 65536) as u32
        } else {
            ((i * 37 + (i / 3) * 11 + y * 5) % 256) as u32
        }
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
        (a.to_bits() as i64 - b.to_bits() as i64).unsigned_abs()
    }

    /// parse `row <yy> <off> <size> <hex...>` lines into a lens of `odd` rows
    fn parse_lens<'a>(lines: &mut std::iter::Peekable<impl Iterator<Item = &'a str>>, odd: usize) -> Lens {
        let mut l = Lens { offsets: vec![0; odd], sizes: vec![0; odd], ratio: vec![vec![]; odd] };
        for yy in 0..odd {
            let t: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            assert_eq!((t[0], t[1]), ("row", yy.to_string().as_str()));
            l.offsets[yy] = t[2].parse().unwrap();
            l.sizes[yy] = t[3].parse().unwrap();
            l.ratio[yy] = t[4..].iter().map(|x| f64::from_bits(u64::from_str_radix(x, 16).unwrap())).collect();
            assert_eq!(l.ratio[yy].len(), l.sizes[yy].max(0) as usize);
        }
        l
    }

    /// returns whether every weight was bit-identical
    fn compare_lens(what: &str, got: &Lens, want: &Lens) -> bool {
        assert_eq!(got.offsets, want.offsets, "{what}: offsets");
        assert_eq!(got.sizes, want.sizes, "{what}: sizes");
        let mut same = true;
        for yy in 0..want.sizes.len() {
            let n = want.sizes[yy].max(0) as usize;
            for i in 0..n {
                let (a, b) = (got.ratio[yy][i], want.ratio[yy][i]);
                if a.to_bits() != b.to_bits() {
                    same = false;
                    assert!(ulps(a, b) <= 4, "{what}: row {yy} weight {i}: {a:e} vs {b:e}");
                }
            }
        }
        same
    }

    /// Replays `gen_igs_maxmin_cases.py`: the original `igs_maxmin.cpp`,
    /// `igs_maxmin_lens_matrix.cpp` and `igs_maxmin_slrender.cpp` (from git, built with
    /// the real `igs_resource_multithread` on pthreads) over 14 lens configurations.
    ///
    /// * lens matrices (fresh and reshaped to 0.6x the radius): offsets / sizes exact,
    ///   weights within 4 ULP (libm `cos`/`sin`/`atan2` for polygons; circles are exact);
    /// * end-to-end image hashes (FNV-1a of the output bytes): required exactly for
    ///   circles, and for polygons whenever Rust's lenses were bit-identical to the
    ///   recorded ones. Multi-threaded cases are only recorded for 1 and 3 channels
    ///   without a reference: the C++ RGBA/reference path races on its shared lens.
    #[test]
    fn matches_the_cpp_reference() {
        let golden = include_str!("igs_maxmin_cases.txt");
        let mut lines = golden.lines().peekable();
        let mut lens_exact = [true; 14];
        for (ci, cfg) in CFGS.iter().enumerate() {
            // fresh lens
            let head: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            assert_eq!((head[0], head[1]), ("lens", ci.to_string().as_str()));
            let odd: usize = head[2].parse().unwrap();
            let want = parse_lens(&mut lines, odd);
            let mut got = Lens::default();
            let d = alloc_and_shape_lens_matrix(cfg.0, cfg.0 + cfg.1, cfg.2, cfg.3, &mut got).unwrap();
            assert_eq!(d as usize, odd, "cfg {ci}");
            lens_exact[ci] &= compare_lens(&format!("cfg {ci} lens"), &got, &want);
            // reshaped lens
            let head: Vec<&str> = lines.next().unwrap().split_whitespace().collect();
            assert_eq!((head[0], head[1]), ("rlens", ci.to_string().as_str()));
            let rodd: usize = head[2].parse().unwrap();
            assert_eq!(rodd, odd);
            if odd > 0 {
                let want = parse_lens(&mut lines, odd);
                let r2 = cfg.0 * 0.6;
                crate::igs_maxmin_lens_matrix::reshape_lens_matrix(
                    r2,
                    crate::igs_maxmin_lens_matrix::outer_radius_from_radius(r2, cfg.1),
                    odd as i32,
                    cfg.2,
                    cfg.3,
                    &mut got,
                );
                lens_exact[ci] &= compare_lens(&format!("cfg {ci} reshaped lens"), &got, &want);
            }
        }

        let (w, h) = (14usize, 10usize);
        let mut e2e = 0;
        for l in lines {
            let t: Vec<&str> = l.split_whitespace().collect();
            assert_eq!(t[0], "e2e", "{l}");
            let ci: usize = t[1].parse().unwrap();
            let (bits, cc, rbits, rmode): (i32, usize, i32, i32) =
                (t[2].parse().unwrap(), t[3].parse().unwrap(), t[4].parse().unwrap(), t[5].parse().unwrap());
            let want = u64::from_str_radix(t[6], 16).unwrap();
            let cfg = CFGS[ci];
            let p = Params {
                ref_mode: rmode,
                radius: cfg.0,
                smooth_outer_range: cfg.1,
                polygon_number: cfg.2,
                roll_degree: cfg.3,
                min_sw: cfg.4,
                alpha_rendering_sw: cfg.5,
                add_blend_sw: false,
                number_of_thread: if cc == 4 || rbits != 0 { 1 } else { cfg.6 },
            };
            let n = w * h * cc;
            let mk = |bits: i32| -> Vec<u32> { (0..n).map(|i| val(bits, i, i / (w * cc))).collect() };
            let ref8: Vec<u8> = if rbits == 8 { mk(8).iter().map(|&v| ((v * 7 + 3) % 256) as u8).collect() } else { vec![] };
            let ref16: Vec<u16> =
                if rbits == 16 { mk(16).iter().map(|&v| ((v * 5 + 1) % 65536) as u16).collect() } else { vec![] };
            let refp: *const u8 = match rbits {
                8 => ref8.as_ptr(),
                16 => ref16.as_ptr() as *const u8,
                _ => std::ptr::null(),
            };
            let got = match bits {
                8 => {
                    let inn: Vec<u8> = mk(8).iter().map(|&v| v as u8).collect();
                    let mut out = vec![0u8; n];
                    let st = unsafe { convert_raw(inn.as_ptr(), out.as_mut_ptr(), h as i32, w as i32, cc as i32, 8, refp, rbits, &p) };
                    assert_eq!(st, OK, "{l}");
                    fnv(out.iter().copied())
                }
                16 => {
                    let inn: Vec<u16> = mk(16).iter().map(|&v| v as u16).collect();
                    let mut out = vec![0u16; n];
                    let st = unsafe {
                        convert_raw(inn.as_ptr() as *const u8, out.as_mut_ptr() as *mut u8, h as i32, w as i32, cc as i32, 16, refp, rbits, &p)
                    };
                    assert_eq!(st, OK, "{l}");
                    fnv(out.iter().flat_map(|v| v.to_le_bytes()))
                }
                _ => {
                    let inn: Vec<f32> = mk(8).iter().map(|&v| v as f32 / 255.0).collect();
                    let mut out = vec![0f32; n];
                    let st = unsafe {
                        convert_raw(inn.as_ptr() as *const u8, out.as_mut_ptr() as *mut u8, h as i32, w as i32, cc as i32, 24, refp, rbits, &p)
                    };
                    assert_eq!(st, OK, "{l}");
                    fnv(out.iter().flat_map(|v| v.to_le_bytes()))
                }
            };
            if cfg.2 < 3 || lens_exact[ci] {
                assert_eq!(got, want, "{l}");
                e2e += 1;
            } else {
                eprintln!("note: libm differs from the recording for cfg {ci}; hash not compared: {l}");
            }
        }
        assert!(e2e >= 100, "only {e2e} end-to-end cases were compared");
    }
}
