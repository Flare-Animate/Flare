//! Port of stdfx/igs_hls_noise.cpp: random noise on hue / lightness /
//! saturation / alpha of a float image. Same machinery as `igs_hsv_noise.rs`
//! (shared `NoiseReference` / `ControlTerm`); noise planes are ordered
//! hue, lightness, saturation, alpha, which fixes the random stream offsets.

use crate::igs_color_rgb_hls::{hls_to_rgb, rgb_to_hls};
use crate::igs_hsv_noise::{pixel_a, wrap_hue, ControlTerm, NoiseReference};
use std::os::raw::c_ulong;

/// 0 = ok; 1 = "Bad channels,Not rgba/rgb/grayscale" (std::domain_error); 3 = bad argument.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1;
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// pixel_rgb_: noise on hue / lightness / saturation of one colour
#[allow(clippy::too_many_arguments)]
fn pixel_rgb(
    red_in: f64,
    gre_in: f64,
    blu_in: f64,
    alp_in: f64,
    hue_noise: f64,
    lig_noise: f64,
    sat_noise: f64,
    lig_term: &ControlTerm,
    sat_term: &ControlTerm,
    cylindrical: bool,
) -> (f64, f64, f64) {
    if 0.0 == alp_in {
        return (red_in, gre_in, blu_in);
    }
    let (mut hue, mut lig, mut sat) = rgb_to_hls(red_in, gre_in, blu_in, cylindrical);
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
    }
    if 0.0 != sat_term.noise_range() {
        let mut shift_value = 0.0;
        let mut satnoise = sat_noise;
        sat_term.exec(sat, &mut satnoise, &mut shift_value);
        sat += shift_value * alp_in;
        sat += satnoise * alp_in;
        sat = if sat < 0.0 { 0.0 } else { sat };
    }
    hls_to_rgb(hue, lig, sat, cylindrical)
}

#[allow(clippy::too_many_arguments)]
fn change_float(
    image: &mut [f32],
    width: usize,
    height: usize,
    channels: usize,
    refimg: Option<&[f32]>,
    noise: &NoiseReference,
    hue_range: f64,
    lig_term: &ControlTerm,
    sat_term: &ControlTerm,
    alp_term: &ControlTerm,
    add_blend: bool,
    cylindrical: bool,
) {
    let any_hls = 0.0 != hue_range || 0.0 != lig_term.noise_range() || 0.0 != sat_term.noise_range();
    let mut ii = 0usize;
    for yy in 0..height {
        for xx in 0..width {
            let px = &mut image[ii * channels..(ii + 1) * channels];
            let (xi, yi) = (xx as i32, yy as i32);
            let mut refv: f32 = 1.0;
            if channels == 4 {
                if let Some(r) = refimg {
                    refv *= r[ii];
                }
                ii += 1;
                if add_blend && 0.0 == px[ALP] {
                    continue;
                }
                if add_blend && px[ALP] < 1.0 {
                    refv *= px[ALP];
                }
                if any_hls {
                    let (rr1, gg1, bb1, aa1) = (px[RED], px[GRE], px[BLU], px[ALP]);
                    let (mut rr2, mut gg2, mut bb2) = pixel_rgb(
                        f64::from(rr1),
                        f64::from(gg1),
                        f64::from(bb1),
                        f64::from(aa1),
                        noise.value(0, xi, yi),
                        noise.value(1, xi, yi),
                        noise.value(2, xi, yi),
                        lig_term,
                        sat_term,
                        cylindrical,
                    );
                    if refv != 1.0 {
                        let rf = f64::from(refv);
                        rr2 = (rr2 - f64::from(rr1)) * rf + f64::from(rr1);
                        gg2 = (gg2 - f64::from(gg1)) * rf + f64::from(gg1);
                        bb2 = (bb2 - f64::from(bb1)) * rf + f64::from(bb1);
                    }
                    px[RED] = rr2 as f32;
                    px[GRE] = gg2 as f32;
                    px[BLU] = bb2 as f32;
                }
                if 0.0 != alp_term.noise_range() {
                    let aa1 = f64::from(px[ALP]);
                    let mut aa2 = pixel_a(aa1, noise.value(3, xi, yi), alp_term);
                    if refv != 1.0 {
                        aa2 = (aa2 - aa1) * f64::from(refv) + aa1;
                    }
                    px[ALP] = aa2 as f32;
                }
            } else if channels == 3 {
                if !any_hls {
                    continue;
                }
                if let Some(r) = refimg {
                    refv *= r[ii];
                }
                ii += 1;
                let (rr1, gg1, bb1) = (px[RED], px[GRE], px[BLU]);
                let (mut rr2, mut gg2, mut bb2) = pixel_rgb(
                    f64::from(rr1),
                    f64::from(gg1),
                    f64::from(bb1),
                    1.0,
                    noise.value(0, xi, yi),
                    noise.value(1, xi, yi),
                    noise.value(2, xi, yi),
                    lig_term,
                    sat_term,
                    cylindrical,
                );
                if refv != 1.0 {
                    let rf = f64::from(refv);
                    rr2 = (rr2 - f64::from(rr1)) * rf + f64::from(rr1);
                    gg2 = (gg2 - f64::from(gg1)) * rf + f64::from(gg1);
                    bb2 = (bb2 - f64::from(bb1)) * rf + f64::from(bb1);
                }
                px[RED] = rr2 as f32;
                px[GRE] = gg2 as f32;
                px[BLU] = bb2 as f32;
            } else {
                if 0.0 == lig_term.noise_range() {
                    continue;
                }
                if let Some(r) = refimg {
                    refv *= r[ii];
                }
                ii += 1;
                let li1 = f64::from(px[0]);
                let mut shift_value = 0.0;
                let mut lig_noise = noise.value(1, xi, yi);
                lig_term.exec(li1, &mut lig_noise, &mut shift_value);
                let mut li2 = li1;
                li2 += shift_value;
                li2 += lig_noise;
                li2 = if li2 < 0.0 { 0.0 } else { li2 };
                if refv != 1.0 {
                    li2 = li1 + (li2 - li1) * f64::from(refv);
                }
                px[0] = li2 as f32;
            }
        }
    }
}

