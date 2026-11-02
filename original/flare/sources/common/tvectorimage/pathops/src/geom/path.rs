//! The anchor-based editable path model.

use std::collections::BTreeSet;

use kurbo::{Affine, BezPath, CubicBez, ParamCurve, ParamCurveExtrema, ParamCurveNearest, PathEl, PathSeg, Point, Rect, Shape};
use serde::{Deserialize, Serialize};

use super::EPS;

/// Fill rule for paths and compound paths.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FillRule {
    /// Illustrator's default for compound paths created from overlapping shapes.
    #[default]
    NonZero,
    EvenOdd,
}

/// How an anchor's handles behave when edited.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AnchorKind {
    /// Handles move independently (or there are none).
    #[default]
    Corner,
    /// Handles stay collinear (dragging one rotates the other).
    Smooth,
}

/// One anchor point with absolute handle positions. A handle equal to `p` means "no handle".
/// Serialized without handles that equal `p` and without the default `kind` (smaller files).
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "AnchorRepr", into = "AnchorRepr")]
pub struct Anchor {
    pub p: Point,
    #[serde(rename = "in")]
    pub h_in: Point,
    #[serde(rename = "out")]
    pub h_out: Point,
    #[serde(default)]
    pub kind: AnchorKind,
}

/// Wire form of [`Anchor`]: points as `[x, y]` (format v2; `{"x", "y"}` maps from v1 files are
/// still read), and absent handles mean "no handle" (equal to `p`).
#[derive(Serialize, Deserialize)]
struct AnchorRepr {
    p: WirePoint,
    #[serde(rename = "in", default, skip_serializing_if = "Option::is_none")]
    h_in: Option<WirePoint>,
    #[serde(rename = "out", default, skip_serializing_if = "Option::is_none")]
    h_out: Option<WirePoint>,
    #[serde(default, skip_serializing_if = "is_corner")]
    kind: AnchorKind,
}

fn is_corner(k: &AnchorKind) -> bool {
    *k == AnchorKind::Corner
}

impl From<AnchorRepr> for Anchor {
    fn from(r: AnchorRepr) -> Self {
        let p = r.p.0;
        Self { p, h_in: r.h_in.map_or(p, |h| h.0), h_out: r.h_out.map_or(p, |h| h.0), kind: r.kind }
    }
}

impl From<Anchor> for AnchorRepr {
    fn from(a: Anchor) -> Self {
        let handle = |h: Point| (h != a.p).then_some(WirePoint(h));
        Self { p: WirePoint(a.p), h_in: handle(a.h_in), h_out: handle(a.h_out), kind: a.kind }
    }
}

/// A point written as `[x, y]`, read from `[x, y]` or `{"x": …, "y": …}`.
struct WirePoint(Point);

impl Serialize for WirePoint {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        [self.0.x, self.0.y].serialize(s)
    }
}

impl<'de> Deserialize<'de> for WirePoint {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = WirePoint;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a point as [x, y] or {\"x\": x, \"y\": y}")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut a: A) -> Result<WirePoint, A::Error> {
                let x = a.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let y = a.next_element()?.ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                Ok(WirePoint(Point::new(x, y)))
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(self, mut a: A) -> Result<WirePoint, A::Error> {
                let (mut x, mut y) = (None, None);
                while let Some(k) = a.next_key::<std::borrow::Cow<'de, str>>()? {
                    match k.as_ref() {
                        "x" => x = Some(a.next_value()?),
                        "y" => y = Some(a.next_value()?),
                        _ => {
                            a.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(WirePoint(Point::new(
                    x.ok_or_else(|| serde::de::Error::missing_field("x"))?,
                    y.ok_or_else(|| serde::de::Error::missing_field("y"))?,
                )))
            }
        }
        d.deserialize_any(V)
    }
}

