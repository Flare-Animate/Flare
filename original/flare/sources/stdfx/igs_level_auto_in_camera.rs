//! Port of stdfx/igs_level_auto_in_camera.cpp: like `igs_level_auto`, but the
//! input min/max are measured only inside the camera rectangle; the result is
//! applied to the whole image. 8/16-bit use the lookup-table path of
//! `igs_level_auto`; float (bits == 24, `numeric_limits<float>::digits`) converts every sample directly (and,
//! as in the C++, ignores `act_sw`).
//!
//! The C++ read at `(camera_x, camera_y)` unconditionally and trusted the
//! rectangle; here the rectangle is clamped to the image, which is identical
//! for every in-bounds rectangle the fx passes.

use crate::igs_level_auto::{apply_table, level_ctable, Sample};
use crate::igs_levels::FLOAT_BITS;

/// 0 = ok; 1 / 2 = std::domain_error texts; 3 = bad argument.
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1; // "Bad channels,Not rgba/rgb/grayscale"
pub const BAD_BITS: i32 = 2; // "Bad bits,Not uchar/ushort"
pub const BAD_ARG: i32 = 3;

const BLU: usize = 0;
const GRE: usize = 1;
const RED: usize = 2;
const ALP: usize = 3;

/// Camera rectangle clamped into the image: (x0, y0, x1, y1), x1/y1 exclusive.
fn clamp_camera(width: usize, height: usize, cx: i32, cy: i32, cw: i32, ch: i32) -> (usize, usize, usize, usize) {
    let x0 = i64::from(cx).clamp(0, width as i64 - 1);
    let y0 = i64::from(cy).clamp(0, height as i64 - 1);
    let x1 = (i64::from(cx) + i64::from(cw.max(0))).clamp(x0, width as i64);
    let y1 = (i64::from(cy) + i64::from(ch.max(0))).clamp(y0, height as i64);
    (x0 as usize, y0 as usize, x1 as usize, y1 as usize)
}

#[allow(clippy::too_many_arguments)]
fn change_int<T: Sample>(
    image: &mut [T],
    height: usize,
    width: usize,
    channels: usize,
    act_sw: &[bool],
    in_min_shift: &[f64],
    in_max_shift: &[f64],
    out_min: &[f64],
    out_max: &[f64],
    gamma: &[f64],
    cam: (usize, usize, usize, usize),
) {
    let _ = height;
    let (x0, y0, x1, y1) = cam;
    let mut in_min = [0i32; 4];
    let mut in_max = [0i32; 4];
    let first = (y0 * width + x0) * channels;
    for zz in 0..channels {
        in_min[zz] = image[first + zz].into();
        in_max[zz] = in_min[zz];
    }
    for yy in y0..y1 {
        for xx in x0..x1 {
            let base = (yy * width + xx) * channels;
            for zz in 0..channels {
                let v: i32 = image[base + zz].into();
                if v < in_min[zz] {
                    in_min[zz] = v;
                } else if in_max[zz] < v {
                    in_max[zz] = v;
                }
            }
        }
    }
    let table = level_ctable(channels, act_sw, &in_min, &in_max, in_min_shift, in_max_shift, out_min, out_max, gamma, T::MAX);
    apply_table(image, channels, &table);
}

/// igs LevelAutoValueF (float path)
struct ValueF {
    in_min: [f64; 4],
    out_min: [f64; 4],
    gamma: [f64; 4],
    in_range: [f64; 4],
    out_range: [f64; 4],
}

impl ValueF {
    #[allow(clippy::too_many_arguments)]
    fn new(channels: usize, in_min: &[f32; 4], in_max: &[f32; 4], in_min_shift: &[f64], in_max_shift: &[f64], out_min: &[f64], out_max: &[f64], gamma: &[f64]) -> Self {
        let mut v = ValueF { in_min: [0.0; 4], out_min: [0.0; 4], gamma: [0.0; 4], in_range: [0.0; 4], out_range: [0.0; 4] };
        for c in 0..channels {
            v.in_min[c] = f64::from(in_min[c]) + in_min_shift[c];
            v.out_min[c] = out_min[c];
            v.gamma[c] = gamma[c];
            v.in_range[c] = f64::from(in_max[c]) + in_max_shift[c] - v.in_min[c];
            v.out_range[c] = out_max[c] - out_min[c];
        }
        v
    }

