//! Port of stdfx/igs_median_filter.cpp: circular median filter on 8/16-bit
//! images, optionally weighted per pixel by a reference image.
//!
//! The median of an even count is the upper of the two middle samples (the
//! original pixel values are never invented). Outside the image the edge is
//! spread / flipped / black / repeated according to `out_side_type`.

use crate::igs_levels::{ref_value, refchk_int, IntPx, RefPx};

/// 0 = ok; 1 = bad channels, 2 = bad bits (std::domain_error); 3 = bad argument
/// (std::invalid_argument).
pub const OK: i32 = 0;
pub const BAD_CHANNELS: i32 = 1;
pub const BAD_BITS: i32 = 2;
pub const BAD_ARG: i32 = 3;

const BLU: i32 = 0;
const GRE: i32 = 1;
const RED: i32 = 2;
const ALP: i32 = 3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum OutOfImage {
    SpreadEdge,
    FlipRepeat,
    Black,
    Repeat,
}

/// igs::median_filter::pixrender
pub struct PixRender {
    pub xp: Vec<i32>,
    pub yp: Vec<i32>,
    ty: OutOfImage,
}

impl PixRender {
    /// Offsets of the pixels inside the circle; `None` when the circle holds no pixel
    /// (the C++ then threw std::out_of_range from `around.at(0)`) or is unreasonably large.
    pub fn new(radius: f64, ty: OutOfImage) -> Option<PixRender> {
        let ri = radius.ceil();
        if !ri.is_finite() || ri < 0.0 || ri > 4096.0 {
            return None;
        }
        let ri = ri as i32;
        let (mut xp, mut yp) = (Vec::new(), Vec::new());
        for yy in -ri..=ri {
            for xx in -ri..=ri {
                let (xr, yr) = (f64::from(xx), f64::from(yy));
                if xr * xr + yr * yr <= radius * radius + 1e-6 {
                    xp.push(xx);
                    yp.push(yy);
                }
            }
        }
        if xp.is_empty() {
            return None;
        }
        Some(PixRender { xp, yp, ty })
    }

    /// pixrender::position
    pub fn position(&self, ww: i32, hh: i32, xx: &mut i32, yy: &mut i32) {
        match self.ty {
            OutOfImage::SpreadEdge => {
                *xx = if *xx < 0 { 0 } else if ww <= *xx { ww - 1 } else { *xx };
                *yy = if *yy < 0 { 0 } else if hh <= *yy { hh - 1 } else { *yy };
            }
            OutOfImage::FlipRepeat => {
                if *xx < 0 {
                    *xx = (*xx).rem_euclid(ww); // same as adding ww until >= 0
                    *xx = ww - 1 - *xx;
                }
                if ww <= *xx {
                    *xx %= ww;
                    *xx = ww - 1 - *xx;
                }
                if *yy < 0 {
                    *yy = (*yy).rem_euclid(hh);
                    *yy = hh - 1 - *yy;
                }
                if hh <= *yy {
                    *yy %= hh;
                    *yy = hh - 1 - *yy;
                }
            }
            OutOfImage::Black => {
                if *xx < 0 || ww <= *xx {
                    *xx = -1;
                }
                if *yy < 0 || hh <= *yy {
                    *yy = -1;
                }
            }
            OutOfImage::Repeat => {
                *xx = (*xx).rem_euclid(ww);
                *yy = (*yy).rem_euclid(hh);
            }
        }
    }
}

/// getter_(): sample at (xx, yy) after re-positioning, 0 when "black"
fn getter<I: IntPx>(pixr: &PixRender, image: &[I], hh: i32, ww: i32, ch: i32, mut xx: i32, mut yy: i32, zz: i32) -> i32 {
    pixr.position(ww, hh, &mut xx, &mut yy);
    if xx < 0 || yy < 0 {
        return 0;
    }
    image[(ww as usize) * (ch as usize) * (yy as usize) + (ch as usize) * (xx as usize) + zz as usize].to_i32()
}