impl Anchor {
    /// A corner anchor without handles.
    pub fn corner(p: Point) -> Self {
        Self { p, h_in: p, h_out: p, kind: AnchorKind::Corner }
    }
    /// A smooth anchor with a symmetric out handle `out` (the in handle is mirrored).
    pub fn smooth(p: Point, out: Point) -> Self {
        Self { p, h_in: p - (out - p), h_out: out, kind: AnchorKind::Smooth }
    }
    pub fn with_handles(p: Point, h_in: Point, h_out: Point) -> Self {
        let kind =
            if is_collinear(p, h_in, h_out) && h_in.distance(p) > EPS && h_out.distance(p) > EPS { AnchorKind::Smooth } else { AnchorKind::Corner };
        Self { p, h_in, h_out, kind }
    }
    pub fn has_in(&self) -> bool {
        self.h_in.distance(self.p) > EPS
    }
    pub fn has_out(&self) -> bool {
        self.h_out.distance(self.p) > EPS
    }
    pub fn transform(&self, a: Affine) -> Self {
        Self { p: a * self.p, h_in: a * self.h_in, h_out: a * self.h_out, kind: self.kind }
    }
    pub fn translate(&mut self, d: kurbo::Vec2) {
        self.p += d;
        self.h_in += d;
        self.h_out += d;
    }
    /// Place the end of the outgoing (`out`) or incoming handle at `pos`. `independent` makes the
    /// anchor a corner first; a smooth anchor keeps its other handle in line, at its length.
    pub fn set_handle(&mut self, out: bool, pos: Point, independent: bool) {
        if independent {
            self.kind = AnchorKind::Corner;
        }
        let p = self.p;
        let smooth = self.kind == AnchorKind::Smooth;
        let (moved, other) = if out { (&mut self.h_out, &mut self.h_in) } else { (&mut self.h_in, &mut self.h_out) };
        *moved = pos;
        if smooth {
            let len = (*other - p).hypot();
            let dir = p - pos;
            let l = dir.hypot();
            if l > 1e-9 {
                *other = p + dir * (len / l);
            }
        }
    }
    /// Swap handles (used when reversing path direction).
    pub fn reversed(&self) -> Self {
        Self { p: self.p, h_in: self.h_out, h_out: self.h_in, kind: self.kind }
    }
    /// Remove both handles (convert to a sharp corner).
    pub fn retract(&mut self) {
        self.h_in = self.p;
        self.h_out = self.p;
        self.kind = AnchorKind::Corner;
    }
}

fn is_collinear(p: Point, a: Point, b: Point) -> bool {
    let va = a - p;
    let vb = b - p;
    let cross = va.cross(vb);
    cross.abs() <= 1e-6 * va.hypot().max(1.0) * vb.hypot().max(1.0) && va.dot(vb) < 0.0
}

/// An open or closed run of anchors.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct SubPath {
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub closed: bool,
}

