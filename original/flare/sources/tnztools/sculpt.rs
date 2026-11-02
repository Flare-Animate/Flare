//! Vector deformation tools from OpenToonz's `tnztools` (`magnettool.cpp`,
//! `pinchtool.cpp`, `pumptool.cpp`, `bendertool.cpp`, `irontool.cpp` and the
//! stroke deformations in `tnzext/`; BSD-3-Clause, © DWANGO and OpenToonz
//! contributors; see THIRD-PARTY-NOTICES.md).
//!
//! OpenToonz deforms the control points of its centreline strokes. Here the
//! same gestures work on our two kinds of vector art:
//! * drawing objects (`Shape`): Bezier segments that the deformation only
//!   moves rigidly keep their curves; segments in the falloff zone are
//!   resampled into short lines first, so the result stays smooth;
//! * merge shapes: fills and lines share their boundary vertices, so every
//!   deformation is a map of positions and shared edges stay glued together.
//!
//! * **Magnet**: points within `size` of the cursor follow it, with falloff.
//! * **Pinch**: drag a point of a line; the line follows over `size` units of
//!   its length on each side (smooth, or `sharp` for a corner).
//! * **Pump**: drag up/down on a line to thicken/thin it over `size` units.
//! * **Bender**: draw a segment across lines, then drag on one side: the part
//!   beyond the segment turns (by the drag's angle around the segment's
//!   start), each line bending where it crosses the segment.
//! * **Iron**: brush over lines to smooth their wiggles.

use crate::geom::{pt, Path, Pt, Seg};
use crate::merge::MergeShape;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Magnet,
    Pinch,
    Pump,
    Bender,
    Iron,
}

impl Kind {
    pub fn name(self) -> &'static str {
        match self {
            Kind::Magnet => "Magnet",
            Kind::Pinch => "Pinch",
            Kind::Pump => "Pump",
            Kind::Bender => "Bender",
            Kind::Iron => "Iron",
        }
    }
    pub fn from_id(s: &str) -> Option<Kind> {
        [Kind::Magnet, Kind::Pinch, Kind::Pump, Kind::Bender, Kind::Iron].into_iter().find(|k| k.name().eq_ignore_ascii_case(s))
    }
}

/// Tool options (Properties ▸ Tool).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opts {
    /// Radius (Magnet, Iron) or reach along the line (Pinch, Pump), stage units.
    pub size: f32,
    /// Pinch: make a corner instead of a smooth bump.
    pub sharp: bool,
    /// Iron: smoothing per pass (0…1).
    pub strength: f32,
}

impl Default for Opts {
    fn default() -> Self {
        Opts { size: 40.0, sharp: false, strength: 0.5 }
    }
}

/// OpenToonz bender's falloff band either side of the segment (`c_LengthOfBenderRegion`).
pub const BENDER_BAND: f32 = 10.0;

// ------------------------------------------------------------ falloffs

/// Smooth radial falloff: 1 at the centre, 0 at `r`.
pub fn falloff(d: f32, r: f32) -> f32 {
    if r <= 0.0 || d >= r {
        return 0.0;
    }
    let x = d / r;
    (1.0 - x * x) * (1.0 - x * x)
}

/// Pinch falloff along the line: `x` = distance / reach.
fn pinch_weight(x: f32, sharp: bool) -> f32 {
    if x >= 1.0 {
        0.0
    } else if sharp {
        1.0 - x
    } else {
        0.5 * (1.0 + (std::f32::consts::PI * x).cos())
    }
}

