//! Port of stdfx/igs_hsv_noise_in_camera.cpp: random noise on hue / saturation / value /
//! alpha of an 8 or 16-bit raster, with the noise pattern anchored to the camera
//! rectangle. The noise planes and the end-of-range control are the ones of
//! `igs_hsv_noise.rs` (the C++ carried a verbatim copy of `noise_reference` and
//! `control_term_within_limits`; only the pixel functions and the integer pass differ):
//! unlike `igs_hsv_noise`, saturation AND value are clamped to 0..1 here, there is no
//! reference image / blend switch, and the result is written back as
//! `static_cast<T>(v * (max + 0.999999))`.
//!
//! Status codes of the C ABI: 0 ok; 1 `std::domain_error("Bad channels,Not
//! rgba/rgb/grayscale")`; 2 `std::domain_error("Bad bits,Not uchar/ushort")`;
//! 3 null buffer / negative size / misaligned 16-bit buffer (`std::invalid_argument`).
//! Check order is the C++ one: all ranges zero -> ok (nothing touched), channels, bits.
//!
//! Deviations (only where the C++ was undefined behaviour): `static_cast<T>` of an
//! out-of-range / NaN double saturates; a hue sector outside 0..5 (NaN input) keeps
//! the pixel's input colour (the C++ read the previous pixel's or an uninitialised
//! value); hue wrapping skips non-finite / huge hues instead of looping for ever.

use crate::igs_color_rgb_hsv::{hsv_to_rgb, rgb_to_hsv};
use crate::igs_hsv_noise::{pixel_a, wrap_hue, ControlTerm, NoiseReference};
use std::os::raw::{c_int, c_ulong, c_void};

pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1;
pub const BAD_BITS: i32 = 2;
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// Integer channel type of the raster (`unsigned char` / `unsigned short`).
pub(crate) trait Chan: Copy {
    const MAX: f64;
    fn to_f64(self) -> f64;
    fn from_f64(v: f64) -> Self;
}
impl Chan for u8 {
    const MAX: f64 = 255.0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> u8 {
        v as u8
    }
}
impl Chan for u16 {
    const MAX: f64 = 65535.0;
    fn to_f64(self) -> f64 {
        f64::from(self)
    }
    fn from_f64(v: f64) -> u16 {
        v as u16
    }
}

/// pixel_rgb: noise on hue / saturation / value of one colour (sat and val clamped to 0..1)
#[allow(clippy::too_many_arguments)]
fn pixel_rgb(red_in: f64, gre_in: f64, blu_in: f64, alp_in: f64, hue_noise: f64, sat_noise: f64, val_noise: f64, sat_term: &ControlTerm, val_term: &ControlTerm) -> (f64, f64, f64) {
    if 0.0 == alp_in {
        return (red_in, gre_in, blu_in);
    }
    let (mut hue, mut sat, mut val) = rgb_to_hsv(red_in, gre_in, blu_in);
    if 0.0 != hue_noise {
        hue += 360.0 * hue_noise * alp_in;
        hue = wrap_hue(hue);
    }
    if 0.0 != sat_term.noise_range() {
        let mut shift_value = 0.0;
        let mut satnoise = sat_noise;
        sat_term.exec(sat, &mut satnoise, &mut shift_value);
        sat += shift_value * alp_in;
        sat += satnoise * alp_in;
        if sat < 0.0 {
            sat = 0.0;
        } else if 1.0 < sat {
            sat = 1.0;
        }
    }
    if 0.0 != val_term.noise_range() {
        let mut shift_value = 0.0;
        let mut valnoise = val_noise;
        val_term.exec(val, &mut valnoise, &mut shift_value);
        val += shift_value * alp_in;
        val += valnoise * alp_in;
        if val < 0.0 {
            val = 0.0;
        } else if 1.0 < val {
            val = 1.0;
        }
    }
    let (mut r, mut g, mut b) = (red_in, gre_in, blu_in);
    hsv_to_rgb(hue, sat, val, &mut r, &mut g, &mut b);
    (r, g, b)
}

