//! Drawing assistants and symmetry, after OpenToonz's `tnztools/assistants`
//! (guidelines: `assistantline.cpp`, `assistantvanishingpoint.cpp`,
//! `assistantellipse.cpp`, `assistantperspective.cpp`, `assistantfisheye.cpp`;
//! replicators: `replicatormirror.cpp`, `replicatorstar.cpp`,
//! `replicatorgrid.cpp`, `replicatoraffine.cpp`, `replicatorjitter.cpp` with
//! the noise of `modifiers/modifierjitter.cpp`; guideline choice from
//! `assistant.cpp`; BSD-3-Clause, © DWANGO and OpenToonz contributors; see
//! THIRD-PARTY-NOTICES.md).
//!
//! * **Guides** (Line, Vanishing point, Ellipse, Perspective, Fisheye) pull freehand strokes onto a
//!   line or curve. The first ~40 screen pixels of a stroke choose the guide
//!   that fits best (OpenToonz's log-normal weighting); every point is then
//!   pulled onto it by the guide's magnetism.
//! * **Replicators** (Mirror, Star, Grid, Affine) repeat each stroke: symmetry,
//!   kaleidoscopes and repeated transforms while you draw. **Jitter** shakes
//!   every stroke (and copy) with smooth noise instead.

use crate::geom::{pt, Matrix, Pt};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AssistantKind {
    /// A straight guide through `a` and `b`; `parallel`: any line parallel to it.
    Line {
        a: Pt,
        b: Pt,
        #[serde(default)]
        parallel: bool,
    },
    /// Lines through a vanishing point (combine two or three for perspective).
    VanishingPoint { c: Pt },
    /// An ellipse: centre `c`, first axis to `a`, second axis length from `b`;
    /// `concentric`: any ellipse of the same shape around `c`.
    Ellipse {
        c: Pt,
        a: Pt,
        b: Pt,
        #[serde(default)]
        concentric: bool,
    },
    /// Mirror symmetry across the line through `a` and `b`.
    Mirror { a: Pt, b: Pt },
    /// Radial symmetry: `count` copies around `c` (`a` sets the start angle),
    /// optionally mirrored (kaleidoscope).
    Star {
        c: Pt,
        a: Pt,
        count: u32,
        #[serde(default)]
        mirror: bool,
    },
    /// Copies on a grid with steps `a − c` and `b − c`: `count_a` along the
    /// first (forward) and `count_a_inv` backward, likewise for b.
    Grid {
        c: Pt,
        a: Pt,
        b: Pt,
        count_a: u32,
        #[serde(default)]
        count_a_inv: u32,
        count_b: u32,
        #[serde(default)]
        count_b_inv: u32,
    },
    /// Two- or three-point perspective: lines to the vanishing points `vx`,
    /// `vy` (on the horizon) and, with `three_point`, `vz`; otherwise verticals.
    Perspective {
        vx: Pt,
        vy: Pt,
        vz: Pt,
        #[serde(default)]
        three_point: bool,
    },
    /// Fisheye (curvilinear) perspective inside the ellipse `c`, `a`, `b`:
    /// radial lines, and the "meridian" ellipses through the poles.
    Fisheye { c: Pt, a: Pt, b: Pt },
    /// Repeat a transform: the frame (`c0`, `a0`, `b0`) maps to (`c1`, `a1`,
    /// `b1`); each stroke is copied `count` times forward and `count_inv`
    /// times backward through that map (spirals, tunnels, receding rows).
    Affine {
        c0: Pt,
        a0: Pt,
        b0: Pt,
        c1: Pt,
        a1: Pt,
        b1: Pt,
        count: u32,
        #[serde(default)]
        count_inv: u32,
    },
    /// Smooth random wobble of `amplitude` stage units every `period` units
    /// along each stroke; `keep_first` / `keep_last` pin the ends. `c` only
    /// places its marker.
    Jitter {
        c: Pt,
        period: f32,
        amplitude: f32,
        #[serde(default)]
        keep_first: bool,
        #[serde(default)]
        keep_last: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Assistant {
    pub kind: AssistantKind,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// Guides: how strongly strokes are pulled on (0…1).
    #[serde(default = "one")]
    pub magnetism: f32,
}

fn yes() -> bool {
    true
}
fn one() -> f32 {
    1.0
}

impl Assistant {
    pub fn new(kind: AssistantKind) -> Assistant {
        Assistant { kind, enabled: true, magnetism: 1.0 }
    }
    pub fn name(&self) -> &'static str {
        self.kind.name()
    }
    pub fn is_guide(&self) -> bool {
        matches!(self.kind, AssistantKind::Line { .. } | AssistantKind::VanishingPoint { .. } | AssistantKind::Ellipse { .. } | AssistantKind::Perspective { .. } | AssistantKind::Fisheye { .. })
    }
    /// Editable points (handles).
    pub fn points(&self) -> Vec<Pt> {
        let mut k = self.kind.clone();
        k.points_mut().into_iter().map(|p| *p).collect()
    }
    /// Move handle `i` to `p`. Moving the first handle (the centre / start)
    /// carries the others along, as in OpenToonz (the second frame's centre
    /// of an Affine carries its own frame).
    pub fn set_point(&mut self, i: usize, p: Pt) {
        let old = self.points();
        let Some(&o) = old.get(i) else { return };
        let d = pt(p.x - o.x, p.y - o.y);
        let carry: Vec<usize> = match (&self.kind, i) {
            (AssistantKind::Line { .. } | AssistantKind::VanishingPoint { .. } | AssistantKind::Perspective { .. } | AssistantKind::Jitter { .. }, _) => vec![i],
            (AssistantKind::Affine { .. }, 3) => vec![3, 4, 5],
            (_, 0) => (0..old.len()).collect(),
            _ => vec![i],
        };
        for (j, q) in self.kind.points_mut().into_iter().enumerate() {
            if carry.contains(&j) {
                *q = pt(q.x + d.x, q.y + d.y);
            }
        }
    }
    /// Move every point by `d`.
    pub fn translate(&mut self, d: Pt) {
        for q in self.kind.points_mut() {
            *q = pt(q.x + d.x, q.y + d.y);
        }
    }
}

impl AssistantKind {
    pub const NAMES: [&'static str; 10] = ["Line", "Vanishing point", "Ellipse", "Perspective", "Fisheye", "Mirror", "Star", "Grid", "Affine", "Jitter"];
    /// The handles, in order (the first is the centre / start where there is one).
    pub fn points_mut(&mut self) -> Vec<&mut Pt> {
        match self {
            AssistantKind::Line { a, b, .. } | AssistantKind::Mirror { a, b } => vec![a, b],
            AssistantKind::VanishingPoint { c } | AssistantKind::Jitter { c, .. } => vec![c],
            AssistantKind::Ellipse { c, a, b, .. } | AssistantKind::Grid { c, a, b, .. } | AssistantKind::Fisheye { c, a, b } => vec![c, a, b],
            AssistantKind::Star { c, a, .. } => vec![c, a],
            AssistantKind::Perspective { vx, vy, vz, three_point } => {
                if *three_point {
                    vec![vx, vy, vz]
                } else {
                    vec![vx, vy]
                }
            }
            AssistantKind::Affine { c0, a0, b0, c1, a1, b1, .. } => vec![c0, a0, b0, c1, a1, b1],
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            AssistantKind::Perspective { .. } => "Perspective",
            AssistantKind::Fisheye { .. } => "Fisheye",
            AssistantKind::Affine { .. } => "Affine",
            AssistantKind::Jitter { .. } => "Jitter",
            AssistantKind::Line { .. } => "Line",
            AssistantKind::VanishingPoint { .. } => "Vanishing point",
            AssistantKind::Ellipse { .. } => "Ellipse",
            AssistantKind::Mirror { .. } => "Mirror",
            AssistantKind::Star { .. } => "Star",
            AssistantKind::Grid { .. } => "Grid",
        }
    }
    /// A new assistant of a type (by name or script id) centred at `c` with
    /// handles `r` units out.
    pub fn create(name: &str, c: Pt, r: f32) -> Option<AssistantKind> {
        let n = name.to_ascii_lowercase().replace(['_', '-'], " ");
        Some(match n.as_str() {
            "line" => AssistantKind::Line { a: pt(c.x - r, c.y), b: pt(c.x + r, c.y), parallel: false },
            "vanishing point" | "vanishing" | "vp" => AssistantKind::VanishingPoint { c },
            "ellipse" => AssistantKind::Ellipse { c, a: pt(c.x + r, c.y), b: pt(c.x, c.y - r * 0.6), concentric: true },
            "mirror" => AssistantKind::Mirror { a: c, b: pt(c.x, c.y - r) },
            "star" => AssistantKind::Star { c, a: pt(c.x, c.y - r), count: 6, mirror: false },
            "grid" => AssistantKind::Grid { c, a: pt(c.x + r, c.y), b: pt(c.x, c.y + r), count_a: 3, count_a_inv: 0, count_b: 3, count_b_inv: 0 },
            "perspective" => AssistantKind::Perspective { vx: pt(c.x - 3.0 * r, c.y), vy: pt(c.x + 3.0 * r, c.y), vz: pt(c.x, c.y + 6.0 * r), three_point: false },
            "fisheye" => AssistantKind::Fisheye { c, a: pt(c.x + 2.0 * r, c.y), b: pt(c.x, c.y - 2.0 * r) },
            "affine" => {
                // second frame: turned 15° and scaled to 85 %, a little to the right
                let (s, t) = (0.85 * 0.5 * r, 15f32.to_radians());
                let c1 = pt(c.x + 0.6 * r, c.y);
                AssistantKind::Affine {
                    c0: c,
                    a0: pt(c.x + 0.5 * r, c.y),
                    b0: pt(c.x, c.y + 0.5 * r),
                    c1,
                    a1: pt(c1.x + s * t.cos(), c1.y + s * t.sin()),
                    b1: pt(c1.x - s * t.sin(), c1.y + s * t.cos()),
                    count: 6,
                    count_inv: 0,
                }
            }
            "jitter" => AssistantKind::Jitter { c, period: 30.0, amplitude: 10.0, keep_first: false, keep_last: false },
            _ => return None,
        })
    }
}

// ------------------------------------------------------------ guidelines

/// One guide a stroke can snap to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Guide {
    /// Infinite line through two points.
    Line(Pt, Pt),
    /// Unit circle mapped by the matrix (and its inverse).
    Ellipse(Matrix, Matrix),
}

impl Guide {
    pub fn nearest(&self, p: Pt) -> Pt {
        match *self {
            Guide::Line(a, b) => {
                let (dx, dy) = (b.x - a.x, b.y - a.y);
                let l2 = dx * dx + dy * dy;
                if l2 < 1e-12 {
                    return a;
                }
                let t = ((p.x - a.x) * dx + (p.y - a.y) * dy) / l2;
                pt(a.x + dx * t, a.y + dy * t)
            }
            Guide::Ellipse(m, inv) => {
                let q = inv.apply(p);
                let l = (q.x * q.x + q.y * q.y).sqrt();
                if l < 1e-9 {
                    return m.apply(pt(1.0, 0.0));
                }
                m.apply(pt(q.x / l, q.y / l))
            }
        }
    }
}

/// Unit circle → the ellipse with centre `c`, first axis to `a`, second axis
/// as long as `b`'s distance from the first axis.
pub fn ellipse_matrix(c: Pt, a: Pt, b: Pt) -> Option<Matrix> {
    let x = pt(a.x - c.x, a.y - c.y);
    let lx = (x.x * x.x + x.y * x.y).sqrt();
    if lx < 1e-6 {
        return None;
    }
    let n = pt(-x.y / lx, x.x / lx);
    let ry = ((b.x - c.x) * n.x + (b.y - c.y) * n.y).abs().max(1e-3);
    Some(Matrix { a: x.x, b: x.y, c: n.x * ry, d: n.y * ry, tx: c.x, ty: c.y })
}

/// The guides an assistant offers for a stroke starting at `p0`, with its magnetism.
pub fn guides(list: &[Assistant], p0: Pt) -> Vec<(Guide, f32)> {
    let mut out = vec![];
    for a in list.iter().filter(|a| a.enabled) {
        let g = match &a.kind {
            AssistantKind::Line { a, b, parallel } => {
                if *parallel {
                    Some(Guide::Line(p0, pt(p0.x + b.x - a.x, p0.y + b.y - a.y)))
                } else {
                    Some(Guide::Line(*a, *b))
                }
            }
            AssistantKind::VanishingPoint { c } => (c.dist(p0) > 1e-3).then_some(Guide::Line(*c, p0)),
            AssistantKind::Ellipse { c, a: ax, b, concentric } => ellipse_matrix(*c, *ax, *b).and_then(|m| {
                let m = if *concentric {
                    let inv = m.invert()?;
                    let q = inv.apply(p0);
                    let s = (q.x * q.x + q.y * q.y).sqrt().max(1e-3);
                    Matrix { a: m.a * s, b: m.b * s, c: m.c * s, d: m.d * s, ..m }
                } else {
                    m
                };
                Some(Guide::Ellipse(m, m.invert()?))
            }),
            AssistantKind::Perspective { vx, vy, vz, three_point } => {
                let mut v: Vec<Guide> = [vx, vy].into_iter().filter(|v| v.dist(p0) > 1e-3).map(|v| Guide::Line(*v, p0)).collect();
                if !*three_point {
                    v.push(Guide::Line(p0, pt(p0.x, p0.y + 1.0)));
                } else if vz.dist(p0) > 1e-3 {
                    v.push(Guide::Line(*vz, p0));
                }
                let m = a.magnetism.clamp(0.0, 1.0);
                out.extend(v.into_iter().map(|g| (g, m)));
                None
            }
            AssistantKind::Fisheye { c, a: ax, b } => {
                let m = a.magnetism.clamp(0.0, 1.0);
                if let Some(e) = ellipse_matrix(*c, *ax, *b) {
                    out.extend(fisheye_guides(e, p0).into_iter().map(|g| (g, m)));
                }
                None
            }
            _ => None,
        };
        if let Some(g) = g {
            out.push((g, a.magnetism.clamp(0.0, 1.0)));
        }
    }
    out
}

/// Fisheye guides through `p0` for the unit circle mapped by `e`: the radius,
/// and the two ellipses through the poles scaled to pass through `p0`
/// (outside the circle: the circle itself), as in `assistantfisheye.cpp`.
fn fisheye_guides(e: Matrix, p0: Pt) -> Vec<Guide> {
    let Some(inv) = e.invert() else { return vec![] };
    let p = inv.apply(p0);
    let l = (p.x * p.x + p.y * p.y).sqrt();
    let mut out = vec![];
    if l > 1e-6 {
        out.push(Guide::Line(e.apply(pt(0.0, 0.0)), e.apply(pt(p.x / l, p.y / l))));
    }
    if l >= 1.0 - 1e-6 {
        out.push(Guide::Ellipse(e, inv));
    } else {
        for (sx, sy) in [((p.x / (1.0 - p.y * p.y).sqrt()).abs(), 1.0), (1.0, (p.y / (1.0 - p.x * p.x).sqrt()).abs())] {
            if sx > 1e-4 && sy > 1e-4 {
                let m = e.concat(&Matrix { a: sx, b: 0.0, c: 0.0, d: sy, tx: 0.0, ty: 0.0 });
                if let Some(mi) = m.invert() {
                    out.push(Guide::Ellipse(m, mi));
                }
            }
        }
    }
    out
}

/// OpenToonz's `TGuideline::calcTrackWeight`: mean deviation of the stroke's
/// first ~40 screen pixels from the guide, weighted log-normally around 20 px.
/// `px` = stage units per screen pixel.
pub fn track_weight(g: &Guide, pts: &[Pt], px: f32) -> f32 {
    const SNAP: f32 = 20.0;
    const MAX: f32 = 2.0 * SNAP;
    if pts.len() < 2 {
        return f32::INFINITY;
    }
    let (mut sum_w, mut sum_len, mut sum_dev) = (0.0f32, 0.0f32, 0.0f32);
    for i in 1..pts.len() {
        let len = pts[i].dist(pts[i - 1]) / px;
        sum_len += len;
        let mid_len = sum_len - 0.5 * len;
        if mid_len > 1e-6 {
            let w = len * (-0.5 * (mid_len / SNAP).ln().powi(2)).exp() / mid_len;
            sum_w += w;
            let mid = pts[i - 1].lerp(pts[i], 0.5);
            sum_dev += w * mid.dist(g.nearest(mid)) / px;
        }
        if sum_len >= MAX {
            break;
        }
    }
    if sum_w > 1e-9 {
        sum_dev / sum_w
    } else {
        f32::INFINITY
    }
}

/// The guide a stroke snaps to (best fitting start), if any.
pub fn best_guide(list: &[Assistant], pts: &[Pt], px: f32) -> Option<(Guide, f32)> {
    let p0 = *pts.first()?;
    guides(list, p0).into_iter().map(|(g, m)| (track_weight(&g, pts, px), g, m)).filter(|(w, _, _)| w.is_finite()).min_by(|a, b| a.0.partial_cmp(&b.0).unwrap()).map(|(_, g, m)| (g, m))
}

/// Pull a stroke onto its best guide.
pub fn snap(list: &[Assistant], pts: &[Pt], px: f32) -> Vec<Pt> {
    match best_guide(list, pts, px) {
        Some((g, m)) => pts.iter().map(|p| p.lerp(g.nearest(*p), m)).collect(),
        None => pts.to_vec(),
    }
}

// ------------------------------------------------------------ replicators

fn reflect(a: Pt, b: Pt) -> Option<Matrix> {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let l2 = dx * dx + dy * dy;
    if l2 < 1e-9 {
        return None;
    }
    let (c2, s2) = ((dx * dx - dy * dy) / l2, 2.0 * dx * dy / l2);
    // reflection about the line through a: p' = a + R (p − a)
    let r = Matrix { a: c2, b: s2, c: s2, d: -c2, tx: 0.0, ty: 0.0 };
    Some(Matrix::translate(a.x, a.y).concat(&r).concat(&Matrix::translate(-a.x, -a.y)))
}

/// The extra copies every stroke gets (identity not included), as maps of
/// stage points. Replicators combine: a mirror plus a star gives the star's
/// copies of both the stroke and its mirror image.
pub fn replicas(list: &[Assistant]) -> Vec<Matrix> {
    let mut all = vec![Matrix::IDENTITY];
    for a in list.iter().filter(|a| a.enabled) {
        let mine: Vec<Matrix> = match &a.kind {
            AssistantKind::Mirror { a, b } => reflect(*a, *b).into_iter().collect(),
            AssistantKind::Star { c, a, count, mirror } => {
                let n = (*count).clamp(1, 64);
                let mut v = vec![];
                for i in 0..n {
                    let rot = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(std::f32::consts::TAU * i as f32 / n as f32)).concat(&Matrix::translate(-c.x, -c.y));
                    if i > 0 {
                        v.push(rot);
                    }
                    if *mirror {
                        if let Some(r) = reflect(*c, *a) {
                            v.push(rot.concat(&r));
                        }
                    }
                }
                v
            }
            AssistantKind::Grid { c, a, b, count_a, count_a_inv, count_b, count_b_inv } => {
                let (da, db) = (pt(a.x - c.x, a.y - c.y), pt(b.x - c.x, b.y - c.y));
                let mut v = vec![];
                for ib in -(*count_b_inv as i32)..(*count_b).max(1) as i32 {
                    for ia in -(*count_a_inv as i32)..(*count_a).max(1) as i32 {
                        if ia != 0 || ib != 0 {
                            v.push(Matrix::translate(da.x * ia as f32 + db.x * ib as f32, da.y * ia as f32 + db.y * ib as f32));
                        }
                    }
                }
                v.truncate(400);
                v
            }
            AssistantKind::Affine { c0, a0, b0, c1, a1, b1, count, count_inv } => {
                let frame = |c: &Pt, a: &Pt, b: &Pt| Matrix { a: a.x - c.x, b: a.y - c.y, c: b.x - c.x, d: b.y - c.y, tx: c.x, ty: c.y };
                let Some(step) = frame(c0, a0, b0).invert().map(|i| frame(c1, a1, b1).concat(&i)) else { continue };
                let mut v = vec![];
                for (m, n) in [(Some(step), *count), (step.invert(), *count_inv)] {
                    let Some(m) = m else { continue };
                    let mut acc = Matrix::IDENTITY;
                    for _ in 0..n.min(200) {
                        acc = m.concat(&acc);
                        v.push(acc);
                    }
                }
                v
            }
            _ => continue,
        };
        let mut next = all.clone();
        for m in &mine {
            for base in &all {
                next.push(m.concat(base));
            }
        }
        all = next;
        all.truncate(1000);
    }
    all.into_iter().skip(1).collect()
}