fn smoothstep(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn rotate_about(p: Pt, c: Pt, a: f32) -> Pt {
    let (s, co) = a.sin_cos();
    let (dx, dy) = (p.x - c.x, p.y - c.y);
    pt(c.x + dx * co - dy * s, c.y + dx * s + dy * co)
}

fn cross(a: Pt, b: Pt) -> f32 {
    a.x * b.y - a.y * b.x
}

/// The Bender: points on the `versus` side of the line through `p0` along
/// `dir` rotate about `p0` by `angle`, eased in over `band` units.
#[derive(Clone, Copy, Debug)]
pub struct BendMap {
    pub p0: Pt,
    pub dir: Pt,
    pub versus: f32,
    pub angle: f32,
    pub band: f32,
}

impl BendMap {
    fn side(&self, p: Pt) -> f32 {
        let l = (self.dir.x * self.dir.x + self.dir.y * self.dir.y).sqrt().max(1e-6);
        cross(pt(self.dir.x / l, self.dir.y / l), pt(p.x - self.p0.x, p.y - self.p0.y)) * self.versus
    }
    pub fn apply(&self, p: Pt) -> Pt {
        let s = self.side(p);
        if s <= 0.0 {
            return p;
        }
        rotate_about(p, self.p0, self.angle * smoothstep(s / self.band.max(1e-3)))
    }
    /// Does a box straddle the eased band (where the map is not rigid)?
    fn bends(&self, lo: Pt, hi: Pt) -> bool {
        let s = [pt(lo.x, lo.y), pt(hi.x, lo.y), pt(lo.x, hi.y), pt(hi.x, hi.y)].map(|c| self.side(c));
        let (mn, mx) = (s.iter().cloned().fold(f32::MAX, f32::min), s.iter().cloned().fold(f32::MIN, f32::max));
        mx > 0.0 && mn < self.band
    }
}

// ------------------------------------------------------------ paths

/// A path split into subpaths (each starts with `M`).
pub fn subpaths(path: &Path) -> Vec<Vec<Seg>> {
    let mut out: Vec<Vec<Seg>> = vec![];
    for s in &path.segs {
        if matches!(s, Seg::M(_)) || out.is_empty() {
            out.push(vec![]);
        }
        out.last_mut().unwrap().push(*s);
    }
    out
}

fn seg_point(a: Pt, s: &Seg, t: f32) -> Pt {
    let u = 1.0 - t;
    match *s {
        Seg::L(b) => a.lerp(b, t),
        Seg::Q(c, b) => pt(u * u * a.x + 2.0 * u * t * c.x + t * t * b.x, u * u * a.y + 2.0 * u * t * c.y + t * t * b.y),
        Seg::C(c1, c2, b) => pt(
            u * u * u * a.x + 3.0 * u * u * t * c1.x + 3.0 * u * t * t * c2.x + t * t * t * b.x,
            u * u * u * a.y + 3.0 * u * u * t * c1.y + 3.0 * u * t * t * c2.y + t * t * t * b.y,
        ),
        _ => a,
    }
}

fn seg_end(s: &Seg) -> Option<Pt> {
    match *s {
        Seg::M(p) | Seg::L(p) | Seg::Q(_, p) | Seg::C(_, _, p) => Some(p),
        Seg::Z => None,
    }
}

fn seg_box(a: Pt, s: &Seg) -> (Pt, Pt) {
    let mut lo = a;
    let mut hi = a;
    let mut grow = |p: Pt| {
        lo = pt(lo.x.min(p.x), lo.y.min(p.y));
        hi = pt(hi.x.max(p.x), hi.y.max(p.y));
    };
    match *s {
        Seg::M(p) | Seg::L(p) => grow(p),
        Seg::Q(c, p) => {
            grow(c);
            grow(p)
        }
        Seg::C(c1, c2, p) => {
            grow(c1);
            grow(c2);
            grow(p)
        }
        Seg::Z => {}
    }
    (lo, hi)
}

fn approx_len(a: Pt, s: &Seg) -> f32 {
    let mut l = 0.0;
    let mut prev = a;
    for i in 1..=8 {
        let q = seg_point(a, s, i as f32 / 8.0);
        l += prev.dist(q);
        prev = q;
    }
    l
}

/// Map a path through `f`. Segments whose box `touched` reports are resampled
/// into lines about `step` long before mapping; the others map their control
/// points (exact for the rigid parts of a deformation).
pub fn map_path(path: &Path, touched: impl Fn(Pt, Pt) -> bool, step: f32, f: impl Fn(Pt) -> Pt) -> Path {
    let mut out = Vec::with_capacity(path.segs.len());
    let mut cur = pt(0.0, 0.0);
    for s in &path.segs {
        match *s {
            Seg::M(p) => out.push(Seg::M(f(p))),
            Seg::Z => out.push(Seg::Z),
            _ => {
                let (lo, hi) = seg_box(cur, s);
                if touched(lo, hi) {
                    let n = (approx_len(cur, s) / step.max(0.25)).ceil().clamp(1.0, 400.0) as usize;
                    for i in 1..=n {
                        out.push(Seg::L(f(seg_point(cur, s, i as f32 / n as f32))));
                    }
                } else {
                    out.push(match *s {
                        Seg::L(p) => Seg::L(f(p)),
                        Seg::Q(c, p) => Seg::Q(f(c), f(p)),
                        Seg::C(c1, c2, p) => Seg::C(f(c1), f(c2), f(p)),
                        other => other,
                    });
                }
            }
        }
        if let Some(e) = seg_end(s) {
            cur = e;
        }
    }
    Path { segs: out }
}

/// One subpath resampled into points about `step` apart (closed: no repeated end).
fn sample_subpath(segs: &[Seg], step: f32) -> (Vec<Pt>, bool) {
    let mut pts = vec![];
    let mut cur = pt(0.0, 0.0);
    let mut closed = false;
    for s in segs {
        match *s {
            Seg::M(p) => pts.push(p),
            Seg::Z => closed = true,
            _ => {
                let n = (approx_len(cur, s) / step.max(0.25)).ceil().clamp(1.0, 400.0) as usize;
                for i in 1..=n {
                    pts.push(seg_point(cur, s, i as f32 / n as f32));
                }
            }
        }
        if let Some(e) = seg_end(s) {
            cur = e;
        }
    }
    if closed && pts.len() > 2 && pts[0].dist(*pts.last().unwrap()) < 1e-3 {
        pts.pop();
    }
    (pts, closed)
}

fn polyline_path(pts: &[Pt], closed: bool) -> Vec<Seg> {
    let mut v = vec![];
    if let Some(f) = pts.first() {
        v.push(Seg::M(*f));
        v.extend(pts[1..].iter().map(|p| Seg::L(*p)));
        if closed {
            v.push(Seg::Z);
        }
    }
    v
}

fn seg_hit(p: Pt, a: Pt, b: Pt) -> (f32, f32) {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 1e-12 { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (p.dist(a.lerp(b, t)), t)
}

/// Nearest point of a polyline: (distance, arc length there, total length).
fn nearest_on(pts: &[Pt], closed: bool, p: Pt) -> (f32, f32, f32) {
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    let (mut best, mut at, mut acc) = (f32::MAX, 0.0, 0.0);
    for i in 0..segs {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let (d, t) = seg_hit(p, a, b);
        if d < best {
            best = d;
            at = acc + a.dist(b) * t;
        }
        acc += a.dist(b);
    }
    (best, at, acc)
}

fn segments_cross(a: Pt, b: Pt, c: Pt, d: Pt) -> bool {
    let d1 = cross(pt(b.x - a.x, b.y - a.y), pt(c.x - a.x, c.y - a.y));
    let d2 = cross(pt(b.x - a.x, b.y - a.y), pt(d.x - a.x, d.y - a.y));
    let d3 = cross(pt(d.x - c.x, d.y - c.y), pt(a.x - c.x, a.y - c.y));
    let d4 = cross(pt(d.x - c.x, d.y - c.y), pt(b.x - c.x, b.y - c.y));
    (d1 > 0.0) != (d2 > 0.0) && (d3 > 0.0) != (d4 > 0.0)
}

/// Where a polyline first crosses the segment `a`–`b`.
fn crossing(pts: &[Pt], closed: bool, a: Pt, b: Pt) -> Option<Pt> {
    let n = pts.len();
    let segs = if closed { n } else { n.saturating_sub(1) };
    (0..segs).find_map(|i| {
        let (c, d) = (pts[i], pts[(i + 1) % n]);
        if !segments_cross(c, d, a, b) {
            return None;
        }
        let r = pt(d.x - c.x, d.y - c.y);
        let sv = pt(b.x - a.x, b.y - a.y);
        let den = cross(r, sv);
        if den.abs() < 1e-9 {
            return Some(c);
        }
        let t = cross(pt(a.x - c.x, a.y - c.y), sv) / den;
        Some(c.lerp(d, t))
    })
}

fn circle_box(c: Pt, r: f32) -> impl Fn(Pt, Pt) -> bool {
    move |lo: Pt, hi: Pt| {
        let dx = (lo.x - c.x).max(0.0).max(c.x - hi.x);
        let dy = (lo.y - c.y).max(0.0).max(c.y - hi.y);
        dx * dx + dy * dy < r * r
    }
}

/// Resampling step for a deformation of `size` units.
fn step_for(size: f32) -> f32 {
    (size / 16.0).clamp(0.75, 6.0)
}

// ---- drawing objects

/// Magnet on a path: points near `c` move by `delta` (falloff over `r`).
pub fn magnet_path(path: &Path, c: Pt, r: f32, delta: Pt) -> Path {
    map_path(path, circle_box(c, r), step_for(r), |p| {
        let k = falloff(p.dist(c), r);
        pt(p.x + delta.x * k, p.y + delta.y * k)
    })
}

/// Pinch on a path: the subpath nearest `grab` (within `tol`) follows `delta`
/// over `reach` units of its length on each side. `None` if no line is near.
pub fn pinch_path(path: &Path, grab: Pt, tol: f32, reach: f32, sharp: bool, delta: Pt) -> Option<Path> {
    let subs = subpaths(path);
    let step = step_for(reach);
    let mut best: Option<(f32, usize, Vec<Pt>, bool, f32, f32)> = None;
    for (i, s) in subs.iter().enumerate() {
        let (pts, closed) = sample_subpath(s, step);
        if pts.len() < 2 {
            continue;
        }
        let (d, at, total) = nearest_on(&pts, closed, grab);
        if d <= tol && best.as_ref().map(|b| d < b.0).unwrap_or(true) {
            best = Some((d, i, pts, closed, at, total));
        }
    }
    let (_, idx, pts, closed, at, total) = best?;
    // arc length of each sample
    let mut s = 0.0;
    let mut moved = Vec::with_capacity(pts.len());
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            s += pts[i - 1].dist(*p);
        }
        let mut ds = (s - at).abs();
        if closed {
            ds = ds.min(total - ds);
        }
        let k = pinch_weight(ds / reach.max(1e-3), sharp);
        moved.push(pt(p.x + delta.x * k, p.y + delta.y * k));
    }
    let mut segs = vec![];
    for (i, s) in subs.into_iter().enumerate() {
        if i == idx {
            segs.extend(polyline_path(&moved, closed));
        } else {
            segs.extend(s);
        }
    }
    Some(Path { segs })
}

