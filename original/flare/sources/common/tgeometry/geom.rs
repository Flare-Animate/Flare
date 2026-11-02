//! Geometry primitives: points, Flash-style affine matrices, vector paths.

use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Pt {
    pub x: f32,
    pub y: f32,
}

pub const fn pt(x: f32, y: f32) -> Pt {
    Pt { x, y }
}

impl Pt {
    pub fn lerp(self, o: Pt, t: f32) -> Pt {
        pt(self.x + (o.x - self.x) * t, self.y + (o.y - self.y) * t)
    }
    pub fn dist(self, o: Pt) -> f32 {
        ((self.x - o.x).powi(2) + (self.y - o.y).powi(2)).sqrt()
    }
}

/// Affine matrix in Flash/Animate layout:
/// `x' = a*x + c*y + tx`, `y' = b*x + d*y + ty`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Matrix {
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub tx: f32,
    pub ty: f32,
}

impl Default for Matrix {
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// Matrix decomposed the way Animate's classic tween sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Decomposed {
    pub tx: f32,
    pub ty: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    /// radians
    pub skew_x: f32,
    /// radians; equals rotation when there is no skew
    pub skew_y: f32,
}

impl Matrix {
    pub const IDENTITY: Matrix = Matrix { a: 1.0, b: 0.0, c: 0.0, d: 1.0, tx: 0.0, ty: 0.0 };

    pub fn translate(tx: f32, ty: f32) -> Self {
        Matrix { tx, ty, ..Self::IDENTITY }
    }
    pub fn scale(sx: f32, sy: f32) -> Self {
        Matrix { a: sx, d: sy, ..Self::IDENTITY }
    }
    pub fn rotate(rad: f32) -> Self {
        let (s, c) = rad.sin_cos();
        Matrix { a: c, b: s, c: -s, d: c, tx: 0.0, ty: 0.0 }
    }

    pub fn apply(&self, p: Pt) -> Pt {
        pt(self.a * p.x + self.c * p.y + self.tx, self.b * p.x + self.d * p.y + self.ty)
    }

    /// `self * other`: apply `other` first, then `self`.
    pub fn concat(&self, o: &Matrix) -> Matrix {
        Matrix {
            a: self.a * o.a + self.c * o.b,
            b: self.b * o.a + self.d * o.b,
            c: self.a * o.c + self.c * o.d,
            d: self.b * o.c + self.d * o.d,
            tx: self.a * o.tx + self.c * o.ty + self.tx,
            ty: self.b * o.tx + self.d * o.ty + self.ty,
        }
    }

    pub fn invert(&self) -> Option<Matrix> {
        let det = self.a * self.d - self.b * self.c;
        if det.abs() < 1e-9 {
            return None;
        }
        let id = 1.0 / det;
        Some(Matrix {
            a: self.d * id,
            b: -self.b * id,
            c: -self.c * id,
            d: self.a * id,
            tx: (self.c * self.ty - self.d * self.tx) * id,
            ty: (self.b * self.tx - self.a * self.ty) * id,
        })
    }

    pub fn decompose(&self) -> Decomposed {
        let scale_x = (self.a * self.a + self.b * self.b).sqrt();
        let mut scale_y = (self.c * self.c + self.d * self.d).sqrt();
        let det = self.a * self.d - self.b * self.c;
        let skew_y = self.b.atan2(self.a);
        let mut skew_x = (-self.c).atan2(self.d);
        if det < 0.0 {
            // a mirrored matrix: keep scale_x positive, flip y
            scale_y = -scale_y;
            skew_x = (self.c).atan2(-self.d);
        }
        Decomposed { tx: self.tx, ty: self.ty, scale_x, scale_y, skew_x, skew_y }
    }

    pub fn compose(d: &Decomposed) -> Matrix {
        Matrix {
            a: d.scale_x * d.skew_y.cos(),
            b: d.scale_x * d.skew_y.sin(),
            c: -d.scale_y * d.skew_x.sin(),
            d: d.scale_y * d.skew_x.cos(),
            tx: d.tx,
            ty: d.ty,
        }
    }

    /// Classic-tween interpolation: decompose both ends, interpolate scale,
    /// skew (shortest way round, plus `extra_turns` full CW rotations) and translation.
    pub fn interpolate(&self, o: &Matrix, t: f32, extra_turns: i32) -> Matrix {
        self.interpolate_split(o, t, t, t, extra_turns)
    }
    /// Like `interpolate`, with separate progress for position, rotation/skew
    /// and scale (classic tweens with per-property eases).
    pub fn interpolate_split(&self, o: &Matrix, pos: f32, rot: f32, scale: f32, extra_turns: i32) -> Matrix {
        let a = self.decompose();
        let b = o.decompose();
        let turn = extra_turns as f32 * 2.0 * PI;
        let ang = |x: f32, y: f32| x + (shortest(x, y) + turn) * rot;
        Matrix::compose(&Decomposed {
            tx: a.tx + (b.tx - a.tx) * pos,
            ty: a.ty + (b.ty - a.ty) * pos,
            scale_x: a.scale_x + (b.scale_x - a.scale_x) * scale,
            scale_y: a.scale_y + (b.scale_y - a.scale_y) * scale,
            skew_x: ang(a.skew_x, b.skew_x),
            skew_y: ang(a.skew_y, b.skew_y),
        })
    }
}