#[allow(clippy::too_many_arguments)]
fn change_template<T: Chan>(image: &mut [T], ww: usize, hh: usize, ch: usize, noise: &NoiseReference, hue_range: f64, sat_term: &ControlTerm, val_term: &ControlTerm, alp_term: &ControlTerm) {
    let div_val = T::MAX;
    let mul_val = T::MAX + 0.999999;
    let any_hsv = 0.0 != hue_range || 0.0 != val_term.noise_range() || 0.0 != sat_term.noise_range();
    let mut at = 0usize;
    if ch == 4 {
        for yy in 0..hh {
            for xx in 0..ww {
                let px = &mut image[at..at + 4];
                let (xi, yi) = (xx as i32, yy as i32);
                if any_hsv {
                    let (rr, gg, bb) = pixel_rgb(
                        px[RED].to_f64() / div_val,
                        px[GRE].to_f64() / div_val,
                        px[BLU].to_f64() / div_val,
                        px[ALP].to_f64() / div_val,
                        noise.value(0, xi, yi),
                        noise.value(1, xi, yi),
                        noise.value(2, xi, yi),
                        sat_term,
                        val_term,
                    );
                    px[RED] = T::from_f64(rr * mul_val);
                    px[GRE] = T::from_f64(gg * mul_val);
                    px[BLU] = T::from_f64(bb * mul_val);
                }
                if 0.0 != alp_term.noise_range() {
                    let aa = pixel_a(px[ALP].to_f64() / div_val, noise.value(3, xi, yi), alp_term);
                    px[ALP] = T::from_f64(aa * mul_val);
                }
                at += 4;
            }
        }
    } else if ch == 3 {
        if any_hsv {
            for yy in 0..hh {
                for xx in 0..ww {
                    let px = &mut image[at..at + 3];
                    let (xi, yi) = (xx as i32, yy as i32);
                    let (rr, gg, bb) = pixel_rgb(
                        px[RED].to_f64() / div_val,
                        px[GRE].to_f64() / div_val,
                        px[BLU].to_f64() / div_val,
                        1.0,
                        noise.value(0, xi, yi),
                        noise.value(1, xi, yi),
                        noise.value(2, xi, yi),
                        sat_term,
                        val_term,
                    );
                    px[RED] = T::from_f64(rr * mul_val);
                    px[GRE] = T::from_f64(gg * mul_val);
                    px[BLU] = T::from_f64(bb * mul_val);
                    at += 3;
                }
            }
        }
    } else if ch == 1 && 0.0 != val_term.noise_range() {
        for yy in 0..hh {
            for xx in 0..ww {
                let mut val = image[at].to_f64() / div_val;
                let mut shift_value = 0.0;
                let mut val_noise = noise.value(2, xx as i32, yy as i32);
                val_term.exec(val, &mut val_noise, &mut shift_value);
                val += shift_value;
                val += val_noise;
                if val < 0.0 {
                    val = 0.0;
                } else if 1.0 < val {
                    val = 1.0;
                }
                image[at] = T::from_f64(val * mul_val);
                at += 1;
            }
        }
    }
}

