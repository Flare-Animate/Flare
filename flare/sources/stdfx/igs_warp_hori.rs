//! Port of stdfx/igs_warp_hori.cpp: horizontal displacement ("warp") of a float
//! image driven by one channel of a reference image.
//!
//! Per scanline the source row is copied, then every pixel `xx` is replaced by
//! the source sample at `xx + (ref - 0.5) * maxlen` (linear interpolation when
//! `anti_aliasing`, nearest otherwise), clamped to the row.
//! Channel 3 is the alpha channel of the Toonz6 (B,G,R,A) order and is kept
//! untouched when `alpha_rendering` is false.

/// 0 = ok; the C++ header throws `std::invalid_argument` for the others.
pub const OK: i32 = 0;
pub const BAD_ARG: i32 = 3;

const ALP: usize = 3;

/// `image`: height*width*channels floats; `refer`: height*width*refchannels floats.
#[allow(clippy::too_many_arguments)]
pub fn hori_change(
    image: &mut [f32],
    height: usize,
    width: usize,
    channels: usize,
    refer: &[f32],
    refchannels: usize,
    refcc: usize,
    maxlen: f64,
    alpha_rendering: bool,
    anti_aliasing: bool,
) {
    let mut buf_s: Vec<Vec<f32>> = vec![vec![0.0; width]; channels];
    let mut buf_r: Vec<f64> = vec![0.0; width];
    let wi = width as i64;

    for yy in 0..height {
        let row = &mut image[yy * channels * width..(yy + 1) * channels * width];
        let rrow = &refer[yy * refchannels * width..(yy + 1) * refchannels * width];

        for xx in 0..width {
            for zz in 0..channels {
                buf_s[zz][xx] = row[xx * channels + zz];
            }
        }
        for xx in 0..width {
            let pos = rrow[xx * refchannels + refcc];
            // clamp 0.f to 1.f in case using TPixelF (f32::clamp would keep NaN)
            let pos = pos.max(0.0).min(1.0);
            buf_r[xx] = f64::from(pos - 0.5f32) * maxlen;
        }

        if anti_aliasing {
            for xx in 0..width {
                let pos = buf_r[xx] as f32;
                let fl = pos.floor();
                let div = pos - fl;
                let fl_pos = (xx as i64 + fl as i64).clamp(0, wi - 1) as usize;
                let ce_pos = (xx as i64 + pos.ceil() as i64).clamp(0, wi - 1) as usize;
                for zz in 0..channels {
                    row[xx * channels + zz] = if !alpha_rendering && ALP == zz {
                        buf_s[zz][xx]
                    } else {
                        // C++ types: float * double + float * float (the second product is f32)
                        (f64::from(buf_s[zz][fl_pos]) * (1.0 - f64::from(div)) + f64::from(buf_s[zz][ce_pos] * div)) as f32
                    };
                }
            }
        } else {
            for xx in 0..width {
                let pos = (xx as i64 + (buf_r[xx] + 0.5).floor() as i64).clamp(0, wi - 1) as usize;
                for zz in 0..channels {
                    row[xx * channels + zz] = if !alpha_rendering && ALP == zz {
                        buf_s[zz][xx]
                    } else {
                        buf_s[zz][pos]
                    };
                }
            }
        }
    }
}

/// Validated raw-pointer entry. Returns [`OK`] or [`BAD_ARG`] (null pointer,
/// non-positive geometry, `refcc` outside `refchannels`, size overflow).
///
/// # Safety
/// `image` must be valid for `height*width*channels` f32 read/write and
/// `refer` for `height*width*refchannels` f32 reads.
#[allow(clippy::too_many_arguments)]
pub unsafe fn hori_change_raw(
    image: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    refer: *const f32,
    refchannels: i32,
    refcc: i32,
    maxlen: f64,
    alpha_rendering: bool,
    anti_aliasing: bool,
) -> i32 {
    if image.is_null() || refer.is_null() || height <= 0 || width <= 0 || channels <= 0 || refchannels <= 0 || refcc < 0 || refcc >= refchannels {
        return BAD_ARG;
    }
    let (h, w, c, rc) = (height as usize, width as usize, channels as usize, refchannels as usize);
    let (Some(n), Some(rn)) = (h.checked_mul(w).and_then(|v| v.checked_mul(c)), h.checked_mul(w).and_then(|v| v.checked_mul(rc))) else {
        return BAD_ARG;
    };
    if n > isize::MAX as usize / 4 || rn > isize::MAX as usize / 4 {
        return BAD_ARG;
    }
    let img = std::slice::from_raw_parts_mut(image, n);
    let rf = std::slice::from_raw_parts(refer, rn);
    hori_change(img, h, w, c, rf, rc, refcc as usize, maxlen, alpha_rendering, anti_aliasing);
    OK
}