/// A stroke as drawn with the assistants: snapped, then replicated (the
/// first polyline is the stroke itself), then jittered.
pub fn apply(list: &[Assistant], pts: &[Pt], px: f32) -> Vec<Vec<Pt>> {
    let s = snap(list, pts, px);
    let mut out = vec![s.clone()];
    for m in replicas(list) {
        out.push(s.iter().map(|p| m.apply(*p)).collect());
    }
    for a in list.iter().filter(|a| a.enabled) {
        if let AssistantKind::Jitter { period, amplitude, keep_first, keep_last, .. } = a.kind {
            // each stroke shakes differently (OpenToonz seeds by track): seed from where it starts
            let base = pts.first().map(|p| p.x.to_bits() ^ p.y.to_bits().rotate_left(16)).unwrap_or(0);
            for (i, line) in out.iter_mut().enumerate() {
                *line = jitter(line, period, amplitude, keep_first, keep_last, base.wrapping_add(i as u32));
            }
        }
    }
    out
}

// ------------------------------------------------------------ jitter

/// OpenToonz's jitter noise (`modifierjitter.cpp`): a C rand()-style
/// generator places one random knot per unit, joined by a Hermite spline.
mod noise {
    fn next(seed: &mut u32) -> f64 {
        *seed = (seed.wrapping_mul(1_103_515_245).wrapping_add(12345) >> 16) & 32767;
        *seed as f64 / 32767.0
    }
    fn knot(seed: u32, i: i32, prev_x: Option<f64>, depth: u32) -> (f64, f64) {
        let mut s = seed ^ (i as u32);
        let mut dx = next(&mut s);
        let dy = next(&mut s) * 2.0 - 1.0;
        if dx < 0.5 {
            let px = prev_x.unwrap_or_else(|| if depth < 32 { knot(seed, i - 1, None, depth + 1).0 } else { i as f64 - 1.0 }) + 0.5 - i as f64;
            if dx < px {
                dx = next(&mut s) * (1.0 - px) + px;
            }
        }
        (dx + i as f64, dy)
    }
    fn spline(x: f64, p: &[(f64, f64)]) -> f64 {
        let ((x0, y0), (x1, y1)) = (p[1], p[2]);
        let t0 = (y1 - p[0].1) / (x1 - p[0].0) * (x1 - x0);
        let t1 = (p[3].1 - y0) / (p[3].0 - x0) * (x1 - x0);
        let l = (x - x0) / (x1 - x0);
        let (ll, lll) = (l * l, l * l * l);
        y0 * (2.0 * lll - 3.0 * ll + 1.0) + y1 * (-2.0 * lll + 3.0 * ll) + t0 * (lll - 2.0 * ll + l) + t1 * (lll - ll)
    }
    /// Smooth noise in −1…1 (roughly) with about one wave per unit of `x`.
    pub fn func(seed: u32, x: f64) -> f64 {
        let i = x.floor() as i32;
        let mut p = [(0.0, 0.0); 5];
        p[0] = knot(seed, i - 2, None, 0);
        for j in 1..5 {
            p[j] = knot(seed, i - 2 + j as i32, Some(p[j - 1].0), 0);
        }
        spline(x, &p[if x < p[2].0 { 0 } else { 1 }..])
    }
    pub fn seeds(stroke: u32) -> (u32, u32) {
        let (mut a, mut b) = (stroke, stroke ^ 32143);
        next(&mut a);
        next(&mut b);
        (a, b)
    }
}