fn shortest(from: f32, to: f32) -> f32 {
    let mut d = (to - from) % (2.0 * PI);
    if d > PI {
        d -= 2.0 * PI;
    }
    if d < -PI {
        d += 2.0 * PI;
    }
    d
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Seg {
    M(Pt),
    L(Pt),
    Q(Pt, Pt),
    C(Pt, Pt, Pt),
    Z,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Path {
    pub segs: Vec<Seg>,
}

impl Path {
    pub fn rect(x: f32, y: f32, w: f32, h: f32) -> Path {
        Path {
            segs: vec![
                Seg::M(pt(x, y)),
                Seg::L(pt(x + w, y)),
                Seg::L(pt(x + w, y + h)),
                Seg::L(pt(x, y + h)),
                Seg::Z,
            ],
        }
    }

    /// Ellipse from 4 cubic arcs.
    pub fn ellipse(cx: f32, cy: f32, rx: f32, ry: f32) -> Path {
        const K: f32 = 0.552_284_8;
        let (ox, oy) = (rx * K, ry * K);
        Path {
            segs: vec![
                Seg::M(pt(cx + rx, cy)),
                Seg::C(pt(cx + rx, cy + oy), pt(cx + ox, cy + ry), pt(cx, cy + ry)),
                Seg::C(pt(cx - ox, cy + ry), pt(cx - rx, cy + oy), pt(cx - rx, cy)),
                Seg::C(pt(cx - rx, cy - oy), pt(cx - ox, cy - ry), pt(cx, cy - ry)),
                Seg::C(pt(cx + ox, cy - ry), pt(cx + rx, cy - oy), pt(cx + rx, cy)),
                Seg::Z,
            ],
        }
    }

    pub fn polygon(points: &[Pt]) -> Path {
        let mut segs = Vec::with_capacity(points.len() + 1);
        for (i, p) in points.iter().enumerate() {
            segs.push(if i == 0 { Seg::M(*p) } else { Seg::L(*p) });
        }
        segs.push(Seg::Z);
        Path { segs }
    }

    pub fn star(cx: f32, cy: f32, r_out: f32, r_in: f32, points: usize) -> Path {
        let n = points * 2;
        let pts: Vec<Pt> = (0..n)
            .map(|i| {
                let r = if i % 2 == 0 { r_out } else { r_in };
                let a = -PI / 2.0 + i as f32 * PI / points as f32;
                pt(cx + r * a.cos(), cy + r * a.sin())
            })
            .collect();
        Path::polygon(&pts)
    }

    /// Smooth open curve through freehand points (pencil "smooth" mode):
    /// quadratic segments through the midpoints of a simplified polyline.
    pub fn smooth_polyline(raw: &[Pt], tolerance: f32) -> Path {
        let pts = simplify(raw, tolerance);
        let mut segs = Vec::new();
        if pts.is_empty() {
            return Path { segs };
        }
        segs.push(Seg::M(pts[0]));
        if pts.len() < 3 {
            for p in &pts[1..] {
                segs.push(Seg::L(*p));
            }
            return Path { segs };
        }
        for i in 1..pts.len() - 1 {
            let mid = pts[i].lerp(pts[i + 1], 0.5);
            let end = if i == pts.len() - 2 { pts[i + 1] } else { mid };
            segs.push(Seg::Q(pts[i], end));
        }
        Path { segs }
    }

    pub fn transformed(&self, m: &Matrix) -> Path {
        let f = |p: &Pt| m.apply(*p);
        Path {
            segs: self
                .segs
                .iter()
                .map(|s| match s {
                    Seg::M(p) => Seg::M(f(p)),
                    Seg::L(p) => Seg::L(f(p)),
                    Seg::Q(a, b) => Seg::Q(f(a), f(b)),
                    Seg::C(a, b, c) => Seg::C(f(a), f(b), f(c)),
                    Seg::Z => Seg::Z,
                })
                .collect(),
        }
    }

    /// Flatten into polylines (one per subpath). `closed` flag per polyline.
    pub fn flatten(&self, steps: usize) -> Vec<(Vec<Pt>, bool)> {
        let mut out: Vec<(Vec<Pt>, bool)> = Vec::new();
        let mut cur: Vec<Pt> = Vec::new();
        let mut last = Pt::default();
        let mut start = Pt::default();
        for s in &self.segs {
            match *s {
                Seg::M(p) => {
                    if cur.len() > 1 {
                        out.push((std::mem::take(&mut cur), false));
                    }
                    cur.clear();
                    cur.push(p);
                    last = p;
                    start = p;
                }
                Seg::L(p) => {
                    cur.push(p);
                    last = p;
                }
                Seg::Q(c, p) => {
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        cur.push(last.lerp(c, t).lerp(c.lerp(p, t), t));
                    }
                    last = p;
                }
                Seg::C(c1, c2, p) => {
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        cur.push(cubic_at(last, c1, c2, p, t));
                    }
                    last = p;
                }
                Seg::Z => {
                    if cur.len() > 1 {
                        out.push((std::mem::take(&mut cur), true));
                    }
                    last = start;
                }
            }
        }
        if cur.len() > 1 {
            out.push((cur, false));
        }
        out
    }

    pub fn bounds(&self) -> Option<(Pt, Pt)> {
        let mut it = self.flatten(8).into_iter().flat_map(|(v, _)| v);
        let first = it.next()?;
        let (mut lo, mut hi) = (first, first);
        for p in it {
            lo = pt(lo.x.min(p.x), lo.y.min(p.y));
            hi = pt(hi.x.max(p.x), hi.y.max(p.y));
        }
        Some((lo, hi))
    }

    /// Hit test in path-local coordinates: inside a closed subpath (even-odd),
    /// or within `stroke_tol` of any edge.
    pub fn hit(&self, p: Pt, filled: bool, stroke_tol: f32) -> bool {
        for (poly, closed) in self.flatten(12) {
            if filled && closed && point_in_poly(&poly, p) {
                return true;
            }
            let n = poly.len();
            let edges = if closed { n } else { n - 1 };
            for i in 0..edges {
                if dist_to_seg(p, poly[i], poly[(i + 1) % n]) <= stroke_tol {
                    return true;
                }
            }
        }
        false
    }

    /// Normalise to subpaths made only of cubic segments (used by shape tweens).
    pub fn to_cubic_subpaths(&self) -> Vec<CubicSubpath> {
        let mut out = Vec::new();
        let mut cur: Option<CubicSubpath> = None;
        let mut last = Pt::default();
        for s in &self.segs {
            match *s {
                Seg::M(p) => {
                    if let Some(c) = cur.take() {
                        out.push(c);
                    }
                    cur = Some(CubicSubpath { start: p, curves: vec![], closed: false });
                    last = p;
                }
                Seg::L(p) => {
                    if let Some(c) = cur.as_mut() {
                        c.curves.push([last.lerp(p, 1.0 / 3.0), last.lerp(p, 2.0 / 3.0), p]);
                    }
                    last = p;
                }
                Seg::Q(q, p) => {
                    if let Some(c) = cur.as_mut() {
                        c.curves.push([last.lerp(q, 2.0 / 3.0), p.lerp(q, 2.0 / 3.0), p]);
                    }
                    last = p;
                }
                Seg::C(a, b, p) => {
                    if let Some(c) = cur.as_mut() {
                        c.curves.push([a, b, p]);
                    }
                    last = p;
                }
                Seg::Z => {
                    if let Some(mut c) = cur.take() {
                        if last.dist(c.start) > 1e-4 {
                            let s = c.start;
                            c.curves.push([last.lerp(s, 1.0 / 3.0), last.lerp(s, 2.0 / 3.0), s]);
                        }
                        c.closed = true;
                        last = c.start;
                        out.push(c);
                    }
                }
            }
        }
        if let Some(c) = cur {
            out.push(c);
        }
        out
    }

    pub fn from_cubic_subpaths(subs: &[CubicSubpath]) -> Path {
        let mut segs = Vec::new();
        for s in subs {
            segs.push(Seg::M(s.start));
            for c in &s.curves {
                segs.push(Seg::C(c[0], c[1], c[2]));
            }
            if s.closed {
                segs.push(Seg::Z);
            }
        }
        Path { segs }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CubicSubpath {
    pub start: Pt,
    pub curves: Vec<[Pt; 3]>,
    pub closed: bool,
}

impl CubicSubpath {
    fn curve_len(&self, i: usize) -> f32 {
        let p0 = if i == 0 { self.start } else { self.curves[i - 1][2] };
        let c = &self.curves[i];
        p0.dist(c[0]) + c[0].dist(c[1]) + c[1].dist(c[2])
    }

    /// Split the longest curve in half (de Casteljau) until `n` curves exist.
    pub fn subdivide_to(&mut self, n: usize) {
        while self.curves.len() < n {
            if self.curves.is_empty() {
                let s = self.start;
                self.curves.push([s, s, s]);
                continue;
            }
            let (i, _) = (0..self.curves.len())
                .map(|i| (i, self.curve_len(i)))
                .fold((0, -1.0), |acc, x| if x.1 > acc.1 { x } else { acc });
            let p0 = if i == 0 { self.start } else { self.curves[i - 1][2] };
            let [c1, c2, p3] = self.curves[i];
            let m01 = p0.lerp(c1, 0.5);
            let m12 = c1.lerp(c2, 0.5);
            let m23 = c2.lerp(p3, 0.5);
            let a = m01.lerp(m12, 0.5);
            let b = m12.lerp(m23, 0.5);
            let mid = a.lerp(b, 0.5);
            self.curves[i] = [m01, a, mid];
            self.curves.insert(i + 1, [b, m23, p3]);
        }
    }

    /// Rotate the start of a closed subpath so its anchors best match `other`'s
    /// (automatic correspondence; a stand-in until shape hints exist).
    fn align_to(&mut self, other: &CubicSubpath) {
        let n = self.curves.len();
        if n < 2 || n != other.curves.len() {
            return;
        }
        let anchors_other: Vec<Pt> = other.curves.iter().map(|c| c[2]).collect();
        let best = (0..n)
            .map(|k| {
                let cost: f32 = (0..n).map(|i| {
                    let p = self.curves[(i + k) % n][2];
                    let q = anchors_other[i];
                    (p.x - q.x).powi(2) + (p.y - q.y).powi(2)
                }).sum();
                (k, cost)
            })
            .fold((0, f32::MAX), |a, b| if b.1 < a.1 { b } else { a })
            .0;
        if best != 0 {
            self.curves.rotate_left(best);
            self.start = self.curves[n - 1][2];
        }
    }

    fn centroid(&self) -> Pt {
        let n = self.curves.len() as f32 + 1.0;
        let mut s = self.start;
        for c in &self.curves {
            s.x += c[2].x;
            s.y += c[2].y;
        }
        pt(s.x / n, s.y / n)
    }

    fn collapsed(at: Pt, n: usize, closed: bool) -> CubicSubpath {
        CubicSubpath { start: at, curves: vec![[at, at, at]; n], closed }
    }
}

/// Morph two paths (shape tween). Subpath and curve counts are equalised first
/// (Animate uses shape hints for correspondence; that is a later milestone).
pub fn morph_paths(a: &Path, b: &Path, t: f32) -> Path {
    let mut sa = a.to_cubic_subpaths();
    let mut sb = b.to_cubic_subpaths();
    while sa.len() < sb.len() {
        let src = &sb[sa.len()];
        let at = sa.last().map(|s| s.centroid()).unwrap_or(src.centroid());
        sa.push(CubicSubpath::collapsed(at, src.curves.len(), src.closed));
    }
    while sb.len() < sa.len() {
        let src = &sa[sb.len()];
        let at = sb.last().map(|s| s.centroid()).unwrap_or(src.centroid());
        sb.push(CubicSubpath::collapsed(at, src.curves.len(), src.closed));
    }
    let mut out = Vec::with_capacity(sa.len());
    for (x, y) in sa.iter_mut().zip(sb.iter_mut()) {
        let n = x.curves.len().max(y.curves.len());
        x.subdivide_to(n);
        y.subdivide_to(n);
        if x.closed && y.closed {
            y.align_to(x);
        }
        out.push(CubicSubpath {
            start: x.start.lerp(y.start, t),
            curves: x
                .curves
                .iter()
                .zip(&y.curves)
                .map(|(p, q)| [p[0].lerp(q[0], t), p[1].lerp(q[1], t), p[2].lerp(q[2], t)])
                .collect(),
            closed: x.closed || y.closed,
        });
    }
    Path::from_cubic_subpaths(&out)
}

impl CubicSubpath {
    fn anchor_before(&self, i: usize) -> Pt {
        if i == 0 {
            self.start
        } else {
            self.curves[i - 1][2]
        }
    }
    /// Nearest point to `p`: (curve index, parameter, distance).
    fn nearest(&self, p: Pt) -> (usize, f32, f32) {
        let mut best = (0, 0.0, f32::MAX);
        for i in 0..self.curves.len() {
            let p0 = self.anchor_before(i);
            let [c1, c2, p3] = self.curves[i];
            for k in 0..=32 {
                let s = k as f32 / 32.0;
                let d = cubic_at(p0, c1, c2, p3, s).dist(p);
                if d < best.2 {
                    best = (i, s, d);
                }
            }
        }
        best
    }
    /// Make the point nearest `p` an anchor; returns its curve index (the
    /// curve that now ends there).
    fn split_near(&mut self, p: Pt) -> usize {
        let (i, s, _) = self.nearest(p);
        if s < 0.02 {
            return if i == 0 { self.curves.len() - 1 } else { i - 1 };
        }
        if s > 0.98 {
            return i;
        }
        let p0 = self.anchor_before(i);
        let [c1, c2, p3] = self.curves[i];
        let m01 = p0.lerp(c1, s);
        let m12 = c1.lerp(c2, s);
        let m23 = c2.lerp(p3, s);
        let a = m01.lerp(m12, s);
        let b = m12.lerp(m23, s);
        let mid = a.lerp(b, s);
        self.curves[i] = [m01, a, mid];
        self.curves.insert(i + 1, [b, m23, p3]);
        i
    }
}

/// Shape tween with shape hints: hint i on `a` morphs onto hint i on `b`.
/// Hints apply to the first subpath (as Animate's hints apply to one shape);
/// further subpaths use the automatic correspondence.
pub fn morph_paths_hinted(a: &Path, b: &Path, t: f32, ha: &[Pt], hb: &[Pt]) -> Path {
    let mut sa = a.to_cubic_subpaths();
    let mut sb = b.to_cubic_subpaths();
    if sa.is_empty() || sb.is_empty() || ha.is_empty() || !sa[0].closed || !sb[0].closed {
        return morph_paths(a, b, t);
    }
    let (x, y) = (sa.remove(0), sb.remove(0));
    let (x, y) = hinted_pair(x, y, ha, hb);
    let first = CubicSubpath {
        start: x.start.lerp(y.start, t),
        curves: x.curves.iter().zip(&y.curves).map(|(p, q)| [p[0].lerp(q[0], t), p[1].lerp(q[1], t), p[2].lerp(q[2], t)]).collect(),
        closed: true,
    };
    let rest = morph_paths(&Path::from_cubic_subpaths(&sa), &Path::from_cubic_subpaths(&sb), t);
    let mut out = vec![first];
    out.extend(rest.to_cubic_subpaths());
    Path::from_cubic_subpaths(&out)
}

/// Rebuild two closed subpaths so their hint points are anchors at matching
/// curve indices (equal curve counts between consecutive hints).
fn hinted_pair(mut x: CubicSubpath, mut y: CubicSubpath, ha: &[Pt], hb: &[Pt]) -> (CubicSubpath, CubicSubpath) {
    // hint order along `a`
    let mut order: Vec<usize> = (0..ha.len()).collect();
    let pos = |sp: &CubicSubpath, p: Pt| {
        let (i, s, _) = sp.nearest(p);
        i as f32 + s
    };
    order.sort_by(|&i, &j| pos(&x, ha[i]).total_cmp(&pos(&x, ha[j])));
    let anchor_pts = |sp: &mut CubicSubpath, hs: &[Pt]| -> Vec<Pt> {
        let mut pts = vec![];
        for &h in hs {
            let i = sp.split_near(h);
            pts.push(sp.curves[i][2]);
        }
        pts
    };
    let pa = anchor_pts(&mut x, &order.iter().map(|&i| ha[i]).collect::<Vec<_>>());
    let pb = anchor_pts(&mut y, &order.iter().map(|&i| hb[i]).collect::<Vec<_>>());
    let segs = |sp: &CubicSubpath, pts: &[Pt]| -> Vec<CubicSubpath> {
        let n = sp.curves.len();
        let idx: Vec<usize> = pts.iter().map(|p| (0..n).min_by(|&i, &j| sp.curves[i][2].dist(*p).total_cmp(&sp.curves[j][2].dist(*p))).unwrap_or(0)).collect();
        let k = idx.len();
        (0..k)
            .map(|m| {
                let (from, to) = (idx[m], idx[(m + 1) % k]);
                let mut curves = vec![];
                let mut i = (from + 1) % n;
                let steps = if k == 1 { n } else { (to + n - from) % n };
                for _ in 0..steps {
                    curves.push(sp.curves[i]);
                    i = (i + 1) % n;
                }
                let start = sp.curves[from][2];
                if curves.is_empty() {
                    curves.push([start, start, start]);
                }
                CubicSubpath { start, curves, closed: false }
            })
            .collect()
    };
    let (mut xa, mut yb) = (segs(&x, &pa), segs(&y, &pb));
    let join = |parts: &[CubicSubpath]| CubicSubpath { start: parts[0].start, curves: parts.iter().flat_map(|p| p.curves.clone()).collect(), closed: true };
    for (p, q) in xa.iter_mut().zip(yb.iter_mut()) {
        let n = p.curves.len().max(q.curves.len());
        p.subdivide_to(n);
        q.subdivide_to(n);
    }
    (join(&xa), join(&yb))
}

fn cubic_at(p0: Pt, p1: Pt, p2: Pt, p3: Pt, t: f32) -> Pt {
    let u = 1.0 - t;
    let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    pt(
        a * p0.x + b * p1.x + c * p2.x + d * p3.x,
        a * p0.y + b * p1.y + c * p2.y + d * p3.y,
    )
}

/// Is `p` inside the polygon `poly` (even-odd)?
pub fn point_in_polygon(p: Pt, poly: &[Pt]) -> bool {
    point_in_poly(poly, p)
}

fn point_in_poly(poly: &[Pt], p: Pt) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

fn dist_to_seg(p: Pt, a: Pt, b: Pt) -> f32 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let len2 = dx * dx + dy * dy;
    if len2 < 1e-9 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * dx + (p.y - a.y) * dy) / len2).clamp(0.0, 1.0);
    p.dist(pt(a.x + dx * t, a.y + dy * t))
}