/// The Bender on a path: subpaths crossing the segment `a`–`b` bend.
pub fn bend_path(path: &Path, a: Pt, b: Pt, m: &BendMap) -> Path {
    let mut segs = vec![];
    for s in subpaths(path) {
        let (pts, closed) = sample_subpath(&s, 2.0);
        let sub = Path { segs: s };
        if let Some(x) = crossing(&pts, closed, a, b) {
            // bend about where this line crosses the segment, so it stays joined
            let m = BendMap { p0: x, ..*m };
            segs.extend(map_path(&sub, |lo, hi| m.bends(lo, hi), step_for(m.band * 4.0), |p| m.apply(p)).segs);
        } else {
            segs.extend(sub.segs);
        }
    }
    Path { segs }
}

/// One Iron pass at `c` (radius `r`): smooths the subpaths it touches.
pub fn iron_path(path: &Path, c: Pt, r: f32, strength: f32) -> Path {
    let hit = circle_box(c, r);
    let mut segs = vec![];
    for s in subpaths(path) {
        let sub = Path { segs: s };
        let touched = sub.bounds().map(|(lo, hi)| hit(lo, hi)).unwrap_or(false);
        if !touched {
            segs.extend(sub.segs);
            continue;
        }
        let (pts, closed) = sample_subpath(&sub.segs, step_for(r));
        if pts.len() < 3 || !pts.iter().any(|p| p.dist(c) < r) {
            segs.extend(sub.segs);
            continue;
        }
        let sm = smooth_polyline(&pts, closed, iron_window(r), |p| falloff(p.dist(c), r) * strength);
        segs.extend(polyline_path(&sm, closed));
    }
    Path { segs }
}

