//! Least-squares cubic Bézier fitting of point sequences (the classic "fit a cubic with fixed end
//! tangents, then reparameterise with Newton steps, then split at the worst point" approach from
//! Graphics Gems, written from the published description).

use kurbo::{CubicBez, ParamCurve, ParamCurveDeriv, Point, Vec2};

/// Maximum Newton reparameterisation rounds per fit.
const MAX_REPARAM: usize = 6;

/// Unit vector or `None` if (nearly) zero.
pub(crate) fn unit(v: Vec2) -> Option<Vec2> {
    let l = v.hypot();
    if l > 1e-12 && l.is_finite() { Some(v / l) } else { None }
}

/// Direction the curve leaves `p0` (robust to retracted handles).
pub(crate) fn start_tangent(c: &CubicBez) -> Vec2 {
    unit(c.p1 - c.p0).or_else(|| unit(c.p2 - c.p0)).or_else(|| unit(c.p3 - c.p0)).unwrap_or(Vec2::new(1.0, 0.0))
}

/// Direction the curve arrives at `p3`, i.e. pointing *forward* along the travel direction.
pub(crate) fn end_tangent(c: &CubicBez) -> Vec2 {
    unit(c.p3 - c.p2).or_else(|| unit(c.p3 - c.p1)).or_else(|| unit(c.p3 - c.p0)).unwrap_or(Vec2::new(1.0, 0.0))
}

/// Fit a chain of cubics through `pts` with maximum (parametric) error `tol`.
///
/// `t0` is the unit tangent leaving `pts[0]`; `t1` is the unit tangent arriving at the last point
/// (forward direction). Consecutive output cubics are G1 continuous.
pub(crate) fn fit_cubics(pts: &[Point], t0: Vec2, t1: Vec2, tol: f64) -> Vec<CubicBez> {
    let mut out = Vec::new();
    let pts = dedup(pts);
    if pts.len() < 2 {
        return out;
    }
    fit_rec(&pts, t0, t1, tol.max(1e-9), &mut out, 0);
    out
}

fn dedup(pts: &[Point]) -> Vec<Point> {
    let mut v: Vec<Point> = Vec::with_capacity(pts.len());
    for &p in pts {
        if v.last().is_none_or(|q: &Point| q.distance(p) > 1e-12) {
            v.push(p);
        }
    }
    v
}

fn fit_rec(pts: &[Point], t0: Vec2, t1: Vec2, tol: f64, out: &mut Vec<CubicBez>, depth: usize) {
    let n = pts.len();
    if n == 2 {
        let d = pts[0].distance(pts[1]) / 3.0;
        out.push(CubicBez::new(pts[0], pts[0] + t0 * d, pts[1] - t1 * d, pts[1]));
        return;
    }
    let (c, err, split) = fit_single(pts, t0, t1);
    if err <= tol || depth > 40 {
        out.push(c);
        return;
    }
    // Split at the worst point with a centre tangent estimated from its neighbours.
    let split = split.clamp(1, n - 2);
    let tc = unit(pts[split + 1] - pts[split - 1]).unwrap_or(t0);
    fit_rec(&pts[..=split], t0, tc, tol, out, depth + 1);
    fit_rec(&pts[split..], tc, t1, tol, out, depth + 1);
}

/// Best single cubic for `pts` with the given end tangents. Returns (curve, max error, index of
/// the worst point).
pub(crate) fn fit_single(pts: &[Point], t0: Vec2, t1: Vec2) -> (CubicBez, f64, usize) {
    fit_single_from(pts, chord_params(pts), t0, t1)
}

/// [`fit_single`] starting from the parameters `u` (one per point, 0 to 1) instead of the
/// chord-length ones.
pub(crate) fn fit_single_from(pts: &[Point], mut u: Vec<f64>, t0: Vec2, t1: Vec2) -> (CubicBez, f64, usize) {
    let mut best = generate(pts, &u, t0, t1);
    let (mut best_err, mut best_split) = max_error(pts, &best, &u);
    for _ in 0..MAX_REPARAM {
        for (i, ui) in u.iter_mut().enumerate() {
            *ui = newton(&best, pts[i], *ui);
        }
        let c = generate(pts, &u, t0, t1);
        let (e, s) = max_error(pts, &c, &u);
        if e < best_err {
            best = c;
            best_err = e;
            best_split = s;
        } else {
            break;
        }
    }
    (best, best_err, best_split)
}

fn chord_params(pts: &[Point]) -> Vec<f64> {
    let mut u = Vec::with_capacity(pts.len());
    let mut acc = 0.0;
    u.push(0.0);
    for w in pts.windows(2) {
        acc += w[0].distance(w[1]);
        u.push(acc);
    }
    if acc > 0.0 {
        for x in &mut u {
            *x /= acc;
        }
    }
    u
}

fn bern(t: f64) -> [f64; 4] {
    let s = 1.0 - t;
    [s * s * s, 3.0 * s * s * t, 3.0 * s * t * t, t * t * t]
}

