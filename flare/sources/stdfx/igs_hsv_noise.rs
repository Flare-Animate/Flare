//! Port of stdfx/igs_hsv_noise.cpp: random noise on hue / saturation / value /
//! alpha of a float image. The noise field is generated for the whole image
//! from a portable LCG (`igs_math_random`), with a separate stream for the
//! margin outside the camera rectangle so the pattern inside the camera does
//! not change when the margin changes.
//!
//! `NoiseReference` and `ControlTerm` are shared with `igs_hls_noise.rs`
//! (the two C++ files carried identical copies; only the order of the
//! parameters, and so of the random streams, differs).
//!
//! The C++ also contained an `IT`/`RT` integer template that nothing
//! instantiated (its call site is commented out); only the float path is live
//! and is ported.

use crate::igs_color_rgb_hsv::{hsv_to_rgb, rgb_to_hsv};
use crate::igs_math_random::next_d;
use std::os::raw::c_ulong;

/// 0 = ok; 1 = "Bad channels,Not rgba/rgb/grayscale" (std::domain_error); 3 = bad argument.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1;
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// igs noise_reference_: four noise planes (hue, A, B, alpha) in image size.
/// Plane order is the parameter order of the owning fx (hsv: sat, val; hls: lig, sat),
/// which also fixes the seed offsets of the random streams.
pub(crate) struct NoiseReference {
    w: i64,
    h: i64,
    nblur: f64,
    arrays: [Vec<f64>; 4],
}

impl NoiseReference {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(ww: i32, hh: i32, ranges: [f64; 4], random_seed: c_ulong, near_blur: f64, camera_x: i32, camera_y: i32, camera_w: i32, camera_h: i32) -> NoiseReference {
        let mut nr = NoiseReference { w: i64::from(ww), h: i64::from(hh), nblur: near_blur, arrays: [vec![], vec![], vec![], vec![]] };
        if ww == 0 || hh == 0 {
            return nr;
        }
        /* no range at all: no noise */
        if ranges.iter().all(|r| 0.0 == *r) {
            return nr;
        }
        let sz = (ww as usize) * (hh as usize);
        for i in 0..4 {
            if 0.0 != ranges[i] {
                nr.arrays[i] = vec![0.0; sz];
            }
        }
        /* seeds: main streams first, then the margin ("ext") streams */
        let mut step: c_ulong = 0;
        let mut rand = [1 as c_ulong; 4];
        for i in 0..4 {
            if 0.0 != ranges[i] {
                rand[i] = random_seed.wrapping_add(step);
                step = step.wrapping_add(1);
            }
        }
        let mut rand_ext = [1 as c_ulong; 4];
        for i in 0..4 {
            if 0.0 != ranges[i] {
                rand_ext[i] = random_seed.wrapping_add(step);
                step = step.wrapping_add(1);
            }
        }
        let x1 = i64::from(camera_x);
        let y1 = i64::from(camera_y);
        let x2 = i64::from(camera_x) + i64::from(camera_w) - 1;
        let y2 = i64::from(camera_y) + i64::from(camera_h) - 1;
        for i in 0..4 {
            if 0.0 == ranges[i] {
                continue;
            }
            let mut pos = 0usize;
            for yy in 0..i64::from(hh) {
                for xx in 0..i64::from(ww) {
                    nr.arrays[i][pos] = if xx < x1 || x2 < xx || yy < y1 || y2 < yy {
                        /* margin: a different pattern */
                        ranges[i] * (next_d(&mut rand_ext[i]) - 0.5)
                    } else {
                        ranges[i] * (next_d(&mut rand[i]) - 0.5)
                    };
                    pos += 1;
                }
            }
        }
        nr
    }

    fn accum_in(&self, a: &[f64], xx: i64, yy: i64, accum_val: &mut f64, accum_count: &mut i32) {
        if 0 <= xx && xx < self.w && 0 <= yy && yy < self.h {
            *accum_val += a[(yy * self.w + xx) as usize];
            *accum_count += 1;
        }
    }

