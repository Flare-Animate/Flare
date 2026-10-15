//! Geometry that bends shapes: Distort (bilinear quad), Envelope (3×3 mesh),
//! variable-width outlines (Fluid Brush, Width tool), Asset Warp (rigid
//! moving-least-squares deformation with pins) and 3D perspective projection.

use crate::geom::{pt, Matrix, Path, Pt, Seg};
use serde::{Deserialize, Serialize};

impl Path {
    /// Apply `f` to every point (anchors and control points).
    pub fn map(&self, f: impl Fn(Pt) -> Pt) -> Path {
        Path {
            segs: self
                .segs
                .iter()
                .map(|s| match *s {
                    Seg::M(p) => Seg::M(f(p)),
                    Seg::L(p) => Seg::L(f(p)),
                    Seg::Q(a, b) => Seg::Q(f(a), f(b)),
                    Seg::C(a, b, c) => Seg::C(f(a), f(b), f(c)),
                    Seg::Z => Seg::Z,
                })
                .collect(),
        }
    }

    /// Split curves/lines so no segment is longer than `max_len` (control
    /// polygon length). Non-linear warps of control points stay faithful.
    pub fn subdivided(&self, max_len: f32) -> Path {
        let max_len = max_len.max(0.5);
        let mut subs = self.to_cubic_subpaths();
        for s in &mut subs {
            let mut i = 0;
            let mut guard = 0;
            while i < s.curves.len() && guard < 10_000 {
                guard += 1;
                let p0 = if i == 0 { s.start } else { s.curves[i - 1][2] };
                let c = s.curves[i];
                let len = p0.dist(c[0]) + c[0].dist(c[1]) + c[1].dist(c[2]);
                if len > max_len {
                    let n = s.curves.len() + 1;
                    let before = s.curves.len();
                    // split this curve in half (subdivide_to splits the longest, so do it directly)
                    let m01 = p0.lerp(c[0], 0.5);
                    let m12 = c[0].lerp(c[1], 0.5);
                    let m23 = c[1].lerp(c[2], 0.5);
                    let a = m01.lerp(m12, 0.5);
                    let b = m12.lerp(m23, 0.5);
                    let mid = a.lerp(b, 0.5);
                    s.curves[i] = [m01, a, mid];
                    s.curves.insert(i + 1, [b, m23, c[2]]);
                    debug_assert_eq!(s.curves.len(), before + 1);
                    let _ = n;
                } else {
                    i += 1;
                }
            }
        }
        Path::from_cubic_subpaths(&subs)
    }
}

/// Bilinear point in a quad given as [top-left, top-right, bottom-right, bottom-left].
pub fn bilinear(q: &[Pt; 4], u: f32, v: f32) -> Pt {
    let top = q[0].lerp(q[1], u);
    let bot = q[3].lerp(q[2], u);
    top.lerp(bot, v)
}

/// Distort: map a path from the box `lo..hi` onto the quad `q`.
pub fn distort(path: &Path, lo: Pt, hi: Pt, q: &[Pt; 4]) -> Path {
    let (w, h) = ((hi.x - lo.x).max(1e-3), (hi.y - lo.y).max(1e-3));
    path.subdivided(w.max(h) / 8.0).map(|p| bilinear(q, (p.x - lo.x) / w, (p.y - lo.y) / h))
}

/// The 9 envelope handles of a box, row-major (top-left … bottom-right).
pub fn envelope_grid(lo: Pt, hi: Pt) -> [Pt; 9] {
    let mut g = [Pt::default(); 9];
    for r in 0..3 {
        for c in 0..3 {
            g[r * 3 + c] = pt(lo.x + (hi.x - lo.x) * c as f32 / 2.0, lo.y + (hi.y - lo.y) * r as f32 / 2.0);
        }
    }
    g
}