/// Solve the 2x2 normal equations for the handle lengths along the fixed tangents.
fn generate(pts: &[Point], u: &[f64], t0: Vec2, t1: Vec2) -> CubicBez {
    let (Some(&p0), Some(&p3)) = (pts.first(), pts.last()) else { return CubicBez::new(Point::ZERO, Point::ZERO, Point::ZERO, Point::ZERO) };
    let dir_in = -t1; // handle at the end points backwards
    let (mut c00, mut c01, mut c11, mut x0, mut x1) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for (p, &t) in pts.iter().zip(u) {
        let b = bern(t);
        let a0 = t0 * b[1];
        let a1 = dir_in * b[2];
        c00 += a0.dot(a0);
        c01 += a0.dot(a1);
        c11 += a1.dot(a1);
        let tmp = *p - (p0.to_vec2() * (b[0] + b[1]) + p3.to_vec2() * (b[2] + b[3])).to_point();
        x0 += a0.dot(tmp);
        x1 += a1.dot(tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = p0.distance(p3);
    let eps = 1e-6 * seg;
    let (mut al, mut ar) = if det.abs() > 1e-12 { ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det) } else { (seg / 3.0, seg / 3.0) };
    if !al.is_finite() || !ar.is_finite() || al < eps || ar < eps {
        al = seg / 3.0;
        ar = seg / 3.0;
    }
    // Guard against wild handles on nearly-degenerate data.
    let cap = seg * 4.0 + 1e-9;
    al = al.min(cap);
    ar = ar.min(cap);
    CubicBez::new(p0, p0 + t0 * al, p3 + dir_in * ar, p3)
}

fn max_error(pts: &[Point], c: &CubicBez, u: &[f64]) -> (f64, usize) {
    let mut e = 0.0;
    let mut idx = pts.len() / 2;
    for (i, (p, &t)) in pts.iter().zip(u).enumerate() {
        let d = c.eval(t).distance(*p);
        if d > e {
            e = d;
            idx = i;
        }
    }
    (e, idx)
}

fn newton(c: &CubicBez, p: Point, t: f64) -> f64 {
    let d1 = c.deriv();
    let d2 = d1.deriv();
    let q = c.eval(t) - p;
    let q1 = d1.eval(t).to_vec2();
    let q2 = d2.eval(t).to_vec2();
    let num = q.dot(q1);
    let den = q1.dot(q1) + q.dot(q2);
    if den.abs() < 1e-12 {
        return t;
    }
    let nt = t - num / den;
    if nt.is_finite() { nt.clamp(0.0, 1.0) } else { t }
}

/// Is the cubic (within `tol`) a straight line from p0 to p3 with handles inside the chord?
pub(crate) fn is_straight(c: &CubicBez, tol: f64) -> bool {
    let chord = c.p3 - c.p0;
    let len = chord.hypot();
    if len < 1e-12 {
        return c.p1.distance(c.p0) <= tol && c.p2.distance(c.p0) <= tol;
    }
    let dir = chord / len;
    [c.p1, c.p2].iter().all(|&h| {
        let v = h - c.p0;
        let along = v.dot(dir);
        v.cross(dir).abs() <= tol && along >= -tol && along <= len + tol
    })
}

/// Sample a cubic at `n+1` evenly spaced parameters (including both ends).
pub(crate) fn sample(c: &CubicBez, n: usize, out: &mut Vec<Point>, include_start: bool) {
    let start = if include_start { 0 } else { 1 };
    for i in start..=n {
        out.push(c.eval(i as f64 / n as f64));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fits_a_cubic_exactly() {
        let c = CubicBez::new((0.0, 0.0), (30.0, 60.0), (70.0, 60.0), (100.0, 0.0));
        let mut pts = Vec::new();
        sample(&c, 40, &mut pts, true);
        let out = fit_cubics(&pts, start_tangent(&c), end_tangent(&c), 0.05);
        assert_eq!(out.len(), 1);
        assert!(out[0].p1.distance(c.p1) < 1.0, "{:?}", out[0]);
    }

    #[test]
    fn splits_when_needed() {
        // A zig-zag cannot be fitted with one cubic.
        let pts: Vec<Point> = (0..=40).map(|i| Point::new(i as f64, ((i as f64) * 0.5).sin() * 10.0)).collect();
        let out = fit_cubics(&pts, Vec2::new(1.0, 0.0), Vec2::new(1.0, 0.0), 0.1);
        assert!(out.len() > 1);
        for w in out.windows(2) {
            assert!(w[0].p3.distance(w[1].p0) < 1e-9);
        }
    }

    #[test]
    fn straight_detection() {
        let c = CubicBez::new((0.0, 0.0), (3.0, 0.0), (6.0, 0.0), (9.0, 0.0));
        assert!(is_straight(&c, 1e-6));
        let c = CubicBez::new((0.0, 0.0), (3.0, 1.0), (6.0, 0.0), (9.0, 0.0));
        assert!(!is_straight(&c, 1e-6));
    }
}