/// Validated entry. `bits` is 8 or 16 (`std::numeric_limits<unsigned char/short>::digits`).
///
/// # Safety
/// `image_array`: `height*width*channels` elements of the `bits`-wide type (r/w), or null when
/// there are no pixels.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image_array: *mut c_void,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    camera_x: i32,
    camera_y: i32,
    camera_w: i32,
    camera_h: i32,
    hue_range: f64,
    sat_range: f64,
    val_range: f64,
    alp_range: f64,
    random_seed: c_ulong,
    near_blur: f64,
    sat_effective: f64,
    sat_center: f64,
    sat_type: i32,
    val_effective: f64,
    val_center: f64,
    val_type: i32,
    alp_effective: f64,
    alp_center: f64,
    alp_type: i32,
) -> i32 {
    if 0.0 == hue_range && 0.0 == sat_range && 0.0 == val_range && 0.0 == alp_range {
        return OK;
    }
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if bits != 8 && bits != 16 {
        return BAD_BITS;
    }
    if height < 0 || width < 0 {
        return BAD_ARG;
    }
    let pixels = (height as usize) * (width as usize);
    if pixels == 0 {
        return OK;
    }
    if image_array.is_null() || pixels > isize::MAX as usize / 64 || (bits == 16 && (image_array as usize) % std::mem::align_of::<u16>() != 0) {
        return BAD_ARG;
    }
    let c = channels as usize;
    let noise = NoiseReference::new(width, height, [hue_range, sat_range, val_range, alp_range], random_seed, near_blur, camera_x, camera_y, camera_w, camera_h);
    let sat_term = ControlTerm::new(sat_effective, sat_effective, sat_center, sat_type, sat_range);
    let val_term = ControlTerm::new(val_effective, val_effective, val_center, val_type, val_range);
    let alp_term = ControlTerm::new(alp_effective, alp_effective, alp_center, alp_type, alp_range);
    if bits == 8 {
        let img = std::slice::from_raw_parts_mut(image_array as *mut u8, pixels * c);
        change_template(img, width as usize, height as usize, c, &noise, hue_range, &sat_term, &val_term, &alp_term);
    } else {
        let img = std::slice::from_raw_parts_mut(image_array as *mut u16, pixels * c);
        change_template(img, width as usize, height as usize, c, &noise, hue_range, &sat_term, &val_term, &alp_term);
    }
    OK
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_hsv_noise_in_camera_change(
    image_array: *mut c_void,
    height: c_int,
    width: c_int,
    channels: c_int,
    bits: c_int,
    camera_x: c_int,
    camera_y: c_int,
    camera_w: c_int,
    camera_h: c_int,
    hue_range: f64,
    sat_range: f64,
    val_range: f64,
    alp_range: f64,
    random_seed: c_ulong,
    near_blur: f64,
    sat_effective: f64,
    sat_center: f64,
    sat_type: c_int,
    val_effective: f64,
    val_center: f64,
    val_type: c_int,
    alp_effective: f64,
    alp_center: f64,
    alp_type: c_int,
) -> c_int {
    // a panic must never unwind into C++: report invalid_argument (3)
    crate::guard(BAD_ARG, || {
        change_raw(
            image_array, height, width, channels, bits, camera_x, camera_y, camera_w, camera_h, hue_range, sat_range, val_range, alp_range, random_seed, near_blur, sat_effective, sat_center, sat_type,
            val_effective, val_center, val_type, alp_effective, alp_center, alp_type,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run8(img: &mut [u8], w: i32, h: i32, ch: i32, hue: f64, sat: f64, val: f64, alp: f64) -> i32 {
        unsafe { flare_igs_hsv_noise_in_camera_change(img.as_mut_ptr() as *mut c_void, h, w, ch, 8, 0, 0, w, h, hue, sat, val, alp, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) }
    }

    #[test]
    fn all_zero_ranges_touch_nothing_even_with_bad_arguments() {
        let mut img = [9u8; 12];
        assert_eq!(run8(&mut img, 1, 3, 2, 0.0, 0.0, 0.0, 0.0), OK);
        assert_eq!(img, [9u8; 12]);
    }

    #[test]
    fn bad_channels_and_bits_are_reported_in_cpp_order() {
        let mut img = [0u8; 16];
        assert_eq!(run8(&mut img, 2, 2, 2, 0.1, 0.0, 0.0, 0.0), BAD_CHANNELS);
        let st = unsafe { flare_igs_hsv_noise_in_camera_change(img.as_mut_ptr() as *mut c_void, 2, 2, 4, 24, 0, 0, 2, 2, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, BAD_BITS);
    }

    #[test]
    fn null_and_negative_are_refused_without_touching_memory() {
        let st = unsafe { flare_igs_hsv_noise_in_camera_change(std::ptr::null_mut(), 2, 2, 4, 8, 0, 0, 2, 2, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, BAD_ARG);
        let mut img = [0u8; 16];
        assert_eq!(run8(&mut img, -2, 2, 4, 0.1, 0.0, 0.0, 0.0), BAD_ARG);
        // zero-size image with a null pointer is fine (the C++ loops never run)
        let st = unsafe { flare_igs_hsv_noise_in_camera_change(std::ptr::null_mut(), 0, 5, 4, 8, 0, 0, 5, 0, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, OK);
    }

    #[test]
    fn misaligned_16_bit_buffer_is_refused() {
        let mut buf = [0u8; 40];
        let p = unsafe { buf.as_mut_ptr().add(if (buf.as_ptr() as usize) % 2 == 0 { 1 } else { 0 }) };
        let st = unsafe { flare_igs_hsv_noise_in_camera_change(p as *mut c_void, 2, 2, 4, 16, 0, 0, 2, 2, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, BAD_ARG);
    }

    #[test]
    fn hue_noise_leaves_alpha_and_grey_pixels_alone() {
        // grey pixels have no hue to rotate; alpha is not touched without alp_range
        let mut img = [100u8, 100, 100, 200, 50, 50, 50, 255];
        assert_eq!(run8(&mut img, 2, 1, 4, 0.5, 0.0, 0.0, 0.0), OK);
        assert_eq!([img[3], img[7]], [200, 255]);
        assert_eq!(&img[..3], &[100, 100, 100]);
    }

    #[test]
    fn grayscale_uses_the_value_noise_only() {
        let mut img = [128u8; 16];
        assert_eq!(run8(&mut img, 4, 4, 1, 0.5, 0.5, 0.0, 0.0), OK);
        assert_eq!(img, [128u8; 16]);
        assert_eq!(run8(&mut img, 4, 4, 1, 0.0, 0.0, 0.2, 0.0), OK);
        assert!(img.iter().any(|&v| v != 128));
    }

    // Replays images recorded from the ORIGINAL igs_hsv_noise_in_camera.cpp (LP64 `unsigned long`),
    // see tests/native/gen_noise_cases.py. Line: `H <bits> <ch> <w> <h> <cam x y w h> <hue sat val alp
    // (f64 bits hex)> <seed> <blur> <sat eff,center,type> <val ...> <alp ...> <in hex> <out hex>`
    #[test]
    fn matches_recorded_cpp() {
        if std::mem::size_of::<c_ulong>() != 8 {
            return; // golden recorded with a 64-bit unsigned long (igs_math_random wraps differently on LLP64)
        }
        let mut n = 0;
        for line in GOLDEN.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            let i = |s: &str| s.parse::<i32>().unwrap();
            let f = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
            let bits = i(t[1]);
            let (ch, w, h) = (i(t[2]), i(t[3]), i(t[4]));
            let (st, got, want) = testutil::replay(bits, t[24], t[25], |p| unsafe {
                flare_igs_hsv_noise_in_camera_change(
                    p, h, w, ch, bits, i(t[5]), i(t[6]), i(t[7]), i(t[8]), f(t[9]), f(t[10]), f(t[11]), f(t[12]),
                    t[13].parse::<u64>().unwrap() as c_ulong, f(t[14]), f(t[15]), f(t[16]), i(t[17]), f(t[18]), f(t[19]), i(t[20]), f(t[21]), f(t[22]), i(t[23]),
                )
            });
            assert_eq!(st, OK, "{line}");
            assert_eq!(got, want, "{line}");
            n += 1;
        }
        assert!(n >= 100, "{n}");
    }

    const GOLDEN: &str = include_str!("igs_hsv_noise_in_camera_cases.txt");
}

/// Shared by the hls twin's tests.
#[cfg(test)]
pub(crate) mod testutil {
    use std::os::raw::c_void;

    fn parse(hex: &str, bits: i32) -> Vec<u16> {
        let w = if bits == 8 { 2 } else { 4 };
        hex.as_bytes().chunks(w).map(|c| u16::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap()).collect()
    }

    /// Runs `call` on a buffer of `in_hex` (8-bit: 2 hex digits per sample, 16-bit: 4) and returns
    /// (status, result widened to u16, expected widened to u16).
    pub(crate) fn replay(bits: i32, in_hex: &str, out_hex: &str, call: impl Fn(*mut c_void) -> i32) -> (i32, Vec<u16>, Vec<u16>) {
        let input = parse(in_hex, bits);
        let want = parse(out_hex, bits);
        if bits == 8 {
            let mut buf: Vec<u8> = input.iter().map(|&v| v as u8).collect();
            let st = call(buf.as_mut_ptr() as *mut c_void);
            (st, buf.iter().map(|&v| u16::from(v)).collect(), want)
        } else {
            let mut buf = input;
            let st = call(buf.as_mut_ptr() as *mut c_void);
            (st, buf, want)
        }
    }
}
