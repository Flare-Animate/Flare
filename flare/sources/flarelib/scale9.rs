//! 9-slice scaling (Modify › Symbol › Scale grid; XFL `scaleGridLeft/Right/Top/Bottom`).
//!
//! A scaled instance of a symbol with a grid keeps its four corners at their
//! original size, stretches the four edges along one axis only, and stretches
//! the centre both ways. Each axis is a piecewise-linear map `x -> X` with
//! three pieces (slope 1, slope `sm`, slope 1); a shape is mapped by splitting
//! every segment where it crosses a grid line (so each piece lies in exactly
//! one of the nine regions) and moving its control points with that region's
//! affine map, which is exact for lines and Bezier curves alike. The HTML5
//! publish shares this code, because it draws the same display items.

use crate::geom::{pt, Matrix, Path, Pt, Seg};
use crate::model::*;
use std::sync::Arc;

/// One axis of the grid: content extent `lo..hi`, centre band `a..b`, scale `s`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Axis {
    pub lo: f32,
    pub a: f32,
    pub b: f32,
    pub hi: f32,
    pub s: f32,
    /// Scale of the centre band.
    pub sm: f32,
}

/// One linear piece: input interval and `X = slope * x + offset`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Piece {
    pub from: f32,
    pub to: f32,
    pub slope: f32,
    pub offset: f32,
}

impl Axis {
    /// The axis map for content `lo..hi` and grid band `a..b` scaled by `s`.
    /// `None` when 9-slice does not apply (no scaling, empty band, or the
    /// scaled content is smaller than its corners): the caller scales normally.
    pub fn new(lo: f32, hi: f32, a: f32, b: f32, s: f32) -> Option<Axis> {
        let (a, b) = (a.clamp(lo, hi), b.clamp(lo, hi));
        if (s - 1.0).abs() < 1e-4 || b - a < 1e-4 || hi - lo < 1e-4 {
            return None;
        }
        let sm = ((hi - lo) * s - (a - lo) - (hi - b)) / (b - a);
        (sm > 1e-4).then_some(Axis { lo, a, b, hi, s, sm })
    }
    /// The three pieces, left/top first. The outer ones extend to infinity.
    pub fn pieces(&self) -> [Piece; 3] {
        const BIG: f32 = 1.0e6;
        [
            Piece { from: -BIG, to: self.a, slope: 1.0, offset: self.lo * self.s - self.lo },
            Piece { from: self.a, to: self.b, slope: self.sm, offset: self.lo * self.s + (self.a - self.lo) - self.a * self.sm },
            Piece { from: self.b, to: BIG, slope: 1.0, offset: self.hi * self.s - self.hi },
        ]
    }
    pub fn map(&self, v: f32) -> f32 {
        let p = self.pieces()[self.region(v)];
        p.slope * v + p.offset
    }
    /// 0, 1 or 2: the band `v` lies in.
    pub fn region(&self, v: f32) -> usize {
        if v < self.a {
            0
        } else if v <= self.b {
            1
        } else {
            2
        }
    }
}

fn cubic(p: [Pt; 4], t: f32) -> Pt {
    let u = 1.0 - t;
    let (w0, w1, w2, w3) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
    pt(w0 * p[0].x + w1 * p[1].x + w2 * p[2].x + w3 * p[3].x, w0 * p[0].y + w1 * p[1].y + w2 * p[2].y + w3 * p[3].y)
}

/// De Casteljau split of a cubic at `t`.
fn split(p: [Pt; 4], t: f32) -> ([Pt; 4], [Pt; 4]) {
    let l = |a: Pt, b: Pt| a.lerp(b, t);
    let (p01, p12, p23) = (l(p[0], p[1]), l(p[1], p[2]), l(p[2], p[3]));
    let (p012, p123) = (l(p01, p12), l(p12, p23));
    let m = l(p012, p123);
    ([p[0], p01, p012, m], [m, p123, p23, p[3]])
}