/// Ramer–Douglas–Peucker simplification.
pub fn simplify(pts: &[Pt], tol: f32) -> Vec<Pt> {
    if pts.len() < 3 {
        return pts.to_vec();
    }
    let (first, last) = (pts[0], pts[pts.len() - 1]);
    let (mut idx, mut max) = (0, 0.0);
    for (i, p) in pts.iter().enumerate().take(pts.len() - 1).skip(1) {
        let d = dist_to_seg(*p, first, last);
        if d > max {
            idx = i;
            max = d;
        }
    }
    if max > tol {
        let mut l = simplify(&pts[..=idx], tol);
        let r = simplify(&pts[idx..], tol);
        l.pop();
        l.extend(r);
        l
    } else {
        vec![first, last]
    }
}

// ------------------------------------------------------------------ shape builders

impl Path {
    /// Rectangle with rounded corners (Animate's Rectangle tool corner radius).
    pub fn rounded_rect(x: f32, y: f32, w: f32, h: f32, r: f32) -> Path {
        let r = r.max(0.0).min(w.abs() / 2.0).min(h.abs() / 2.0);
        if r < 0.01 {
            return Path::rect(x, y, w, h);
        }
        const K: f32 = 0.552_284_8;
        let k = r * (1.0 - K);
        let (x1, y1) = (x + w, y + h);
        Path {
            segs: vec![
                Seg::M(pt(x + r, y)),
                Seg::L(pt(x1 - r, y)),
                Seg::C(pt(x1 - k, y), pt(x1, y + k), pt(x1, y + r)),
                Seg::L(pt(x1, y1 - r)),
                Seg::C(pt(x1, y1 - k), pt(x1 - k, y1), pt(x1 - r, y1)),
                Seg::L(pt(x + r, y1)),
                Seg::C(pt(x + k, y1), pt(x, y1 - k), pt(x, y1 - r)),
                Seg::L(pt(x, y + r)),
                Seg::C(pt(x, y + k), pt(x + k, y), pt(x + r, y)),
                Seg::Z,
            ],
        }
    }