    fn convert(&self, c: usize, value: f32) -> f32 {
        let mut value = value;
        if self.in_range[c] == 0.0 {
            value = self.in_min[c] as f32;
        } else {
            value = ((f64::from(value) - self.in_min[c]) / self.in_range[c]) as f32;
        }
        if 1.0 != self.gamma[c] && 0.0 != self.gamma[c] && f64::from(value) > 0.0 {
            value = f64::from(value).powf(1.0 / self.gamma[c]) as f32;
        }
        (self.out_min[c] + f64::from(value) * self.out_range[c]) as f32
    }
}

#[allow(clippy::too_many_arguments)]
fn change_float(
    image: &mut [f32],
    width: usize,
    channels: usize,
    in_min_shift: &[f64],
    in_max_shift: &[f64],
    out_min: &[f64],
    out_max: &[f64],
    gamma: &[f64],
    cam: (usize, usize, usize, usize),
) {
    let (x0, y0, x1, y1) = cam;
    let mut in_min = [0f32; 4];
    let mut in_max = [0f32; 4];
    let first = (y0 * width + x0) * channels;
    for zz in 0..channels {
        in_min[zz] = image[first + zz];
        in_max[zz] = in_min[zz];
    }
    for yy in y0..y1 {
        for xx in x0..x1 {
            let base = (yy * width + xx) * channels;
            for zz in 0..channels {
                let v = image[base + zz];
                if v < in_min[zz] {
                    in_min[zz] = v;
                } else if in_max[zz] < v {
                    in_max[zz] = v;
                }
            }
        }
    }
    let conv = ValueF::new(channels, &in_min, &in_max, in_min_shift, in_max_shift, out_min, out_max, gamma);
    if channels == 4 {
        for px in image.chunks_exact_mut(4) {
            px[RED] = conv.convert(0, px[RED]);
            px[GRE] = conv.convert(1, px[GRE]);
            px[BLU] = conv.convert(2, px[BLU]);
            px[ALP] = conv.convert(3, px[ALP]);
        }
    } else if channels == 3 {
        for px in image.chunks_exact_mut(3) {
            px[RED] = conv.convert(0, px[RED]);
            px[GRE] = conv.convert(1, px[GRE]);
            px[BLU] = conv.convert(2, px[BLU]);
        }
    } else {
        for v in image.iter_mut() {
            *v = conv.convert(0, *v);
        }
    }
}

