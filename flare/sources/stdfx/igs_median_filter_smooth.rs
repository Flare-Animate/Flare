//! Port of stdfx/igs_median_filter_smooth.cpp: median filter whose circle
//! pixels carry an influence weight (1 inside, falling to 0 at the rim), so
//! the result varies smoothly with the radius.
//!
//! Faithful to the C++ including its quirk: the loop's increment expression
//! reads the *next* element into `before_*`, so the interpolation branches
//! always reduce to "the first sorted pixel whose accumulated weight reaches
//! half of the total" (see `median_smooth`).

use crate::igs_levels::{ref_value, IntPx, RefPx};
use crate::igs_median_filter::{channel_for, dispatch, out_of_image, OutOfImage, BAD_ARG, BAD_BITS};

/// igs pixel_geometry_
pub struct PixelGeometry {
    pub xp: Vec<i32>,
    pub yp: Vec<i32>,
    pub in_out_ratio: Vec<f64>,
    pub ratio_total: f64,
    ty: OutOfImage,
}

impl PixelGeometry {
    /// `None` only for a non-finite or absurd radius (an empty circle is allowed: median is 0).
    pub fn new(radius: f64, ty: OutOfImage) -> Option<PixelGeometry> {
        let ri = radius.ceil();
        if !ri.is_finite() || ri > 4096.0 {
            return None;
        }
        let ri = ri as i32;
        let rxr = radius * radius + 1e-6;
        let mut g = PixelGeometry { xp: vec![], yp: vec![], in_out_ratio: vec![], ratio_total: 0.0, ty };
        for yy in -ri..=ri {
            let yxy = f64::from(yy) * f64::from(yy);
            for xx in -ri..=ri {
                let xxx_plus_yxy = f64::from(xx) * f64::from(xx) + yxy;
                if xxx_plus_yxy <= rxr {
                    g.xp.push(xx);
                    g.yp.push(yy);
                    let pixel_radius = xxx_plus_yxy.sqrt();
                    let mut ratio = radius - pixel_radius;
                    if 1.0 < ratio {
                        ratio = 1.0;
                    }
                    g.in_out_ratio.push(ratio);
                    g.ratio_total += ratio;
                }
            }
        }
        Some(g)
    }

