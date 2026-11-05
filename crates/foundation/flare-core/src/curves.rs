//! Tone curves: a control curve drawn by the user (contrast, lift, any monotone shape)
//! applied to the RGB of an RGBA8 buffer. Buffers are row-major, 4 bytes/pixel, like
//! [`crate::image_ops`].

/// A control curve that cannot be used: its x positions must strictly increase.
#[derive(Debug, PartialEq, Eq)]
pub struct BadCurve;

/// Applies the tone curve to every pixel's RGB (the alpha is untouched): the curve is
/// sampled once into a 256-entry look-up through a monotone cubic Hermite through the
/// control points, clamping before the first and after the last. With two points the
/// curve is exactly linear, and with more it runs through every point of them.
///
/// `points` are (x, y) pairs in 0..=255; `y` may go any way (an inverted curve is a
/// allowed), but an `x` position may not repeat (`BadCurve`); fewer than two points cannot
/// bend, so that is rejected too. Positions before the first and after the last point
/// stay at its `y`.
pub fn tone_curve(buf: &mut [u8], points: &[(u8, u8)]) -> Result<(), BadCurve> {
    if points.len() < 2 {
        return Err(BadCurve);
    }
    let x = |p: (u8, u8)| f64::from(p.0) / 255.0;
    let y = |p: (u8, u8)| f64::from(p.1) / 255.0;
    for pair in points.windows(2) {
        if x(pair[0]) >= x(pair[1]) {
            return Err(BadCurve);
        }
    }
    let xs: Vec<f64> = points.iter().map(|p| x(*p)).collect();
    let ys: Vec<f64> = points.iter().map(|p| y(*p)).collect();
    let lut = curve_lut(&xs, &ys);
    for px in buf.as_chunks_mut::<4>().0 {
        for c in &mut px[..3] {
            *c = lut[*c as usize];
        }
    }
    Ok(())
}

/// The monotone cubic Hermite (Fritsch and Carlson's tangents) through the points,
/// sampled at 0..=255. Outside the ends the curve holds the end `y` flat.
fn curve_lut(xs: &[f64], ys: &[f64]) -> [u8; 256] {
    // The secant slope of every span, then each point's tangent: the end points take the
    // span they sit on, and a point between two spans of the same sign takes their
    // harmonic mean (so that the cubic stays monotone inside each span); a point between
    // opposite slopes is a local extreme and goes flat.
    let spans: Vec<f64> = (0..xs.len() - 1)
        .map(|i| (ys[i + 1] - ys[i]) / (xs[i + 1] - xs[i]))
        .collect();
    let mut tangents = vec![0.0; xs.len()];
    tangents[0] = spans[0];
    tangents[xs.len() - 1] = spans[spans.len() - 1];
    for i in 1..xs.len() - 1 {
        let (before, after) = (spans[i - 1], spans[i]);
        tangents[i] = if before * after > 0.0 {
            2.0 / (1.0 / before + 1.0 / after)
        } else {
            0.0
        };
    }
    let mut lut = [0u8; 256];
    let mut span = 0usize;
    for (i, out) in lut.iter_mut().enumerate() {
        let at = f64::from(i as u32) / 255.0;
        while span + 2 < xs.len() && xs[span + 1] <= at {
            span += 1;
        }
        *out = if at <= xs[0] {
            u(ys[0])
        } else if at >= xs[xs.len() - 1] {
            u(ys[ys.len() - 1])
        } else {
            let (x0, y0, x1, y1) = (xs[span], ys[span], xs[span + 1], ys[span + 1]);
            let h = x1 - x0;
            let t = (at - x0) / h;
            let m0 = tangents[span];
            let m1 = tangents[span + 1];
            // Hermite basis: h fixes the ends, h10/h11 fix the slope at them.
            let v = h00(t) * y0 + h10(t) * h * m0 + h01(t) * y1 + h11(t) * h * m1;
            u(v)
        };
    }
    lut
}