/// Validated raw entry.
///
/// # Safety
/// `image_array` covers `height*width*channels` samples of `bits` bits (aligned for the
/// sample type); `act_sw` and the five double arrays hold `channels` entries.
#[allow(clippy::too_many_arguments)]
pub unsafe fn change_raw(
    image_array: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    act_sw: *const bool,
    in_min_shift: *const f64,
    in_max_shift: *const f64,
    out_min: *const f64,
    out_max: *const f64,
    gamma: *const f64,
    camera_x: i32,
    camera_y: i32,
    camera_w: i32,
    camera_h: i32,
) -> i32 {
    if channels != 4 && channels != 3 && channels != 1 {
        return BAD_CHANNELS;
    }
    if bits != 8 && bits != 16 && bits != FLOAT_BITS {
        return BAD_BITS;
    }
    if height < 0 || width < 0 || act_sw.is_null() || in_min_shift.is_null() || in_max_shift.is_null() || out_min.is_null() || out_max.is_null() || gamma.is_null() {
        return BAD_ARG;
    }
    let c = channels as usize;
    let (h, w) = (height as usize, width as usize);
    let Some(n) = h.checked_mul(w).and_then(|v| v.checked_mul(c)) else {
        return BAD_ARG;
    };
    if n == 0 {
        return OK;
    }
    let sample = if bits == FLOAT_BITS { 4 } else { bits as usize / 8 };
    if image_array.is_null() || n > isize::MAX as usize / 8 {
        return BAD_ARG;
    }
    if (image_array as usize) % sample != 0 {
        // Misaligned image: work on an aligned copy and write it back.
        let bytes = n * sample;
        let mut copy = vec![0u64; bytes.div_ceil(8)];
        std::ptr::copy_nonoverlapping(image_array, copy.as_mut_ptr() as *mut u8, bytes);
        let st = change_raw(
            copy.as_mut_ptr() as *mut u8, height, width, channels, bits, act_sw, in_min_shift, in_max_shift, out_min, out_max, gamma,
            camera_x, camera_y, camera_w, camera_h,
        );
        if st == OK {
            std::ptr::copy_nonoverlapping(copy.as_ptr() as *const u8, image_array, bytes);
        }
        return st;
    }
    let cam = clamp_camera(w, h, camera_x, camera_y, camera_w, camera_h);
    let act = std::slice::from_raw_parts(act_sw, c);
    let (imin, imax, omin, omax, gam) = (
        std::slice::from_raw_parts(in_min_shift, c),
        std::slice::from_raw_parts(in_max_shift, c),
        std::slice::from_raw_parts(out_min, c),
        std::slice::from_raw_parts(out_max, c),
        std::slice::from_raw_parts(gamma, c),
    );
    match bits {
        8 => change_int::<u8>(std::slice::from_raw_parts_mut(image_array, n), h, w, c, act, imin, imax, omin, omax, gam, cam),
        16 => change_int::<u16>(std::slice::from_raw_parts_mut(image_array as *mut u16, n), h, w, c, act, imin, imax, omin, omax, gam, cam),
        _ => change_float(std::slice::from_raw_parts_mut(image_array as *mut f32, n), w, c, imin, imax, omin, omax, gam, cam),
    }
    OK
}