/// Envelope: smooth (biquadratic) warp that passes through the 9 handles.
pub fn envelope(path: &Path, lo: Pt, hi: Pt, handles: &[Pt; 9]) -> Path {
    // Convert interpolating handles to Bézier control points.
    let mut c = *handles;
    let mid = |a: Pt, m: Pt, b: Pt| pt(2.0 * m.x - (a.x + b.x) / 2.0, 2.0 * m.y - (a.y + b.y) / 2.0);
    c[1] = mid(handles[0], handles[1], handles[2]);
    c[7] = mid(handles[6], handles[7], handles[8]);
    c[3] = mid(handles[0], handles[3], handles[6]);
    c[5] = mid(handles[2], handles[5], handles[8]);
    // centre: solve B(0.5,0.5) = handle[4]
    let b = [0.25f32, 0.5, 0.25];
    let mut acc = pt(0.0, 0.0);
    for r in 0..3 {
        for k in 0..3 {
            if r == 1 && k == 1 {
                continue;
            }
            acc.x += b[r] * b[k] * c[r * 3 + k].x;
            acc.y += b[r] * b[k] * c[r * 3 + k].y;
        }
    }
    c[4] = pt((handles[4].x - acc.x) / 0.25, (handles[4].y - acc.y) / 0.25);
    let (w, h) = ((hi.x - lo.x).max(1e-3), (hi.y - lo.y).max(1e-3));
    path.subdivided(w.max(h) / 10.0).map(|p| {
        let (u, v) = ((p.x - lo.x) / w, (p.y - lo.y) / h);
        let bu = [(1.0 - u) * (1.0 - u), 2.0 * u * (1.0 - u), u * u];
        let bv = [(1.0 - v) * (1.0 - v), 2.0 * v * (1.0 - v), v * v];
        let mut q = pt(0.0, 0.0);
        for r in 0..3 {
            for k in 0..3 {
                let wgt = bv[r] * bu[k];
                q.x += wgt * c[r * 3 + k].x;
                q.y += wgt * c[r * 3 + k].y;
            }
        }
        q
    })
}

/// Filled outline of a polyline with a width per point (round caps).
/// Used by the Fluid Brush and by variable-width strokes (Width tool).
pub fn outline_polyline(pts: &[Pt], widths: &[f32], closed: bool) -> Path {
    let n = pts.len();
    if n == 0 {
        return Path::default();
    }
    let w = |i: usize| widths.get(i).copied().unwrap_or(1.0).max(0.05) / 2.0;
    if n == 1 {
        return Path::ellipse(pts[0].x, pts[0].y, w(0), w(0));
    }
    let normal = |i: usize| -> Pt {
        let (a, b) = if closed {
            (pts[(i + n - 1) % n], pts[(i + 1) % n])
        } else {
            (pts[i.saturating_sub(1)], pts[(i + 1).min(n - 1)])
        };
        let (dx, dy) = (b.x - a.x, b.y - a.y);
        let len = (dx * dx + dy * dy).sqrt().max(1e-6);
        pt(-dy / len, dx / len)
    };
    let left: Vec<Pt> = (0..n).map(|i| { let nn = normal(i); pt(pts[i].x + nn.x * w(i), pts[i].y + nn.y * w(i)) }).collect();
    let right: Vec<Pt> = (0..n).map(|i| { let nn = normal(i); pt(pts[i].x - nn.x * w(i), pts[i].y - nn.y * w(i)) }).collect();
    if closed {
        // two rings; even-odd fill gives the band
        let mut p = Path::polygon(&left);
        let mut r = right.clone();
        r.reverse();
        p.segs.extend(Path::polygon(&r).segs);
        return p;
    }
    let cap = |c: Pt, from: Pt, r: f32, out: &mut Vec<Pt>| {
        let a0 = (from.y - c.y).atan2(from.x - c.x);
        for k in 1..8 {
            let a = a0 + std::f32::consts::PI * k as f32 / 8.0;
            out.push(pt(c.x + r * a.cos(), c.y + r * a.sin()));
        }
    };
    let mut ring = left.clone();
    cap(pts[n - 1], left[n - 1], w(n - 1), &mut ring);
    ring.extend(right.iter().rev());
    cap(pts[0], right[0], w(0), &mut ring);
    Path::polygon(&ring)
}

