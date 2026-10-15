//! Port of stdfx/igs_hsv_add.cpp: adds a (wrapped, offset) noise image to the
//! hue / valhtness / saturation / alpha of a float image.
//!
//! `noi` is an RGBA float image (4 floats per pixel, `from_rgba` selects the
//! plane) that is tiled over the target with `xoffset`/`yoffset`.
//! Channel order of the target is the Toonz6 one: B,G,R,A.

use crate::igs_color_rgb_hsv::{hsv_to_rgb, rgb_to_hsv};

/// 0 = ok; the C++ header throws `std::domain_error` for 1, 4, 5 and
/// `std::invalid_argument` for 3.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1; // "Bad channels,Not rgba/rgb/grayscale"
pub const BAD_ARG: i32 = 3;
pub const NOISE_NO_DATA: i32 = 4; // "noise_ref_  no data"
pub const NOISE_BAD_ZZ: i32 = 5; // "noise_ref_  bad zz"

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// Tiled access into the noise image (igs noise_ref_).
struct NoiseRef<'a> {
    array: &'a [f32],
    height: i64,
    width: i64,
    xoffset: i64,
    yoffset: i64,
    zz: usize,
}

impl NoiseRef<'_> {
    fn noise(&self, xx: usize, yy: usize) -> f32 {
        // same result as the C++ `while` wrap loops, without overflow
        let x = (xx as i64 - self.xoffset).rem_euclid(self.width);
        let y = (yy as i64 - self.yoffset).rem_euclid(self.height);
        self.array[(4 * self.width * y + 4 * x) as usize + self.zz]
    }
}

#[allow(clippy::too_many_arguments)]
fn pixel(
    red_in: f64,
    gre_in: f64,
    blu_in: f64,
    alp_in: f64,
    hue_noise: f64,
    sat_noise: f64,
    val_noise: f64,
    alp_noise: f64,
) -> (f64, f64, f64, f64) {
    let (mut hue, mut sat, mut val) = rgb_to_hsv(red_in, gre_in, blu_in);
    let mut alp = alp_in;
    if 0.0 != hue_noise {
        hue += 360.0 * hue_noise;
        if hue.abs() < 1.0e7 {
            while hue < 0.0 {
                hue += 360.0;
            }
            while 360.0 <= hue {
                hue -= 360.0;
            }
        }
    }
    if 0.0 != sat_noise {
        sat += sat_noise;
        if sat < 0.0 {
            sat = 0.0;
        }
    }
    if 0.0 != val_noise {
        val += val_noise;
    }
    if 0.0 != alp_noise {
        alp += alp_noise;
        alp = if alp < 0.0 {
            0.0
        } else if 1.0 < alp {
            1.0
        } else {
            alp
        };
    }
    let (mut r, mut g, mut b) = (red_in, gre_in, blu_in);
    hsv_to_rgb(hue, sat, val, &mut r, &mut g, &mut b);
    (r, g, b, alp)
}

#[derive(Clone, Copy)]
pub struct Params {
    pub offset: f64,
    pub hue_scale: f64,
    pub sat_scale: f64,
    pub val_scale: f64,
    pub alp_scale: f64,
    pub add_blend: bool,
}