/// # Safety
/// See [`change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_level_auto_in_camera_change(
    image_array: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    act_sw: *const bool,
    in_min_shift: *const f64,
    in_max_shift: *const f64,
    out_min: *const f64,
    out_max: *const f64,
    gamma: *const f64,
    camera_x: i32,
    camera_y: i32,
    camera_w: i32,
    camera_h: i32,
) -> i32 {
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    change_raw(image_array, height, width, channels, bits, act_sw, in_min_shift, in_max_shift, out_min, out_max, gamma, camera_x, camera_y, camera_w, camera_h)
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ON: [bool; 4] = [true; 4];
    const Z: [f64; 4] = [0.0; 4];
    const ONE: [f64; 4] = [1.0; 4];

    unsafe fn run(img: *mut u8, h: i32, w: i32, ch: i32, bits: i32, cam: [i32; 4]) -> i32 {
        change_raw(img, h, w, ch, bits, ON.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr(), cam[0], cam[1], cam[2], cam[3])
    }

    #[test]
    fn range_is_measured_inside_the_camera_only() {
        // 4x1 gray: [0, 100, 200, 255]; camera = middle two pixels -> 100..200 stretched, outside clamps
        let mut img = [0u8, 100, 200, 255];
        assert_eq!(unsafe { run(img.as_mut_ptr(), 1, 4, 1, 8, [1, 0, 2, 1]) }, OK);
        assert_eq!(img[0], 0);
        assert_eq!(img[1], 0);
        assert_eq!(img[2], 255);
        assert_eq!(img[3], 255);
    }

    #[test]
    fn full_camera_matches_whole_image_auto_level() {
        let mut a = [50u8, 100, 150];
        let mut b = a;
        unsafe { run(a.as_mut_ptr(), 1, 3, 1, 8, [0, 0, 3, 1]) };
        crate::igs_level_auto::change(&mut b, 1, 8, &ON, &Z, &Z, &Z, &ONE, &ONE);
        assert_eq!(a, b);
    }

    #[test]
    fn float_path_is_linear_and_ignores_act_sw() {
        let mut f = [0.25f32, 0.5, 0.75];
        let off = [false; 4];
        let r = unsafe {
            change_raw(f.as_mut_ptr() as *mut u8, 1, 3, 1, FLOAT_BITS, off.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), ONE.as_ptr(), 0, 0, 3, 1)
        };
        assert_eq!(r, OK);
        assert_eq!(f, [0.0, 0.5, 1.0]);
    }

    #[test]
    fn float_flat_range_maps_to_in_min() {
        // in_max+shift - in_min == 0 -> value = in_min (then scaled to the output range)
        let mut f = [0.4f32, 0.4];
        unsafe { run(f.as_mut_ptr() as *mut u8, 1, 2, 1, FLOAT_BITS, [0, 0, 2, 1]) };
        assert_eq!(f, [0.4, 0.4]);
    }

    #[test]
    fn float_gamma_only_applies_to_positive_values() {
        let mut f = [-1.0f32, 0.0, 1.0];
        let g = [2.0f64; 4];
        // camera over everything: in_min -1, in_max 1 -> normalized 0, .5, 1 -> gamma 2 -> sqrt
        unsafe { change_raw(f.as_mut_ptr() as *mut u8, 1, 3, 1, FLOAT_BITS, ON.as_ptr(), Z.as_ptr(), Z.as_ptr(), Z.as_ptr(), ONE.as_ptr(), g.as_ptr(), 0, 0, 3, 1) };
        assert_eq!(f[0], 0.0);
        assert!((f[1] - 0.5f32.sqrt()).abs() < 1e-6);
        assert_eq!(f[2], 1.0);
    }

    #[test]
    fn camera_outside_the_image_is_clamped_not_ub() {
        let mut img = [10u8, 20, 30, 40];
        assert_eq!(unsafe { run(img.as_mut_ptr(), 2, 2, 1, 8, [-5, -5, 100, 100]) }, OK);
        assert_eq!(unsafe { run(img.as_mut_ptr(), 2, 2, 1, 8, [50, 50, 3, 3]) }, OK);
    }

    #[test]
    fn misaligned_float_buffer_works_like_an_aligned_one() {
        let vals = [0.2f32, 0.4, 0.6];
        let mut aligned = vals;
        assert_eq!(unsafe { run(aligned.as_mut_ptr() as *mut u8, 1, 3, 1, FLOAT_BITS, [0, 0, 3, 1]) }, OK);
        // same samples stored at an odd byte offset
        let mut raw = [0u8; 13];
        for (k, v) in vals.iter().enumerate() {
            raw[1 + k * 4..5 + k * 4].copy_from_slice(&v.to_ne_bytes());
        }
        assert_eq!(unsafe { run(raw.as_mut_ptr().add(1), 1, 3, 1, FLOAT_BITS, [0, 0, 3, 1]) }, OK);
        for (k, want) in aligned.iter().enumerate() {
            let got = f32::from_ne_bytes(raw[1 + k * 4..5 + k * 4].try_into().unwrap());
            assert_eq!(got, *want);
        }
    }

    #[test]
    fn errors() {
        let mut img = [0u8; 16];
        unsafe {
            assert_eq!(run(img.as_mut_ptr(), 2, 2, 2, 8, [0, 0, 2, 2]), BAD_CHANNELS);
            assert_eq!(run(img.as_mut_ptr(), 2, 2, 1, 12, [0, 0, 2, 2]), BAD_BITS);
            assert_eq!(run(std::ptr::null_mut(), 2, 2, 1, 8, [0, 0, 2, 2]), BAD_ARG);
            assert_eq!(run(img.as_mut_ptr(), -2, 2, 1, 8, [0, 0, 2, 2]), BAD_ARG);
            assert_eq!(run(img.as_mut_ptr(), 0, 2, 1, 8, [0, 0, 2, 2]), OK);
        }
    }
}