/// Variable-width stroke of a path. `profile` = (t, width) pairs over the
/// whole path length (t in 0..1), linearly interpolated.
/// A width profile ((t, width) pairs, sorted) at `t`, linearly interpolated.
pub fn profile_at(profile: &[(f32, f32)], t: f32) -> f32 {
    if profile.is_empty() {
        return 1.0;
    }
    let mut prev = profile[0];
    if t <= prev.0 {
        return prev.1;
    }
    for &(pt_, w) in &profile[1..] {
        if t <= pt_ {
            let k = (t - prev.0) / (pt_ - prev.0).max(1e-6);
            return prev.1 + (w - prev.1) * k;
        }
        prev = (pt_, w);
    }
    prev.1
}

pub fn variable_stroke(path: &Path, profile: &[(f32, f32)]) -> Path {
    let polys = path.flatten(10);
    let total: f32 = polys.iter().map(|(p, closed)| poly_len(p, *closed)).sum::<f32>().max(1e-6);
    let width_at = |t: f32| profile_at(profile, t);
    let mut out = Path::default();
    let mut acc = 0.0;
    // Resample so width points between the path's own vertices take effect.
    let step = (total / 96.0).max(0.5);
    let polys: Vec<(Vec<Pt>, bool)> = polys
        .into_iter()
        .map(|(poly, closed)| {
            let mut out = Vec::with_capacity(poly.len() * 2);
            let n = poly.len();
            let edges = if closed { n } else { n.saturating_sub(1) };
            for i in 0..edges {
                let (a, b) = (poly[i], poly[(i + 1) % n]);
                let k = (a.dist(b) / step).ceil().max(1.0) as usize;
                for j in 0..k {
                    out.push(a.lerp(b, j as f32 / k as f32));
                }
            }
            if !closed {
                if let Some(l) = poly.last() {
                    out.push(*l);
                }
            }
            if out.is_empty() {
                out = poly;
            }
            (out, closed)
        })
        .collect();
    for (poly, closed) in polys {
        let mut d = 0.0;
        let widths: Vec<f32> = poly
            .iter()
            .enumerate()
            .map(|(i, p)| {
                if i > 0 {
                    d += p.dist(poly[i - 1]);
                }
                width_at((acc + d) / total)
            })
            .collect();
        acc += poly_len(&poly, closed);
        out.segs.extend(outline_polyline(&poly, &widths, closed).segs);
    }
    out
}

fn poly_len(p: &[Pt], closed: bool) -> f32 {
    let mut l: f32 = p.windows(2).map(|w| w[0].dist(w[1])).sum();
    if closed && p.len() > 1 {
        l += p[p.len() - 1].dist(p[0]);
    }
    l
}

/// Arc-length position (0..1) of the point on `path` nearest to `p`, and the distance.
pub fn nearest_t(path: &Path, p: Pt) -> Option<(f32, f32)> {
    let polys = path.flatten(16);
    let total: f32 = polys.iter().map(|(q, c)| poly_len(q, *c)).sum::<f32>().max(1e-6);
    let mut acc = 0.0;
    let mut best: Option<(f32, f32)> = None;
    for (poly, closed) in polys {
        let n = poly.len();
        let edges = if closed { n } else { n - 1 };
        for i in 0..edges {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l2 = dx * dx + dy * dy;
            let k = if l2 < 1e-9 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
            let q = pt(a.x + dx * k, a.y + dy * k);
            let dist = q.dist(p);
            let t = (acc + l2.sqrt() * k) / total;
            if best.map(|bb| dist < bb.1).unwrap_or(true) {
                best = Some((t, dist));
            }
            acc += l2.sqrt();
        }
    }
    best
}

// ------------------------------------------------------------------ Asset Warp

/// Non-destructive pin warp: `base` is the shape before warping; each pin
/// maps an original point to where it has been dragged.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Warp {
    pub base: Path,
    pub pins: Vec<(Pt, Pt)>,
}

impl Warp {
    pub fn new(base: &Path) -> Warp {
        let (lo, hi) = base.bounds().unwrap_or((pt(0.0, 0.0), pt(1.0, 1.0)));
        Warp { base: base.subdivided((hi.x - lo.x).max(hi.y - lo.y) / 12.0), pins: vec![] }
    }
    pub fn apply(&self) -> Path {
        if self.pins.is_empty() {
            return self.base.clone();
        }
        let pins = self.pins.clone();
        self.base.map(|v| rigid_mls(&pins, v))
    }
}