/// median_filter_(): the upper median of the samples inside the circle
#[allow(clippy::too_many_arguments)]
fn median<I: IntPx>(pixr: &PixRender, around: &mut [i32], image: &[I], hh: i32, ww: i32, ch: i32, xx: i32, yy: i32, zz: i32) -> I {
    for (ii, a) in around.iter_mut().enumerate() {
        *a = getter(pixr, image, hh, ww, ch, xx + pixr.xp[ii], yy + pixr.yp[ii], zz);
    }
    let mid = around.len() / 2;
    // value at index len/2 of the sorted list, without sorting everything
    let (_, m, _) = around.select_nth_unstable(mid);
    I::from_i32(*m)
}

#[allow(clippy::too_many_arguments)]
fn convert_template<I: IntPx, R: RefPx>(
    inp: &[I],
    out: &mut [I],
    hh: i32,
    ww: i32,
    ch: i32,
    refimg: Option<&[R]>,
    ref_mode: i32,
    zz: Option<i32>,
    pixr: &PixRender,
) {
    let c = ch as usize;
    let mut around = vec![0i32; pixr.xp.len()];
    let mut p = 0usize; // pixel index
    for yy in 0..hh {
        for xx in 0..ww {
            let (ip, op) = (&inp[p * c..(p + 1) * c], p * c);
            let mut refv = 1.0f64;
            if let Some(r) = refimg {
                refv *= ref_value(&r[p * c..(p + 1) * c], c, R::MAX, ref_mode);
            }
            if let Some(z) = zz {
                let v1 = median(pixr, &mut around, inp, hh, ww, ch, xx, yy, z);
                let v2 = I::from_f64(refchk_int(ip[z as usize].to_i32(), v1.to_i32(), refv));
                for k in 0..c {
                    out[op + k] = v2;
                }
            } else {
                for z in 0..ch {
                    let v1 = median(pixr, &mut around, inp, hh, ww, ch, xx, yy, z);
                    out[op + z as usize] = I::from_f64(refchk_int(ip[z as usize].to_i32(), v1.to_i32(), refv));
                }
            }
            p += 1;
        }
    }
}

/// z2 of the C++: the colour channel to process, `None` = each channel on its own.
pub(crate) fn channel_for(zz: i32, channels: i32) -> Result<Option<i32>, i32> {
    let z2 = match channels {
        4 => match zz {
            0 => RED,
            1 => GRE,
            2 => BLU,
            3 => ALP,
            _ => 4,
        },
        3 => match zz {
            0 => RED,
            1 => GRE,
            2 => BLU,
            _ => 3,
        },
        1 => zz,
        _ => return Err(BAD_CHANNELS),
    };
    Ok(if 0 <= z2 && z2 < channels { Some(z2) } else { None })
}