/// Parameters in (0,1) where the cubic crosses `x = line` (or `y = line`).
fn crossings(p: [Pt; 4], line: f32, on_x: bool, out: &mut Vec<f32>) {
    let f = |t: f32| {
        let q = cubic(p, t);
        (if on_x { q.x } else { q.y }) - line
    };
    const N: usize = 32;
    let mut prev = f(0.0);
    for i in 1..=N {
        let t1 = i as f32 / N as f32;
        let cur = f(t1);
        if (prev < 0.0 && cur > 0.0) || (prev > 0.0 && cur < 0.0) {
            let (mut lo, mut hi) = ((i - 1) as f32 / N as f32, t1);
            let neg_first = prev < 0.0;
            for _ in 0..40 {
                let mid = (lo + hi) / 2.0;
                if (f(mid) < 0.0) == neg_first {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out.push((lo + hi) / 2.0);
        }
        prev = cur;
    }
}

struct Mapper<'a> {
    ax: Option<&'a Axis>,
    ay: Option<&'a Axis>,
}

impl Mapper<'_> {
    /// The affine map (x slope, x offset, y slope, y offset) of the region holding `p`.
    fn region_of(&self, p: Pt) -> (f32, f32, f32, f32) {
        let (sx, ox) = self.ax.map(|a| a.pieces()[a.region(p.x)]).map(|q| (q.slope, q.offset)).unwrap_or((1.0, 0.0));
        let (sy, oy) = self.ay.map(|a| a.pieces()[a.region(p.y)]).map(|q| (q.slope, q.offset)).unwrap_or((1.0, 0.0));
        (sx, ox, sy, oy)
    }
    fn pt(&self, r: (f32, f32, f32, f32), p: Pt) -> Pt {
        pt(r.0 * p.x + r.1, r.2 * p.y + r.3)
    }
    /// Emit one segment (as a cubic, `line` = it was straight) split at the grid lines.
    fn segment(&self, p: [Pt; 4], line: bool, out: &mut Vec<Seg>) {
        let mut ts = vec![];
        for a in self.ax.into_iter() {
            crossings(p, a.a, true, &mut ts);
            crossings(p, a.b, true, &mut ts);
        }
        for a in self.ay.into_iter() {
            crossings(p, a.a, false, &mut ts);
            crossings(p, a.b, false, &mut ts);
        }
        ts.retain(|t| *t > 1e-4 && *t < 1.0 - 1e-4);
        ts.sort_by(|a, b| a.total_cmp(b));
        ts.dedup_by(|a, b| (*a - *b).abs() < 1e-4);
        let mut rest = p;
        let mut done = 0.0;
        let mut pieces = vec![];
        for t in ts {
            let (head, tail) = split(rest, (t - done) / (1.0 - done));
            pieces.push(head);
            rest = tail;
            done = t;
        }
        pieces.push(rest);
        for c in pieces {
            let r = self.region_of(cubic(c, 0.5));
            let q = c.map(|v| self.pt(r, v));
            out.push(if line { Seg::L(q[3]) } else { Seg::C(q[1], q[2], q[3]) });
        }
    }
}

/// Map `path` (symbol coordinates) through the 9-slice grid. Either axis may be
/// `None` (left unscaled).
pub fn map_path(path: &Path, ax: Option<&Axis>, ay: Option<&Axis>) -> Path {
    let m = Mapper { ax, ay };
    let mut segs = Vec::with_capacity(path.segs.len() * 2);
    let (mut cur, mut start) = (Pt::default(), Pt::default());
    for s in &path.segs {
        match *s {
            Seg::M(p) => {
                let r = m.region_of(p);
                segs.push(Seg::M(m.pt(r, p)));
                cur = p;
                start = p;
            }
            Seg::L(p) => {
                m.segment([cur, cur, p, p], true, &mut segs);
                cur = p;
            }
            Seg::Q(c, p) => {
                // elevate to a cubic so every piece can be split
                let c1 = cur.lerp(c, 2.0 / 3.0);
                let c2 = p.lerp(c, 2.0 / 3.0);
                m.segment([cur, c1, c2, p], false, &mut segs);
                cur = p;
            }
            Seg::C(c1, c2, p) => {
                m.segment([cur, c1, c2, p], false, &mut segs);
                cur = p;
            }
            Seg::Z => {
                // the closing edge is a straight line that may cross grid lines
                if cur.dist(start) > 1e-6 {
                    m.segment([cur, cur, start, start], true, &mut segs);
                }
                segs.push(Seg::Z);
                cur = start;
            }
        }
    }
    Path { segs }
}

/// Bounds of a set of items in the space `to` maps their matrices into.
fn bounds_in(items: &[DrawItem], to: &Matrix) -> Option<(Pt, Pt)> {
    items
        .iter()
        .filter_map(|it| it.path.transformed(&to.concat(&it.matrix)).bounds())
        .reduce(|a, b| (pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))))
}

/// The scale an instance matrix applies along the symbol's x and y axes, and
/// the matrix with that scale taken out (`m = rest * diag(sx, sy)`).
pub fn split_scale(m: &Matrix) -> (Matrix, f32, f32) {
    let sx = (m.a * m.a + m.b * m.b).sqrt();
    let sy = (m.c * m.c + m.d * m.d).sqrt();
    if sx < 1e-6 || sy < 1e-6 {
        return (*m, 1.0, 1.0);
    }
    (Matrix { a: m.a / sx, b: m.b / sx, c: m.c / sy, d: m.d / sy, tx: m.tx, ty: m.ty }, sx, sy)
}

