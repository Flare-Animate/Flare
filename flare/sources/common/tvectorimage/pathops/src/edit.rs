//! Object → Path commands: Simplify, Smooth, Remove redundant points, Add Anchor Points, Average,
//! Join, Split Into Grid.

use kurbo::{CubicBez, ParamCurveArclen, Point, Rect, Vec2};
use crate::geom::{Anchor, PathData, SubPath};

use crate::boolean::{Seg, segs_to_subpath};
use crate::fit::{end_tangent, fit_cubics, fit_single, fit_single_from, is_straight, sample, start_tangent};

/// Options for [`simplify_with`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SimplifyOptions {
    /// Maximum distance (document points) between the original and simplified path.
    pub tolerance: f64,
    /// Anchors where the path turns by more than this many degrees stay corners.
    pub corner_angle_deg: f64,
    /// Produce only straight segments (Illustrator's "Convert to Straight Lines").
    pub straight_lines: bool,
}

impl Default for SimplifyOptions {
    fn default() -> Self {
        Self { tolerance: 0.5, corner_angle_deg: 30.0, straight_lines: false }
    }
}

/// Simplify with the default corner threshold (30°).
pub fn simplify(path: &PathData, tolerance: f64) -> PathData {
    simplify_with(path, &SimplifyOptions { tolerance, ..Default::default() })
}

/// Simplify never adds anchors: keep whichever subpath is smaller.
fn no_worse(original: &SubPath, fitted: SubPath) -> SubPath {
    if fitted.anchors.len() > original.anchors.len() { original.clone() } else { fitted }
}

fn subpath_segs(sp: &SubPath) -> Vec<Seg> {
    (0..sp.segment_count()).map(|i| Seg { c: sp.segment(i), line: sp.segment_is_line(i) }).collect()
}

fn turn_deg(a: &Seg, b: &Seg) -> f64 {
    let d = end_tangent(&a.c).dot(start_tangent(&b.c)).clamp(-1.0, 1.0);
    d.acos().to_degrees()
}

/// Least-squares Bézier refit of every subpath, keeping corners sharper than the threshold.
pub fn simplify_with(path: &PathData, opts: &SimplifyOptions) -> PathData {
    let tol = opts.tolerance.max(1e-6);
    let mut out = Vec::new();
    for sp in &path.subpaths {
        let mut segs = subpath_segs(sp);
        let n = segs.len();
        if n == 0 {
            out.push(sp.clone());
            continue;
        }
        let corner_at = |segs: &[Seg], i: usize| -> bool {
            if !sp.closed && (i == 0 || i == n) {
                return true;
            }
            turn_deg(&segs[(i + n - 1) % n], &segs[i % n]) > opts.corner_angle_deg
        };
        let mut corners: Vec<usize> = (0..n).filter(|&i| corner_at(&segs, i)).collect();
        if sp.closed {
            if let Some(&k) = corners.first() {
                segs.rotate_left(k);
                corners = corners.iter().map(|&c| c - k).collect();
            } else {
                corners = vec![0];
            }
            corners.push(n);
        } else if corners.last() != Some(&n) {
            corners.push(n);
        }
        let mut result: Vec<Seg> = Vec::new();
        for w in corners.windows(2) {
            let run = &segs[w[0]..w[1]];
            if run.is_empty() {
                continue;
            }
            let mut pts = Vec::new();
            for (k, s) in run.iter().enumerate() {
                let len = s.c.arclen(1e-6);
                let m = ((len / (tol * 0.5)).ceil() as usize).clamp(4, 256);
                sample(&s.c, m, &mut pts, k == 0);
            }
            if opts.straight_lines {
                let keep = rdp(&pts, tol);
                result.extend(keep.windows(2).map(|p| Seg::line(p[0], p[1])));
                continue;
            }
            let t0 = start_tangent(&run[0].c);
            let t1 = end_tangent(&run[run.len() - 1].c);
            for c in fit_cubics(&pts, t0, t1, tol) {
                let line = is_straight(&c, tol * 0.02);
                result.push(if line { Seg::line(c.p0, c.p3) } else { Seg { c, line } });
            }
        }
        if let Some(s) = segs_to_subpath(&result, sp.closed) {
            out.push(if opts.straight_lines { s } else { no_worse(sp, s) });
        }
    }
    PathData::new(out)
}