/// `image`: height*width*channels; `refimg`: optional height*width;
/// `noi`: height*width*4. Returns [`OK`] or a status code.
#[allow(clippy::too_many_arguments)]
pub fn change(
    image: &mut [f32],
    height: usize,
    width: usize,
    channels: usize,
    noi: Option<&[f32]>,
    refimg: Option<&[f32]>,
    xoffset: i32,
    yoffset: i32,
    from_rgba: i32,
    p: &Params,
) -> i32 {
    if 0.0 == p.hue_scale && 0.0 == p.sat_scale && 0.0 == p.val_scale && 0.0 == p.alp_scale {
        return OK;
    }
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    let Some(arr) = noi else { return NOISE_NO_DATA };
    if !(0..4).contains(&from_rgba) {
        return NOISE_BAD_ZZ;
    }
    if height == 0 || width == 0 {
        return OK;
    }
    let nr = NoiseRef { array: arr, height: height as i64, width: width as i64, xoffset: i64::from(xoffset), yoffset: i64::from(yoffset), zz: from_rgba as usize };

    let mut ii = 0usize;
    for yy in 0..height {
        for xx in 0..width {
            let px = &mut image[ii * channels..(ii + 1) * channels];
            let mut refv: f32 = 1.0;
            if let Some(r) = refimg {
                refv *= r[ii];
            }
            ii += 1;
            if channels == 4 {
                if p.add_blend && 0.0 == px[ALP] {
                    continue;
                }
                refv = (f64::from(refv) * (f64::from(nr.noise(xx, yy)) - p.offset)) as f32;
                if p.add_blend && px[ALP] < 1.0 {
                    refv *= px[ALP];
                }
                let rf = f64::from(refv);
                let (rr, gg, bb, aa) = pixel(
                    f64::from(px[RED]),
                    f64::from(px[GRE]),
                    f64::from(px[BLU]),
                    f64::from(px[ALP]),
                    rf * p.hue_scale,
                    rf * p.sat_scale,
                    rf * p.val_scale,
                    rf * p.alp_scale,
                );
                px[RED] = rr as f32;
                px[GRE] = gg as f32;
                px[BLU] = bb as f32;
                px[ALP] = aa as f32;
            } else if channels == 3 {
                refv = (f64::from(refv) * (f64::from(nr.noise(xx, yy)) - p.offset)) as f32;
                let rf = f64::from(refv);
                let (rr, gg, bb, _) = pixel(
                    f64::from(px[RED]),
                    f64::from(px[GRE]),
                    f64::from(px[BLU]),
                    1.0,
                    rf * p.hue_scale,
                    rf * p.sat_scale,
                    rf * p.val_scale,
                    0.0,
                );
                px[RED] = rr as f32;
                px[GRE] = gg as f32;
                px[BLU] = bb as f32;
            } else {
                refv = (f64::from(refv) * (f64::from(nr.noise(xx, yy)) - p.offset)) as f32;
                if 0.0 == f64::from(refv) * p.val_scale {
                    continue;
                }
                px[0] = (f64::from(px[0]) + f64::from(refv) * p.val_scale) as f32;
            }
        }
    }
    OK
}