impl SubPath {
    pub fn new(anchors: Vec<Anchor>, closed: bool) -> Self {
        Self { anchors, closed }
    }
    /// Polyline from points.
    pub fn polyline(points: &[Point], closed: bool) -> Self {
        Self { anchors: points.iter().map(|&p| Anchor::corner(p)).collect(), closed }
    }
    /// Number of segments (a closed path has one more, back to the start).
    pub fn segment_count(&self) -> usize {
        let n = self.anchors.len();
        if n < 2 {
            0
        } else if self.closed {
            n
        } else {
            n - 1
        }
    }
    /// Segment `i` as a cubic (lines are cubics with handles on the anchors).
    pub fn segment(&self, i: usize) -> CubicBez {
        let n = self.anchors.len();
        let a = &self.anchors[i % n];
        let b = &self.anchors[(i + 1) % n];
        CubicBez::new(a.p, a.h_out, b.h_in, b.p)
    }
    /// True if segment `i` is a straight line.
    pub fn segment_is_line(&self, i: usize) -> bool {
        let n = self.anchors.len();
        let a = &self.anchors[i % n];
        let b = &self.anchors[(i + 1) % n];
        !a.has_out() && !b.has_in()
    }
    /// Arc-length at each anchor from the start of this subpath (one entry per anchor).
    pub fn anchor_arc_positions(&self) -> Vec<f64> {
        let n = self.anchors.len();
        if n == 0 {
            return vec![];
        }
        let mut pos = vec![0.0];
        for i in 0..self.segment_count() {
            let len = kurbo::ParamCurveArclen::arclen(&self.segment(i), 1e-3);
            pos.push(pos.last().copied().unwrap_or(0.0) + len);
        }
        pos
    }
    pub fn to_bezpath_into(&self, out: &mut BezPath) {
        let Some(first) = self.anchors.first() else { return };
        out.move_to(first.p);
        for i in 0..self.segment_count() {
            let c = self.segment(i);
            if self.segment_is_line(i) {
                out.line_to(c.p3);
            } else {
                out.curve_to(c.p1, c.p2, c.p3);
            }
        }
        if self.closed {
            out.close_path();
        }
    }
    pub fn reverse(&mut self) {
        self.anchors.reverse();
        for a in &mut self.anchors {
            *a = a.reversed();
        }
    }
    /// Signed area, closing an open subpath with a line: positive when it runs clockwise on
    /// screen (y down), negative when counter-clockwise.
    pub fn signed_area(&self) -> f64 {
        use kurbo::ParamCurveArea;
        let a: f64 = (0..self.segment_count()).map(|i| self.segment(i).signed_area()).sum();
        match (self.closed, self.anchors.first(), self.anchors.last()) {
            (false, Some(f), Some(l)) => a + kurbo::Line::new(l.p, f.p).signed_area(),
            _ => a,
        }
    }
    /// Bend segment `seg` so its point at `t` moves by `dv`: both inner control points move by
    /// dv / 3t(1-t), since B(t) moves by 3t(1-t) times that. A smooth anchor stays smooth: its other
    /// handle turns with the moved one. False when there is no such segment.
    pub fn bend_segment(&mut self, seg: usize, t: f64, dv: kurbo::Vec2) -> bool {
        let n = self.anchors.len();
        if seg >= self.segment_count() || n == 0 {
            return false;
        }
        let v = dv / (3.0 * t * (1.0 - t));
        if let Some(a) = self.anchors.get_mut(seg % n) {
            a.set_handle(true, a.h_out + v, false);
        }
        if let Some(b) = self.anchors.get_mut((seg + 1) % n) {
            b.set_handle(false, b.h_in + v, false);
        }
        true
    }
    /// Bend segment `seg` as a Shift-drag does: both of its handles stand at right angles to the
    /// line between its anchors, on the same side and at the same length, so that its point at `t`
    /// lies as far from that line as the old point at `t` moved by `dv` does. A smooth anchor stays
    /// smooth, as with [`Self::bend_segment`]. A segment whose anchors coincide bends as
    /// [`Self::bend_segment`] bends it. False when there is no such segment.
    pub fn bend_segment_perpendicular(&mut self, seg: usize, t: f64, dv: kurbo::Vec2) -> bool {
        let n = self.anchors.len();
        if seg >= self.segment_count() || n == 0 {
            return false;
        }
        let c = self.segment(seg);
        let chord = c.p3 - c.p0;
        let len = chord.hypot();
        if len <= EPS {
            return self.bend_segment(seg, t, dv);
        }
        let normal = kurbo::Vec2::new(-chord.y, chord.x) / len;
        // Handles of length l along the normal put the point at t 3t(1-t)·l from the line.
        let v = normal * ((c.eval(t) + dv - c.p0).dot(normal) / (3.0 * t * (1.0 - t)));
        if let Some(a) = self.anchors.get_mut(seg % n) {
            a.set_handle(true, a.p + v, false);
        }
        if let Some(b) = self.anchors.get_mut((seg + 1) % n) {
            b.set_handle(false, b.p + v, false);
        }
        true
    }

