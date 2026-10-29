//! Cubic Beziers to the quadratic segments XFL edges can express.
//!
//! XFL edges only support lines and quadratic curves, so a cubic is split into equal
//! parameter pieces and each piece is replaced by the quadratic with the same endpoints.
//! The piece count comes from a published bound on the error of that approximation, so the
//! result stays within the requested tolerance instead of using a fixed guess.

pub type Point = (f64, f64);

/// Most pieces one cubic is split into, however tight the tolerance.
pub const MAX_PIECES: usize = 64;

fn lerp(a: Point, b: Point, t: f64) -> Point {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// The part of the cubic between parameters `t0` and `t1` as its own cubic.
fn sub_cubic(c: [Point; 4], t0: f64, t1: f64) -> [Point; 4] {
    // de Casteljau: split at t1, then split the left part at t0 / t1.
    let split = |c: [Point; 4], t: f64| -> ([Point; 4], [Point; 4]) {
        let (ab, bc, cd) = (
            lerp(c[0], c[1], t),
            lerp(c[1], c[2], t),
            lerp(c[2], c[3], t),
        );
        let (abc, bcd) = (lerp(ab, bc, t), lerp(bc, cd, t));
        let mid = lerp(abc, bcd, t);
        ([c[0], ab, abc, mid], [mid, bcd, cd, c[3]])
    };
    let (left, _) = split(c, t1);
    if t1 <= 0.0 {
        return left;
    }
    let (_, piece) = split(left, t0 / t1);
    piece
}

/// How many quadratic pieces keep the error under `tolerance` (same units as the points).
///
/// For a cubic with third difference `d = |p3 - 3 p2 + 3 p1 - p0|`, replacing it with one
/// quadratic over a piece that spans `1/n` of the parameter range errs by at most
/// `sqrt(3) / 36 * d / n^3`.
pub fn pieces_for(cubic: [Point; 4], tolerance: f64) -> usize {
    let [p0, p1, p2, p3] = cubic;
    let dx = p3.0 - 3.0 * p2.0 + 3.0 * p1.0 - p0.0;
    let dy = p3.1 - 3.0 * p2.1 + 3.0 * p1.1 - p0.1;
    let d = dx.hypot(dy);
    let n = ((3f64.sqrt() / 36.0 * d / tolerance.max(f64::EPSILON)).cbrt()).ceil();
    if !n.is_finite() || n < 1.0 {
        1
    } else {
        (n as usize).min(MAX_PIECES)
    }
}

/// Quadratic segments `(control, end)` approximating `cubic`, starting at its first point.
pub fn to_quads(cubic: [Point; 4], tolerance: f64) -> Vec<(Point, Point)> {
    let n = pieces_for(cubic, tolerance);
    let last = cubic[3];
    (0..n)
        .map(|i| {
            let piece = sub_cubic(cubic, i as f64 / n as f64, (i + 1) as f64 / n as f64);
            // The quadratic whose control point is the average of the two cubic handles'
            // projections: (3 c1 - p0 + 3 c2 - p3) / 4.
            let control = (
                (3.0 * piece[1].0 - piece[0].0 + 3.0 * piece[2].0 - piece[3].0) / 4.0,
                (3.0 * piece[1].1 - piece[0].1 + 3.0 * piece[2].1 - piece[3].1) / 4.0,
            );
            // The final piece ends exactly on the cubic's end point, free of round-off.
            (control, if i + 1 == n { last } else { piece[3] })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad_at(p0: Point, c: Point, p1: Point, t: f64) -> Point {
        lerp(lerp(p0, c, t), lerp(c, p1, t), t)
    }

    fn cubic_at(c: [Point; 4], t: f64) -> Point {
        let (a, b, d) = (
            lerp(c[0], c[1], t),
            lerp(c[1], c[2], t),
            lerp(c[2], c[3], t),
        );
        lerp(lerp(a, b, t), lerp(b, d, t), t)
    }

    /// The largest distance between sampled points of the cubic and of its quads.
    fn max_error(cubic: [Point; 4], tolerance: f64) -> f64 {
        let quads = to_quads(cubic, tolerance);
        let n = quads.len();
        let mut worst = 0.0_f64;
        let mut start = cubic[0];
        for (i, (control, end)) in quads.iter().enumerate() {
            for step in 0..=16 {
                let local = f64::from(step) / 16.0;
                let global = (i as f64 + local) / n as f64;
                let (q, c) = (
                    quad_at(start, *control, *end, local),
                    cubic_at(cubic, global),
                );
                worst = worst.max((q.0 - c.0).hypot(q.1 - c.1));
            }
            start = *end;
        }
        worst
    }

    #[test]
    fn straight_and_quadratic_like_cubics_need_one_piece() {
        let line = [(0.0, 0.0), (1.0, 1.0), (2.0, 2.0), (3.0, 3.0)];
        assert_eq!(pieces_for(line, 0.01), 1);
        // A cubic that is exactly a degree-elevated quadratic has a zero third difference.
        let elevated = [(0.0, 0.0), (20.0, 40.0), (40.0, 40.0), (60.0, 0.0)];
        assert_eq!(pieces_for(elevated, 0.01), 1);
        assert!(max_error(elevated, 0.01) < 1e-9);
    }

    #[test]
    fn the_error_stays_within_the_tolerance_for_awkward_curves() {
        let curves = [
            [(0.0, 0.0), (0.0, 100.0), (100.0, 100.0), (100.0, 0.0)],
            [(0.0, 0.0), (300.0, 0.0), (-200.0, 100.0), (100.0, 100.0)],
            [(0.0, 0.0), (1000.0, 1000.0), (-1000.0, 1000.0), (0.0, 0.0)],
            [(5.0, 5.0), (5.0, 5.0), (5.0, 5.0), (5.0, 5.0)],
        ];
        for tolerance in [1.0, 0.1, 0.01] {
            for curve in curves {
                let error = max_error(curve, tolerance);
                // The bound is an upper bound; allow small slack for the sampling.
                assert!(
                    error <= tolerance * 1.01,
                    "error {error} exceeds {tolerance} for {curve:?}"
                );
            }
        }
    }

    #[test]
    fn pieces_are_capped_and_degenerate_input_is_safe() {
        let wild = [(0.0, 0.0), (1e12, 0.0), (-1e12, 1e12), (0.0, 0.0)];
        assert_eq!(pieces_for(wild, 1e-9), MAX_PIECES);
        assert_eq!(to_quads(wild, 1e-9).len(), MAX_PIECES);
        assert_eq!(pieces_for([(0.0, 0.0); 4], 0.0), 1);
        let nan = [(f64::NAN, 0.0); 4];
        assert_eq!(
            pieces_for(nan, 1.0),
            1,
            "no panic and no unbounded loop on NaN"
        );
    }

    #[test]
    fn the_last_quad_ends_exactly_where_the_cubic_does() {
        let cubic = [(0.0, 0.0), (10.0, 80.0), (60.0, -20.0), (90.0, 30.0)];
        let quads = to_quads(cubic, 0.05);
        assert_eq!(quads.last().map(|q| q.1), Some((90.0, 30.0)));
        assert!(quads.len() > 1);
    }
}
