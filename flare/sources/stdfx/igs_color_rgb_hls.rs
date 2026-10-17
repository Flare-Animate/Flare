//! Port of stdfx/igs_color_rgb_hls.cpp: RGB <-> HLS (cylinder or bicone colour space).

/// rgb_to_hls: returns (hue 0..360, lig 0..1, sat 0..1)
pub fn rgb_to_hls(red: f64, gre: f64, blu: f64, cylindrical: bool) -> (f64, f64, f64) {
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

    let lig = (maxi + mini) / 2.0;
    let (mut hue, sat);

    if maxi == mini {
        /* no difference between the RGB values (grey) */
        sat = 0.0; /* saturation is zero */
        hue = 0.0; /* hue is meaningless */
    } else {
        /* coloured */
        if cylindrical {
            if lig <= 0.5 {
                sat = (maxi - mini) / (maxi + mini);
            } else {
                sat = (maxi - mini) / (2.0 - (maxi + mini));
            }
        } else {
            sat = maxi - mini;
        }

        /* Hue */
        let rmid = (maxi - red) / (maxi - mini);
        let gmid = (maxi - gre) / (maxi - mini);
        let bmid = (maxi - blu) / (maxi - mini);

        hue = 0.0;
        /* -1 .. 1 between magenta (M) and yellow (Y) */
        if red == maxi {
            hue = bmid - gmid;
        }
        /* 1 .. 3 between yellow (Y) and cyan (C) */
        else if gre == maxi {
            hue = 2.0 + rmid - bmid;
        }
        /* 3 .. 5 between cyan (C) and magenta (M) */
        else if blu == maxi {
            hue = 4.0 + gmid - rmid;
        }
        /*
                M-R-Y-G-C-B-M
               -1 0 1 2 3 4 5
        */
        hue *= 60.0; /* -60 ... 300 */
        if hue < 0.0 {
            hue += 360.0;
        }
    }
    (hue, lig, sat)
}

fn hls2rgb_calc(m1: f64, m2: f64, hue: f64) -> f64 {
    let mut hh = hue;
    while 360.0 < hh {
        hh -= 360.0;
    }
    while hh < 0.0 {
        hh += 360.0;
    }
    if hh < 60.0 {
        return m1 + (m2 - m1) * hh / 60.0;
    } else if hh < 180.0 {
        return m2;
    } else if hh < 240.0 {
        return m1 + (m2 - m1) * (240.0 - hh) / 60.0;
    }
    m1 /* 240 ... 360 */
}

/// hls_to_rgb: returns (red, gre, blu)
pub fn hls_to_rgb(hue: f64, lig: f64, sat: f64, cylindrical: bool) -> (f64, f64, f64) {
    if 0.0 == sat {
        /* grey */
        (lig, lig, lig)
    } else {
        let (m2, m1);
        if cylindrical {
            m2 = if lig <= 0.5 { lig * (1.0 + sat) } else { lig + sat - lig * sat };
            m1 = 2.0 * lig - m2;
        } else {
            m2 = lig + sat * 0.5;
            m1 = lig - sat * 0.5;
        }
        (
            hls2rgb_calc(m1, m2, hue + 120.0),
            hls2rgb_calc(m1, m2, hue),
            hls2rgb_calc(m1, m2, hue - 120.0),
        )
    }
}

/// # Safety
/// `hue`, `lig`, `sat` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_rgb_to_hls(
    red: f64, gre: f64, blu: f64, hue: *mut f64, lig: *mut f64, sat: *mut f64, cylindrical: bool,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let (h, l, s) = rgb_to_hls(red, gre, blu, cylindrical);
    *hue = h;
    *lig = l;
    *sat = s;
}));
}

/// # Safety
/// `red`, `gre`, `blu` must be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn flare_igs_hls_to_rgb(
    hue: f64, lig: f64, sat: f64, red: *mut f64, gre: *mut f64, blu: *mut f64, cylindrical: bool,
) {
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
    let (r, g, b) = hls_to_rgb(hue, lig, sat, cylindrical);
    *red = r;
    *gre = g;
    *blu = b;
}));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-12
    }

    #[test]
    fn grey_has_no_hue_or_saturation() {
        assert_eq!(rgb_to_hls(0.4, 0.4, 0.4, true), (0.0, 0.4, 0.0));
        assert_eq!(hls_to_rgb(123.0, 0.4, 0.0, true), (0.4, 0.4, 0.4));
    }

    #[test]
    fn primaries() {
        assert_eq!(rgb_to_hls(1.0, 0.0, 0.0, true), (0.0, 0.5, 1.0));
        // green: hue = 2 + rmid - bmid = 2 + 1 - 1 = 2 -> 120
        assert_eq!(rgb_to_hls(0.0, 1.0, 0.0, true), (120.0, 0.5, 1.0));
        // blue: hue = 4 + gmid - rmid = 4 -> 240
        assert_eq!(rgb_to_hls(0.0, 0.0, 1.0, true), (240.0, 0.5, 1.0));
        // magenta: red == max: hue = bmid - gmid = 0 - 1 = -1 -> -60 -> 300
        assert_eq!(rgb_to_hls(1.0, 0.0, 1.0, true).0, 300.0);
    }

    #[test]
    fn bicone_saturation_is_chroma() {
        let (_, l, s) = rgb_to_hls(1.0, 0.5, 0.5, false);
        assert!(near(l, 0.75) && near(s, 0.5));
        let (_, _, s) = rgb_to_hls(1.0, 0.5, 0.5, true);
        assert!(near(s, 1.0)); // (1-.5)/(2-1.5)
    }

    #[test]
    fn roundtrip() {
        for &(r, g, b) in &[(0.2, 0.5, 0.9), (0.9, 0.1, 0.3), (0.3, 0.8, 0.2), (1.0, 1.0, 0.0)] {
            for cyl in [true, false] {
                let (h, l, s) = rgb_to_hls(r, g, b, cyl);
                let (r2, g2, b2) = hls_to_rgb(h, l, s, cyl);
                assert!(near(r, r2) && near(g, g2) && near(b, b2), "{:?} {}", (r, g, b), cyl);
            }
        }
    }

    #[test]
    fn hue_wraps_in_hls2rgb() {
        // hue 360 is not wrapped (`360 < hh`), hue 361 is
        let a = hls_to_rgb(0.0, 0.5, 1.0, true);
        let b = hls_to_rgb(360.0, 0.5, 1.0, true);
        let c = hls_to_rgb(-360.0, 0.5, 1.0, true);
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_eq!(a, (1.0, 0.0, 0.0));
    }

    #[test]
    fn ffi() {
        let (mut h, mut l, mut s) = (0.0, 0.0, 0.0);
        unsafe { flare_igs_rgb_to_hls(0.0, 1.0, 0.0, &mut h, &mut l, &mut s, true) };
        assert_eq!((h, l, s), (120.0, 0.5, 1.0));
        let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
        unsafe { flare_igs_hls_to_rgb(h, l, s, &mut r, &mut g, &mut b, true) };
        assert!(near(r, 0.0) && near(g, 1.0) && near(b, 0.0));
    }
}