/// Validated raw entry.
///
/// # Safety
/// `image`: height*width*channels f32 (r/w); `noi`: height*width*4 f32 or null;
/// `refp`: height*width f32 or null.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    noi: *const f32,
    refp: *const f32,
    xoffset: i32,
    yoffset: i32,
    from_rgba: i32,
    p: &Params,
) -> i32 {
    if 0.0 == p.hue_scale && 0.0 == p.sat_scale && 0.0 == p.val_scale && 0.0 == p.alp_scale {
        return OK;
    }
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if noi.is_null() {
        return NOISE_NO_DATA;
    }
    if !(0..4).contains(&from_rgba) {
        return NOISE_BAD_ZZ;
    }
    if height < 0 || width < 0 {
        return BAD_ARG;
    }
    let pixels = (height as usize) * (width as usize);
    if pixels == 0 {
        return OK;
    }
    if image.is_null() || pixels > isize::MAX as usize / 32 {
        return BAD_ARG;
    }
    let c = channels as usize;
    let img = std::slice::from_raw_parts_mut(image, pixels * c);
    let n = std::slice::from_raw_parts(noi, pixels * 4);
    let r = if refp.is_null() { None } else { Some(std::slice::from_raw_parts(refp, pixels)) };
    change(img, height as usize, width as usize, c, Some(n), r, xoffset, yoffset, from_rgba, p)
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_hsv_add_change(
    image_array: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    noi_image_array: *const f32,
    refp: *const f32,
    xoffset: i32,
    yoffset: i32,
    from_rgba: i32,
    offset: f64,
    hue_scale: f64,
    sat_scale: f64,
    val_scale: f64,
    alp_scale: f64,
    add_blend_sw: bool,
) -> i32 {
    let p = Params { offset, hue_scale, sat_scale, val_scale, alp_scale, add_blend: add_blend_sw };
    change_raw(image_array, height, width, channels, noi_image_array, refp, xoffset, yoffset, from_rgba, &p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(val: f64) -> Params {
        Params { offset: 0.5, hue_scale: 0.0, sat_scale: 0.0, val_scale: val, alp_scale: 0.0, add_blend: false }
    }
    fn near(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn all_scales_zero_is_a_no_op_before_any_validation() {
        let mut px = [0.1f32];
        let q = Params { val_scale: 0.0, ..p(0.0) };
        assert_eq!(change(&mut px, 1, 1, 7, None, None, 0, 0, 9, &q), OK);
    }

    #[test]
    fn validation_order_channels_then_noise_then_plane() {
        let mut px = [0.1f32; 2];
        let noi = [0.5f32; 8];
        assert_eq!(change(&mut px, 1, 1, 2, None, None, 0, 0, 0, &p(1.0)), BAD_CHANNELS);
        assert_eq!(change(&mut px, 1, 1, 1, None, None, 0, 0, 0, &p(1.0)), NOISE_NO_DATA);
        assert_eq!(change(&mut px, 1, 1, 1, Some(&noi), None, 0, 0, 4, &p(1.0)), NOISE_BAD_ZZ);
        assert_eq!(change(&mut px, 1, 1, 1, Some(&noi), None, 0, 0, -1, &p(1.0)), NOISE_BAD_ZZ);
    }

    #[test]
    fn grayscale_adds_offset_noise_times_scale() {
        // noise 0.75 - offset 0.5 = 0.25 ; * val_scale 2 = 0.5
        let mut px = [0.25f32];
        let noi = [0.75f32, 0.0, 0.0, 0.0];
        assert_eq!(change(&mut px, 1, 1, 1, Some(&noi), None, 0, 0, 0, &p(2.0)), OK);
        assert!(near(px[0], 0.75));
        // noise == offset: no change
        let mut q = [0.25f32];
        change(&mut q, 1, 1, 1, Some(&[0.5, 0.0, 0.0, 0.0]), None, 0, 0, 0, &p(2.0));
        assert_eq!(q[0], 0.25);
    }

    #[test]
    fn noise_plane_is_selected_by_from_rgba() {
        let mut a = [0.0f32];
        let noi = [0.5f32, 0.5, 0.5, 1.0]; // plane 3 = 1.0
        change(&mut a, 1, 1, 1, Some(&noi), None, 0, 0, 3, &p(1.0));
        assert!(near(a[0], 0.5));
    }

    #[test]
    fn noise_tiles_with_offsets_and_wraps() {
        // 2x1 noise, plane 0 = [0.5, 1.0]; 2x1 gray zeros, xoffset 1
        let noi = [0.5f32, 0., 0., 0., 1.0, 0., 0., 0.];
        // the noise image has the target's width (noise_ref_ is built from height,width)
        let mut two = [0.0f32; 2];
        change(&mut two, 1, 2, 1, Some(&noi), None, 1, 0, 0, &p(1.0));
        // xx=0 -> (0-1) wrap -> noise[1]=1.0 -> +0.5 ; xx=1 -> noise[0]=0.5 -> +0
        assert!(near(two[0], 0.5) && near(two[1], 0.0));
    }

    #[test]
    fn rgba_skips_transparent_in_add_blend_mode_and_clamps_alpha() {
        let noi = [1.0f32, 0.0, 0.0, 0.0];
        let q = Params { offset: 0.5, hue_scale: 0.0, sat_scale: 0.0, val_scale: 0.0, alp_scale: 4.0, add_blend: true };
        let mut t = [0.3f32, 0.3, 0.3, 0.0];
        change(&mut t, 1, 1, 4, Some(&noi), None, 0, 0, 0, &q);
        assert_eq!(t, [0.3, 0.3, 0.3, 0.0]);
        // alpha .5 : refv = 0.5*0.5 = .25 ; alp += .25*4 = 1.5 -> clamp 1.0
        let mut h = [0.3f32, 0.3, 0.3, 0.5];
        change(&mut h, 1, 1, 4, Some(&noi), None, 0, 0, 0, &q);
        assert_eq!(h[3], 1.0);
        assert!(near(h[0], 0.3));
    }

    #[test]
    fn rgb_hue_noise_wraps_into_0_360() {
        // red + hue noise 0.5*(0.5 offset noise 1.0)... noise 1.0 -> refv .5, hue_scale 1 -> +180deg -> cyan
        let noi = [1.0f32, 0.0, 0.0, 0.0];
        let q = Params { offset: 0.5, hue_scale: 1.0, sat_scale: 0.0, val_scale: 0.0, alp_scale: 0.0, add_blend: false };
        let mut px = [0.0f32, 0.0, 1.0]; // B,G,R
        change(&mut px, 1, 1, 3, Some(&noi), None, 0, 0, 0, &q);
        assert!(near(px[BLU], 1.0) && near(px[GRE], 1.0) && near(px[RED], 0.0));
    }

    #[test]
    fn raw_entry_validates() {
        let mut px = [0.1f32; 4];
        let noi = [0.5f32; 4];
        unsafe {
            assert_eq!(change_raw(std::ptr::null_mut(), 1, 1, 4, noi.as_ptr(), std::ptr::null(), 0, 0, 0, &p(1.0)), BAD_ARG);
            assert_eq!(change_raw(px.as_mut_ptr(), 1, 1, 4, std::ptr::null(), std::ptr::null(), 0, 0, 0, &p(1.0)), NOISE_NO_DATA);
            assert_eq!(change_raw(px.as_mut_ptr(), -1, 1, 4, noi.as_ptr(), std::ptr::null(), 0, 0, 0, &p(1.0)), BAD_ARG);
            assert_eq!(change_raw(px.as_mut_ptr(), 1, 1, 4, noi.as_ptr(), std::ptr::null(), 0, 0, 0, &p(1.0)), OK);
        }
    }
}