/// One smoothing pass: each vertex moves towards the average of its `k`
/// neighbours on each side, weighted by `w(p)` (open ends stay).
fn smooth_polyline(pts: &[Pt], closed: bool, k: usize, w: impl Fn(Pt) -> f32) -> Vec<Pt> {
    let n = pts.len();
    let k = k.max(1);
    (0..n)
        .map(|i| {
            let p = pts[i];
            if !closed && (i == 0 || i + 1 == n) {
                return p;
            }
            let wt = w(p).clamp(0.0, 1.0);
            if wt <= 0.0 {
                return p;
            }
            let (mut sx, mut sy, mut c) = (0.0, 0.0, 0.0);
            for d in 1..=k {
                for j in [i as isize - d as isize, i as isize + d as isize] {
                    let q = if closed {
                        pts[j.rem_euclid(n as isize) as usize]
                    } else if j < 0 || j >= n as isize {
                        continue;
                    } else {
                        pts[j as usize]
                    };
                    sx += q.x;
                    sy += q.y;
                    c += 1.0;
                }
            }
            if c == 0.0 {
                return p;
            }
            let m = pt(sx / c, sy / c);
            pt(p.x + (m.x - p.x) * wt, p.y + (m.y - p.y) * wt)
        })
        .collect()
}

/// Iron's smoothing window: about a quarter of the radius each side.
fn iron_window(r: f32) -> usize {
    ((r * 0.25) / step_for(r)).round().max(1.0) as usize
}