/// Asset Warp on bitmaps, symbols and groups: pins in element coordinates
/// (rest point → where it was dragged) deform a grid laid over what the
/// element draws, rigidly as possible (moving least squares, as for shapes;
/// OpenToonz deforms textured meshes the same way with its plastic tool).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct MeshWarp {
    pub pins: Vec<(Pt, Pt)>,
}

/// A deformed grid in stage (or device) space: `src` points are where the
/// undeformed picture is, `dst` where they go; `cols × rows` cells.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshMap {
    pub cols: usize,
    pub rows: usize,
    pub src: Vec<Pt>,
    pub dst: Vec<Pt>,
}

impl MeshWarp {
    /// Where the picture point now shown at `d` was before the warp (the
    /// inverse of the deformation, by fixed-point iteration), so a new pin
    /// lands on what is under the pointer without moving anything.
    pub fn rest_of(&self, d: Pt) -> Pt {
        if self.pins.is_empty() {
            return d;
        }
        let mut v = d;
        for _ in 0..40 {
            let f = rigid_mls(&self.pins, v);
            let e = pt(d.x - f.x, d.y - f.y);
            v = pt(v.x + e.x, v.y + e.y);
            if e.x.abs() + e.y.abs() < 1e-3 {
                break;
            }
        }
        v
    }

    /// The grid over local bounds `lo`–`hi`, placed by `m` (local → stage).
    pub fn map(&self, lo: Pt, hi: Pt, m: &crate::geom::Matrix) -> MeshMap {
        let (w, h) = ((hi.x - lo.x).max(1.0), (hi.y - lo.y).max(1.0));
        // about 24 cells along the longer side
        let cell = w.max(h) / 24.0;
        let cols = ((w / cell).ceil() as usize).clamp(2, 48);
        let rows = ((h / cell).ceil() as usize).clamp(2, 48);
        let mut src = Vec::with_capacity((cols + 1) * (rows + 1));
        let mut dst = Vec::with_capacity(src.capacity());
        for r in 0..=rows {
            for c in 0..=cols {
                let v = pt(lo.x + w * c as f32 / cols as f32, lo.y + h * r as f32 / rows as f32);
                src.push(m.apply(v));
                dst.push(m.apply(rigid_mls(&self.pins, v)));
            }
        }
        MeshMap { cols, rows, src, dst }
    }
}

impl MeshMap {
    pub fn transformed(&self, m: &crate::geom::Matrix) -> MeshMap {
        MeshMap { cols: self.cols, rows: self.rows, src: self.src.iter().map(|p| m.apply(*p)).collect(), dst: self.dst.iter().map(|p| m.apply(*p)).collect() }
    }
    /// The two triangles of every cell: (src, dst) corner triples.
    pub fn triangles(&self) -> Vec<([Pt; 3], [Pt; 3])> {
        let mut v = Vec::with_capacity(self.cols * self.rows * 2);
        let idx = |c: usize, r: usize| r * (self.cols + 1) + c;
        for r in 0..self.rows {
            for c in 0..self.cols {
                let (a, b, d, e) = (idx(c, r), idx(c + 1, r), idx(c, r + 1), idx(c + 1, r + 1));
                v.push(([self.src[a], self.src[b], self.src[e]], [self.dst[a], self.dst[b], self.dst[e]]));
                v.push(([self.src[a], self.src[e], self.src[d]], [self.dst[a], self.dst[e], self.dst[d]]));
            }
        }
        v
    }
    pub fn dst_bounds(&self) -> Option<(Pt, Pt)> {
        let mut it = self.dst.iter();
        let f = *it.next()?;
        Some(it.fold((f, f), |(lo, hi), p| (pt(lo.x.min(p.x), lo.y.min(p.y)), pt(hi.x.max(p.x), hi.y.max(p.y)))))
    }
}