    /// re_position (note: the flip mode mirrors only after an odd number of wraps,
    /// unlike `igs_median_filter`)
    pub fn re_position(&self, ww: i32, hh: i32, xx: &mut i32, yy: &mut i32) {
        let wrap = |v: &mut i32, n: i32| {
            if *v < 0 {
                let k = (-i64::from(*v) + i64::from(n) - 1) / i64::from(n);
                *v = (i64::from(*v) + k * i64::from(n)) as i32;
                if k % 2 == 1 {
                    *v = n - 1 - *v;
                }
            }
            if n <= *v {
                let k = *v / n;
                *v -= k * n;
                if k % 2 == 1 {
                    *v = n - 1 - *v;
                }
            }
        };
        match self.ty {
            OutOfImage::SpreadEdge => {
                *xx = if *xx < 0 { 0 } else if ww <= *xx { ww - 1 } else { *xx };
                *yy = if *yy < 0 { 0 } else if hh <= *yy { hh - 1 } else { *yy };
            }
            OutOfImage::FlipRepeat => {
                wrap(xx, ww);
                wrap(yy, hh);
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

/// get_pixel_value_(): sample after re-positioning, 0 for "black"
fn pixel_value<I: IntPx>(g: &PixelGeometry, image: &[I], hh: i32, ww: i32, ch: i32, mut xx: i32, mut yy: i32, zz: i32) -> f64 {
    g.re_position(ww, hh, &mut xx, &mut yy);
    if xx < 0 || yy < 0 {
        return 0.0;
    }
    f64::from(image[(ww as usize) * (ch as usize) * (yy as usize) + (ch as usize) * (xx as usize) + zz as usize].to_i32())
}

/// median_filter_smooth_()
#[allow(clippy::too_many_arguments)]
fn median_smooth<I: IntPx>(g: &PixelGeometry, pixels: &mut Vec<(f64, f64)>, image: &[I], hh: i32, ww: i32, ch: i32, xx: i32, yy: i32, zz: i32) -> f64 {
    /* (value, weight) pairs sorted by value; equal values keep insertion order (std::multimap) */
    pixels.clear();
    for ii in 0..g.xp.len() {
        pixels.push((pixel_value(g, image, hh, ww, ch, xx + g.xp[ii], yy + g.yp[ii], zz), g.in_out_ratio[ii]));
    }
    pixels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let len_median = g.ratio_total / 2.0;
    let mut accum = 0.0f64;
    let mut before_value = 0.0f64;
    let mut before_ratio = 0.0f64;
    let mut before_accum = 0.0f64;
    let n = pixels.len();
    for i in 0..n {
        let (v, r) = pixels[i];
        let pixw = r / 2.0 + before_ratio / 2.0;
        accum += pixw;
        if len_median <= accum {
            if i == 0 {
                return v;
            }
            if before_value < v {
                return (v - before_value) * (len_median - before_accum) / pixw + before_value;
            }
            return (before_value - v) * (accum - len_median) / pixw + v;
        }
        /* the C++ for-increment: `++it, before_value = (*it).first, ...` reads the next element
        (for the last one it read past end(); the last element is used instead) */
        let (nv, nr) = if i + 1 < n { pixels[i + 1] } else { (v, r) };
        before_value = nv;
        before_ratio = nr;
        before_accum = accum;
    }
    before_value
}

/// refchk_(double, double, double)
fn refchk(src: f64, tgt: f64, refv: f64) -> f64 {
    (if src < tgt { (tgt - src) * refv + src } else { (src - tgt) * (1.0 - refv) + tgt }) + 0.999999
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
    g: &PixelGeometry,
) {
    let c = ch as usize;
    let mut buf: Vec<(f64, f64)> = Vec::with_capacity(g.xp.len());
    let mut p = 0usize;
    for yy in 0..hh {
        for xx in 0..ww {
            let ip = &inp[p * c..(p + 1) * c];
            let mut refv = 1.0f64;
            if let Some(r) = refimg {
                refv *= ref_value(&r[p * c..(p + 1) * c], c, R::MAX, ref_mode);
            }
            if let Some(z) = zz {
                let v1 = median_smooth(g, &mut buf, inp, hh, ww, ch, xx, yy, z);
                let v2 = I::from_f64(refchk(f64::from(ip[z as usize].to_i32()), v1, refv));
                for k in 0..c {
                    out[p * c + k] = v2;
                }
            } else {
                for z in 0..ch {
                    let v1 = median_smooth(g, &mut buf, inp, hh, ww, ch, xx, yy, z);
                    out[p * c + z as usize] = I::from_f64(refchk(f64::from(ip[z as usize].to_i32()), v1, refv));
                }
            }
            p += 1;
        }
    }
}

/// igs::median_filter_smooth::convert, on raw pointers. Returns [`OK`] or a status code.
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
    let Some(g) = PixelGeometry::new(radius, out_of_image(out_side_type)) else {
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
        &mut |i, o, r| convert_template::<u8, u8>(i, o, h, w, c, r, ref_mode, z2, &g),
        &mut |i, o, r| convert_template::<u16, u8>(i, o, h, w, c, r, ref_mode, z2, &g),
        &mut |i, o, r| convert_template::<u16, u16>(i, o, h, w, c, r, ref_mode, z2, &g),
        &mut |i, o, r| convert_template::<u8, u16>(i, o, h, w, c, r, ref_mode, z2, &g),
    )
}

/// # Safety
/// See [`convert_raw`].
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn flare_igs_median_filter_smooth_convert(
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
    convert_raw(in_image, out_image, height, width, channels, bits, refp, ref_bits, ref_mode, zz, radius, out_side_type)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::igs_median_filter::OK;

    fn run(img: &[u8], h: i32, w: i32, ch: i32, zz: i32, radius: f64, ty: i32) -> Vec<u8> {
        let mut out = vec![0u8; img.len()];
        let r = unsafe { convert_raw(img.as_ptr(), out.as_mut_ptr(), h, w, ch, 8, std::ptr::null(), 0, 5, zz, radius, ty) };
        assert_eq!(r, OK);
        out
    }

    #[test]
    fn geometry_weights_fall_to_the_rim() {
        let g = PixelGeometry::new(1.5, OutOfImage::SpreadEdge).unwrap();
        // |offset|^2 <= 2.25+1e-6 -> 3x3 block
        assert_eq!(g.xp.len(), 9);
        let centre = g.xp.iter().zip(&g.yp).position(|(x, y)| *x == 0 && *y == 0).unwrap();
        assert_eq!(g.in_out_ratio[centre], 1.0); // 1.5 - 0 clamped to 1
        let corner = g.xp.iter().zip(&g.yp).position(|(x, y)| *x == 1 && *y == 1).unwrap();
        assert!((g.in_out_ratio[corner] - (1.5 - 2f64.sqrt())).abs() < 1e-12);
        let total: f64 = g.in_out_ratio.iter().sum();
        assert_eq!(g.ratio_total, total);
        // radius 0: one pixel with weight 0
        let z = PixelGeometry::new(0.0, OutOfImage::SpreadEdge).unwrap();
        assert_eq!((z.xp.len(), z.ratio_total), (1, 0.0));
        // empty circle is allowed (median 0), NaN is not
        assert_eq!(PixelGeometry::new(-2.0, OutOfImage::SpreadEdge).unwrap().xp.len(), 0);
        assert!(PixelGeometry::new(f64::NAN, OutOfImage::SpreadEdge).is_none());
    }

    #[test]
    fn median_is_the_weighted_half_point_of_the_sorted_pixels() {
        let g = PixelGeometry::new(1.0, OutOfImage::SpreadEdge).unwrap();
        // cross: weights 1 for the centre (1-0 clamped) and 0 for the four neighbours (1-1)
        assert_eq!(g.in_out_ratio, vec![0.0, 0.0, 1.0, 0.0, 0.0]);
        let img = [10u8, 20, 30, 40, 50, 60, 70, 80, 90];
        let mut buf = vec![];
        // total 1 -> half 0.5 ; centre pixel (1,1)=50 : sorted values 20,40,50,60,80 weights 0,0,1,0,0
        let m = median_smooth(&g, &mut buf, &img, 3, 3, 1, 1, 1, 0);
        // accum: 20->0 , 40->0 , 50-> 0+0+1 (pixw = r/2 + prev/2 = .5+.5=1) -> 1 >= .5 -> 50 (quirk: no interpolation)
        assert_eq!(m, 50.0);
    }

    #[test]
    fn flat_regions_stay_flat_and_spike_is_suppressed() {
        let mut img = vec![40u8; 49];
        img[24] = 250;
        let out = run(&img, 7, 7, 1, 0, 2.5, 0);
        assert!(out.iter().all(|&v| (i32::from(v) - 40).abs() <= 1), "{out:?}");
    }

    #[test]
    fn re_position_flip_mirrors_only_after_odd_wraps() {
        let g = PixelGeometry::new(1.0, OutOfImage::FlipRepeat).unwrap();
        let pos = |x: i32| {
            let (mut a, mut b) = (x, 0);
            g.re_position(4, 3, &mut a, &mut b);
            a
        };
        assert_eq!(pos(-1), 0); // one wrap (odd): 3 -> mirrored 0
        assert_eq!(pos(-5), 3); // two wraps: -5+8=3, even -> not mirrored
        assert_eq!(pos(4), 3); // one wrap: 0 -> mirrored 3
        assert_eq!(pos(9), 1); // two wraps: 9-8 = 1, even -> not mirrored
    }

    #[test]
    fn each_channel_and_reference_and_errors() {
        let mut img = Vec::new();
        for i in 0..25 {
            img.extend_from_slice(&[5u8, 9, if i == 12 { 200 } else { 1 }]);
        }
        let each = run(&img, 5, 5, 3, 4, 1.5, 0);
        assert_eq!(&each[36..39], &[5, 9, 1]);
        // reference 0 keeps the source
        let zero = vec![0u8; 25];
        let mut gray = vec![10u8; 25];
        gray[12] = 250;
        let mut out = vec![0u8; 25];
        let r = unsafe { convert_raw(gray.as_ptr(), out.as_mut_ptr(), 5, 5, 1, 8, zero.as_ptr(), 8, 0, 0, 1.5, 0) };
        assert_eq!(r, OK);
        assert_eq!(out[12], 250);
        unsafe {
            assert_eq!(convert_raw(gray.as_ptr(), out.as_mut_ptr(), 5, 5, 2, 8, std::ptr::null(), 0, 5, 0, 1.0, 0), 1);
            assert_eq!(convert_raw(gray.as_ptr(), out.as_mut_ptr(), 5, 5, 1, 32, std::ptr::null(), 0, 5, 0, 1.0, 0), BAD_BITS);
            assert_eq!(convert_raw(gray.as_ptr(), out.as_mut_ptr(), 5, 5, 1, 8, std::ptr::null(), 0, 5, 0, f64::NAN, 0), BAD_ARG);
        }
    }
}