/// Pump on a width profile ((t, × width), sorted): around `t0`, over `span`
/// (in t), the width is multiplied by up to `factor`.
pub fn pump_profile(base: Option<&[(f32, f32)]>, t0: f32, span: f32, factor: f32) -> Vec<(f32, f32)> {
    let base: Vec<(f32, f32)> = match base {
        Some(b) if !b.is_empty() => b.to_vec(),
        _ => vec![(0.0, 1.0), (1.0, 1.0)],
    };
    let span = span.max(1e-3);
    let at = |t: f32| crate::deform::profile_at(&base, t);
    let mut ts: Vec<f32> = base.iter().map(|p| p.0).filter(|t| (t - t0).abs() >= span).collect();
    for i in 0..=16 {
        let t = t0 - span + 2.0 * span * i as f32 / 16.0;
        if (0.0..=1.0).contains(&t) {
            ts.push(t);
        }
    }
    ts.push(0.0);
    ts.push(1.0);
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
    ts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
    ts.into_iter()
        .map(|t| {
            let x = ((t - t0).abs() / span).min(1.0);
            let k = 0.5 * (1.0 + (std::f32::consts::PI * x).cos());
            (t, (at(t) * (1.0 + (factor - 1.0) * k)).clamp(0.02, 50.0))
        })
        .collect()
}

/// Pump factor from a vertical drag (OpenToonz: up thickens, down thins).
pub fn pump_factor(dy: f32) -> f32 {
    (-dy / 40.0).exp()
}

// ---- merge shapes

fn key(p: Pt) -> (i64, i64) {
    ((p.x * 256.0).round() as i64, (p.y * 256.0).round() as i64)
}

fn polylines(m: &MergeShape) -> Vec<(Vec<Pt>, bool)> {
    let mut v: Vec<(Vec<Pt>, bool)> = m.strokes.iter().map(|s| (s.pts.clone(), s.closed)).collect();
    v.extend(m.fills.iter().flat_map(|r| r.contours.iter().map(|c| (c.clone(), true))));
    v
}

