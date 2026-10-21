//! Port of stdfx/igs_hls_noise_in_camera.cpp: random noise on hue / lightness /
//! saturation / alpha of an 8 or 16-bit raster, anchored to the camera rectangle.
//! Twin of `igs_hsv_noise_in_camera.rs` (same shared `NoiseReference` /
//! `ControlTerm`, same status codes and check order); the noise planes are ordered
//! hue, lightness, saturation, alpha, which fixes the random stream offsets.
//! Lightness and saturation are clamped to 0..1; the HLS conversion is the
//! cylindrical one (the C++ default argument).
//!
//! Deviations (only where the C++ was undefined behaviour): `static_cast<T>` of an
//! out-of-range / NaN double saturates; hue wrapping skips non-finite / huge hues
//! instead of looping for ever.

use crate::igs_color_rgb_hls::{hls_to_rgb, rgb_to_hls};
use crate::igs_hsv_noise::{pixel_a, wrap_hue, ControlTerm, NoiseReference};
use crate::igs_hsv_noise_in_camera::Chan;
use std::os::raw::{c_int, c_ulong, c_void};

pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1;
pub const BAD_BITS: i32 = 2;
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// pixel_rgb: noise on hue / lightness / saturation of one colour (lig and sat clamped to 0..1)
#[allow(clippy::too_many_arguments)]
fn pixel_rgb(red_in: f64, gre_in: f64, blu_in: f64, alp_in: f64, hue_noise: f64, lig_noise: f64, sat_noise: f64, lig_term: &ControlTerm, sat_term: &ControlTerm) -> (f64, f64, f64) {
    if 0.0 == alp_in {
        return (red_in, gre_in, blu_in);
    }
    let (mut hue, mut lig, mut sat) = rgb_to_hls(red_in, gre_in, blu_in, true);
    if 0.0 != hue_noise {
        hue += 360.0 * hue_noise * alp_in;
        hue = wrap_hue(hue);
    }
    if 0.0 != lig_term.noise_range() {
        let mut shift_value = 0.0;
        let mut lignoise = lig_noise;
        lig_term.exec(lig, &mut lignoise, &mut shift_value);
        lig += shift_value * alp_in;
        lig += lignoise * alp_in;
        if lig < 0.0 {
            lig = 0.0;
        } else if 1.0 < lig {
            lig = 1.0;
        }
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
    hls_to_rgb(hue, lig, sat, true)
}

#[allow(clippy::too_many_arguments)]
fn change_template<T: Chan>(image: &mut [T], ww: usize, hh: usize, ch: usize, noise: &NoiseReference, hue_range: f64, lig_term: &ControlTerm, sat_term: &ControlTerm, alp_term: &ControlTerm) {
    let div_val = T::MAX;
    let mul_val = T::MAX + 0.999999;
    let any_hls = 0.0 != hue_range || 0.0 != lig_term.noise_range() || 0.0 != sat_term.noise_range();
    let mut at = 0usize;
    if ch == 4 {
        for yy in 0..hh {
            for xx in 0..ww {
                let px = &mut image[at..at + 4];
                let (xi, yi) = (xx as i32, yy as i32);
                if any_hls {
                    let (rr, gg, bb) = pixel_rgb(
                        px[RED].to_f64() / div_val,
                        px[GRE].to_f64() / div_val,
                        px[BLU].to_f64() / div_val,
                        px[ALP].to_f64() / div_val,
                        noise.value(0, xi, yi),
                        noise.value(1, xi, yi),
                        noise.value(2, xi, yi),
                        lig_term,
                        sat_term,
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
        if any_hls {
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
                        lig_term,
                        sat_term,
                    );
                    px[RED] = T::from_f64(rr * mul_val);
                    px[GRE] = T::from_f64(gg * mul_val);
                    px[BLU] = T::from_f64(bb * mul_val);
                    at += 3;
                }
            }
        }
    } else if ch == 1 && 0.0 != lig_term.noise_range() {
        for yy in 0..hh {
            for xx in 0..ww {
                let mut lig = image[at].to_f64() / div_val;
                let mut shift_value = 0.0;
                let mut lig_noise = noise.value(1, xx as i32, yy as i32);
                lig_term.exec(lig, &mut lig_noise, &mut shift_value);
                lig += shift_value;
                lig += lig_noise;
                if lig < 0.0 {
                    lig = 0.0;
                } else if 1.0 < lig {
                    lig = 1.0;
                }
                image[at] = T::from_f64(lig * mul_val);
                at += 1;
            }
        }
    }
}

/// Validated entry; see `igs_hsv_noise_in_camera::change_raw` for the contract.
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
    lig_range: f64,
    sat_range: f64,
    alp_range: f64,
    random_seed: c_ulong,
    near_blur: f64,
    lig_effective: f64,
    lig_center: f64,
    lig_type: i32,
    sat_effective: f64,
    sat_center: f64,
    sat_type: i32,
    alp_effective: f64,
    alp_center: f64,
    alp_type: i32,
) -> i32 {
    if 0.0 == hue_range && 0.0 == lig_range && 0.0 == sat_range && 0.0 == alp_range {
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
    let noise = NoiseReference::new(width, height, [hue_range, lig_range, sat_range, alp_range], random_seed, near_blur, camera_x, camera_y, camera_w, camera_h);
    let lig_term = ControlTerm::new(lig_effective, lig_effective, lig_center, lig_type, lig_range);
    let sat_term = ControlTerm::new(sat_effective, sat_effective, sat_center, sat_type, sat_range);
    let alp_term = ControlTerm::new(alp_effective, alp_effective, alp_center, alp_type, alp_range);
    if bits == 8 {
        let img = std::slice::from_raw_parts_mut(image_array as *mut u8, pixels * c);
        change_template(img, width as usize, height as usize, c, &noise, hue_range, &lig_term, &sat_term, &alp_term);
    } else {
        let img = std::slice::from_raw_parts_mut(image_array as *mut u16, pixels * c);
        change_template(img, width as usize, height as usize, c, &noise, hue_range, &lig_term, &sat_term, &alp_term);
    }
    OK
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_hls_noise_in_camera_change(
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
    lig_range: f64,
    sat_range: f64,
    alp_range: f64,
    random_seed: c_ulong,
    near_blur: f64,
    lig_effective: f64,
    lig_center: f64,
    lig_type: c_int,
    sat_effective: f64,
    sat_center: f64,
    sat_type: c_int,
    alp_effective: f64,
    alp_center: f64,
    alp_type: c_int,
) -> c_int {
    // a panic must never unwind into C++: report invalid_argument (3)
    crate::guard(BAD_ARG, || {
        change_raw(
            image_array, height, width, channels, bits, camera_x, camera_y, camera_w, camera_h, hue_range, lig_range, sat_range, alp_range, random_seed, near_blur, lig_effective, lig_center, lig_type,
            sat_effective, sat_center, sat_type, alp_effective, alp_center, alp_type,
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::igs_hsv_noise_in_camera::testutil;

    fn run8(img: &mut [u8], w: i32, h: i32, ch: i32, hue: f64, lig: f64, sat: f64, alp: f64) -> i32 {
        unsafe { flare_igs_hls_noise_in_camera_change(img.as_mut_ptr() as *mut c_void, h, w, ch, 8, 0, 0, w, h, hue, lig, sat, alp, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) }
    }

    #[test]
    fn all_zero_ranges_touch_nothing() {
        let mut img = [9u8; 12];
        assert_eq!(run8(&mut img, 1, 3, 2, 0.0, 0.0, 0.0, 0.0), OK);
        assert_eq!(img, [9u8; 12]);
    }

    #[test]
    fn bad_channels_then_bits() {
        let mut img = [0u8; 16];
        assert_eq!(run8(&mut img, 2, 2, 2, 0.1, 0.0, 0.0, 0.0), BAD_CHANNELS);
        let st = unsafe { flare_igs_hls_noise_in_camera_change(img.as_mut_ptr() as *mut c_void, 2, 2, 4, 24, 0, 0, 2, 2, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, BAD_BITS);
    }

    #[test]
    fn null_negative_and_empty() {
        let st = unsafe { flare_igs_hls_noise_in_camera_change(std::ptr::null_mut(), 2, 2, 4, 8, 0, 0, 2, 2, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, BAD_ARG);
        let mut img = [0u8; 16];
        assert_eq!(run8(&mut img, 2, -2, 4, 0.1, 0.0, 0.0, 0.0), BAD_ARG);
        let st = unsafe { flare_igs_hls_noise_in_camera_change(std::ptr::null_mut(), 0, 0, 3, 16, 0, 0, 0, 0, 0.1, 0.0, 0.0, 0.0, 1, 0.5, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0) };
        assert_eq!(st, OK);
    }

    #[test]
    fn grayscale_uses_the_lightness_noise_only() {
        let mut img = [128u8; 16];
        assert_eq!(run8(&mut img, 4, 4, 1, 0.5, 0.0, 0.5, 0.0), OK);
        assert_eq!(img, [128u8; 16]);
        assert_eq!(run8(&mut img, 4, 4, 1, 0.0, 0.2, 0.0, 0.0), OK);
        assert!(img.iter().any(|&v| v != 128));
    }

    // See igs_hsv_noise_in_camera.rs; ranges / terms are hue, lightness, saturation, alpha here.
    #[test]
    fn matches_recorded_cpp() {
        let mut n = 0;
        // igs_math_random wraps at the width of `unsigned long`: pick the matching golden
        let golden = if std::mem::size_of::<c_ulong>() == 8 { GOLDEN } else { GOLDEN_32 };
        for line in golden.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            let i = |s: &str| s.parse::<i32>().unwrap();
            let f = |s: &str| f64::from_bits(u64::from_str_radix(s, 16).unwrap());
            let bits = i(t[1]);
            let (ch, w, h) = (i(t[2]), i(t[3]), i(t[4]));
            let (st, got, want) = testutil::replay(bits, t[24], t[25], |p| unsafe {
                flare_igs_hls_noise_in_camera_change(
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

    const GOLDEN: &str = include_str!("igs_hls_noise_in_camera_cases.txt");
    const GOLDEN_32: &str = include_str!("igs_hls_noise_in_camera_cases_32.txt");
}