/// The affine map taking triangle `a` onto triangle `b` (None if degenerate).
pub fn triangle_affine(a: &[Pt; 3], b: &[Pt; 3]) -> Option<crate::geom::Matrix> {
    let (u1, u2) = (pt(a[1].x - a[0].x, a[1].y - a[0].y), pt(a[2].x - a[0].x, a[2].y - a[0].y));
    let (v1, v2) = (pt(b[1].x - b[0].x, b[1].y - b[0].y), pt(b[2].x - b[0].x, b[2].y - b[0].y));
    let det = u1.x * u2.y - u2.x * u1.y;
    if det.abs() < 1e-9 {
        return None;
    }
    // M · U = V with U = [u1 u2], V = [v1 v2]  →  M = V · U⁻¹
    let (i11, i12, i21, i22) = (u2.y / det, -u2.x / det, -u1.y / det, u1.x / det);
    let (ma, mc) = (v1.x * i11 + v2.x * i21, v1.x * i12 + v2.x * i22);
    let (mb, md) = (v1.y * i11 + v2.y * i21, v1.y * i12 + v2.y * i22);
    let tx = b[0].x - (ma * a[0].x + mc * a[0].y);
    let ty = b[0].y - (mb * a[0].x + md * a[0].y);
    Some(crate::geom::Matrix { a: ma, b: mb, c: mc, d: md, tx, ty })
}

/// Rigid moving-least-squares deformation (Schaefer, McPhail & Warren 2006).
pub fn rigid_mls(pins: &[(Pt, Pt)], v: Pt) -> Pt {
    if pins.len() == 1 {
        let (p, q) = pins[0];
        return pt(v.x + q.x - p.x, v.y + q.y - p.y);
    }
    let mut ws = Vec::with_capacity(pins.len());
    for (p, q) in pins {
        let d2 = (p.x - v.x).powi(2) + (p.y - v.y).powi(2);
        if d2 < 1e-8 {
            return *q;
        }
        ws.push(1.0 / d2);
    }
    let wsum: f32 = ws.iter().sum();
    let mut ps = pt(0.0, 0.0);
    let mut qs = pt(0.0, 0.0);
    for ((p, q), w) in pins.iter().zip(&ws) {
        ps.x += w * p.x;
        ps.y += w * p.y;
        qs.x += w * q.x;
        qs.y += w * q.y;
    }
    ps = pt(ps.x / wsum, ps.y / wsum);
    qs = pt(qs.x / wsum, qs.y / wsum);
    let vp = pt(v.x - ps.x, v.y - ps.y);
    // f(v) = |v - p*| * fr / |fr| + q*, fr = Σ q̂ᵢ Aᵢ
    let mut fr = pt(0.0, 0.0);
    for ((p, q), w) in pins.iter().zip(&ws) {
        let ph = pt(p.x - ps.x, p.y - ps.y);
        let qh = pt(q.x - qs.x, q.y - qs.y);
        // Aᵢ = wᵢ [ph; -ph⊥] [vp; -vp⊥]ᵀ, with ⊥(x,y) = (-y, x)
        let a11 = w * (ph.x * vp.x + ph.y * vp.y);
        let a12 = w * (ph.x * vp.y - ph.y * vp.x);
        let a21 = -a12;
        let a22 = a11;
        fr.x += qh.x * a11 + qh.y * a21;
        fr.y += qh.x * a12 + qh.y * a22;
    }
    let len = (fr.x * fr.x + fr.y * fr.y).sqrt();
    let vlen = (vp.x * vp.x + vp.y * vp.y).sqrt();
    if len < 1e-9 {
        return pt(v.x + qs.x - ps.x, v.y + qs.y - ps.y);
    }
    pt(vlen * fr.x / len + qs.x, vlen * fr.y / len + qs.y)
}

// ------------------------------------------------------------------ 3D

/// Animate-style 3D properties of an object (3D Rotation / Translation tools).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Transform3D {
    /// Rotation around the X, Y and Z axes in degrees.
    pub rx: f32,
    pub ry: f32,
    pub rz: f32,
    /// Depth: positive moves away from the viewer.
    pub z: f32,
    /// 3D centre in the object's local coordinates.
    pub center: Pt,
}