/// Ramer–Douglas–Peucker polyline reduction.
fn rdp(pts: &[Point], tol: f64) -> Vec<Point> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let mut keep = vec![false; pts.len()];
    keep[0] = true;
    keep[pts.len() - 1] = true;
    let mut stack = vec![(0, pts.len() - 1)];
    while let Some((a, b)) = stack.pop() {
        let (pa, pb) = (pts[a], pts[b]);
        let chord = pb - pa;
        let len = chord.hypot();
        let mut best = (0.0, a);
        for (i, &p) in pts.iter().enumerate().take(b).skip(a + 1) {
            let d = if len < 1e-12 { p.distance(pa) } else { (p - pa).cross(chord).abs() / len };
            if d > best.0 {
                best = (d, i);
            }
        }
        if best.0 > tol {
            keep[best.1] = true;
            stack.push((a, best.1));
            stack.push((best.1, b));
        }
    }
    pts.iter().zip(keep).filter(|(_, k)| *k).map(|(p, _)| *p).collect()
}

/// Object → Path → Smooth: pull handles towards Catmull-Rom-like tangents. `amount` ∈ [0, 1]
/// (0 = unchanged, 1 = fully smooth). Endpoints of open paths keep their handles.
pub fn smooth(path: &PathData, amount: f64) -> PathData {
    let t = amount.clamp(0.0, 1.0);
    let mut out = path.clone();
    for sp in &mut out.subpaths {
        let n = sp.anchors.len();
        if n < 3 {
            continue;
        }
        let orig = sp.anchors.clone();
        for i in 0..n {
            if !sp.closed && (i == 0 || i == n - 1) {
                continue;
            }
            let prev = orig[(i + n - 1) % n].p;
            let next = orig[(i + 1) % n].p;
            let p = orig[i].p;
            let Some(dir) = crate::fit::unit(next - prev) else { continue };
            let tin = p - dir * (p.distance(prev) / 3.0);
            let tout = p + dir * (p.distance(next) / 3.0);
            let a = &orig[i];
            let h_in = a.h_in.lerp(tin, t);
            let h_out = a.h_out.lerp(tout, t);
            sp.anchors[i] = Anchor::with_handles(p, h_in, h_out);
        }
    }
    out
}

/// Remove anchors that can be deleted without changing the shape by more than `tolerance`
/// (collinear corner points, duplicate points, and curve joints that one cubic can replace).
pub fn remove_redundant_points(path: &PathData, tolerance: f64) -> PathData {
    let tol = tolerance.max(1e-9);
    let mut out = Vec::new();
    for sp in &path.subpaths {
        let mut segs = subpath_segs(sp);
        if segs.is_empty() {
            out.push(sp.clone());
            continue;
        }
        // Drop zero-length segments.
        segs.retain(|s| !(s.c.p0.distance(s.c.p3) < 1e-9 && s.c.p1.distance(s.c.p0) < 1e-9 && s.c.p2.distance(s.c.p0) < 1e-9));
        let min_segs = if sp.closed { 2 } else { 1 };
        let mut changed = true;
        while changed && segs.len() > min_segs {
            changed = false;
            let n = segs.len();
            let joints = if sp.closed { n } else { n - 1 };
            for j in 0..joints {
                let (i0, i1) = if sp.closed { ((j + n - 1) % n, j) } else { (j, j + 1) };
                if let Some(m) = merge_pair(&segs[i0], &segs[i1], tol) {
                    segs[i0] = m;
                    segs.remove(i1);
                    if sp.closed && i1 == 0 {
                        // The merged segment now starts the loop.
                        if let Some(last) = segs.pop() {
                            segs.insert(0, last);
                        }
                    }
                    changed = true;
                    break;
                }
            }
        }
        if let Some(s) = segs_to_subpath(&segs, sp.closed) {
            out.push(s);
        }
    }
    PathData::new(out)
}

fn merge_pair(a: &Seg, b: &Seg, tol: f64) -> Option<Seg> {
    if a.line && b.line {
        let chord = b.c.p3 - a.c.p0;
        let len = chord.hypot();
        if len < 1e-12 {
            return None;
        }
        let mid = a.c.p3 - a.c.p0;
        let along = mid.dot(chord) / len;
        return ((mid.cross(chord) / len).abs() <= tol && along >= 0.0 && along <= len).then(|| Seg::line(a.c.p0, b.c.p3));
    }
    // Only merge across (near-)smooth joints.
    if end_tangent(&a.c).dot(start_tangent(&b.c)) < 0.99 {
        return None;
    }
    let mut pts = Vec::new();
    sample(&a.c, 16, &mut pts, true);
    sample(&b.c, 16, &mut pts, false);
    let (c, err, _) = fit_single(&pts, start_tangent(&a.c), end_tangent(&b.c));
    (err <= tol).then_some(Seg { c, line: false })
}