/// Validated raw entry shared by the median filters: checks everything, then calls `run`
/// with typed slices. `which` selects the (image, ref) depth pair.
///
/// # Safety
/// `in_image`/`out_image`: `height*width*channels` samples of `bits` bits (aligned);
/// `refp`: null or the same count of `ref_bits`-bit samples (aligned).
#[allow(clippy::too_many_arguments)]
pub(crate) unsafe fn dispatch(
    in_image: *const u8,
    out_image: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    refp: *const u8,
    ref_bits: i32,
    run8_8: &mut dyn FnMut(&[u8], &mut [u8], Option<&[u8]>),
    run16_8: &mut dyn FnMut(&[u16], &mut [u16], Option<&[u8]>),
    run16_16: &mut dyn FnMut(&[u16], &mut [u16], Option<&[u16]>),
    run8_16: &mut dyn FnMut(&[u8], &mut [u8], Option<&[u16]>),
) -> i32 {
    // pair as in the C++ if-chain
    let (isz, rsz) = if bits == 8 && (ref_bits == 8 || ref_bits == 0) {
        (1, 1)
    } else if bits == 16 && (ref_bits == 8 || ref_bits == 0) {
        (2, 1)
    } else if bits == 16 && ref_bits == 16 {
        (2, 2)
    } else if bits == 8 && ref_bits == 16 {
        (1, 2)
    } else {
        return BAD_BITS;
    };
    if height < 0 || width < 0 {
        return BAD_ARG;
    }
    if height == 0 || width == 0 {
        return OK; // nothing to filter (the C++ loops simply did not run)
    }
    if in_image.is_null() || out_image.is_null() {
        return BAD_ARG;
    }
    let Some(n) = (height as usize).checked_mul(width as usize).and_then(|v| v.checked_mul(channels as usize)) else {
        return BAD_ARG;
    };
    if n > isize::MAX as usize / 8 {
        return BAD_ARG;
    }
    let bytes = n * isz;
    let (a, b) = (in_image as usize, out_image as usize);
    // `in` is read while `out` is written: they must not overlap
    if a < b + bytes && b < a + bytes {
        return BAD_ARG;
    }
    if a % isz != 0 || b % isz != 0 || (!refp.is_null() && (refp as usize) % rsz != 0) {
        return BAD_ARG;
    }
    macro_rules! slices {
        ($i:ty, $r:ty, $f:ident) => {{
            let r = if refp.is_null() { None } else { Some(std::slice::from_raw_parts(refp as *const $r, n)) };
            $f(std::slice::from_raw_parts(in_image as *const $i, n), std::slice::from_raw_parts_mut(out_image as *mut $i, n), r);
        }};
    }
    match (isz, rsz) {
        (1, 1) => slices!(u8, u8, run8_8),
        (2, 1) => slices!(u16, u8, run16_8),
        (2, 2) => slices!(u16, u16, run16_16),
        _ => slices!(u8, u16, run8_16),
    }
    OK
}

pub fn out_of_image(out_side_type: i32) -> OutOfImage {
    match out_side_type {
        1 => OutOfImage::FlipRepeat,
        2 => OutOfImage::Black,
        3 => OutOfImage::Repeat,
        _ => OutOfImage::SpreadEdge, // 0 and anything unknown (the C++ default)
    }
}

/// igs::median_filter::convert, on raw pointers. Returns [`OK`] or a status code.
///
/// # Safety
/// See [`dispatch`].
#[allow(clippy::too_many_arguments)]
pub unsafe fn convert_raw(
    in_image: *const u8,
    out_image: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    refp: *const u8,
    ref_bits: i32,
    ref_mode: i32,
    zz: i32,
    radius: f64,
    out_side_type: i32,
) -> i32 {
    let z2 = match channel_for(zz, channels) {
        Ok(z) => z,
        Err(e) => return e,
    };
    let ty = out_of_image(out_side_type);
    // the circle is built once, like the C++ pixrender (empty circle -> bad argument)
    let Some(pixr) = PixRender::new(radius, ty) else {
        // C++: std::out_of_range from around.at(0); depth errors still win
        return if !(bits == 8 || bits == 16) || !(ref_bits == 0 || ref_bits == 8 || ref_bits == 16) { BAD_BITS } else { BAD_ARG };
    };
    let (h, w, c) = (height, width, channels);
    dispatch(
        in_image,
        out_image,
        height,
        width,
        channels,
        bits,
        refp,
        ref_bits,
        &mut |i, o, r| convert_template::<u8, u8>(i, o, h, w, c, r, ref_mode, z2, &pixr),
        &mut |i, o, r| convert_template::<u16, u8>(i, o, h, w, c, r, ref_mode, z2, &pixr),
        &mut |i, o, r| convert_template::<u16, u16>(i, o, h, w, c, r, ref_mode, z2, &pixr),
        &mut |i, o, r| convert_template::<u8, u16>(i, o, h, w, c, r, ref_mode, z2, &pixr),
    )
}

