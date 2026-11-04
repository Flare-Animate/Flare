//! Cubic bezier curves in time and value: the shape of a segment that has handles.

/// Halving the search interval 64 times gets below the resolution of an f64 fraction, so the
/// search always ends, whatever the curve.
const BISECTION_STEPS: u32 = 64;

/// The value at time `x` of the cubic whose time coordinates are `xs` and value coordinates `ys`.
///
/// `xs[0]` must be 0, `xs[3]` must be 1, and the inner time coordinates must be in 0..=1. Then
/// time increases along the curve, so there is exactly one parameter for each `x`, and bisection
/// finds it. `x` outside 0..=1 is clamped.
pub(crate) fn cubic_at(xs: [f64; 4], ys: [f64; 4], x: f64) -> f64 {
    let x = x.clamp(xs[0], xs[3]);
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for _ in 0..BISECTION_STEPS {
        let mid = 0.5 * (lo + hi);
        if bernstein(xs, mid) < x {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    bernstein(ys, 0.5 * (lo + hi))
}

fn bernstein(c: [f64; 4], s: f64) -> f64 {
    let u = 1.0 - s;
    u * u * u * c[0] + 3.0 * u * u * s * c[1] + 3.0 * u * s * s * c[2] + s * s * s * c[3]
}
