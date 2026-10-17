//! Port of stdfx/igs_color_rgb_hsv.cpp: RGB <-> HSV (negative-value aware).

/// rgb_to_hsv: returns (hue 0..360, sat, val)
pub fn rgb_to_hsv(red: f64, gre: f64, blu: f64) -> (f64, f64, f64) {
    let maxi = if red < gre {
        if gre < blu {
            blu
        } else {
            gre
        }
    } else if red < blu {
        blu
    } else {
        red
    };
    let mini = if gre < red {
        if blu < gre {
            blu
        } else {
            gre
        }
    } else if blu < red {
        blu
    } else {
        red
    };

    let invert_value = mini.abs() > maxi.abs();

    let val = if invert_value { mini } else { maxi }; /* value */
    let (mut hue, sat);

    if maxi == mini {
        /* no difference between the RGB values (grey) */
        sat = 0.0;
        hue = 0.0;
    } else {
        sat = if invert_value { (mini - maxi) / mini } else { (maxi - mini) / maxi };

        let maxmin = maxi - mini;

        hue = 0.0;
        /* -1 .. 1 between magenta (M) and yellow (Y) */
        if red == maxi {
            hue = (gre - blu) / maxmin;
        }
        /* 1 .. 3 between yellow (Y) and cyan (C) */
        else if gre == maxi {
            hue = 2.0 + (blu - red) / maxmin;
        }
        /* 3 .. 5 between cyan (C) and magenta (M) */
        else if blu == maxi {
            hue = 4.0 + (red - gre) / maxmin;
        }
        /*
                M-R-Y-G-C-B-M
               -1 0 1 2 3 4 5
        */
        hue *= 60.0; /* -60 ... 300 */
        if invert_value {
            hue -= 180.0;
        }
        if hue < 0.0 {
            hue += 360.0;
        }
    }
    (hue, sat, val)
}

/// hsv_to_rgb.  `red`/`gre`/`blu` hold the caller's values on entry and are left
/// untouched when the hue sector is outside 0..=5 (the C++ switch has no default).
pub fn hsv_to_rgb(hue: f64, sat: f64, val: f64, red: &mut f64, gre: &mut f64, blu: &mut f64) {
    if 0.0 == sat {
        /* grey */
        *red = val;
        *gre = val;
        *blu = val;
    } else {
        let mut hh = hue;
        while 360.0 <= hh {
            hh -= 360.0;
        }
        hh /= 60.0; /* 0..359.999... --> 0..5.999... */

        let ii = hh.floor(); /* largest integer value */
        let ff = hh - ii; /* fractional part of hue */
        let pp = val * (1.0 - sat);
        let qq = val * (1.0 - (sat * ff));
        let tt = val * (1.0 - (sat * (1.0 - ff)));

        match ii as i32 {
            0 => {
                *red = val;
                *gre = tt;
                *blu = pp;
            }
            1 => {
                *red = qq;
                *gre = val;
                *blu = pp;
            }
            2 => {
                *red = pp;
                *gre = val;
                *blu = tt;
            }
            3 => {
                *red = pp;
                *gre = qq;
                *blu = val;
            }
            4 => {
                *red = tt;
                *gre = pp;
                *blu = val;
            }
            5 => {
                *red = val;
                *gre = pp;
                *blu = qq;
            }
            _ => {}
        }
    }
}

/// # Safety
/// `hue`, `sat`, `val` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_rgb_to_hsv(
    red: f64, gre: f64, blu: f64, hue: *mut f64, sat: *mut f64, val: *mut f64,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let (h, s, v) = rgb_to_hsv(red, gre, blu);
    *hue = h;
    *sat = s;
    *val = v;
}));
}

/// # Safety
/// `red`, `gre`, `blu` must be valid for reads and writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_hsv_to_rgb(
    hue: f64, sat: f64, val: f64, red: *mut f64, gre: *mut f64, blu: *mut f64,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    hsv_to_rgb(hue, sat, val, &mut *red, &mut *gre, &mut *blu);
}));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn primaries_and_grey() {
        assert_eq!(rgb_to_hsv(1.0, 0.0, 0.0), (0.0, 1.0, 1.0));
        assert_eq!(rgb_to_hsv(0.0, 1.0, 0.0), (120.0, 1.0, 1.0));
        assert_eq!(rgb_to_hsv(0.0, 0.0, 1.0), (240.0, 1.0, 1.0));
        assert_eq!(rgb_to_hsv(1.0, 0.0, 1.0).0, 300.0);
        assert_eq!(rgb_to_hsv(0.25, 0.25, 0.25), (0.0, 0.0, 0.25));
    }

    #[test]
    fn negative_values_invert_value() {
        // |min| > |max| -> val = min, sat = (min-max)/min, hue shifted by 180
        let (h, s, v) = rgb_to_hsv(-1.0, 0.0, 0.0);
        // maxi = 0, mini = -1 (green==blue==0 -> `gre == maxi` branch: 2 + (0-(-1))/1 = 3 -> 180 -> 0)
        assert_eq!((v, s), (-1.0, 1.0));
        assert_eq!(h, 0.0);
    }

    #[test]
    fn hsv_to_rgb_sectors() {
        let (mut r, mut g, mut b) = (9.0, 9.0, 9.0);
        hsv_to_rgb(0.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (1.0, 0.0, 0.0));
        hsv_to_rgb(60.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (1.0, 1.0, 0.0));
        hsv_to_rgb(120.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (0.0, 1.0, 0.0));
        hsv_to_rgb(180.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (0.0, 1.0, 1.0));
        hsv_to_rgb(240.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (0.0, 0.0, 1.0));
        hsv_to_rgb(300.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (1.0, 0.0, 1.0));
        // hue >= 360 wraps
        hsv_to_rgb(420.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (1.0, 1.0, 0.0));
        // grey
        hsv_to_rgb(77.0, 0.0, 0.3, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (0.3, 0.3, 0.3));
    }

    #[test]
    fn negative_hue_leaves_outputs_untouched() {
        // floor(-0.5) = -1: no case matches, the out values keep their previous content
        let (mut r, mut g, mut b) = (7.0, 8.0, 9.0);
        hsv_to_rgb(-30.0, 1.0, 1.0, &mut r, &mut g, &mut b);
        assert_eq!((r, g, b), (7.0, 8.0, 9.0));
    }

    #[test]
    fn roundtrip() {
        for &(r, g, b) in &[(0.2, 0.5, 0.9), (0.9, 0.1, 0.3), (0.3, 0.8, 0.2)] {
            let (h, s, v) = rgb_to_hsv(r, g, b);
            let (mut r2, mut g2, mut b2) = (0.0, 0.0, 0.0);
            hsv_to_rgb(h, s, v, &mut r2, &mut g2, &mut b2);
            assert!(near(r, r2) && near(g, g2) && near(b, b2));
        }
    }

    #[test]
    fn ffi() {
        let (mut h, mut s, mut v) = (0.0, 0.0, 0.0);
        unsafe { flare_igs_rgb_to_hsv(0.0, 0.0, 1.0, &mut h, &mut s, &mut v) };
        assert_eq!((h, s, v), (240.0, 1.0, 1.0));
        let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
        unsafe { flare_igs_hsv_to_rgb(h, s, v, &mut r, &mut g, &mut b) };
        assert_eq!((r, g, b), (0.0, 0.0, 1.0));
    }
}