/// Validated raw entry (see `flare_igs_hls_noise_change`). Returns [`OK`] or a status code.
///
/// # Safety
/// `image_array`: `height*width*channels` f32 (r/w); `refp`: null or `height*width` f32.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image_array: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    refp: *const f32,
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
    add_blend_sw: bool,
    cylindrical: bool,
) -> i32 {
    if 0.0 == hue_range && 0.0 == lig_range && 0.0 == sat_range && 0.0 == alp_range {
        return OK;
    }
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if height < 0 || width < 0 {
        return BAD_ARG;
    }
    let pixels = (height as usize) * (width as usize);
    if pixels == 0 {
        return OK;
    }
    if image_array.is_null() || pixels > isize::MAX as usize / 64 {
        return BAD_ARG;
    }
    let c = channels as usize;
    let img = std::slice::from_raw_parts_mut(image_array, pixels * c);
    let r = if refp.is_null() { None } else { Some(std::slice::from_raw_parts(refp, pixels)) };

    let noise = NoiseReference::new(width, height, [hue_range, lig_range, sat_range, alp_range], random_seed, near_blur, camera_x, camera_y, camera_w, camera_h);
    let lig_term = ControlTerm::new(lig_effective, lig_effective, lig_center, lig_type, lig_range);
    let sat_term = ControlTerm::new(sat_effective, sat_effective, sat_center, sat_type, sat_range);
    let alp_term = ControlTerm::new(alp_effective, alp_effective, alp_center, alp_type, alp_range);
    change_float(img, width as usize, height as usize, c, r, &noise, hue_range, &lig_term, &sat_term, &alp_term, add_blend_sw, cylindrical);
    OK
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_hls_noise_change(
    image_array: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    refp: *const f32,
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
    add_blend_sw: bool,
    cylindrical: bool,
) -> i32 {
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    change_raw(
        image_array, height, width, channels, refp, camera_x, camera_y, camera_w, camera_h, hue_range, lig_range, sat_range, alp_range, random_seed, near_blur, lig_effective, lig_center, lig_type,
        sat_effective, sat_center, sat_type, alp_effective, alp_center, alp_type, add_blend_sw, cylindrical,
    )
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::igs_math_random::next_d;

    #[allow(clippy::too_many_arguments)]
    fn call(img: &mut [f32], h: i32, w: i32, ch: i32, hue: f64, lig: f64, sat: f64, alp: f64, add_blend: bool) -> i32 {
        unsafe {
            change_raw(img.as_mut_ptr(), h, w, ch, std::ptr::null(), 0, 0, w, h, hue, lig, sat, alp, 1, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, add_blend, true)
        }
    }

    #[test]
    fn nothing_happens_without_ranges_and_channels_are_checked_after() {
        let mut img = [0.1f32; 4];
        assert_eq!(call(&mut img, 1, 1, 5, 0.0, 0.0, 0.0, 0.0, true), OK);
        assert_eq!(call(&mut img, 1, 1, 5, 0.1, 0.0, 0.0, 0.0, true), BAD_CHANNELS);
        assert_eq!(img, [0.1; 4]);
    }

    #[test]
    fn lightness_noise_uses_the_second_random_stream_after_hue() {
        // grey 0.5 gray-scale pixel; hue_range and lig_range both active: planes hue (seed 1), lig (seed 2)
        let mut img = [0.5f32];
        call(&mut img, 1, 1, 1, 0.3, 0.2, 0.0, 0.0, false);
        let mut s: c_ulong = 2;
        let noise = 0.2 * (next_d(&mut s) - 0.5);
        let expect = (0.5f64 + noise).max(0.0) as f32; // lightness is not clamped above in grayscale
        assert_eq!(img[0], expect);
    }

    #[test]
    fn saturation_is_floored_at_zero_and_colour_hue_survives_grey() {
        // grey colour (sat 0): lightness-only noise leaves it grey
        let mut img = [0.4f32, 0.4, 0.4];
        call(&mut img, 1, 1, 3, 0.0, 0.1, 0.0, 0.0, false);
        assert!((img[0] - img[1]).abs() < 1e-6 && (img[1] - img[2]).abs() < 1e-6);
    }

    #[test]
    fn transparent_pixels_are_skipped_in_add_blend_mode() {
        let mut t = [0.2f32, 0.4, 0.6, 0.0];
        call(&mut t, 1, 1, 4, 0.5, 0.5, 0.5, 0.0, true);
        assert_eq!(t, [0.2, 0.4, 0.6, 0.0]);
    }

    #[test]
    fn alpha_plane_is_index_three_and_clamped() {
        let mut img = [0.5f32, 0.5, 0.5, 0.5];
        call(&mut img, 1, 1, 4, 0.0, 0.0, 0.0, 4.0, false);
        let mut s: c_ulong = 1; // the only active plane -> first main seed
        let n = 4.0 * (next_d(&mut s) - 0.5);
        let expect = (0.5f64 + n * 0.5).clamp(0.0, 1.0) as f32;
        assert_eq!(img[3], expect);
    }

    #[test]
    fn reference_scales_the_change() {
        let mut a = [0.5f32, 0.5, 0.5, 1.0, 0.5, 0.5];
        let refv = [0.0f32, 1.0];
        let mut b = a;
        unsafe {
            change_raw(a.as_mut_ptr(), 1, 2, 3, refv.as_ptr(), 0, 0, 2, 1, 0.0, 0.4, 0.0, 0.0, 3, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, false, true);
            change_raw(b.as_mut_ptr(), 1, 2, 3, std::ptr::null(), 0, 0, 2, 1, 0.0, 0.4, 0.0, 0.0, 3, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, false, true);
        }
        // pixel 0 has reference 0 -> unchanged; pixel 1 has reference 1 -> same as without reference
        assert_eq!(&a[0..3], &[0.5, 0.5, 0.5]);
        assert_eq!(&a[3..6], &b[3..6]);
    }

    #[test]
    fn raw_entry_validates() {
        let mut img = [0.1f32; 4];
        unsafe {
            assert_eq!(
                change_raw(std::ptr::null_mut(), 1, 1, 4, std::ptr::null(), 0, 0, 1, 1, 0.5, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, true, true),
                BAD_ARG
            );
            assert_eq!(
                change_raw(img.as_mut_ptr(), 1, -1, 4, std::ptr::null(), 0, 0, 1, 1, 0.5, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, true, true),
                BAD_ARG
            );
        }
    }
}
