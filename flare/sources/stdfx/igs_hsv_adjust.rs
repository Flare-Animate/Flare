//! Port of stdfx/igs_hsv_adjust.cpp: pivot/scale/shift adjustment of hue,
//! saturation and value on a float image (rgba / rgb / grayscale).
//! Channel order is the Toonz6 one: B,G,R,A (`igs_ifx_common.h`).

use crate::igs_color_rgb_hsv::{hsv_to_rgb, rgb_to_hsv};

/// 0 = ok; 1 = "Bad channels,Not rgba/rgb/grayscale" (std::domain_error); 3 = bad argument.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1;
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

#[derive(Clone, Copy)]
pub struct Params {
    pub hue_pivot: f64,
    pub hue_scale: f64,
    pub hue_shift: f64,
    pub sat_pivot: f64,
    pub sat_scale: f64,
    pub sat_shift: f64,
    pub val_pivot: f64,
    pub val_scale: f64,
    pub val_shift: f64,
}

fn pixel(r: f64, g: f64, b: f64, p: &Params) -> (f64, f64, f64) {
    let (mut hue, mut sat, mut val) = rgb_to_hsv(r, g, b);
    if 1.0 != p.hue_scale || 0.0 != p.hue_shift {
        hue -= p.hue_pivot;
        // huge / non-finite values would spin forever in the C++ loops; leave them alone
        if hue.abs() < 1.0e7 {
            while hue < -180.0 {
                hue += 360.0;
            }
            while 180.0 <= hue {
                hue -= 360.0;
            }
        }
        hue *= p.hue_scale;
        hue += p.hue_pivot;
        hue += p.hue_shift;
        if hue.abs() < 1.0e7 {
            while hue < 0.0 {
                hue += 360.0;
            }
            while 360.0 <= hue {
                hue -= 360.0;
            }
        }
    }
    if 1.0 != p.sat_scale || 0.0 != p.sat_shift {
        sat -= p.sat_pivot;
        sat *= p.sat_scale;
        sat += p.sat_pivot;
        sat += p.sat_shift;
        sat = if sat < 0.0 { 0.0 } else { sat };
    }
    if 1.0 != p.val_scale || 0.0 != p.val_shift {
        val -= p.val_pivot;
        val *= p.val_scale;
        val += p.val_pivot;
        val += p.val_shift;
    }
    let (mut rr, mut gg, mut bb) = (r, g, b);
    hsv_to_rgb(hue, sat, val, &mut rr, &mut gg, &mut bb);
    (rr, gg, bb)
}

/// `image`: `pixels*channels` floats; `refv`: one float per pixel (optional).
/// Returns [`OK`] or [`BAD_CHANNELS`].
pub fn change(
    image: &mut [f32],
    pixels: usize,
    channels: usize,
    refv_img: Option<&[f32]>,
    p: &Params,
    add_blend: bool,
) -> i32 {
    if 1.0 == p.hue_scale && 0.0 == p.hue_shift && 1.0 == p.sat_scale && 0.0 == p.sat_shift && 1.0 == p.val_scale && 0.0 == p.val_shift {
        return OK;
    }
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    let has_ref = refv_img.is_some();
    for ii in 0..pixels {
        let px = &mut image[ii * channels..(ii + 1) * channels];
        let mut refv: f32 = 1.0;
        if let Some(r) = refv_img {
            refv *= r[ii];
        }
        if channels == 4 {
            if add_blend && 0.0 == px[ALP] {
                continue;
            }
            let (r0, g0, b0) = (f64::from(px[RED]), f64::from(px[GRE]), f64::from(px[BLU]));
            let (mut rr, mut gg, mut bb) = pixel(r0, g0, b0, p);
            if add_blend && px[ALP] < 1.0 {
                refv *= px[ALP];
            }
            if has_ref || (add_blend && px[ALP] < 1.0) {
                let rf = f64::from(refv);
                rr = r0 + (rr - r0) * rf;
                gg = g0 + (gg - g0) * rf;
                bb = b0 + (bb - b0) * rf;
            }
            px[RED] = rr as f32;
            px[GRE] = gg as f32;
            px[BLU] = bb as f32;
        } else if channels == 3 {
            let (r0, g0, b0) = (f64::from(px[RED]), f64::from(px[GRE]), f64::from(px[BLU]));
            let (mut rr, mut gg, mut bb) = pixel(r0, g0, b0, p);
            if has_ref {
                let rf = f64::from(refv);
                rr = r0 + (rr - r0) * rf;
                gg = g0 + (gg - g0) * rf;
                bb = b0 + (bb - b0) * rf;
            }
            px[RED] = rr as f32;
            px[GRE] = gg as f32;
            px[BLU] = bb as f32;
        } else {
            let v0 = f64::from(px[0]);
            let mut li = (v0 - p.val_pivot) * p.val_scale + p.val_pivot + p.val_shift;
            if has_ref {
                li = v0 + (li - v0) * f64::from(refv);
            }
            px[0] = li as f32;
        }
    }
    OK
}

