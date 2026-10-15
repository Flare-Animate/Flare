//! Port of stdfx/hsvutil.cpp: the "old" double-precision RGB <-> HSV helpers
//! (hue in degrees 0..360, sat/value 0..1) used by hsvkeyfx, hsvscalefx and
//! the particles code. Scalar math, so the C ABI is per call (like tmathutil).
//!
//! Behaviour kept from the C++ on purpose: `OLDHSV2RGB` clamps sat/value, wraps hue
//! with integer-truncating arithmetic, and leaves the outputs untouched when the
//! sextant index falls outside 0..=5.
//! Deviation (C++ read an uninitialised `*h`): `OLDRGB2HSV` with NaN input yields hue 0.

/// OLDRGB2HSV: returns (h, s, v).
pub fn old_rgb2hsv(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    // std::max({r,g,b}) picks by `<`; with non-NaN input both agree
    let v = max;
    let s = if max != 0.0 { (max - min) / max } else { 0.0 };
    let mut h = 0.0;
    if s != 0.0 {
        let delta = max - min;
        if r == max {
            h = (g - b) / delta;
        } else if g == max {
            h = 2.0 + (b - r) / delta;
        } else if b == max {
            h = 4.0 + (r - g) / delta;
        }
        h *= 60.0;
        if h < 0.0 {
            h += 360.0;
        }
    }
    (h, s, v)
}

/// OLDHSV2RGB: returns Some((r, g, b)), or None when the C++ would have left the
/// outputs unwritten (sextant index outside 0..=5).
pub fn old_hsv2rgb(mut hue: f64, mut sat: f64, mut value: f64) -> Option<(f64, f64, f64)> {
    if hue > 360.0 {
        hue -= (((hue as i32) / 360).wrapping_mul(360)) as f64;
    }
    if hue < 0.0 {
        hue += (1i32.wrapping_sub((hue as i32) / 360).wrapping_mul(360)) as f64;
    }
    if sat < 0.0 {
        sat = 0.0;
    }
    if sat > 1.0 {
        sat = 1.0;
    }
    if value < 0.0 {
        value = 0.0;
    }
    if value > 1.0 {
        value = 1.0;
    }
    if sat == 0.0 {
        return Some((value, value, value));
    }
    if hue == 360.0 {
        hue = 0.0;
    }
    hue /= 60.0;
    let i = hue as i32;
    let f = hue - i as f64;
    let p = value * (1.0 - sat);
    let q = value * (1.0 - (sat * f));
    let t = value * (1.0 - (sat * (1.0 - f)));
    match i {
        0 => Some((value, t, p)),
        1 => Some((q, value, p)),
        2 => Some((p, value, t)),
        3 => Some((p, q, value)),
        4 => Some((t, p, value)),
        5 => Some((value, p, q)),
        _ => None,
    }
}

/// # Safety
/// Each output pointer must be null (skipped) or valid for a write of one `f64`.
#[no_mangle]
pub unsafe extern "C" fn flare_old_rgb2hsv(r: f64, g: f64, b: f64, h: *mut f64, s: *mut f64, v: *mut f64) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (hh, ss, vv) = old_rgb2hsv(r, g, b);
        if !h.is_null() {
            *h = hh;
        }
        if !s.is_null() {
            *s = ss;
        }
        if !v.is_null() {
            *v = vv;
        }
    }));
}

/// # Safety
/// Each output pointer must be null (skipped) or valid for a write of one `f64`.
#[no_mangle]
pub unsafe extern "C" fn flare_old_hsv2rgb(hue: f64, sat: f64, value: f64, red: *mut f64, green: *mut f64, blue: *mut f64) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some((r, g, b)) = old_hsv2rgb(hue, sat, value) {
            if !red.is_null() {
                *red = r;
            }
            if !green.is_null() {
                *green = g;
            }
            if !blue.is_null() {
                *blue = b;
            }
        }
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn primaries_and_grays() {
        assert_eq!(old_rgb2hsv(1.0, 0.0, 0.0), (0.0, 1.0, 1.0));
        assert_eq!(old_rgb2hsv(0.0, 1.0, 0.0), (120.0, 1.0, 1.0));
        assert_eq!(old_rgb2hsv(0.0, 0.0, 1.0), (240.0, 1.0, 1.0));
        assert_eq!(old_rgb2hsv(0.5, 0.5, 0.5), (0.0, 0.0, 0.5));
        assert_eq!(old_rgb2hsv(0.0, 0.0, 0.0), (0.0, 0.0, 0.0));
        // negative hue wraps to 300 (magenta)
        assert_eq!(old_rgb2hsv(1.0, 0.0, 1.0).0, 300.0);
    }

    #[test]
    fn hsv2rgb_sextants() {
        assert_eq!(old_hsv2rgb(0.0, 1.0, 1.0), Some((1.0, 0.0, 0.0)));
        assert_eq!(old_hsv2rgb(120.0, 1.0, 1.0), Some((0.0, 1.0, 0.0)));
        assert_eq!(old_hsv2rgb(240.0, 1.0, 1.0), Some((0.0, 0.0, 1.0)));
        assert_eq!(old_hsv2rgb(300.0, 1.0, 1.0), Some((1.0, 0.0, 1.0)));
        assert_eq!(old_hsv2rgb(77.0, 0.0, 0.25), Some((0.25, 0.25, 0.25)));
    }

    #[test]
    fn hue_wrap_and_clamp() {
        assert_eq!(old_hsv2rgb(360.0, 1.0, 1.0), old_hsv2rgb(0.0, 1.0, 1.0));
        assert_eq!(old_hsv2rgb(480.0, 1.0, 1.0), old_hsv2rgb(120.0, 1.0, 1.0));
        assert_eq!(old_hsv2rgb(-120.0, 1.0, 1.0), old_hsv2rgb(240.0, 1.0, 1.0));
        assert_eq!(old_hsv2rgb(-360.0, 1.0, 1.0), old_hsv2rgb(0.0, 1.0, 1.0));
        assert_eq!(old_hsv2rgb(10.0, 5.0, 7.0), old_hsv2rgb(10.0, 1.0, 1.0));
        assert_eq!(old_hsv2rgb(10.0, -5.0, -7.0), Some((0.0, 0.0, 0.0)));
    }

    #[test]
    fn roundtrip() {
        let mut x = 12345u32;
        let mut rnd = || {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            (x >> 8) as f64 / (1u32 << 24) as f64
        };
        for _ in 0..2000 {
            let (r, g, b) = (rnd(), rnd(), rnd());
            let (h, s, v) = old_rgb2hsv(r, g, b);
            let (r2, g2, b2) = old_hsv2rgb(h, s, v).unwrap();
            assert!((r - r2).abs() < 1e-12 && (g - g2).abs() < 1e-12 && (b - b2).abs() < 1e-12);
        }
    }

    #[test]
    fn nan_and_null_safe() {
        assert_eq!(old_hsv2rgb(f64::NAN, 1.0, 1.0).map(|c| c.0), Some(1.0));
        let mut r = 7.0;
        unsafe {
            flare_old_hsv2rgb(0.0, 1.0, 1.0, &mut r, std::ptr::null_mut(), std::ptr::null_mut());
            flare_old_rgb2hsv(1.0, 0.0, 0.0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
        }
        assert_eq!(r, 1.0);
        let (mut h, mut s, mut v) = (9.0, 9.0, 9.0);
        unsafe { flare_old_rgb2hsv(f64::NAN, 0.0, 0.0, &mut h, &mut s, &mut v) };
        assert_eq!(h, 0.0);
    }
}