    /// noise_value_ of plane `which` (0 = hue, 1 = A, 2 = B, 3 = alpha)
    pub(crate) fn value(&self, which: usize, xx: i32, yy: i32) -> f64 {
        let a = &self.arrays[which];
        if a.is_empty() {
            return 0.0;
        }
        let (xx, yy) = (i64::from(xx), i64::from(yy));
        let centre = a[(yy * self.w + xx) as usize];
        if 0.0 == self.nblur {
            return centre;
        }
        let mut accum_val = 0.0;
        let mut accum_count = 0;
        for (dx, dy) in [(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
            self.accum_in(a, xx + dx, yy + dy, &mut accum_val, &mut accum_count);
        }
        if accum_count <= 0 {
            return centre;
        }
        accum_val /= f64::from(accum_count);
        /* balance between the centre pixel and the mean of its 8 neighbours */
        (self.nblur * accum_val) + ((1.0 - self.nblur) * centre)
    }
}

/// igs control_term_within_limits_: tames the noise near the ends of the value range.
pub(crate) struct ControlTerm {
    effective_low: f64,
    effective_high: f64,
    center: f64,
    ty: i32,
    noise_range: f64,
}

impl ControlTerm {
    pub(crate) fn new(effective_low: f64, effective_high: f64, center: f64, ty: i32, noise_range: f64) -> ControlTerm {
        ControlTerm { effective_low, effective_high, center, ty, noise_range }
    }
    pub(crate) fn noise_range(&self) -> f64 {
        self.noise_range
    }

    /// `current_value` 0..1; `noise` is scaled in place, `shift_value` set for the shift types.
    /// Types: 0 shift_all, 1 shift_term, 2 decrease_all, 3 decrease_term (others do nothing).
    pub(crate) fn exec(&self, current_value: f64, noise: &mut f64, shift_value: &mut f64) {
        if 0.0 < self.effective_low && current_value < self.center {
            let (cen, val, ran, eff) = (self.center, current_value, self.noise_range, self.effective_low);
            match self.ty {
                0 => *shift_value = ((cen - val) / cen) * (ran / 2.0) * eff,
                1 => {
                    if val < ran {
                        *shift_value = ((if cen < ran { cen } else { ran }) - val) / 2.0 * eff;
                    }
                }
                2 => {
                    let tmp = (cen - val) / cen * eff;
                    if 0.0 < tmp {
                        *noise *= 1.0 - tmp;
                    }
                }
                3 => {
                    if val < (ran / 2.0) {
                        let stop = if cen < (ran / 2.0) { cen } else { ran / 2.0 };
                        let tmp = (stop - val) / stop * eff;
                        if 0.0 < tmp {
                            *noise *= 1.0 - tmp;
                        }
                    }
                }
                _ => {}
            }
        }

        if 0.0 < self.effective_high && self.center < current_value {
            let (cen, val, ran, eff) = (self.center, current_value, self.noise_range, self.effective_high);
            match self.ty {
                0 => *shift_value = ((cen - val) / (1.0 - cen)) * (ran / 2.0) * eff,
                1 => {
                    if (1.0 - ran) < val {
                        let ira = 1.0 - ran;
                        *shift_value = ((if cen < ira { ira } else { cen }) - val) / 2.0 * eff;
                    }
                }
                2 => {
                    let tmp = (val - cen) / (1.0 - cen) * eff;
                    if 0.0 < tmp {
                        *noise *= 1.0 - tmp;
                    }
                }
                3 => {
                    if (1.0 - (ran / 2.0)) < val {
                        let rpos = 1.0 - (ran / 2.0);
                        let stop = if cen < rpos { rpos } else { cen };
                        let tmp = (val - stop) / (1.0 - stop) * eff;
                        if 0.0 < tmp {
                            *noise *= 1.0 - tmp;
                        }
                    }
                }
                _ => {}
            }
        }
    }
}

/// Wraps a hue into 0..360 as the C++ `while` loops do (huge / non-finite values are left alone
/// instead of looping for ever).
pub(crate) fn wrap_hue(mut hue: f64) -> f64 {
    if hue.abs() < 1.0e7 {
        while hue < 0.0 {
            hue += 360.0;
        }
        while 360.0 <= hue {
            hue -= 360.0;
        }
    }
    hue
}

/// pixel_a_: noise on the alpha value
pub(crate) fn pixel_a(alp_in: f64, alp_noise: f64, alp_term: &ControlTerm) -> f64 {
    let mut alpin = alp_in;
    if 0.0 != alp_term.noise_range() {
        let mut shift_value = 0.0;
        let mut alpnoise = alp_noise;
        alp_term.exec(alpin, &mut alpnoise, &mut shift_value);
        let mask = alpin;
        alpin += shift_value * mask;
        alpin += alpnoise * mask;
        if alpin < 0.0 {
            alpin = 0.0;
        } else if 1.0 < alpin {
            alpin = 1.0;
        }
    }
    alpin
}

/// pixel_rgb_: noise on hue / saturation / value of one colour
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
        }
    }
    if 0.0 != val_term.noise_range() {
        let mut shift_value = 0.0;
        let mut valnoise = val_noise;
        val_term.exec(val, &mut valnoise, &mut shift_value);
        val += shift_value * alp_in;
        val += valnoise * alp_in;
    }
    let (mut r, mut g, mut b) = (red_in, gre_in, blu_in);
    hsv_to_rgb(hue, sat, val, &mut r, &mut g, &mut b);
    (r, g, b)
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
    sat_term: &ControlTerm,
    val_term: &ControlTerm,
    alp_term: &ControlTerm,
    add_blend: bool,
) {
    let any_hsv = 0.0 != hue_range || 0.0 != val_term.noise_range() || 0.0 != sat_term.noise_range();
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
                if any_hsv {
                    let (rr1, gg1, bb1, aa1) = (px[RED], px[GRE], px[BLU], px[ALP]);
                    let (mut rr2, mut gg2, mut bb2) = pixel_rgb(
                        f64::from(rr1),
                        f64::from(gg1),
                        f64::from(bb1),
                        f64::from(aa1),
                        noise.value(0, xi, yi),
                        noise.value(1, xi, yi),
                        noise.value(2, xi, yi),
                        sat_term,
                        val_term,
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
                if !any_hsv {
                    continue;
                }
                if let Some(r) = refimg {
                    refv *= r[ii];
                }
                ii += 1;
                let (rr1, gg1, bb1) = (px[RED], px[GRE], px[BLU]);
                let (mut rr2, mut gg2, mut bb2) = pixel_rgb(f64::from(rr1), f64::from(gg1), f64::from(bb1), 1.0, noise.value(0, xi, yi), noise.value(1, xi, yi), noise.value(2, xi, yi), sat_term, val_term);
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
                if 0.0 == val_term.noise_range() {
                    continue;
                }
                if let Some(r) = refimg {
                    refv *= r[ii];
                }
                ii += 1;
                let va1 = f64::from(px[0]);
                let mut shift_value = 0.0;
                let mut val_noise = noise.value(2, xi, yi);
                val_term.exec(va1, &mut val_noise, &mut shift_value);
                let mut va2 = va1;
                va2 += shift_value;
                va2 += val_noise;
                va2 = if va2 < 0.0 {
                    0.0
                } else if 1.0 < va2 {
                    1.0
                } else {
                    va2
                };
                if refv != 1.0 {
                    va2 = va1 + (va2 - va1) * f64::from(refv);
                }
                px[0] = va2 as f32;
            }
        }
    }
}