/// Validated raw entry (see `flare_igs_hsv_adjust_change`).
///
/// # Safety
/// `image` valid for `height*width*channels` f32; `refp` null or valid for `height*width` f32.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    refp: *const f32,
    p: &Params,
    add_blend: bool,
) -> i32 {
    // The identity case returns before any buffer is touched (as in C++).
    if 1.0 == p.hue_scale && 0.0 == p.hue_shift && 1.0 == p.sat_scale && 0.0 == p.sat_shift && 1.0 == p.val_scale && 0.0 == p.val_shift {
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
    let c = channels as usize;
    if image.is_null() || pixels > isize::MAX as usize / 16 {
        return BAD_ARG;
    }
    let img = std::slice::from_raw_parts_mut(image, pixels * c);
    let r = if refp.is_null() { None } else { Some(std::slice::from_raw_parts(refp, pixels)) };
    change(img, pixels, c, r, p, add_blend)
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_hsv_adjust_change(
    image_array: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    refp: *const f32,
    hue_pivot: f64,
    hue_scale: f64,
    hue_shift: f64,
    sat_pivot: f64,
    sat_scale: f64,
    sat_shift: f64,
    val_pivot: f64,
    val_scale: f64,
    val_shift: f64,
    add_blend_sw: bool,
) -> i32 {
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let p = Params { hue_pivot, hue_scale, hue_shift, sat_pivot, sat_scale, sat_shift, val_pivot, val_scale, val_shift };
    change_raw(image_array, height, width, channels, refp, &p, add_blend_sw)
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ident() -> Params {
        Params { hue_pivot: 0.0, hue_scale: 1.0, hue_shift: 0.0, sat_pivot: 0.0, sat_scale: 1.0, sat_shift: 0.0, val_pivot: 0.0, val_scale: 1.0, val_shift: 0.0 }
    }
    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn identity_parameters_do_nothing_even_for_bad_channels() {
        let mut px = [0.1f32, 0.2, 0.3, 1.0];
        assert_eq!(change(&mut px, 1, 4, None, &ident(), true), OK);
        assert_eq!(px, [0.1, 0.2, 0.3, 1.0]);
        assert_eq!(change(&mut px, 1, 2, None, &ident(), true), OK);
    }

    #[test]
    fn bad_channel_count_is_reported() {
        let mut px = [0.0f32; 4];
        let p = Params { val_shift: 0.1, ..ident() };
        assert_eq!(change(&mut px, 2, 2, None, &p, true), BAD_CHANNELS);
    }

    #[test]
    fn valhtness_shift_on_grey_rgba_adds_to_all_channels() {
        // B,G,R,A grey 0.5 opaque; val_shift +0.25 -> 0.75
        let mut px = [0.5f32, 0.5, 0.5, 1.0];
        let p = Params { val_shift: 0.25, ..ident() };
        assert_eq!(change(&mut px, 1, 4, None, &p, true), OK);
        assert!(near(px[0], 0.75) && near(px[1], 0.75) && near(px[2], 0.75));
        assert_eq!(px[3], 1.0);
    }

    #[test]
    fn add_blend_skips_transparent_and_scales_by_alpha() {
        let p = Params { val_shift: 0.5, ..ident() };
        let mut transparent = [0.2f32, 0.2, 0.2, 0.0];
        change(&mut transparent, 1, 4, None, &p, true);
        assert_eq!(transparent, [0.2, 0.2, 0.2, 0.0]);
        // alpha 0.5: change is halved (0.2 + 0.5*0.5)
        let mut half = [0.2f32, 0.2, 0.2, 0.5];
        change(&mut half, 1, 4, None, &p, true);
        assert!(near(half[0], 0.45));
        // alpha-composite mode (add_blend off): full change, even with alpha 0
        let mut t2 = [0.2f32, 0.2, 0.2, 0.0];
        change(&mut t2, 1, 4, None, &p, false);
        assert!(near(t2[0], 0.7));
    }

    #[test]
    fn reference_image_scales_the_change_per_pixel() {
        let p = Params { val_shift: 0.4, ..ident() };
        let mut px = [0.2f32, 0.2, 0.2, 0.2, 0.2, 0.2];
        change(&mut px, 2, 3, Some(&[1.0, 0.25]), &p, false);
        assert!(near(px[0], 0.6) && near(px[3], 0.3));
    }

    #[test]
    fn grayscale_uses_pivot_scale_shift_on_valhtness() {
        let p = Params { val_pivot: 0.5, val_scale: 2.0, val_shift: 0.1, ..ident() };
        let mut px = [0.6f32];
        change(&mut px, 1, 1, None, &p, false);
        // (0.6-0.5)*2+0.5+0.1 = 0.8
        assert!(near(px[0], 0.8));
    }

    #[test]
    fn hue_shift_rotates_primary_red_to_green() {
        // R,G,B = 1,0,0 ; hue +120 -> green (B,G,R order in memory)
        let mut px = [0.0f32, 0.0, 1.0];
        let p = Params { hue_shift: 120.0, ..ident() };
        change(&mut px, 1, 3, None, &p, false);
        assert!(near(px[GRE], 1.0) && near(px[RED], 0.0) && near(px[BLU], 0.0));
    }

    #[test]
    fn raw_entry_validates() {
        let p = Params { val_shift: 0.1, ..ident() };
        let mut px = [0.1f32; 4];
        unsafe {
            assert_eq!(change_raw(std::ptr::null_mut(), 1, 1, 4, std::ptr::null(), &p, true), BAD_ARG);
            assert_eq!(change_raw(px.as_mut_ptr(), -1, 1, 4, std::ptr::null(), &p, true), BAD_ARG);
            assert_eq!(change_raw(px.as_mut_ptr(), 1, 1, 2, std::ptr::null(), &p, true), BAD_CHANNELS);
            assert_eq!(change_raw(px.as_mut_ptr(), 0, 5, 4, std::ptr::null(), &p, true), OK);
            assert_eq!(change_raw(px.as_mut_ptr(), 1, 1, 4, std::ptr::null(), &p, true), OK);
        }
    }
}