    /// Animate's PolyStar: a regular polygon, or a star whose inner points sit
    /// at `inner` (0..1) of the outer radius. First point faces up.
    pub fn polystar(cx: f32, cy: f32, rx: f32, ry: f32, sides: usize, star: bool, inner: f32) -> Path {
        let sides = sides.clamp(3, 32);
        let n = if star { sides * 2 } else { sides };
        let pts: Vec<Pt> = (0..n)
            .map(|i| {
                let f = if star && i % 2 == 1 { inner.clamp(0.05, 1.0) } else { 1.0 };
                let a = -PI / 2.0 + i as f32 * 2.0 * PI / n as f32;
                pt(cx + rx * f * a.cos(), cy + ry * f * a.sin())
            })
            .collect();
        Path::polygon(&pts)
    }
}

// ------------------------------------------------------------------ anchor editing (Subselection / Pen tools)

/// One editable point of a path: the anchor at the end of segment `seg`
/// and, for curves, the control handles that belong to that segment.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PointRef {
    /// End point of segment `seg` (or the M point).
    Anchor(usize),
    /// Control point `k` (0 or 1) of segment `seg`.
    Handle(usize, usize),
}

impl Path {
    /// All anchors as (segment index, point). The closing anchor of a closed
    /// subpath that repeats its start is skipped.
    pub fn anchors(&self) -> Vec<(usize, Pt)> {
        let mut out = Vec::new();
        let mut start: Option<(usize, Pt)> = None;
        for (i, s) in self.segs.iter().enumerate() {
            let p = match *s {
                Seg::M(p) => {
                    start = Some((i, p));
                    Some(p)
                }
                Seg::L(p) | Seg::Q(_, p) | Seg::C(_, _, p) => Some(p),
                Seg::Z => None,
            };
            if let Some(p) = p {
                // skip an explicit closing point that sits on the start (closed shapes)
                let closes = matches!(self.segs.get(i + 1), Some(Seg::Z)) && start.map(|(_, s)| s.dist(p) < 1e-3).unwrap_or(false) && !matches!(s, Seg::M(_));
                if !closes {
                    out.push((i, p));
                }
            }
        }
        out
    }