/// The unit ramp rounded to a channel byte.
fn u(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Cubic Hermite basis functions: the first blends the two ends, the second and third
/// carry the slopes at the ends.
fn h00(t: f64) -> f64 {
    2.0 * t * t * t - 3.0 * t * t + 1.0
}
fn h10(t: f64) -> f64 {
    t * t * t - 2.0 * t * t + t
}
fn h01(t: f64) -> f64 {
    -2.0 * t * t * t + 3.0 * t * t
}
fn h11(t: f64) -> f64 {
    t * t * t - t * t
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    /// The curve applied to a ramp of source values, as seen through the red channel:
    /// the LUT the curve produced, exactly what thirty-two-bit pain avoids.
    fn lut_through(points: &[(u8, u8)]) -> Result<Vec<u8>, BadCurve> {
        let mut buf: Vec<u8> = (0u8..=255).flat_map(|c| [c, c, c, 255]).collect();
        tone_curve(&mut buf, points)?;
        Ok(buf.as_chunks::<4>().0.iter().map(|p| p[0]).collect())
    }

    #[test]
    fn two_points_are_a_straight_line() {
        let lut = lut_through(&[(0, 0), (255, 255)]).unwrap();
        for (i, &v) in lut.iter().enumerate() {
            assert_eq!(v, i as u8, "identical curve leaves {i} alone");
        }
        // A dimmer line, still straight: every output is the exact 40% rounded.
        let lut = lut_through(&[(0, 0), (255, 102)]).unwrap();
        for (i, &v) in lut.iter().enumerate() {
            assert_eq!(v, ((i * 102 + 127) / 255) as u8, "at {i}");
        }
    }

    #[test]
    fn the_curve_runs_through_its_points_and_bends_between_them() {
        // An S-curve: shadows and highlights pulled, the middle held.
        let lut = lut_through(&[(0, 0), (128, 96), (255, 255)]).unwrap();
        assert_eq!(lut[0], 0);
        assert_eq!(lut[128], 96, "runs through its points");
        assert_eq!(lut[255], 255);
        // Hand-computed Hermite between the points (t = 0.5 in each span):
        // first span bends down to 45, second up to 171, so the curve is an S.
        assert_eq!(lut[64], 45);
        assert_eq!(lut[192], 171);
        assert!(monotone(&lut), "a monotone curve stays monotone");
        // The tangents keep the cubic inside the span's own range: nothing between the
        // points overshoots below the lower point or above the upper one.
        for at in [32usize, 96, 160, 224] {
            let expected_low = if at < 128 { 0 } else { 96 };
            let expected_high = if at < 128 { 96 } else { 255 };
            assert!(
                lut[at] >= expected_low && lut[at] <= expected_high,
                "out of the span range at {at}: {}",
                lut[at]
            );
        }
    }

    fn monotone(lut: &[u8]) -> bool {
        lut.windows(2).all(|pair| pair[0] <= pair[1])
    }

    #[test]
    fn outside_the_ends_the_curve_holds_flat() {
        let lut = lut_through(&[(64, 200), (192, 200)]).unwrap();
        for (i, v) in lut.iter().enumerate() {
            assert_eq!(*v, 200, "flat everywhere, at {i}");
        }
    }

    #[test]
    fn a_broken_curve_is_refused() {
        assert_eq!(tone_curve(&mut [0; 8], &[]), Err(BadCurve), "empty");
        assert_eq!(
            tone_curve(&mut [0; 8], &[(5, 5)]),
            Err(BadCurve),
            "one point"
        );
        assert_eq!(
            tone_curve(&mut [0; 8], &[(5, 5), (5, 9)]),
            Err(BadCurve),
            "a repeated x"
        );
        assert_eq!(
            tone_curve(&mut [0; 8], &[(200, 5), (100, 9)]),
            Err(BadCurve),
            "backwards"
        );
    }

    #[test]
    fn only_the_colour_channels_change() {
        let mut b = [0u8; 4];
        b[..3].fill(0);
        b[3] = 7;
        tone_curve(&mut b, &[(0, 20), (255, 20)]).unwrap();
        assert_eq!(b[..3], [20, 20, 20]);
        assert_eq!(b[3], 7, "the alpha is untouched");
    }
}