/// Subdivide every edge whose box `touched` reports into pieces about `step`
/// long. Equal subdivision gives the same points on a fill contour and the
/// line along it, so they stay glued.
pub fn densify_merge(m: &mut MergeShape, touched: impl Fn(Pt, Pt) -> bool, step: f32) {
    let fix = |c: &[Pt], closed: bool| -> Vec<Pt> {
        let n = c.len();
        if n < 2 {
            return c.to_vec();
        }
        let segs = if closed { n } else { n - 1 };
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            out.push(c[i]);
            if i >= segs {
                continue;
            }
            let (a, b) = (c[i], c[(i + 1) % n]);
            let lo = pt(a.x.min(b.x), a.y.min(b.y));
            let hi = pt(a.x.max(b.x), a.y.max(b.y));
            if !touched(lo, hi) {
                continue;
            }
            let k = (a.dist(b) / step.max(0.25)).ceil().clamp(1.0, 200.0) as usize;
            for j in 1..k {
                out.push(a.lerp(b, j as f32 / k as f32));
            }
        }
        out
    };
    for r in &mut m.fills {
        for c in &mut r.contours {
            *c = fix(c, true);
        }
    }
    for s in &mut m.strokes {
        let closed = s.closed;
        s.pts = fix(&s.pts, closed);
    }
}

/// Move vertices by a position-keyed table (others stay).
fn apply_table(m: &mut MergeShape, table: &HashMap<(i64, i64), Pt>) {
    m.map_points(|x| table.get(&key(x)).copied().unwrap_or(x));
}

pub fn magnet_merge(base: &MergeShape, c: Pt, r: f32, delta: Pt) -> MergeShape {
    let mut m = base.clone();
    densify_merge(&mut m, circle_box(c, r), step_for(r));
    m.map_points(|p| {
        let k = falloff(p.dist(c), r);
        pt(p.x + delta.x * k, p.y + delta.y * k)
    });
    m
}

pub fn pinch_merge(base: &MergeShape, grab: Pt, tol: f32, reach: f32, sharp: bool, delta: Pt) -> Option<MergeShape> {
    let mut m = base.clone();
    densify_merge(&mut m, circle_box(grab, reach * 1.05), step_for(reach));
    let lines = polylines(&m);
    let mut best: Option<(f32, usize, f32, f32)> = None;
    for (i, (pts, closed)) in lines.iter().enumerate() {
        if pts.len() < 2 {
            continue;
        }
        let (d, at, total) = nearest_on(pts, *closed, grab);
        // lines win over fill edges at the same distance
        if d <= tol && best.map(|b| d < b.0 - 1e-3).unwrap_or(true) {
            best = Some((d, i, at, total));
        }
    }
    let (_, li, at, total) = best?;
    let (pts, closed) = &lines[li];
    let mut table = HashMap::new();
    let mut s = 0.0;
    for (i, p) in pts.iter().enumerate() {
        if i > 0 {
            s += pts[i - 1].dist(*p);
        }
        let mut ds = (s - at).abs();
        if *closed {
            ds = ds.min(total - ds);
        }
        let k = pinch_weight(ds / reach.max(1e-3), sharp);
        if k > 0.0 {
            table.insert(key(*p), pt(p.x + delta.x * k, p.y + delta.y * k));
        }
    }
    apply_table(&mut m, &table);
    Some(m)
}

pub fn bend_merge(base: &MergeShape, a: Pt, b: Pt, bm: &BendMap) -> MergeShape {
    let mut m = base.clone();
    densify_merge(&mut m, |lo, hi| bm.bends(lo, hi), step_for(bm.band * 4.0));
    let mut table = HashMap::new();
    let mut add = |pts: &[Pt], pivot: Pt| {
        let bm = BendMap { p0: pivot, ..*bm };
        for p in pts {
            table.entry(key(*p)).or_insert_with(|| bm.apply(*p));
        }
    };
    for s in &m.strokes {
        if let Some(x) = crossing(&s.pts, s.closed, a, b) {
            add(&s.pts, x);
        }
    }
    for r in &m.fills {
        if let Some(x) = r.contours.iter().find_map(|c| crossing(c, true, a, b)) {
            for c in &r.contours {
                add(c, x);
            }
        }
    }
    apply_table(&mut m, &table);
    m
}