    /// Control handles as (segment, k, handle point, anchor it belongs to).
    pub fn handles(&self) -> Vec<(usize, usize, Pt, Pt)> {
        let mut out = Vec::new();
        let mut last = Pt::default();
        for (i, s) in self.segs.iter().enumerate() {
            match *s {
                Seg::M(p) | Seg::L(p) => last = p,
                Seg::Q(c, p) => {
                    out.push((i, 0, c, p));
                    last = p;
                }
                Seg::C(c1, c2, p) => {
                    out.push((i, 0, c1, last));
                    out.push((i, 1, c2, p));
                    last = p;
                }
                Seg::Z => {}
            }
        }
        out
    }

    pub fn get_point(&self, r: PointRef) -> Option<Pt> {
        match (r, self.segs.get(match r { PointRef::Anchor(i) | PointRef::Handle(i, _) => i })?) {
            (PointRef::Anchor(_), Seg::M(p) | Seg::L(p) | Seg::Q(_, p) | Seg::C(_, _, p)) => Some(*p),
            (PointRef::Handle(_, 0), Seg::Q(c, _)) => Some(*c),
            (PointRef::Handle(_, 0), Seg::C(c, _, _)) => Some(*c),
            (PointRef::Handle(_, 1), Seg::C(_, c, _)) => Some(*c),
            _ => None,
        }
    }