    /// Split segment `seg` at parameter `t`, inserting a new anchor. Returns the new anchor index.
    pub fn insert_anchor(&mut self, seg: usize, t: f64) -> usize {
        let n = self.anchors.len();
        let c = self.segment(seg);
        let line = self.segment_is_line(seg);
        let (l, r) = (c.subsegment(0.0..t), c.subsegment(t..1.0));
        let i0 = seg % n;
        let i1 = (seg + 1) % n;
        let mid = if line {
            Anchor::corner(l.p3)
        } else {
            self.anchors[i0].h_out = l.p1;
            self.anchors[i1].h_in = r.p2;
            Anchor { p: l.p3, h_in: l.p2, h_out: r.p1, kind: AnchorKind::Smooth }
        };
        self.anchors.insert(seg + 1, mid);
        seg + 1
    }
    /// Make anchor `i` smooth: handles in line with its neighbours (its one neighbour at an open
    /// end), each a third of the way to that side's neighbour. A smooth anchor with both handles
    /// keeps them. False when there is no such anchor or no direction (the neighbours coincide).
    pub fn smooth_anchor(&mut self, i: usize) -> bool {
        let n = self.anchors.len();
        let prev = if i > 0 {
            Some(i - 1)
        } else if self.closed {
            n.checked_sub(1)
        } else {
            None
        };
        let next = if i + 1 < n {
            Some(i + 1)
        } else if self.closed {
            Some(0)
        } else {
            None
        };
        let at = |j: Option<usize>| j.and_then(|j| self.anchors.get(j)).map(|a| a.p);
        let (prev, next) = (at(prev), at(next));
        let Some(a) = self.anchors.get_mut(i) else { return false };
        if a.kind == AnchorKind::Smooth && a.has_in() && a.has_out() {
            return true;
        }
        let (pp, nn) = (prev.unwrap_or(a.p), next.unwrap_or(a.p));
        let dir = nn - pp;
        let l = dir.hypot();
        if l <= 1e-9 {
            return false;
        }
        let u = dir / l;
        a.h_in = a.p - u * (a.p.distance(pp) / 3.0);
        a.h_out = a.p + u * (a.p.distance(nn) / 3.0);
        a.kind = AnchorKind::Smooth;
        true
    }
    /// Cut at the anchors `at`: each becomes the end of one open piece and the start of the next,
    /// its handle off each piece retracted (the shape doesn't change). A closed subpath opens at
    /// its first cut, so one cut leaves one piece whose ends coincide. The ends of an open subpath
    /// and indices past the end don't cut; with no cut left, the subpath comes back as it is.
    pub fn cut_at(&self, at: &BTreeSet<usize>) -> Vec<SubPath> {
        let n = self.anchors.len();
        let mut cuts = at.iter().copied().filter(|&i| i < n && (self.closed || (i > 0 && i + 1 < n))).peekable();
        let Some(&first) = cuts.peek().filter(|_| n >= 2) else { return vec![self.clone()] };
        // A closed subpath's anchors from the first cut round to it again; an open one's as they are.
        let (run, cuts): (Vec<Anchor>, BTreeSet<usize>) = if self.closed {
            (self.anchors.iter().cycle().skip(first).take(n + 1).copied().collect(), cuts.map(|c| (c + n - first) % n).chain([n]).collect())
        } else {
            (self.anchors.clone(), cuts.collect())
        };
        let mut pieces = vec![];
        let mut cur: Vec<Anchor> = vec![];
        for (i, a) in run.into_iter().enumerate() {
            if !cuts.contains(&i) {
                cur.push(a);
                continue;
            }
            if !cur.is_empty() {
                cur.push(Anchor { h_out: a.p, kind: AnchorKind::Corner, ..a });
                pieces.push(SubPath::new(std::mem::take(&mut cur), false));
            }
            if i < n {
                cur.push(Anchor { h_in: a.p, kind: AnchorKind::Corner, ..a });
            }
        }
        if cur.len() > 1 {
            pieces.push(SubPath::new(cur, false));
        }
        pieces
    }
    /// Signed area via the shoelace formula on the Bézier path (positive = clockwise in y-down).
    pub fn area(&self) -> f64 {
        let mut bp = BezPath::new();
        self.to_bezpath_into(&mut bp);
        bp.area()
    }
}

/// A (possibly multi-subpath) path.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PathData {
    pub subpaths: Vec<SubPath>,
}