/// Remove anchor `index` without opening the path (the Delete Anchor Point tool and Object → Path
/// → Remove Anchor Points). Its neighbours keep their handle directions and their facing handles
/// are refitted so one cubic follows the two segments that met there; two straight segments
/// become one. Removing an end of an open path drops its segment. Returns false, leaving `sp`
/// alone, when `index` is out of range.
pub fn remove_anchor(sp: &mut SubPath, index: usize) -> bool {
    let n = sp.anchors.len();
    if index >= n {
        return false;
    }
    if (sp.closed || (index > 0 && index + 1 < n)) && n >= 3 {
        let (pi, ni) = ((index + n - 1) % n, (index + 1) % n);
        if !(sp.segment_is_line(pi) && sp.segment_is_line(index)) {
            let (left, right) = (sp.segment(pi), sp.segment(index));
            const N: usize = 16;
            let mut pts = Vec::with_capacity(2 * N + 1);
            sample(&left, N, &mut pts, true);
            sample(&right, N, &mut pts, false);
            let (t0, t1) = (start_tangent(&left), end_tangent(&right));
            // The new curve passes the removed point at some parameter k: the left samples sit
            // at k·i/N, the right ones at k + (1 − k)·i/N. Search k for the closest fit.
            let fit = |k: f64| {
                let u = (0..=N).map(|i| k * i as f64 / N as f64).chain((1..=N).map(|i| k + (1.0 - k) * i as f64 / N as f64)).collect();
                fit_single_from(&pts, u, t0, t1)
            };
            let (c, _, _) = golden_min(fit, 0.0, 1.0);
            if c.p1.is_finite() && c.p2.is_finite() {
                sp.anchors[pi].h_out = c.p1;
                sp.anchors[ni].h_in = c.p2;
            }
        }
    }
    sp.anchors.remove(index);
    if !sp.closed {
        // A new end has nothing beyond it.
        if let Some(a) = sp.anchors.first_mut() {
            a.h_in = a.p;
        }
        if let Some(a) = sp.anchors.last_mut() {
            a.h_out = a.p;
        }
    }
    if sp.anchors.len() < 3 {
        sp.closed = sp.closed && sp.anchors.len() == 2 && sp.anchors.iter().any(|a| a.has_in() || a.has_out());
    }
    true
}

/// The fit `f` (curve, error, worst point) with the least error for an argument in `[a, b]`, by a
/// golden-section search (a bounded number of steps).
fn golden_min(f: impl Fn(f64) -> (CubicBez, f64, usize), mut a: f64, mut b: f64) -> (CubicBez, f64, usize) {
    const R: f64 = 0.618_033_988_749_895;
    let (mut x1, mut x2) = (b - R * (b - a), a + R * (b - a));
    let (mut f1, mut f2) = (f(x1), f(x2));
    for _ in 0..40 {
        // A NaN error never wins.
        if f1.1 < f2.1 || f2.1.is_nan() {
            (b, x2, f2) = (x2, x1, f1);
            x1 = b - R * (b - a);
            f1 = f(x1);
        } else {
            (a, x1, f1) = (x1, x2, f2);
            x2 = a + R * (b - a);
            f2 = f(x2);
        }
    }
    if f1.1 <= f2.1 { f1 } else { f2 }
}

/// Object → Path → Add Anchor Points: one new anchor at the middle (t = 0.5) of every segment.
pub fn add_anchor_points(path: &PathData) -> PathData {
    let mut out = path.clone();
    for sp in &mut out.subpaths {
        for seg in (0..sp.segment_count()).rev() {
            sp.insert_anchor(seg, 0.5);
        }
    }
    out
}

/// Axis for [`average`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AverageAxis {
    /// Align on a common horizontal line (same y).
    Horizontal,
    /// Align on a common vertical line (same x).
    Vertical,
    /// Collapse to the common centroid.
    Both,
}

/// Object → Path → Average: move the selected anchors `(subpath, anchor)` (with their handles)
/// to their average position along `axis`.
pub fn average(path: &PathData, selection: &[(usize, usize)], axis: AverageAxis) -> PathData {
    let mut out = path.clone();
    let pts: Vec<Point> = selection.iter().filter_map(|&(s, a)| path.subpaths.get(s).and_then(|sp| sp.anchors.get(a)).map(|a| a.p)).collect();
    if pts.is_empty() {
        return out;
    }
    let c = pts.iter().fold(Vec2::ZERO, |acc, p| acc + p.to_vec2()) / pts.len() as f64;
    for &(s, a) in selection {
        if let Some(an) = out.anchor_mut(s, a) {
            let d = match axis {
                AverageAxis::Horizontal => Vec2::new(0.0, c.y - an.p.y),
                AverageAxis::Vertical => Vec2::new(c.x - an.p.x, 0.0),
                AverageAxis::Both => c - an.p.to_vec2(),
            };
            an.translate(d);
        }
    }
    out
}