/// Validated raw entry (see `flare_igs_hsv_noise_change`). Returns [`OK`] or a status code.
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
    add_blend_sw: bool,
) -> i32 {
    if 0.0 == hue_range && 0.0 == sat_range && 0.0 == val_range && 0.0 == alp_range {
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

    let noise = NoiseReference::new(width, height, [hue_range, sat_range, val_range, alp_range], random_seed, near_blur, camera_x, camera_y, camera_w, camera_h);
    let sat_term = ControlTerm::new(sat_effective, sat_effective, sat_center, sat_type, sat_range);
    let val_term = ControlTerm::new(val_effective, val_effective, val_center, val_type, val_range);
    let alp_term = ControlTerm::new(alp_effective, alp_effective, alp_center, alp_type, alp_range);
    change_float(img, width as usize, height as usize, c, r, &noise, hue_range, &sat_term, &val_term, &alp_term, add_blend_sw);
    OK
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_hsv_noise_change(
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
    add_blend_sw: bool,
) -> i32 {
    change_raw(
        image_array, height, width, channels, refp, camera_x, camera_y, camera_w, camera_h, hue_range, sat_range, val_range, alp_range, random_seed, near_blur, sat_effective, sat_center, sat_type,
        val_effective, val_center, val_type, alp_effective, alp_center, alp_type, add_blend_sw,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_planes_use_main_and_margin_streams_with_consecutive_seeds() {
        // ranges: hue and val active (sat, alp zero) -> main seeds: hue=S, val=S+1 ; ext seeds: hue=S+2, val=S+3
        let s: c_ulong = 100;
        // 3x1 image, camera = x 1..=1 (middle pixel), so pixels 0 and 2 are margin
        let nr = NoiseReference::new(3, 1, [0.5, 0.0, 0.25, 0.0], s, 0.0, 1, 0, 1, 1);
        let mut main_hue = s;
        let mut ext_hue = s + 2;
        let mut main_val = s + 1;
        let mut ext_val = s + 3;
        assert_eq!(nr.value(0, 0, 0), 0.5 * (next_d(&mut ext_hue) - 0.5));
        assert_eq!(nr.value(0, 1, 0), 0.5 * (next_d(&mut main_hue) - 0.5));
        assert_eq!(nr.value(0, 2, 0), 0.5 * (next_d(&mut ext_hue) - 0.5));
        assert_eq!(nr.value(2, 0, 0), 0.25 * (next_d(&mut ext_val) - 0.5));
        assert_eq!(nr.value(2, 1, 0), 0.25 * (next_d(&mut main_val) - 0.5));
        // inactive planes read as zero
        assert_eq!(nr.value(1, 1, 0), 0.0);
        assert_eq!(nr.value(3, 1, 0), 0.0);
    }

    #[test]
    fn margin_size_does_not_change_the_pattern_inside_the_camera() {
        // same 2x1 camera region, once with no margin and once inside a 4x1 image with a left margin of 2
        let a = NoiseReference::new(2, 1, [0.3, 0.0, 0.0, 0.0], 7, 0.0, 0, 0, 2, 1);
        let b = NoiseReference::new(4, 1, [0.3, 0.0, 0.0, 0.0], 7, 0.0, 2, 0, 2, 1);
        assert_eq!(a.value(0, 0, 0), b.value(0, 2, 0));
        assert_eq!(a.value(0, 1, 0), b.value(0, 3, 0));
    }

    #[test]
    fn near_blur_mixes_the_eight_neighbours_and_skips_missing_ones() {
        let mut nr = NoiseReference::new(3, 3, [1.0, 0.0, 0.0, 0.0], 1, 0.5, 0, 0, 3, 3);
        nr.arrays[0] = (0..9).map(f64::from).collect(); // 0..8 row-major
        // centre (1,1)=4 ; neighbours 0,1,2,3,5,6,7,8 mean = 4 -> 0.5*4 + 0.5*4
        assert_eq!(nr.value(0, 1, 1), 4.0);
        // corner (0,0)=0 ; neighbours (1,0)=1,(0,1)=3,(1,1)=4 -> mean 8/3
        let expect = 0.5 * (8.0 / 3.0) + 0.5 * 0.0;
        assert!((nr.value(0, 0, 0) - expect).abs() < 1e-15);
    }

    #[test]
    fn all_ranges_zero_or_empty_image_gives_no_noise() {
        let nr = NoiseReference::new(2, 2, [0.0; 4], 1, 0.0, 0, 0, 2, 2);
        assert_eq!(nr.value(0, 1, 1), 0.0);
        let nr = NoiseReference::new(0, 2, [0.5, 0.0, 0.0, 0.0], 1, 0.0, 0, 0, 2, 2);
        assert_eq!(nr.value(0, 0, 0), 0.0);
    }

    #[test]
    fn control_term_types() {
        let noise_with = |t: &ControlTerm, v: f64| {
            let (mut n, mut s) = (1.0, 0.0);
            t.exec(v, &mut n, &mut s);
            (n, s)
        };
        // shift_all, low side: value 0.1, centre 0.5, range 0.4, eff 1 -> (0.5-0.1)/0.5 * 0.2 = 0.16
        let t = ControlTerm::new(1.0, 1.0, 0.5, 0, 0.4);
        let (n, s) = noise_with(&t, 0.1);
        assert_eq!(n, 1.0);
        assert!((s - 0.16).abs() < 1e-12);
        // shift_all, high side
        let (_, s) = noise_with(&t, 0.9);
        assert!((s - ((0.5 - 0.9) / 0.5 * 0.2)).abs() < 1e-12);
        // decrease_all : (0.5-0.1)/0.5*1 = 0.8 -> noise *= 0.2
        let t = ControlTerm::new(1.0, 1.0, 0.5, 2, 0.4);
        let (n, _) = noise_with(&t, 0.1);
        assert!((n - 0.2).abs() < 1e-12);
        // shift_term: only inside the range from the end
        let t = ControlTerm::new(1.0, 1.0, 0.5, 1, 0.4);
        assert!((noise_with(&t, 0.3).1 - 0.05).abs() < 1e-12); // 0.3 < range 0.4: (0.4 - 0.3) / 2
        assert_eq!(noise_with(&t, 0.45).1, 0.0); // 0.45 >= range: untouched
        // decrease_term
        let t = ControlTerm::new(1.0, 1.0, 0.5, 3, 0.4);
        let (n, _) = noise_with(&t, 0.0);
        assert!((n - 0.0).abs() < 1e-12); // at the very end the noise vanishes
        // effective 0 disables everything; unknown type does nothing
        let t = ControlTerm::new(0.0, 0.0, 0.5, 0, 0.4);
        assert_eq!(noise_with(&t, 0.1), (1.0, 0.0));
        let t = ControlTerm::new(1.0, 1.0, 0.5, 9, 0.4);
        assert_eq!(noise_with(&t, 0.1), (1.0, 0.0));
    }

    fn call(img: &mut [f32], h: i32, w: i32, ch: i32, hue: f64, sat: f64, val: f64, alp: f64, add_blend: bool) -> i32 {
        unsafe {
            change_raw(img.as_mut_ptr(), h, w, ch, std::ptr::null(), 0, 0, w, h, hue, sat, val, alp, 1, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, add_blend)
        }
    }

    #[test]
    fn nothing_happens_without_ranges_and_channels_are_checked_after() {
        let mut img = [0.1f32; 4];
        assert_eq!(call(&mut img, 1, 1, 7, 0.0, 0.0, 0.0, 0.0, true), OK);
        assert_eq!(call(&mut img, 1, 1, 7, 0.1, 0.0, 0.0, 0.0, true), BAD_CHANNELS);
        assert_eq!(img, [0.1; 4]);
    }

    #[test]
    fn hue_noise_rotates_hue_and_keeps_value() {
        // pure red, hue_range 0.5: noise = 0.5*(r-0.5) in -0.25..0.25 -> hue moves by up to +-90 degrees
        let mut img = [0.0f32, 0.0, 1.0]; // B,G,R
        assert_eq!(call(&mut img, 1, 1, 3, 0.5, 0.0, 0.0, 0.0, false), OK);
        let mut sd: c_ulong = 1;
        let n = 0.5 * (next_d(&mut sd) - 0.5);
        let expected_hue = wrap_hue(360.0 * n);
        let (h, s, v) = rgb_to_hsv(f64::from(img[RED]), f64::from(img[GRE]), f64::from(img[BLU]));
        assert!((h - expected_hue).abs() < 1e-3, "{h} vs {expected_hue}");
        assert!((s - 1.0).abs() < 1e-5 && (v - 1.0).abs() < 1e-5);
    }

    #[test]
    fn transparent_pixels_are_skipped_in_add_blend_mode() {
        let mut t = [0.2f32, 0.4, 0.6, 0.0];
        call(&mut t, 1, 1, 4, 0.5, 0.5, 0.5, 0.0, true);
        assert_eq!(t, [0.2, 0.4, 0.6, 0.0]);
        // alpha-composite mode: alpha 0 -> pixel_rgb_ returns the colour untouched
        let mut z = [0.2f32, 0.4, 0.6, 0.0];
        call(&mut z, 1, 1, 4, 0.5, 0.5, 0.5, 0.0, false);
        assert_eq!(z, [0.2, 0.4, 0.6, 0.0]);
    }

    #[test]
    fn alpha_noise_stays_in_0_1() {
        let mut img = [0.5f32, 0.5, 0.5, 0.95];
        for _ in 0..3 {
            call(&mut img, 1, 1, 4, 0.0, 0.0, 0.0, 4.0, false);
            assert!((0.0..=1.0).contains(&img[3]));
        }
    }

    #[test]
    fn grayscale_value_noise_is_clamped_to_unit_range() {
        let mut img = [0.99f32; 4];
        call(&mut img, 1, 4, 1, 0.0, 0.0, 3.0, 0.0, false);
        assert!(img.iter().all(|v| (0.0..=1.0).contains(v)));
    }

    #[test]
    fn raw_entry_validates() {
        let mut img = [0.1f32; 4];
        unsafe {
            assert_eq!(change_raw(std::ptr::null_mut(), 1, 1, 4, std::ptr::null(), 0, 0, 1, 1, 0.5, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, true), BAD_ARG);
            assert_eq!(change_raw(img.as_mut_ptr(), -1, 1, 4, std::ptr::null(), 0, 0, 1, 1, 0.5, 0.0, 0.0, 0.0, 1, 0.0, 0.0, 0.5, 0, 0.0, 0.5, 0, 0.0, 0.5, 0, true), BAD_ARG);
        }
    }
}