/// # Safety
/// See [`hori_change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_warp_hori_change(
    image: *mut f32,
    height: i32,
    width: i32,
    channels: i32,
    refer: *const f32,
    refchannels: i32,
    refcc: i32,
    maxlen: f64,
    alpha_rendering_sw: bool,
    anti_aliasing_sw: bool,
) -> i32 {
    hori_change_raw(image, height, width, channels, refer, refchannels, refcc, maxlen, alpha_rendering_sw, anti_aliasing_sw)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(img: &mut [f32], w: usize, ch: usize, refv: &[f32], maxlen: f64, alpha: bool, aa: bool) {
        // single row, 1-channel reference
        hori_change(img, 1, w, ch, refv, 1, 0, maxlen, alpha, aa);
    }

    #[test]
    fn reference_half_is_identity() {
        let mut img = [1.0f32, 2.0, 3.0, 4.0];
        run(&mut img, 4, 1, &[0.5; 4], 3.0, true, true);
        assert_eq!(img, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn whole_pixel_shift_nearest_and_clamped() {
        // ref=1.0 -> +0.5*maxlen = +2 pixels sampled to the right; clamped at the edge
        let mut img = [10.0f32, 20.0, 30.0, 40.0];
        run(&mut img, 4, 1, &[1.0; 4], 4.0, true, false);
        assert_eq!(img, [30.0, 40.0, 40.0, 40.0]);
        // ref=0.0 -> -2
        let mut img = [10.0f32, 20.0, 30.0, 40.0];
        run(&mut img, 4, 1, &[0.0; 4], 4.0, true, false);
        assert_eq!(img, [10.0, 10.0, 10.0, 20.0]);
    }

    #[test]
    fn antialias_interpolates_between_neighbours() {
        // offset +0.5 pixel -> halfway between xx and xx+1
        let mut img = [0.0f32, 10.0, 20.0, 30.0];
        run(&mut img, 4, 1, &[1.0; 4], 1.0, true, true);
        assert_eq!(img, [5.0, 15.0, 25.0, 30.0]);
    }

    #[test]
    fn reference_is_clamped_to_unit_range() {
        let mut a = [1.0f32, 2.0, 3.0, 4.0];
        let mut b = a;
        run(&mut a, 4, 1, &[7.0; 4], 2.0, true, false);
        run(&mut b, 4, 1, &[1.0; 4], 2.0, true, false);
        assert_eq!(a, b);
    }

    #[test]
    fn alpha_is_not_warped_when_disabled() {
        // 4 channels (B,G,R,A), 3 pixels; shift by one pixel
        let mut img = [1.0f32, 1.0, 1.0, 0.1, 2.0, 2.0, 2.0, 0.2, 3.0, 3.0, 3.0, 0.3];
        run(&mut img, 3, 4, &[1.0; 3], 2.0, false, false);
        assert_eq!(img, [2.0, 2.0, 2.0, 0.1, 3.0, 3.0, 3.0, 0.2, 3.0, 3.0, 3.0, 0.3]);
        let mut img = [1.0f32, 1.0, 1.0, 0.1, 2.0, 2.0, 2.0, 0.2, 3.0, 3.0, 3.0, 0.3];
        run(&mut img, 3, 4, &[1.0; 3], 2.0, true, false);
        assert_eq!(img[3], 0.2);
    }

    #[test]
    fn reference_channel_and_rows_are_independent() {
        // 2 rows, refchannels 2, take channel 1: row0 shift right, row1 identity
        let mut img = [1.0f32, 2.0, 3.0, 1.0, 2.0, 3.0];
        let refr = [0.0f32, 1.0, 0.0, 1.0, 0.0, 1.0, /* row1 */ 0.0, 0.5, 0.0, 0.5, 0.0, 0.5];
        hori_change(&mut img, 2, 3, 1, &refr, 2, 1, 2.0, true, false);
        assert_eq!(img, [2.0, 3.0, 3.0, 1.0, 2.0, 3.0]);
    }

    #[test]
    fn raw_entry_validates_arguments() {
        let mut img = [0.0f32; 4];
        let r = [0.5f32; 4];
        unsafe {
            assert_eq!(hori_change_raw(std::ptr::null_mut(), 1, 4, 1, r.as_ptr(), 1, 0, 1.0, true, true), BAD_ARG);
            assert_eq!(hori_change_raw(img.as_mut_ptr(), 1, 4, 1, std::ptr::null(), 1, 0, 1.0, true, true), BAD_ARG);
            assert_eq!(hori_change_raw(img.as_mut_ptr(), -1, 4, 1, r.as_ptr(), 1, 0, 1.0, true, true), BAD_ARG);
            assert_eq!(hori_change_raw(img.as_mut_ptr(), 1, 4, 1, r.as_ptr(), 1, 1, 1.0, true, true), BAD_ARG);
            assert_eq!(hori_change_raw(img.as_mut_ptr(), i32::MAX, i32::MAX, 4, r.as_ptr(), 1, 0, 1.0, true, true), BAD_ARG);
            assert_eq!(hori_change_raw(img.as_mut_ptr(), 1, 4, 1, r.as_ptr(), 1, 0, 1.0, true, true), OK);
        }
    }
}