/// Object → Path → Join. Open subpaths of all inputs are joined nearest-endpoint-first into one
/// open path (endpoints closer than `tolerance` are merged into a single anchor, otherwise a
/// straight segment connects them). A single open subpath is closed instead. Closed subpaths
/// pass through unchanged.
pub fn join(paths: &[PathData], tolerance: f64) -> PathData {
    let mut closed: Vec<SubPath> = Vec::new();
    let mut open: Vec<SubPath> = Vec::new();
    for p in paths {
        for sp in &p.subpaths {
            if sp.closed || sp.anchors.len() < 2 {
                if !sp.anchors.is_empty() {
                    closed.push(sp.clone());
                }
            } else {
                open.push(sp.clone());
            }
        }
    }
    if open.len() == 1
        && let Some(mut sp) = open.pop()
    {
        let ends_meet = sp.anchors.len() > 2 && end_point(&sp, false).zip(end_point(&sp, true)).is_some_and(|(a, b)| a.distance(b) <= tolerance);
        if ends_meet
            && let Some(last) = sp.anchors.pop()
            && let Some(f) = sp.anchors.first_mut()
        {
            *f = Anchor::with_handles(f.p, last.h_in, f.h_out);
        } else {
            if let Some(f) = sp.anchors.first_mut() {
                f.h_in = f.p;
            }
            if let Some(l) = sp.anchors.last_mut() {
                l.h_out = l.p;
            }
        }
        sp.closed = true;
        closed.push(sp);
        return PathData::new(closed);
    }
    while open.len() > 1 {
        // Find the closest pair of endpoints on different subpaths.
        let mut best = (f64::INFINITY, 0, 0, false, false);
        for i in 0..open.len() {
            for j in (i + 1)..open.len() {
                for ei in [false, true] {
                    for ej in [false, true] {
                        let (Some(pi), Some(pj)) = (end_point(&open[i], ei), end_point(&open[j], ej)) else { continue };
                        let d = pi.distance(pj);
                        if d < best.0 {
                            best = (d, i, j, ei, ej);
                        }
                    }
                }
            }
        }
        let (d, i, j, ei, ej) = best;
        let mut b = open.remove(j);
        let a = &mut open[i];
        if !ei {
            a.reverse();
        }
        if ej {
            b.reverse();
        }
        if d <= tolerance && !b.anchors.is_empty() {
            let first = b.anchors.remove(0);
            if let Some(l) = a.anchors.last_mut() {
                *l = Anchor::with_handles(l.p, l.h_in, first.h_out);
            }
        } else {
            if let Some(l) = a.anchors.last_mut() {
                l.h_out = l.p;
            }
            if let Some(f) = b.anchors.first_mut() {
                f.h_in = f.p;
            }
        }
        a.anchors.extend(b.anchors);
    }
    closed.extend(open);
    PathData::new(closed)
}

/// Object → Path → Split Into Grid: `rows × cols` rectangles filling `rect` with a gutter between
/// the rows and one between the columns (row-major, top-left first). No cells when they would have
/// no size.
pub fn split_into_grid(rect: Rect, rows: usize, cols: usize, (row_gutter, col_gutter): (f64, f64)) -> Vec<PathData> {
    if rows == 0 || cols == 0 {
        return Vec::new();
    }
    let w = (rect.width() - col_gutter * (cols as f64 - 1.0)) / cols as f64;
    let h = (rect.height() - row_gutter * (rows as f64 - 1.0)) / rows as f64;
    // Written so that a size that is not a number gives no cells either.
    if !(w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()) {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(rows * cols);
    for r in 0..rows {
        for c in 0..cols {
            let x = rect.x0 + c as f64 * (w + col_gutter);
            let y = rect.y0 + r as f64 * (h + row_gutter);
            out.push(PathData::single(SubPath::polyline(
                &[Point::new(x, y), Point::new(x + w, y), Point::new(x + w, y + h), Point::new(x, y + h)],
                true,
            )));
        }
    }
    out
}

/// The first (`last == false`) or last anchor point of a subpath.
fn end_point(sp: &SubPath, last: bool) -> Option<Point> {
    if last { sp.anchors.last() } else { sp.anchors.first() }.map(|a| a.p)
}