/// Rewrite the items a scaled instance of a 9-slice symbol produced.
/// `items` were evaluated with parent matrix `p` (the instance matrix without
/// its scale); `sx`/`sy` are the scale taken out. Returns false when 9-slice
/// does not apply (items untouched).
pub fn apply(items: &mut Vec<DrawItem>, first: usize, p: &Matrix, grid: &ScaleGrid, sx: f32, sy: f32) -> bool {
    let Some(inv) = p.invert() else { return false };
    let Some((lo, hi)) = bounds_in(&items[first..], &inv) else { return false };
    // the grid must lie inside the content; the content extent grows to include it
    let (x0, x1) = (lo.x.min(grid.left), hi.x.max(grid.right));
    let (y0, y1) = (lo.y.min(grid.top), hi.y.max(grid.bottom));
    let ax = Axis::new(x0, x1, grid.left, grid.right, sx);
    let ay = Axis::new(y0, y1, grid.top, grid.bottom, sy);
    if ax.is_none() && ay.is_none() {
        // no band can stretch: plain scaling of the whole symbol (clips too)
        let scale = p.concat(&Matrix::scale(sx, sy)).concat(&inv);
        for it in &mut items[first..] {
            it.matrix = scale.concat(&it.matrix);
            if !it.clips.is_empty() {
                it.clips = it.clips.iter().map(|c| Arc::new(Clip { shapes: c.shapes.iter().map(|(sp, m)| (sp.clone(), scale.concat(m))).collect() })).collect();
            }
        }
        return true;
    }
    // an axis that can't be 9-sliced scales normally
    let axm = ax.unwrap_or(Axis { lo: x0, a: x0, b: x1, hi: x1, s: sx, sm: sx });
    let aym = ay.unwrap_or(Axis { lo: y0, a: y0, b: y1, hi: y1, s: sy, sm: sy });
    let (axr, ayr) = (Some(&axm), Some(&aym));
    let mut out: Vec<DrawItem> = Vec::with_capacity(items.len() - first);
    let mut clip_cache: Vec<(*const Clip, Arc<Clip>)> = vec![];
    for it in items.drain(first..) {
        let rel = inv.concat(&it.matrix);
        let scale = (rel.a * rel.d - rel.b * rel.c).abs().sqrt();
        let mut map_clips = |clips: &[Arc<Clip>]| -> Vec<Arc<Clip>> {
            clips
                .iter()
                .map(|c| {
                    if let Some((_, n)) = clip_cache.iter().find(|(k, _)| std::ptr::eq(*k, Arc::as_ptr(c))) {
                        return n.clone();
                    }
                    let shapes = c.shapes.iter().map(|(sp, sm)| (map_path(&sp.transformed(&inv.concat(sm)), axr, ayr), *p)).collect();
                    let n = Arc::new(Clip { shapes });
                    clip_cache.push((Arc::as_ptr(c), n.clone()));
                    n
                })
                .collect()
        };
        if it.image.is_some() || it.pattern.is_some() {
            // pixels can't be split: nine copies, each clipped to its region and
            // drawn with that region's scale/offset
            let clips = map_clips(&it.clips);
            for (xi, px) in axm.pieces().iter().enumerate() {
                for (yi, py) in aym.pieces().iter().enumerate() {
                    let (fx0, fx1) = (if xi == 0 { px.from } else { axm.map(px.from) - 0.25 }, if xi == 2 { px.to } else { axm.map(px.to) + 0.25 });
                    let (fy0, fy1) = (if yi == 0 { py.from } else { aym.map(py.from) - 0.25 }, if yi == 2 { py.to } else { aym.map(py.to) + 0.25 });
                    let region = Matrix { a: px.slope, b: 0.0, c: 0.0, d: py.slope, tx: px.offset, ty: py.offset };
                    let mut c = it.clone();
                    c.matrix = p.concat(&region).concat(&rel);
                    c.clips = clips.clone();
                    c.clips.push(Arc::new(Clip { shapes: vec![(Path::rect(fx0, fy0, fx1 - fx0, fy1 - fy0), *p)] }));
                    out.push(c);
                }
            }
            continue;
        }
        let mut n = it;
        n.path = map_path(&n.path.transformed(&rel), axr, ayr);
        if let Some(s) = &mut n.stroke {
            s.width *= scale;
        }
        if let Some(g) = &mut n.gradient {
            g.matrix = rel.concat(&g.matrix);
        }
        n.clips = map_clips(&n.clips);
        n.matrix = *p;
        out.push(n);
    }
    items.extend(out);
    true
}