/// Shake a polyline: resampled finely enough for the period, then each point
/// moved by the noise at its distance along the line (in periods), x and y
/// from different seeds. `keep_first` / `keep_last` fade the wobble to zero
/// over the first / last period.
pub fn jitter(line: &[Pt], period: f32, amplitude: f32, keep_first: bool, keep_last: bool, stroke: u32) -> Vec<Pt> {
    if period <= 1e-3 || amplitude <= 1e-3 || line.len() < 2 {
        return line.to_vec();
    }
    // resample at ≤ period/8 so the wobble shows (capped)
    let total: f32 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
    let step = (period / 8.0).max(total / 4000.0).max(0.25);
    let mut pts = vec![(line[0], 0.0f32)];
    let mut along = 0.0f32;
    for w in line.windows(2) {
        let l = w[0].dist(w[1]);
        let n = (l / step).ceil().max(1.0) as usize;
        for k in 1..=n {
            pts.push((w[0].lerp(w[1], k as f32 / n as f32), along + l * k as f32 / n as f32));
        }
        along += l;
    }
    let (sx, sy) = noise::seeds(stroke);
    let ll = (total / period) as f64;
    pts.into_iter()
        .map(|(p, d)| {
            let l = (d / period) as f64;
            let mut a = amplitude as f64;
            if keep_first || keep_last {
                if ll < 1e-9 {
                    a = 0.0;
                } else if keep_first && keep_last && ll < 2.0 {
                    a *= 0.5 - 0.5 * (l / ll * std::f64::consts::TAU).cos();
                } else if keep_first && l < 1.0 {
                    a *= 0.5 - 0.5 * (l * std::f64::consts::PI).cos();
                } else if keep_last && ll - l < 1.0 {
                    a *= 0.5 - 0.5 * ((ll - l) * std::f64::consts::PI).cos();
                }
            }
            pt(p.x + (noise::func(sx, l) * a) as f32, p.y + (noise::func(sy, l) * a) as f32)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wobbly(from: Pt, to: Pt) -> Vec<Pt> {
        (0..=40).map(|i| {
            let t = i as f32 / 40.0;
            let w = if i % 2 == 0 { 1.5 } else { -1.5 };
            let (dx, dy) = (to.x - from.x, to.y - from.y);
            let l = (dx * dx + dy * dy).sqrt();
            pt(from.x + dx * t - dy / l * w, from.y + dy * t + dx / l * w)
        }).collect()
    }

    #[test]
    fn strokes_snap_to_the_best_fitting_guide() {
        let list = vec![
            Assistant::new(AssistantKind::Line { a: pt(0.0, 0.0), b: pt(100.0, 0.0), parallel: true }),
            Assistant::new(AssistantKind::VanishingPoint { c: pt(500.0, 500.0) }),
        ];
        // a roughly horizontal stroke snaps to a horizontal line through its start
        let s = snap(&list, &wobbly(pt(10.0, 50.0), pt(200.0, 50.0)), 1.0);
        assert!(s.iter().all(|p| (p.y - s[0].y).abs() < 1e-3), "flat");
        // a stroke heading for the vanishing point snaps onto the ray
        let s = snap(&list, &wobbly(pt(100.0, 100.0), pt(300.0, 300.0)), 1.0);
        assert!(s.iter().all(|p| (p.x - p.y).abs() < 4.0), "on the diagonal ray: {:?}", s[20]);
        // magnetism 0.5 pulls halfway
        let half = vec![Assistant { magnetism: 0.5, ..list[0].clone() }];
        let raw = wobbly(pt(10.0, 50.0), pt(200.0, 50.0));
        let s = snap(&half, &raw, 1.0);
        assert!((s[1].y - (raw[1].y + raw[0].y) * 0.5).abs() < 0.8);
        // no guides → unchanged
        assert_eq!(snap(&[], &raw, 1.0), raw);
    }

    #[test]
    fn concentric_ellipse_guide_passes_through_the_start() {
        let list = vec![Assistant::new(AssistantKind::Ellipse { c: pt(0.0, 0.0), a: pt(100.0, 0.0), b: pt(0.0, 50.0), concentric: true })];
        let raw: Vec<Pt> = (0..30).map(|i| {
            let a = i as f32 * 0.05;
            pt(60.0 * a.cos() + 0.7, 30.0 * a.sin() - 0.5)
        }).collect();
        let s = snap(&list, &raw, 1.0);
        for p in &s {
            let e = (p.x / 60.0).powi(2) + (p.y / 30.0).powi(2);
            assert!((e - 1.0).abs() < 0.05, "on the 60×30 ellipse: {p:?}");
        }
    }

    #[test]
    fn replicators_mirror_star_and_grid() {
        let m = vec![Assistant::new(AssistantKind::Mirror { a: pt(100.0, 0.0), b: pt(100.0, 10.0) })];
        let r = replicas(&m);
        assert_eq!(r.len(), 1);
        assert!(r[0].apply(pt(90.0, 5.0)).dist(pt(110.0, 5.0)) < 1e-4);
        let star = vec![Assistant::new(AssistantKind::Star { c: pt(0.0, 0.0), a: pt(0.0, -10.0), count: 4, mirror: false })];
        let r = replicas(&star);
        assert_eq!(r.len(), 3);
        assert!(r.iter().any(|m| m.apply(pt(10.0, 0.0)).dist(pt(-10.0, 0.0)) < 1e-4));
        let kaleido = vec![Assistant::new(AssistantKind::Star { c: pt(0.0, 0.0), a: pt(0.0, -10.0), count: 4, mirror: true })];
        assert_eq!(replicas(&kaleido).len(), 7);
        let grid = vec![Assistant::new(AssistantKind::Grid { c: pt(0.0, 0.0), a: pt(50.0, 0.0), b: pt(0.0, 50.0), count_a: 3, count_a_inv: 1, count_b: 2, count_b_inv: 0 })];
        assert_eq!(replicas(&grid).len(), 4 * 2 - 1);
        // mirror + star combine
        let both = [m.clone(), star.clone()].concat();
        assert_eq!(replicas(&both).len(), 2 * 4 - 1);
        assert_eq!(apply(&both, &[pt(1.0, 1.0), pt(2.0, 2.0)], 1.0).len(), 8);
        // disabled assistants do nothing
        let off = vec![Assistant { enabled: false, ..m[0].clone() }];
        assert!(replicas(&off).is_empty());
    }

    #[test]
    fn handles_move_and_serialize() {
        let mut a = Assistant::new(AssistantKind::create("star", pt(0.0, 0.0), 50.0).unwrap());
        a.set_point(0, pt(10.0, 10.0));
        assert_eq!(a.points(), vec![pt(10.0, 10.0), pt(10.0, -40.0)]);
        let j = serde_json::to_string(&a).unwrap();
        assert!(j.contains("\"type\":\"star\""));
        assert_eq!(serde_json::from_str::<Assistant>(&j).unwrap(), a);
        assert!(AssistantKind::create("vanishing_point", pt(0.0, 0.0), 1.0).is_some());
    }

    #[test]
    fn perspective_and_fisheye_guides() {
        // two-point: a stroke heading for vx snaps onto the ray; a near-vertical one stays vertical
        let list = vec![Assistant::new(AssistantKind::Perspective { vx: pt(-1000.0, 0.0), vy: pt(1000.0, 0.0), vz: pt(0.0, 3000.0), three_point: false })];
        let s = snap(&list, &wobbly(pt(0.0, 200.0), pt(-500.0, 100.0)), 1.0);
        for p in &s {
            let expect = s[0].y * (p.x + 1000.0) / (s[0].x + 1000.0); // the line from vx through the start
            assert!((p.y - expect).abs() < 0.05, "on the vx ray: {p:?}");
        }
        let s = snap(&list, &wobbly(pt(50.0, 100.0), pt(52.0, 300.0)), 1.0);
        assert!(s.iter().all(|p| (p.x - s[0].x).abs() < 1e-3), "vertical");
        // fisheye: a stroke from the centre outwards is radial; a stroke near the top follows a meridian ellipse
        let fe = vec![Assistant::new(AssistantKind::Fisheye { c: pt(0.0, 0.0), a: pt(100.0, 0.0), b: pt(0.0, 100.0) })];
        let s = snap(&fe, &wobbly(pt(10.0, 10.0), pt(60.0, 60.0)), 1.0);
        let r = s.iter().map(|p| (p.x * s[0].y - p.y * s[0].x).abs() / s[0].dist(pt(0.0, 0.0))).fold(0.0f32, f32::max);
        assert!(r < 0.05, "radial: {r}");
        let raw: Vec<Pt> = (0..30).map(|i| { let t = -0.6 + i as f32 * 0.04; pt(40.0 * (1.0 - t * t).sqrt(), 100.0 * t) }).collect();
        let s = snap(&fe, &raw, 1.0);
        let kx = s[0].x / (100.0 * (1.0 - (s[0].y / 100.0).powi(2)).sqrt());
        for p in &s {
            let e = (p.x / (100.0 * kx)).powi(2) + (p.y / 100.0).powi(2);
            assert!((e - 1.0).abs() < 0.02, "on one meridian: {p:?}");
        }
    }

    #[test]
    fn affine_repeats_and_jitter_wobbles() {
        let mut a = Assistant::new(AssistantKind::create("affine", pt(0.0, 0.0), 100.0).unwrap());
        if let AssistantKind::Affine { count, count_inv, .. } = &mut a.kind {
            *count = 3;
            *count_inv = 2;
        }
        let r = replicas(std::slice::from_ref(&a));
        assert_eq!(r.len(), 5);
        // one step maps the first frame onto the second
        let AssistantKind::Affine { c0, a0, c1, a1, .. } = a.kind else { unreachable!() };
        assert!(r[0].apply(c0).dist(c1) < 1e-3 && r[0].apply(a0).dist(a1) < 1e-3);
        // backward copies undo forward ones
        assert!(r[3].concat(&r[0]).apply(pt(7.0, 9.0)).dist(pt(7.0, 9.0)) < 1e-3);
        // moving the second centre carries its frame
        a.set_point(3, pt(c1.x + 10.0, c1.y));
        assert!(a.points()[4].dist(pt(a1.x + 10.0, a1.y)) < 1e-4 && a.points()[0] == c0);

        let line: Vec<Pt> = vec![pt(0.0, 0.0), pt(300.0, 0.0)];
        let j = jitter(&line, 30.0, 10.0, true, true, 0);
        assert!(j.len() > 50);
        assert!(j[0].dist(line[0]) < 1e-3 && j.last().unwrap().dist(line[1]) < 0.5, "ends kept");
        let dev = j.iter().map(|p| p.y.abs()).fold(0.0f32, f32::max);
        assert!(dev > 3.0 && dev < 15.0, "wobbles about the amplitude: {dev}");
        // smooth: neighbours close together
        let gap = j.windows(2).map(|w| w[0].dist(w[1])).fold(0.0f32, f32::max);
        assert!(gap < 9.0, "smooth: {gap}");
        // deterministic, different per stroke
        assert_eq!(jitter(&line, 30.0, 10.0, false, false, 1), jitter(&line, 30.0, 10.0, false, false, 1));
        assert_ne!(jitter(&line, 30.0, 10.0, false, false, 1), jitter(&line, 30.0, 10.0, false, false, 2));
        let with = vec![Assistant::new(AssistantKind::create("jitter", pt(0.0, 0.0), 1.0).unwrap())];
        assert!(apply(&with, &line, 1.0)[0].len() > 2);
    }
}