impl PathData {
    pub fn new(subpaths: Vec<SubPath>) -> Self {
        Self { subpaths }
    }
    pub fn single(sp: SubPath) -> Self {
        Self { subpaths: vec![sp] }
    }
    pub fn is_empty(&self) -> bool {
        self.subpaths.iter().all(|s| s.anchors.is_empty())
    }
    pub fn anchor_count(&self) -> usize {
        self.subpaths.iter().map(|s| s.anchors.len()).sum()
    }
    pub fn to_bezpath(&self) -> BezPath {
        let mut bp = BezPath::new();
        for sp in &self.subpaths {
            sp.to_bezpath_into(&mut bp);
        }
        bp
    }
    /// Convert a kurbo path to anchors. Quadratics are elevated to cubics.
    pub fn from_bezpath(bp: &BezPath) -> Self {
        let mut subpaths: Vec<SubPath> = Vec::new();
        let mut cur: Option<SubPath> = None;
        let mut last = Point::ZERO;
        for el in bp.elements() {
            match *el {
                PathEl::MoveTo(p) => {
                    if let Some(sp) = cur.take()
                        && !sp.anchors.is_empty()
                    {
                        subpaths.push(sp);
                    }
                    cur = Some(SubPath { anchors: vec![Anchor::corner(p)], closed: false });
                    last = p;
                }
                PathEl::LineTo(p) => {
                    let sp = cur.get_or_insert_with(|| SubPath { anchors: vec![Anchor::corner(last)], closed: false });
                    sp.anchors.push(Anchor::corner(p));
                    last = p;
                }
                PathEl::QuadTo(q, p) => {
                    let c1 = last + (q - last) * (2.0 / 3.0);
                    let c2 = p + (q - p) * (2.0 / 3.0);
                    push_curve(&mut cur, last, c1, c2, p);
                    last = p;
                }
                PathEl::CurveTo(c1, c2, p) => {
                    push_curve(&mut cur, last, c1, c2, p);
                    last = p;
                }
                PathEl::ClosePath => {
                    if let Some(mut sp) = cur.take() {
                        // Merge a closing anchor that duplicates the start.
                        if sp.anchors.len() > 1
                            && let (Some(first), Some(lastp)) = (sp.anchors.first().map(|a| a.p), sp.anchors.last().map(|a| a.p))
                            && first.distance(lastp) < 1e-7
                            && let Some(l) = sp.anchors.pop()
                            && let Some(a0) = sp.anchors.first_mut()
                        {
                            a0.h_in = l.h_in;
                        }
                        sp.closed = true;
                        for a in &mut sp.anchors {
                            *a = Anchor::with_handles(a.p, a.h_in, a.h_out);
                        }
                        last = sp.anchors.first().map(|a| a.p).unwrap_or(last);
                        subpaths.push(sp);
                    }
                }
            }
        }
        if let Some(mut sp) = cur
            && !sp.anchors.is_empty()
        {
            for a in &mut sp.anchors {
                *a = Anchor::with_handles(a.p, a.h_in, a.h_out);
            }
            subpaths.push(sp);
        }
        Self { subpaths }
    }
    /// Tight geometric bounds (curve extrema, not control points), measured by [`curve_bounds`]: a
    /// segment with a control point that is not finite or past `f64::MAX / 16` in x or y is measured
    /// by its control points.
    pub fn bounds(&self) -> Option<Rect> {
        if self.is_empty() {
            return None;
        }
        let bp = self.to_bezpath();
        if self.anchor_count() == 1
            && let Some(a) = self.subpaths.iter().find_map(|s| s.anchors.first())
        {
            return Some(Rect::from_points(a.p, a.p));
        }
        Some(curve_bounds(&bp))
    }
    /// Bounds including control handles (used for quick culling).
    pub fn control_bounds(&self) -> Option<Rect> {
        let mut it = self.subpaths.iter().flat_map(|s| s.anchors.iter()).flat_map(|a| [a.p, a.h_in, a.h_out]);
        let first = it.next()?;
        Some(it.fold(Rect::from_points(first, first), |r, p| r.union_pt(p)))
    }
    pub fn transform(&mut self, a: Affine) {
        for sp in &mut self.subpaths {
            for an in &mut sp.anchors {
                *an = an.transform(a);
            }
        }
    }
    pub fn transformed(&self, a: Affine) -> Self {
        let mut c = self.clone();
        c.transform(a);
        c
    }
    pub fn is_closed(&self) -> bool {
        !self.subpaths.is_empty() && self.subpaths.iter().all(|s| s.closed)
    }
    /// Nearest point on any segment: (subpath, segment, t, point, distance).
    pub fn nearest(&self, p: Point) -> Option<(usize, usize, f64, Point, f64)> {
        let mut best: Option<(usize, usize, f64, Point, f64)> = None;
        for (si, sp) in self.subpaths.iter().enumerate() {
            for seg in 0..sp.segment_count() {
                let c = sp.segment(seg);
                let n = c.nearest(p, 1e-6);
                let q = c.eval(n.t);
                let d = n.distance_sq.sqrt();
                if best.is_none_or(|b| d < b.4) {
                    best = Some((si, seg, n.t, q, d));
                }
            }
        }
        best
    }
    /// Iterate (subpath index, anchor index, anchor).
    pub fn anchors(&self) -> impl Iterator<Item = (usize, usize, &Anchor)> {
        self.subpaths.iter().enumerate().flat_map(|(si, s)| s.anchors.iter().enumerate().map(move |(ai, a)| (si, ai, a)))
    }
    pub fn anchor_mut(&mut self, si: usize, ai: usize) -> Option<&mut Anchor> {
        self.subpaths.get_mut(si)?.anchors.get_mut(ai)
    }
    pub fn reverse(&mut self) {
        for sp in &mut self.subpaths {
            sp.reverse();
        }
    }
    /// Total length of all segments.
    pub fn length(&self) -> f64 {
        self.to_bezpath().segments().map(|s| kurbo::ParamCurveArclen::arclen(&s, 1e-6)).sum()
    }
}

