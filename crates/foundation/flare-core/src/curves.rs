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
/// allowed), but an `x` position may repeat (`BadCurve`); fewer than two points cannot
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
fn curve_lut(xs: &[f64], ys: &[f64]) -> [u8; 256] {    // The secant slope of every span, then each point's tangent: the end points take the
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
            let v = h00(t) * y0
                + h10(t) * h * m0
                + h01(t) * y1
                + h11(t) * h * m1;
            u(v)
        };
    }
    lut
}

/// The unit ramp rounded to a channel byte.
fn u(v: f64) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Cubic Hermite basis, second and third blend of importance to a slope.
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