pub fn iron_merge(cur: &MergeShape, c: Pt, r: f32, strength: f32) -> MergeShape {
    let mut m = cur.clone();
    densify_merge(&mut m, circle_box(c, r), step_for(r));
    let mut table = HashMap::new();
    for (pts, closed) in polylines(&m) {
        if pts.len() < 3 || !pts.iter().any(|p| p.dist(c) < r) {
            continue;
        }
        let sm = smooth_polyline(&pts, closed, iron_window(r), |p| falloff(p.dist(c), r) * strength);
        for (p, q) in pts.iter().zip(sm) {
            if p.dist(q) > 1e-5 {
                table.entry(key(*p)).or_insert(q);
            }
        }
    }
    apply_table(&mut m, &table);
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line() -> Path {
        Path { segs: vec![Seg::M(pt(0.0, 0.0)), Seg::L(pt(200.0, 0.0))] }
    }

    #[test]
    fn magnet_moves_only_nearby_points() {
        let p = magnet_path(&line(), pt(100.0, 0.0), 30.0, pt(0.0, 20.0));
        let (lo, hi) = p.bounds().unwrap();
        assert!((hi.y - 20.0).abs() < 0.5, "peak follows the cursor: {hi:?}");
        assert_eq!(lo.y, 0.0);
        // ends untouched
        assert_eq!(p.segs.first(), Some(&Seg::M(pt(0.0, 0.0))));
        assert!(matches!(p.segs.last(), Some(Seg::L(e)) if e.dist(pt(200.0, 0.0)) < 1e-4));
        // a curve far away keeps its control points
        let far = Path { segs: vec![Seg::M(pt(0.0, 100.0)), Seg::C(pt(30.0, 80.0), pt(60.0, 120.0), pt(90.0, 100.0))] };
        assert_eq!(magnet_path(&far, pt(300.0, 0.0), 30.0, pt(5.0, 5.0)), far);
    }

    #[test]
    fn pinch_follows_the_line_and_sharp_makes_a_corner() {
        let p = pinch_path(&line(), pt(100.0, 1.0), 4.0, 40.0, false, pt(0.0, -25.0)).unwrap();
        let (lo, _) = p.bounds().unwrap();
        assert!((lo.y + 25.0).abs() < 0.6);
        let at = |path: &Path, x: f32| -> f32 {
            path.flatten(1).into_iter().flat_map(|(v, _)| v).min_by(|a, b| (a.x - x).abs().partial_cmp(&(b.x - x).abs()).unwrap()).unwrap().y
        };
        assert!(at(&p, 50.0).abs() < 0.01 && at(&p, 150.0).abs() < 0.01, "outside the reach");
        let sharp = pinch_path(&line(), pt(100.0, 1.0), 4.0, 40.0, true, pt(0.0, -25.0)).unwrap();
        // halfway along the reach: cosine gives 50%, linear 50% too; at a quarter they differ
        assert!((at(&sharp, 90.0) + 25.0 * 0.75).abs() < 1.0);
        assert!(at(&p, 90.0) < at(&sharp, 90.0) - 0.5, "smooth stays fuller near the grab");
        assert!(pinch_path(&line(), pt(100.0, 50.0), 4.0, 40.0, false, pt(0.0, 1.0)).is_none());
    }

    #[test]
    fn bender_rotates_beyond_the_segment() {
        let bm = BendMap { p0: pt(100.0, -20.0), dir: pt(0.0, 40.0), versus: 0.0, angle: 0.0, band: BENDER_BAND };
        // the side of x > 100: cross((0,1), (x-100, ..)) = -(x-100) → versus = -1
        let bm = BendMap { versus: -1.0, angle: std::f32::consts::FRAC_PI_2, ..bm };
        let p = bend_path(&line(), pt(100.0, -20.0), pt(100.0, 20.0), &bm);
        // (200, 0) turns a quarter round where the line crosses the segment, (100, 0)
        let end = p.flatten(1).last().unwrap().0.last().copied().unwrap();
        assert!(end.dist(pt(100.0, 100.0)) < 0.5, "far end rotated round: {end:?}");
        assert_eq!(p.bounds().unwrap().0.x, 0.0, "the near side stays");
        // a line that doesn't cross the segment stays
        let other = Path { segs: vec![Seg::M(pt(120.0, 60.0)), Seg::L(pt(180.0, 60.0))] };
        assert_eq!(bend_path(&other, pt(100.0, -20.0), pt(100.0, 20.0), &bm), other);
    }

    #[test]
    fn iron_smooths_wiggles() {
        let zig: Vec<Seg> = std::iter::once(Seg::M(pt(0.0, 0.0))).chain((1..=20).map(|i| Seg::L(pt(i as f32 * 5.0, if i % 2 == 0 { 0.0 } else { 6.0 })))).collect();
        let mut p = Path { segs: zig };
        for _ in 0..6 {
            p = iron_path(&p, pt(50.0, 3.0), 40.0, 1.0);
        }
        let ys: Vec<f32> = p.flatten(1).into_iter().flat_map(|(v, _)| v).filter(|q| (q.x - 50.0).abs() < 15.0).map(|q| q.y).collect();
        let spread = ys.iter().cloned().fold(f32::MIN, f32::max) - ys.iter().cloned().fold(f32::MAX, f32::min);
        assert!(spread < 2.0, "wiggles ironed out near the cursor: {spread}");
    }

    #[test]
    fn pump_profile_thickens_locally() {
        let p = pump_profile(None, 0.5, 0.2, 2.0);
        let at = |t| crate::deform::profile_at(&p, t);
        assert!((at(0.5) - 2.0).abs() < 1e-3 && (at(0.1) - 1.0).abs() < 1e-3 && (at(0.95) - 1.0).abs() < 1e-3);
        assert!(pump_factor(-40.0) > 2.5 && pump_factor(40.0) < 0.5);
    }

    #[test]
    fn merge_deformations_keep_fills_and_lines_together() {
        let mut m = MergeShape::default();
        let sq = vec![pt(0.0, 0.0), pt(100.0, 0.0), pt(100.0, 100.0), pt(0.0, 100.0)];
        m.add_fill(std::slice::from_ref(&sq), Some(crate::model::Color::rgb(255, 0, 0)), None, false);
        m.add_stroke(&sq, true, crate::model::Stroke { color: crate::model::Color::rgb(0, 0, 0), width: 2.0 });
        let glued = |m: &MergeShape| {
            let fill: Vec<(i64, i64)> = m.fills.iter().flat_map(|r| r.contours.iter().flatten()).map(|p| key(*p)).collect();
            m.strokes.iter().flat_map(|s| s.pts.iter()).all(|p| fill.contains(&key(*p)))
        };
        let a = magnet_merge(&m, pt(50.0, 0.0), 30.0, pt(0.0, -20.0));
        assert!(glued(&a), "magnet");
        assert!(a.bounds().unwrap().0.y < -19.0);
        let b = pinch_merge(&m, pt(50.0, 0.5), 3.0, 30.0, true, pt(0.0, -20.0)).unwrap();
        assert!(glued(&b), "pinch");
        assert!(b.bounds().unwrap().0.y < -19.0);
        let bm = BendMap { p0: pt(50.0, -10.0), dir: pt(0.0, 120.0), versus: -1.0, angle: 0.3, band: BENDER_BAND };
        let c = bend_merge(&m, pt(50.0, -10.0), pt(50.0, 110.0), &bm);
        assert!(glued(&c), "bender");
        let d = iron_merge(&a, pt(50.0, -10.0), 30.0, 1.0);
        assert!(glued(&d), "iron");
    }
}