/// # Safety
/// See [`convert_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_median_filter_convert(
    in_image: *const u8,
    out_image: *mut u8,
    height: i32,
    width: i32,
    channels: i32,
    bits: i32,
    refp: *const u8,
    ref_bits: i32,
    ref_mode: i32,
    zz: i32,
    radius: f64,
    out_side_type: i32,
) -> i32 {
    // a panic must never unwind into C++: report invalid_argument (3)
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    convert_raw(in_image, out_image, height, width, channels, bits, refp, ref_bits, ref_mode, zz, radius, out_side_type)
})).unwrap_or(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(img: &[u8], h: i32, w: i32, ch: i32, zz: i32, radius: f64, ty: i32) -> Vec<u8> {
        let mut out = vec![0u8; img.len()];
        let r = unsafe { convert_raw(img.as_ptr(), out.as_mut_ptr(), h, w, ch, 8, std::ptr::null(), 0, 5, zz, radius, ty) };
        assert_eq!(r, OK);
        out
    }

    #[test]
    fn circle_offsets_follow_the_radius_with_1e6_slack() {
        // radius 1.7: x^2+y^2 <= 2.89 -> the 3x3 block only ((+-2,0) has 4 > 2.89)
        let p = PixRender::new(1.7, OutOfImage::SpreadEdge).unwrap();
        assert_eq!(p.xp.len(), 9);
        assert!(!p.xp.iter().zip(&p.yp).any(|(x, y)| x * x + y * y > 2));
        // radius 1.0 is the 5-pixel cross, scanned row by row
        let c = PixRender::new(1.0, OutOfImage::SpreadEdge).unwrap();
        assert_eq!(c.xp, vec![0, -1, 0, 1, 0]);
        assert_eq!(c.yp, vec![-1, 0, 0, 0, 1]);
        // radius 0 is a single pixel; radius <= -1 and NaN are empty
        assert_eq!(PixRender::new(0.0, OutOfImage::SpreadEdge).unwrap().xp.len(), 1);
        assert!(PixRender::new(-1.5, OutOfImage::SpreadEdge).is_none());
        assert!(PixRender::new(f64::NAN, OutOfImage::SpreadEdge).is_none());
    }

    #[test]
    fn removes_isolated_spike_in_gray() {
        let mut img = vec![10u8; 25];
        img[12] = 250; // centre of 5x5
        let out = run(&img, 5, 5, 1, 0, 1.0, 0);
        assert!(out.iter().all(|&v| v == 10));
    }

    #[test]
    fn edges_use_the_spread_pixel_and_never_invent_values() {
        // 1x2, radius-1 cross: pixel 0 sees {10,10,10,200,10} -> 10; pixel 1 sees {200,10,200,200,200} -> 200
        assert_eq!(run(&[10, 200], 1, 2, 1, 0, 1.0, 0), vec![10, 200]);
    }

    #[test]
    fn each_channel_mode_filters_channels_independently() {
        // 3x3 RGB (memory B,G,R): constant B=5, G=9, R=1 with a red spike in the centre
        let mut img = Vec::new();
        for i in 0..9 {
            img.extend_from_slice(&[5u8, 9, if i == 4 { 200 } else { 1 }]);
        }
        let each = run(&img, 3, 3, 3, 4, 1.0, 0);
        assert_eq!(&each[12..15], &[5, 9, 1]); // centre: red spike removed, others untouched
        // zz=0 (R) drives all three channels with the red median
        let all = run(&img, 3, 3, 3, 0, 1.0, 0);
        assert_eq!(&all[12..15], &[1, 1, 1]);
        assert_eq!(&all[0..3], &[1, 1, 1]);
    }

    #[test]
    fn black_outside_pulls_edges_down() {
        // uniform 100s; radius 1.7 = 3x3 block. Corner: 5 of 9 samples are outside (black)
        let img = vec![100u8; 9];
        let out = run(&img, 3, 3, 1, 0, 1.7, 2);
        assert_eq!(out[4], 100); // centre sees only inside pixels
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 100); // edge: 3 of 9 outside
        let spread = run(&img, 3, 3, 1, 0, 1.7, 0);
        assert!(spread.iter().all(|&v| v == 100));
    }

    #[test]
    fn position_modes() {
        let p = |t| PixRender::new(1.0, t).unwrap();
        let pos = |t, x, y| {
            let (mut a, mut b) = (x, y);
            p(t).position(4, 3, &mut a, &mut b);
            (a, b)
        };
        assert_eq!(pos(OutOfImage::SpreadEdge, -2, 5), (0, 2));
        assert_eq!(pos(OutOfImage::Repeat, -1, 3), (3, 0));
        assert_eq!(pos(OutOfImage::Black, -1, 1), (-1, 1));
        // flip: the C++ mirrors on every wrap (not only odd ones): -1 -> 3 -> 4-1-3 = 0
        assert_eq!(pos(OutOfImage::FlipRepeat, -1, 0), (0, 0));
        assert_eq!(pos(OutOfImage::FlipRepeat, 4, 0), (3, 0));
        assert_eq!(pos(OutOfImage::FlipRepeat, 5, 0), (2, 0));
    }

    #[test]
    fn reference_zero_keeps_original_and_one_applies_filter() {
        let mut img = vec![10u8; 9];
        img[4] = 250;
        let mut out = vec![0u8; 9];
        let zero_ref = vec![0u8; 9];
        let r = unsafe { convert_raw(img.as_ptr(), out.as_mut_ptr(), 3, 3, 1, 8, zero_ref.as_ptr(), 8, 0, 0, 1.0, 0) };
        assert_eq!(r, OK);
        assert_eq!(out[4], 250);
        let one_ref = vec![255u8; 9];
        unsafe { convert_raw(img.as_ptr(), out.as_mut_ptr(), 3, 3, 1, 8, one_ref.as_ptr(), 8, 0, 0, 1.0, 0) };
        assert_eq!(out[4], 10);
    }

    #[test]
    fn sixteen_bit_and_errors() {
        let img: Vec<u16> = vec![1000; 9];
        let mut out = vec![0u16; 9];
        let r = unsafe { convert_raw(img.as_ptr() as *const u8, out.as_mut_ptr() as *mut u8, 3, 3, 1, 16, std::ptr::null(), 0, 5, 0, 1.0, 0) };
        assert_eq!(r, OK);
        assert_eq!(out, img);
        let b = vec![0u8; 9];
        let mut o = vec![0u8; 9];
        unsafe {
            assert_eq!(convert_raw(b.as_ptr(), o.as_mut_ptr(), 3, 3, 2, 8, std::ptr::null(), 0, 5, 0, 1.0, 0), BAD_CHANNELS);
            assert_eq!(convert_raw(b.as_ptr(), o.as_mut_ptr(), 3, 3, 1, 32, std::ptr::null(), 0, 5, 0, 1.0, 0), BAD_BITS);
            assert_eq!(convert_raw(b.as_ptr(), o.as_mut_ptr(), 0, 3, 1, 8, std::ptr::null(), 0, 5, 0, 1.0, 0), OK);
            assert_eq!(convert_raw(b.as_ptr(), o.as_mut_ptr(), -1, 3, 1, 8, std::ptr::null(), 0, 5, 0, 1.0, 0), BAD_ARG);
            assert_eq!(convert_raw(std::ptr::null(), o.as_mut_ptr(), 3, 3, 1, 8, std::ptr::null(), 0, 5, 0, 1.0, 0), BAD_ARG);
            assert_eq!(convert_raw(b.as_ptr(), o.as_mut_ptr(), 3, 3, 1, 8, std::ptr::null(), 0, 5, 0, -3.0, 0), BAD_ARG);
            // in == out would alias
            assert_eq!(convert_raw(o.as_ptr(), o.as_mut_ptr(), 3, 3, 1, 8, std::ptr::null(), 0, 5, 0, 1.0, 0), BAD_ARG);
        }
    }
}