/// The largest x or y, in size, of a segment's control points for which [`segment_bounds`]
/// measures the segment by its extrema. kurbo adds up to eight coordinates together when it finds a
/// segment's extrema and evaluates the segment there, and a sum of eight coordinates of this size or
/// smaller is finite.
const EXTREMA_LIMIT: f64 = f64::MAX / 16.0;

/// The box of `path`'s segments, each measured by [`segment_bounds`] and joined in order as kurbo's
/// `bounding_box` joins them. A path with no segments gives the empty rect at the origin, as kurbo
/// does.
pub fn curve_bounds(path: &BezPath) -> Rect {
    path.segments().map(segment_bounds).reduce(|bb, seg_bb| bb.union(seg_bb)).unwrap_or_default()
}

/// The box of `seg` as kurbo's `bounding_box` measures it (its start, end and extrema), except
/// that a segment with a control point whose x or y is not finite or is larger in size than
/// `f64::MAX / 16` is measured by its control points. The box then holds every point the segment
/// draws, and it reaches infinity when a control point is infinite.
pub fn segment_bounds(seg: PathSeg) -> Rect {
    let pts = match seg {
        PathSeg::Line(l) => [l.p0, l.p1, l.p1, l.p1],
        PathSeg::Quad(q) => [q.p0, q.p1, q.p2, q.p2],
        PathSeg::Cubic(c) => [c.p0, c.p1, c.p2, c.p3],
    };
    if pts.iter().all(|p| p.x.abs() <= EXTREMA_LIMIT && p.y.abs() <= EXTREMA_LIMIT) {
        ParamCurveExtrema::bounding_box(&seg)
    } else {
        pts.iter().fold(Rect::from_points(pts[0], pts[0]), |r, &p| r.union_pt(p))
    }
}