    /// Move a point. Moving an anchor drags its neighbouring handles along,
    /// and keeps a closed subpath closed when its start anchor moves.
    pub fn set_point(&mut self, r: PointRef, to: Pt) {
        match r {
            PointRef::Anchor(i) => {
                let Some(from) = self.get_point(r) else { return };
                let d = pt(to.x - from.x, to.y - from.y);
                let shift = |p: &mut Pt| {
                    p.x += d.x;
                    p.y += d.y;
                };
                match &mut self.segs[i] {
                    Seg::M(p) | Seg::L(p) => *p = to,
                    Seg::Q(_, p) => *p = to,
                    Seg::C(_, c2, p) => {
                        shift(c2);
                        *p = to;
                    }
                    Seg::Z => {}
                }
                if let Some(Seg::C(c1, _, _)) = self.segs.get_mut(i + 1) {
                    shift(c1);
                }
                // keep closed shapes closed: the last anchor before Z mirrors the M point
                if matches!(self.segs[i], Seg::M(_)) {
                    if let Some(z) = self.segs[i + 1..].iter().position(|s| matches!(s, Seg::Z | Seg::M(_))).map(|k| k + i + 1) {
                        if matches!(self.segs[z], Seg::Z) && z > i + 1 {
                            let last = z - 1;
                            let lp = self.get_point(PointRef::Anchor(last));
                            if lp.map(|p| p.dist(from) < 1e-3).unwrap_or(false) {
                                match &mut self.segs[last] {
                                    Seg::L(p) | Seg::Q(_, p) => *p = to,
                                    Seg::C(_, c2, p) => {
                                        shift(c2);
                                        *p = to;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
            PointRef::Handle(i, k) => match (&mut self.segs[i], k) {
                (Seg::Q(c, _), 0) => *c = to,
                (Seg::C(c, _, _), 0) => *c = to,
                (Seg::C(_, c, _), 1) => *c = to,
                _ => {}
            },
        }
    }

    /// Nearest editable point within `tol` (anchors win over handles).
    pub fn pick_point(&self, p: Pt, tol: f32, with_handles: bool) -> Option<PointRef> {
        let mut best: Option<(PointRef, f32)> = None;
        for (i, a) in self.anchors() {
            let d = a.dist(p);
            if d <= tol && best.map(|b| d < b.1).unwrap_or(true) {
                best = Some((PointRef::Anchor(i), d));
            }
        }
        if best.is_none() && with_handles {
            for (i, k, h, _) in self.handles() {
                let d = h.dist(p);
                if d <= tol && best.map(|b| d < b.1).unwrap_or(true) {
                    best = Some((PointRef::Handle(i, k), d));
                }
            }
        }
        best.map(|b| b.0)
    }

    /// Delete Anchor tool: remove the anchor at segment `i`, joining its neighbours.
    pub fn delete_anchor(&mut self, i: usize) -> bool {
        let n_anchors = self.anchors().len();
        if n_anchors <= 2 || i >= self.segs.len() {
            return false;
        }
        match self.segs[i] {
            Seg::M(_) => {
                // promote the next anchor to be the start
                if let Some(next) = self.segs.get(i + 1).copied() {
                    let p = match next {
                        Seg::L(p) | Seg::Q(_, p) | Seg::C(_, _, p) => p,
                        _ => return false,
                    };
                    self.segs[i] = Seg::M(p);
                    self.segs.remove(i + 1);
                    // a closed shape's closing anchor equals the old start: retarget it
                    if let Some(z) = self.segs[i + 1..].iter().position(|s| matches!(s, Seg::Z)).map(|k| k + i + 1) {
                        if z > i + 1 {
                            match &mut self.segs[z - 1] {
                                Seg::L(q) | Seg::Q(_, q) | Seg::C(_, _, q) => *q = p,
                                _ => {}
                            }
                        }
                    }
                    return true;
                }
                false
            }
            Seg::L(_) | Seg::Q(..) | Seg::C(..) => {
                let removed = self.segs.remove(i);
                // the following segment now starts from the previous anchor; keep it a straight join
                if let Some(next) = self.segs.get_mut(i) {
                    if let (Seg::C(_, _, _), Seg::C(c1, _, _)) = (removed, next) {
                        if let Seg::C(rc1, _, _) = removed {
                            *c1 = rc1;
                        }
                    }
                }
                true
            }
            Seg::Z => false,
        }
    }

    /// Add Anchor tool: split the segment nearest `p` (within `tol`) at that point.
    pub fn insert_anchor(&mut self, p: Pt, tol: f32) -> Option<usize> {
        let mut last = Pt::default();
        let mut start = Pt::default();
        let mut best: Option<(usize, f32, f32)> = None; // (seg, t, dist)
        for (i, s) in self.segs.iter().enumerate() {
            let samples = 24;
            match *s {
                Seg::M(q) => {
                    last = q;
                    start = q;
                    continue;
                }
                Seg::Z => {
                    last = start;
                    continue;
                }
                _ => {}
            }
            for k in 1..samples {
                let t = k as f32 / samples as f32;
                let q = seg_at(last, s, t);
                let d = q.dist(p);
                if d <= tol && best.map(|b| d < b.2).unwrap_or(true) {
                    best = Some((i, t, d));
                }
            }
            last = seg_end(s).unwrap_or(last);
        }
        let (i, t, _) = best?;
        let prev = self.seg_start(i);
        let (a, b) = split_seg(prev, &self.segs[i], t);
        self.segs[i] = a;
        self.segs.insert(i + 1, b);
        Some(i)
    }

    fn seg_start(&self, i: usize) -> Pt {
        let mut last = Pt::default();
        let mut start = Pt::default();
        for s in &self.segs[..i] {
            match *s {
                Seg::M(p) => {
                    last = p;
                    start = p;
                }
                Seg::L(p) | Seg::Q(_, p) | Seg::C(_, _, p) => last = p,
                Seg::Z => last = start,
            }
        }
        last
    }

    /// Convert Anchor tool: toggle the anchor at segment `i` between corner
    /// (straight joins) and smooth (curved joins with symmetric handles).
    pub fn convert_anchor(&mut self, i: usize) {
        let Some(a) = self.get_point(PointRef::Anchor(i)) else { return };
        let prev = self.seg_start(i);
        let next_i = i + 1;
        let next_end = self.segs.get(next_i).and_then(seg_end);
        let is_smooth = matches!(self.segs[i], Seg::C(..)) || matches!(self.segs.get(next_i), Some(Seg::C(..)));
        if is_smooth {
            if !matches!(self.segs[i], Seg::M(_)) {
                self.segs[i] = Seg::L(a);
            }
            if let (Some(Seg::C(..)), Some(e)) = (self.segs.get(next_i), next_end) {
                self.segs[next_i] = Seg::L(e);
            }
        } else {
            let dir = match next_end {
                Some(e) => pt((e.x - prev.x) / 6.0, (e.y - prev.y) / 6.0),
                None => pt((a.x - prev.x) / 3.0, (a.y - prev.y) / 3.0),
            };
            if !matches!(self.segs[i], Seg::M(_)) {
                self.segs[i] = Seg::C(prev.lerp(a, 1.0 / 3.0), pt(a.x - dir.x, a.y - dir.y), a);
            }
            if let (Some(Seg::L(_)), Some(e)) = (self.segs.get(next_i), next_end) {
                self.segs[next_i] = Seg::C(pt(a.x + dir.x, a.y + dir.y), a.lerp(e, 2.0 / 3.0), e);
            }
        }
    }
}

fn seg_end(s: &Seg) -> Option<Pt> {
    match *s {
        Seg::M(p) | Seg::L(p) | Seg::Q(_, p) | Seg::C(_, _, p) => Some(p),
        Seg::Z => None,
    }
}

fn seg_at(p0: Pt, s: &Seg, t: f32) -> Pt {
    match *s {
        Seg::L(p) => p0.lerp(p, t),
        Seg::Q(c, p) => p0.lerp(c, t).lerp(c.lerp(p, t), t),
        Seg::C(c1, c2, p) => cubic_at(p0, c1, c2, p, t),
        Seg::M(p) => p,
        Seg::Z => p0,
    }
}

fn split_seg(p0: Pt, s: &Seg, t: f32) -> (Seg, Seg) {
    match *s {
        Seg::L(p) => {
            let m = p0.lerp(p, t);
            (Seg::L(m), Seg::L(p))
        }
        Seg::Q(c, p) => {
            let a = p0.lerp(c, t);
            let b = c.lerp(p, t);
            let m = a.lerp(b, t);
            (Seg::Q(a, m), Seg::Q(b, p))
        }
        Seg::C(c1, c2, p) => {
            let a = p0.lerp(c1, t);
            let b = c1.lerp(c2, t);
            let c = c2.lerp(p, t);
            let ab = a.lerp(b, t);
            let bc = b.lerp(c, t);
            let m = ab.lerp(bc, t);
            (Seg::C(a, ab, m), Seg::C(bc, c, p))
        }
        other => (other, other),
    }
}

impl Path {
    /// Rectangle with a radius per corner (TL, TR, BR, BL); negative radii cut
    /// the corner inwards (Rectangle Primitive).
    pub fn rounded_rect4(x: f32, y: f32, w: f32, h: f32, radius: [f32; 4]) -> Path {
        let (x, w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
        let (y, h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
        let max = w.min(h) / 2.0;
        let r = radius.map(|v| v.clamp(-max, max));
        if r.iter().all(|v| v.abs() < 1e-3) {
            return Path::rect(x, y, w, h);
        }
        let k = 0.5523;
        // corner centres and the directions along the edges
        let corners = [(pt(x, y), pt(1.0, 0.0), pt(0.0, 1.0)), (pt(x + w, y), pt(0.0, 1.0), pt(-1.0, 0.0)), (pt(x + w, y + h), pt(-1.0, 0.0), pt(0.0, -1.0)), (pt(x, y + h), pt(0.0, -1.0), pt(1.0, 0.0))];
        let mut segs = vec![];
        for (i, (c, next_dir, prev_back)) in corners.iter().enumerate() {
            let rr = r[i];
            let a = rr.abs();
            // entry point (coming from the previous corner) and exit point
            let entry = pt(c.x + prev_back.x * a, c.y + prev_back.y * a);
            let exit = pt(c.x + next_dir.x * a, c.y + next_dir.y * a);
            if i == 0 {
                segs.push(Seg::M(entry));
            } else {
                segs.push(Seg::L(entry));
            }
            if a < 1e-3 {
                continue;
            }
            if rr > 0.0 {
                let c1 = pt(entry.x - prev_back.x * a * k, entry.y - prev_back.y * a * k);
                let c2 = pt(exit.x - next_dir.x * a * k, exit.y - next_dir.y * a * k);
                segs.push(Seg::C(c1, c2, exit));
            } else {
                // inverted corner: curve bulging into the rectangle
                let inner = pt(c.x + (next_dir.x + prev_back.x) * a, c.y + (next_dir.y + prev_back.y) * a);
                let c1 = entry.lerp(inner, k);
                let c2 = exit.lerp(inner, k);
                segs.push(Seg::C(c1, c2, exit));
            }
        }
        segs.push(Seg::Z);
        Path { segs }
    }

    /// Oval with start/end angle (degrees, clockwise from 3 o'clock) and inner
    /// radius in % (Oval tool options, Oval Primitive). `close` joins an open
    /// arc's ends to the centre (or inner arc).
    #[allow(clippy::too_many_arguments)]
    pub fn oval_arc(cx: f32, cy: f32, rx: f32, ry: f32, start: f32, end: f32, inner: f32, close: bool) -> Path {
        let mut sweep = (end - start).rem_euclid(360.0);
        let full = sweep < 0.01;
        if full {
            sweep = 360.0;
        }
        let inner = (inner / 100.0).clamp(0.0, 0.99);
        if full && inner <= 0.0 {
            return Path::ellipse(cx, cy, rx, ry);
        }
        let arc = |k: f32, a0: f32, sweep: f32| -> Vec<Pt> {
            let n = ((sweep / 4.0).ceil() as usize).max(2);
            (0..=n)
                .map(|i| {
                    let a = (a0 + sweep * i as f32 / n as f32).to_radians();
                    pt(cx + rx * k * a.cos(), cy + ry * k * a.sin())
                })
                .collect()
        };
        if full {
            let mut outer = arc(1.0, start, 360.0);
            outer.pop();
            let mut inn = arc(inner, start, 360.0);
            inn.pop();
            inn.reverse();
            let mut p = Path::polygon(&outer);
            p.segs.extend(Path::polygon(&inn).segs);
            return p;
        }
        let mut pts = arc(1.0, start, sweep);
        if !close {
            let mut segs = vec![Seg::M(pts[0])];
            segs.extend(pts[1..].iter().map(|x| Seg::L(*x)));
            return Path { segs };
        }
        if inner > 0.0 {
            let mut i = arc(inner, start, sweep);
            i.reverse();
            pts.extend(i);
        } else {
            pts.push(pt(cx, cy));
        }
        Path::polygon(&pts)
    }
}