impl Transform3D {
    pub fn is_identity(&self) -> bool {
        self.rx.abs() < 1e-4 && self.ry.abs() < 1e-4 && self.rz.abs() < 1e-4 && self.z.abs() < 1e-4
    }
    pub fn lerp(a: Option<Transform3D>, b: Option<Transform3D>, t: f32) -> Option<Transform3D> {
        if a.is_none() && b.is_none() {
            return None;
        }
        let a = a.unwrap_or_else(|| Transform3D { center: b.unwrap().center, ..Default::default() });
        let b = b.unwrap_or_else(|| Transform3D { center: a.center, ..Default::default() });
        let l = |x: f32, y: f32| x + (y - x) * t;
        Some(Transform3D { rx: l(a.rx, b.rx), ry: l(a.ry, b.ry), rz: l(a.rz, b.rz), z: l(a.z, b.z), center: a.center.lerp(b.center, t) })
    }
}

/// Camera for 3D projection: field of view (Animate's "Perspective angle")
/// and vanishing point, both in stage coordinates.
#[derive(Clone, Copy, Debug)]
pub struct Perspective {
    pub angle_deg: f32,
    pub vanishing: Pt,
    pub stage_width: f32,
}

impl Perspective {
    pub fn focal(&self) -> f32 {
        let a = self.angle_deg.clamp(1.0, 179.0).to_radians();
        (self.stage_width / 2.0) / (a / 2.0).tan()
    }
}

/// Project world points of an object with 3D properties. `center` is the
/// object's 3D centre in world space.
pub fn project_3d(t: &Transform3D, center: Pt, persp: &Perspective, p: Pt) -> Pt {
    let (sx, cx) = t.rx.to_radians().sin_cos();
    let (sy, cy) = t.ry.to_radians().sin_cos();
    let (sz, cz) = t.rz.to_radians().sin_cos();
    let (x0, y0) = (p.x - center.x, p.y - center.y);
    // Rz
    let (x1, y1, z1) = (x0 * cz - y0 * sz, x0 * sz + y0 * cz, 0.0f32);
    // Ry
    let (x2, y2, z2) = (x1 * cy + z1 * sy, y1, -x1 * sy + z1 * cy);
    // Rx
    let (x3, y3, z3) = (x2, y2 * cx - z2 * sx, y2 * sx + z2 * cx);
    let (wx, wy, wz) = (center.x + x3, center.y + y3, t.z + z3);
    let f = persp.focal();
    let k = f / (f + wz).max(f * 0.02);
    let v = persp.vanishing;
    pt(v.x + (wx - v.x) * k, v.y + (wy - v.y) * k)
}

/// Project a world-space path (curves flattened first so perspective stays accurate).
pub fn project_path(path: &Path, m: &Matrix, t: &Transform3D, center: Pt, persp: &Perspective) -> Path {
    let mut segs = Vec::new();
    for (poly, closed) in path.transformed(m).flatten(12) {
        for (i, q) in poly.iter().enumerate() {
            let pp = project_3d(t, center, persp, *q);
            segs.push(if i == 0 { Seg::M(pp) } else { Seg::L(pp) });
        }
        if closed {
            segs.push(Seg::Z);
        }
    }
    Path { segs }
}

/// Point at arc-length position `t` (0..1) along a path.
pub fn point_at(path: &Path, t: f32) -> Option<Pt> {
    let polys = path.flatten(16);
    let total: f32 = polys.iter().map(|(q, c)| poly_len(q, *c)).sum();
    if total <= 0.0 {
        return polys.first().and_then(|(q, _)| q.first().copied());
    }
    let mut want = t.clamp(0.0, 1.0) * total;
    for (poly, closed) in &polys {
        let n = poly.len();
        let edges = if *closed { n } else { n - 1 };
        for i in 0..edges {
            let (a, b) = (poly[i], poly[(i + 1) % n]);
            let l = a.dist(b);
            if want <= l {
                return Some(a.lerp(b, if l > 0.0 { want / l } else { 0.0 }));
            }
            want -= l;
        }
    }
    polys.last().and_then(|(q, _)| q.last().copied())
}