fn push_curve(cur: &mut Option<SubPath>, last: Point, c1: Point, c2: Point, p: Point) {
    let sp = cur.get_or_insert_with(|| SubPath { anchors: vec![Anchor::corner(last)], closed: false });
    if let Some(a) = sp.anchors.last_mut() {
        a.h_out = c1;
    }
    sp.anchors.push(Anchor { p, h_in: c2, h_out: p, kind: AnchorKind::Corner });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square() -> PathData {
        PathData::single(SubPath::polyline(&[Point::new(0.0, 0.0), Point::new(10.0, 0.0), Point::new(10.0, 10.0), Point::new(0.0, 10.0)], true))
    }

    #[test]
    fn anchors_serialize_compactly_and_read_v1_maps() {
        let corner = Anchor::corner(Point::new(1.0, 2.0));
        assert_eq!(serde_json::to_string(&corner).unwrap(), r#"{"p":[1.0,2.0]}"#);
        let smooth = Anchor { p: Point::new(1.0, 2.0), h_in: Point::new(0.0, 2.0), h_out: Point::new(2.0, 2.0), kind: AnchorKind::Smooth };
        let back: Anchor = serde_json::from_str(&serde_json::to_string(&smooth).unwrap()).unwrap();
        assert_eq!(back, smooth);
        let v1 = r#"{"p":{"x":1.0,"y":2.0},"in":{"x":0.0,"y":2.0},"out":{"x":2.0,"y":2.0},"kind":"Smooth"}"#;
        assert_eq!(serde_json::from_str::<Anchor>(v1).unwrap(), smooth);
        let v1_corner = r#"{"p":{"x":1.0,"y":2.0},"in":{"x":1.0,"y":2.0},"out":{"x":1.0,"y":2.0},"kind":"Corner"}"#;
        assert_eq!(serde_json::from_str::<Anchor>(v1_corner).unwrap(), corner);
    }

    #[test]
    fn square_bounds_and_area() {
        let p = square();
        assert_eq!(p.bounds(), Some(Rect::new(0.0, 0.0, 10.0, 10.0)));
        assert!((p.subpaths[0].area().abs() - 100.0).abs() < 1e-9);
        assert_eq!(p.subpaths[0].segment_count(), 4);
    }

    #[test]
    fn bezpath_roundtrip_polyline() {
        let p = square();
        let bp = p.to_bezpath();
        let back = PathData::from_bezpath(&bp);
        assert_eq!(back.subpaths.len(), 1);
        assert_eq!(back.subpaths[0].anchors.len(), 4);
        assert!(back.subpaths[0].closed);
        assert_eq!(back.bounds(), p.bounds());
    }

    #[test]
    fn bezpath_roundtrip_curves() {
        let c = kurbo::Circle::new((50.0, 50.0), 20.0);
        let bp = c.to_path(1e-3);
        let pd = PathData::from_bezpath(&bp);
        assert_eq!(pd.subpaths.len(), 1);
        assert!(pd.subpaths[0].closed);
        let b = pd.bounds().unwrap();
        assert!((b.width() - 40.0).abs() < 1e-3);
        // All anchors of a circle are smooth.
        assert!(pd.subpaths[0].anchors.iter().all(|a| a.kind == AnchorKind::Smooth));
    }

    #[test]
    fn insert_anchor_on_line_keeps_shape() {
        let mut p = square();
        let before = p.bounds();
        let idx = p.subpaths[0].insert_anchor(0, 0.5);
        assert_eq!(idx, 1);
        assert_eq!(p.subpaths[0].anchors[1].p, Point::new(5.0, 0.0));
        assert_eq!(p.bounds(), before);
        assert_eq!(p.anchor_count(), 5);
    }

    #[test]
    fn insert_anchor_on_curve_keeps_shape() {
        let bp = kurbo::Circle::new((0.0, 0.0), 10.0).to_path(1e-3);
        let mut pd = PathData::from_bezpath(&bp);
        let area0 = pd.subpaths[0].area();
        pd.subpaths[0].insert_anchor(1, 0.3);
        assert!((pd.subpaths[0].area() - area0).abs() < 1e-6);
    }

    #[test]
    fn nearest_on_edge() {
        let p = square();
        let (_, seg, _, q, d) = p.nearest(Point::new(5.0, -3.0)).unwrap();
        assert_eq!(seg, 0);
        assert!((q.x - 5.0).abs() < 1e-6 && q.y.abs() < 1e-6);
        assert!((d - 3.0).abs() < 1e-6);
    }

    #[test]
    fn reverse_flips_area_sign() {
        let mut p = square();
        let a = p.subpaths[0].area();
        p.reverse();
        assert!((p.subpaths[0].area() + a).abs() < 1e-9);
    }

    #[test]
    fn transform_moves_bounds() {
        let p = square().transformed(Affine::translate((5.0, 5.0)));
        assert_eq!(p.bounds(), Some(Rect::new(5.0, 5.0, 15.0, 15.0)));
    }

    /// A curve from (100, 100) to (300, 100) with the given out and in handles.
    fn curve(h_out: Point, h_in: Point) -> PathData {
        let a = Anchor::with_handles(Point::new(100.0, 100.0), Point::new(100.0, 100.0), h_out);
        let b = Anchor::with_handles(Point::new(300.0, 100.0), h_in, Point::new(300.0, 100.0));
        PathData::single(SubPath::new(vec![a, b], false))
    }

    /// A curve whose handles are at ±1e308 or infinite is measured by its control points: its box
    /// holds every finite point of the curve, and an infinite handle makes the box infinite. Handles
    /// at ±`f64::MAX / 16` are measured by the curve's extrema, and that box holds the curve too.
    #[test]
    fn bounds_hold_a_curve_whose_handles_are_huge() {
        // `eval` rounds: a point may sit a few ulps outside the box.
        let within = |v: f64, lo: f64, hi: f64| {
            let e = 1e-12 * v.abs().max(1.0);
            lo - e <= v && v <= hi + e
        };
        let holds_curve = |h_out: Point, h_in: Point, b: Rect| {
            let c = CubicBez::new(Point::new(100.0, 100.0), h_out, h_in, Point::new(300.0, 100.0));
            for i in 0..=20 {
                let q = c.eval(f64::from(i) / 20.0);
                if q.is_finite() {
                    assert!(within(q.x, b.x0, b.x1) && within(q.y, b.y0, b.y1), "{h_out:?} {h_in:?}: {q:?} is outside {b:?}");
                }
            }
        };
        let cases = [
            (Point::new(100.0, 1e308), Point::new(300.0, 1e308)),
            (Point::new(100.0, -1e308), Point::new(300.0, 1e308)),
            (Point::new(-1.7e308, 100.0), Point::new(1.7e308, 100.0)),
            (Point::new(f64::INFINITY, 100.0), Point::new(250.0, 100.0)),
        ];
        for (h_out, h_in) in cases {
            let p = curve(h_out, h_in);
            let b = p.bounds().unwrap();
            assert_eq!(Some(b), p.control_bounds(), "{h_out:?} {h_in:?}");
            holds_curve(h_out, h_in, b);
        }
        assert!(!curve(Point::new(f64::INFINITY, 100.0), Point::new(250.0, 100.0)).bounds().unwrap().x1.is_finite());
        let m = f64::MAX / 16.0;
        for (h_out, h_in) in [(Point::new(100.0, m), Point::new(300.0, m)), (Point::new(100.0, -m), Point::new(300.0, m))] {
            let b = curve(h_out, h_in).bounds().unwrap();
            assert!(b.y0.is_finite() && b.y1.is_finite() && b.y1 < m, "{h_out:?} {h_in:?}: {b:?}");
            holds_curve(h_out, h_in, b);
        }
    }

    /// Ordinary paths, and curves whose handles are large but below `f64::MAX / 16`, measure as
    /// kurbo's `bounding_box` measures them, bit for bit.
    #[test]
    fn bounds_of_ordinary_paths_match_kurbo_bit_for_bit() {
        let circle = PathData::from_bezpath(&kurbo::Circle::new((50.0, 50.0), 20.0).to_path(1e-3));
        let s_curve = curve(Point::new(180.0, -40.0), Point::new(220.0, 240.0));
        let mut two = square();
        two.subpaths.extend(circle.transformed(Affine::translate((-75.5, 30.25))).subpaths);
        let lone = PathData::new(vec![SubPath::new(vec![Anchor::corner(Point::new(1.0, 2.0))], false), square().subpaths[0].clone()]);
        let quad = PathData::from_bezpath(&BezPath::from_svg("M0 0 Q 50 -80 100 0 Q 150 80 200 0").unwrap());
        for p in [square(), circle, s_curve, two, lone, quad, curve(Point::new(100.0, 1e300), Point::new(300.0, -1e300))] {
            let ours = p.bounds().unwrap();
            let kurbo = p.to_bezpath().bounding_box();
            let bits = |r: Rect| [r.x0, r.y0, r.x1, r.y1].map(f64::to_bits);
            assert_eq!(bits(ours), bits(kurbo), "{ours:?} {kurbo:?}");
        }
    }

    #[test]
    fn smooth_detection() {
        let a = Anchor::with_handles(Point::new(0.0, 0.0), Point::new(-1.0, 0.0), Point::new(2.0, 0.0));
        assert_eq!(a.kind, AnchorKind::Smooth);
        let c = Anchor::with_handles(Point::new(0.0, 0.0), Point::new(-1.0, 0.0), Point::new(0.0, 2.0));
        assert_eq!(c.kind, AnchorKind::Corner);
    }

    #[test]
    fn length_of_square() {
        assert!((square().length() - 40.0).abs() < 1e-6);
    }

    #[test]
    fn serde_roundtrip() {
        let p = square();
        let s = serde_json::to_string(&p).unwrap();
        let back: PathData = serde_json::from_str(&s).unwrap();
        assert_eq!(p, back);
    }
}
