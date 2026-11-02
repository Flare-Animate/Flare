//! Port of stdfx/igs_warp_vert.cpp: vertical displacement ("warp") of a float
//! image driven by one channel of a reference image. Column-wise twin of
//! `igs_warp_hori.rs`, with two differences kept from the C++:
//! the reference value is *not* clamped to 0..1 and the displacement table is
//! stored as `float`.

/// 0 = ok; the C++ header throws `std::invalid_argument` for the others.
pub const OK: i32 = 0;
pub const BAD_ARG: i32 = 3;

const ALP: usize = 3;

/// `image`: height*width*channels floats; `refer`: height*width*refchannels floats.
#[allow(clippy::too_many_arguments)]
pub fn vert_change(
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
    let mut buf_s: Vec<Vec<f32>> = vec![vec![0.0; height]; channels];
    let mut buf_r: Vec<f32> = vec![0.0; height];
    let hi = height as i64;
    let stride = width * channels;

    for xx in 0..width {
        let col = xx * channels;
        for yy in 0..height {
            for zz in 0..channels {
                buf_s[zz][yy] = image[yy * stride + col + zz];
            }
        }
        for yy in 0..height {
            let pos = refer[yy * width * refchannels + xx * refchannels + refcc];
            buf_r[yy] = (f64::from(pos - 0.5f32) * maxlen) as f32;
        }

        if anti_aliasing {
            for yy in 0..height {
                let pos = f64::from(buf_r[yy]);
                let fl = pos.floor();
                let div = (pos - fl) as f32;
                let fl_pos = (yy as i64 + fl as i64).clamp(0, hi - 1) as usize;
                let ce_pos = (yy as i64 + pos.ceil() as i64).clamp(0, hi - 1) as usize;
                for zz in 0..channels {
                    image[yy * stride + col + zz] = if !alpha_rendering && ALP == zz {
                        buf_s[zz][yy]
                    } else {
                        // C++ types: float * double + float * float (the second product is f32)
                        (f64::from(buf_s[zz][fl_pos]) * (1.0 - f64::from(div)) + f64::from(buf_s[zz][ce_pos] * div)) as f32
                    };
                }
            }
        } else {
            for yy in 0..height {
                let pos = (yy as i64 + (f64::from(buf_r[yy]) + 0.5).floor() as i64).clamp(0, hi - 1) as usize;
                for zz in 0..channels {
                    image[yy * stride + col + zz] = if !alpha_rendering && ALP == zz {
                        buf_s[zz][yy]
                    } else {
                        buf_s[zz][pos]
                    };
                }
            }
        }
    }
}

/// Validated raw-pointer entry. Returns [`OK`] or [`BAD_ARG`].
///
/// # Safety
/// `image` must be valid for `height*width*channels` f32 read/write and
/// `refer` for `height*width*refchannels` f32 reads.
#[allow(clippy::too_many_arguments)]
pub unsafe fn vert_change_raw(
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
    vert_change(img, h, w, c, rf, rc, refcc as usize, maxlen, alpha_rendering, anti_aliasing);
    OK
}

/// # Safety
/// See [`vert_change_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_warp_vert_change(
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
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    vert_change_raw(image, height, width, channels, refer, refchannels, refcc, maxlen, alpha_rendering_sw, anti_aliasing_sw)
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(img: &mut [f32], h: usize, ch: usize, refv: &[f32], maxlen: f64, alpha: bool, aa: bool) {
        // single column, 1-channel reference
        vert_change(img, h, 1, ch, refv, 1, 0, maxlen, alpha, aa);
    }

    #[test]
    fn reference_half_is_identity() {
        let mut img = [1.0f32, 2.0, 3.0, 4.0];
        run(&mut img, 4, 1, &[0.5; 4], 3.0, true, true);
        assert_eq!(img, [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn whole_pixel_shift_nearest_and_clamped() {
        let mut img = [10.0f32, 20.0, 30.0, 40.0];
        run(&mut img, 4, 1, &[1.0; 4], 4.0, true, false);
        assert_eq!(img, [30.0, 40.0, 40.0, 40.0]);
        let mut img = [10.0f32, 20.0, 30.0, 40.0];
        run(&mut img, 4, 1, &[0.0; 4], 4.0, true, false);
        assert_eq!(img, [10.0, 10.0, 10.0, 20.0]);
    }

    #[test]
    fn antialias_interpolates_between_neighbours() {
        let mut img = [0.0f32, 10.0, 20.0, 30.0];
        run(&mut img, 4, 1, &[1.0; 4], 1.0, true, true);
        assert_eq!(img, [5.0, 15.0, 25.0, 30.0]);
    }

    #[test]
    fn reference_is_not_clamped_unlike_horizontal() {
        // ref 3.0 -> (3-0.5)*2 = +5 pixels (hori would clamp ref to 1.0 -> +1)
        let mut img = [1.0f32, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
        run(&mut img, 7, 1, &[3.0; 7], 2.0, true, false);
        assert_eq!(img, [6.0, 7.0, 7.0, 7.0, 7.0, 7.0, 7.0]);
    }

    #[test]
    fn alpha_is_not_warped_when_disabled() {
        let mut img = [1.0f32, 1.0, 1.0, 0.1, 2.0, 2.0, 2.0, 0.2, 3.0, 3.0, 3.0, 0.3];
        run(&mut img, 3, 4, &[1.0; 3], 2.0, false, false);
        assert_eq!(img, [2.0, 2.0, 2.0, 0.1, 3.0, 3.0, 3.0, 0.2, 3.0, 3.0, 3.0, 0.3]);
    }

    #[test]
    fn columns_use_their_own_reference_values() {
        // 2 wide, 3 tall, 1 channel, refchannels 1: left column shifts down-sample by 1, right is identity
        let mut img = [1.0f32, 10.0, 2.0, 20.0, 3.0, 30.0];
        let refr = [1.0f32, 0.5, 1.0, 0.5, 1.0, 0.5];
        vert_change(&mut img, 3, 2, 1, &refr, 1, 0, 2.0, true, false);
        assert_eq!(img, [2.0, 10.0, 3.0, 20.0, 3.0, 30.0]);
    }

    #[test]
    fn raw_entry_validates_arguments() {
        let mut img = [0.0f32; 4];
        let r = [0.5f32; 4];
        unsafe {
            assert_eq!(vert_change_raw(std::ptr::null_mut(), 4, 1, 1, r.as_ptr(), 1, 0, 1.0, true, true), BAD_ARG);
            assert_eq!(vert_change_raw(img.as_mut_ptr(), 4, 0, 1, r.as_ptr(), 1, 0, 1.0, true, true), BAD_ARG);
            assert_eq!(vert_change_raw(img.as_mut_ptr(), 4, 1, 1, r.as_ptr(), 1, -1, 1.0, true, true), BAD_ARG);
            assert_eq!(vert_change_raw(img.as_mut_ptr(), 4, 1, 1, r.as_ptr(), 1, 0, 1.0, true, true), OK);
        }
    }
}
