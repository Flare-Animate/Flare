//! Illustrator's Pathfinder panel and Shape Builder regions, over an ordered stack of shapes
//! (index 0 = back-most, last = front-most).

use std::collections::HashMap;

use kurbo::{BezPath, Line, ParamCurve, ParamCurveNearest, PathSeg, Point, Shape as _};
use linesweeper::topology::ContourIdx;
use crate::geom::{FillRule, PathData};

use crate::boolean::{
    Arrangement, Multi, Seg, Tidy, all_contours_to_path, contours_to_path, eps_for, fill_bezpath, normalize_bez, segs_to_subpath, to_seg, unite_all,
};

/// A filled shape in a Pathfinder stack. `key` identifies its paint (e.g. a hashed fill colour);
/// results carry the key of the object whose paint they keep.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub path: PathData,
    pub rule: FillRule,
    pub key: u64,
}

impl Shape {
    pub fn new(path: PathData, rule: FillRule, key: u64) -> Self {
        Self { path, rule, key }
    }
}

/// The ten Pathfinder panel operations.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PathfinderOp {
    /// Shape mode: union of everything; result keeps the front-most key.
    Unite,
    /// Shape mode: back-most minus everything in front; keeps the back-most key.
    MinusFront,
    /// Shape mode: area covered by *all* shapes; keeps the front-most key.
    Intersect,
    /// Shape mode: area covered by an odd number of shapes; keeps the front-most key.
    Exclude,
    /// Split into every non-overlapping face; each face keeps the key of the front-most shape
    /// covering it.
    Divide,
    /// Remove hidden parts; one result per shape that remains visible (no merging).
    Trim,
    /// Trim, then merge touching/overlapping visible parts with the same key.
    Merge,
    /// Keep the visible parts of lower shapes that lie inside the front-most shape (which is removed).
    Crop,
    /// Split every edge at intersections; returns *open* paths keyed by the shape they bound.
    Outline,
    /// Front-most minus everything behind; keeps the front-most key.
    MinusBack,
}

/// One face of the planar arrangement of a set of shapes (Shape Builder / Live Paint face).
#[derive(Clone, Debug, PartialEq)]
pub struct Region {
    /// Face outline (outer contour plus holes, consistently oriented).
    pub path: PathData,
    /// Indices of the input shapes covering this face, ascending.
    pub sources: Vec<usize>,
}

impl Region {
    /// Front-most covering shape (`None` only for an uncovered face, which regions never are).
    pub fn top(&self) -> Option<usize> {
        self.sources.last().copied()
    }
    /// Does the face contain `p`?
    pub fn contains(&self, p: Point) -> bool {
        self.path.to_bezpath().winding(p) != 0
    }
}

fn arrangement(shapes: &[Shape]) -> Option<Arrangement> {
    let bps: Vec<(BezPath, FillRule)> = shapes.iter().map(|s| (fill_bezpath(&s.path), s.rule)).collect();
    Arrangement::new(bps).ok()
}

/// The one item of `it`, if it has exactly one.
fn only(mut it: impl Iterator<Item = usize>) -> Option<usize> {
    let first = it.next()?;
    it.next().is_none().then_some(first)
}

fn one(path: PathData, key: u64) -> Vec<Shape> {
    if path.is_empty() { Vec::new() } else { vec![Shape::new(path, FillRule::NonZero, key)] }
}

/// Run a Pathfinder operation over `shapes` (back → front).
pub fn pathfinder(op: PathfinderOp, shapes: &[Shape]) -> Vec<Shape> {
    let n = shapes.len();
    if n == 0 {
        return Vec::new();
    }
    let front_key = shapes[n - 1].key;
    let unite = || {
        let v: Vec<(&PathData, FillRule)> = shapes.iter().map(|s| (&s.path, s.rule)).collect();
        one(unite_all(&v), front_key)
    };
    // Unite doesn't need the arrangement.
    if op == PathfinderOp::Unite {
        return unite();
    }
    // Open paths are cutting edges for Divide; the filled-only operations keep their
    // implicit closure. Reuse Shape Builder's bounded planar map and its fallback.
    if op == PathfinderOp::Divide && crate::has_open_path(shapes) {
        return crate::shape_builder(shapes, false)
            .regions
            .into_iter()
            .filter_map(|r| {
                let key = shapes.get(r.top()?)?.key;
                Some(Shape::new(r.path, FillRule::NonZero, key))
            })
            .collect();
    }
    let Some(arr) = arrangement(shapes) else { return Vec::new() };
    let p = &arr.tidy;
    match op {
        PathfinderOp::Unite => unite(),
        // The tests read the covering inputs straight off the windings ([`Arrangement::covered`]);
        // each is the coverage-mask test in its comment.
        // m[0] && !m[1..].any()
        PathfinderOp::MinusFront => one(all_contours_to_path(&arr.contours_where(|w| only(arr.covered(w)) == Some(0)), p), shapes[0].key),
        // m[n - 1] && !m[..n - 1].any()
        PathfinderOp::MinusBack => one(all_contours_to_path(&arr.contours_where(|w| only(arr.covered(w)) == Some(n - 1)), p), front_key),
        // m.all()
        PathfinderOp::Intersect => one(all_contours_to_path(&arr.contours_where(|w| arr.covered(w).count() == n), p), front_key),
        // m.count() is odd
        PathfinderOp::Exclude => one(all_contours_to_path(&arr.contours_where(|w| arr.covered(w).count() % 2 == 1), p), front_key),
        PathfinderOp::Divide => regions_of(&arr)
            .into_iter()
            .filter_map(|r| {
                let key = shapes.get(r.top()?)?.key;
                Some(Shape::new(r.path, FillRule::NonZero, key))
            })
            .collect(),
        // topmost(m) == Some(i)
        PathfinderOp::Trim => {
            (0..n).flat_map(|i| one(all_contours_to_path(&arr.contours_where(|w| arr.covered(w).next_back() == Some(i)), p), shapes[i].key)).collect()
        }
        PathfinderOp::Merge => {
            let mut keys: Vec<u64> = Vec::new();
            for s in shapes {
                if !keys.contains(&s.key) {
                    keys.push(s.key);
                }
            }
            let mut out = Vec::new();
            for k in keys {
                // topmost(m) is a shape with key k
                let c = arr.contours_where(|w| arr.covered(w).next_back().is_some_and(|t| shapes[t].key == k));
                for g in c.grouped() {
                    out.extend(one(contours_to_path(&c, g, p), k));
                }
            }
            out
        }
        PathfinderOp::Crop => {
            if n < 2 {
                return Vec::new();
            }
            (0..n - 1)
                .flat_map(|i| {
                    // m[n - 1] && topmost(&m[..n - 1]) == Some(i): the front shape, then i below it
                    let c = arr.contours_where(|w| {
                        let mut c = arr.covered(w).rev();
                        c.next() == Some(n - 1) && c.next() == Some(i)
                    });
                    one(all_contours_to_path(&c, p), shapes[i].key)
                })
                .collect()
        }
        PathfinderOp::Outline => outline_edges(&arr, shapes),
    }
}

/// The faces of `arr`, each distinct coverage mask's in turn (sorted), each face grouped as
/// `arr.contours(|m| m == mask).grouped()` groups it.
///
/// A contour pass walks the whole arrangement, so one pass per mask would cost masks × edges
/// (two blends crossing make thousands of each). Instead the masks go into batches whose faces
/// share no vertex (so no edge either): one pass per batch walks each face's contours exactly as
/// its own mask's pass would, and the mask a contour bounds is the one of its batch at its first
/// point.
fn regions_of(arr: &Arrangement) -> Vec<Region> {
    let masks = arr.distinct_masks();
    let ids: HashMap<&[bool], usize> = masks.iter().enumerate().map(|(i, m)| (m.as_slice(), i)).collect();
    let id = |w: &Multi| ids.get(arr.mask(w).as_slice()).copied();
    let top = &arr.top;
    // The masks of the faces around each vertex.
    let mut around: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
    for s in top.segment_indices() {
        let h = s.first_half();
        let sides = [id(top.winding_clockwise(h)), id(top.winding_counter_clockwise(h))];
        for end in [h, s.second_half()] {
            let at = around.entry(vertex_key(top.point(end).to_kurbo())).or_default();
            for m in sides.into_iter().flatten() {
                if !at.contains(&m) {
                    at.push(m);
                }
            }
        }
    }
    let mut near = vec![Vec::new(); masks.len()];
    for at in around.values() {
        for &a in at {
            near[a].extend(at.iter().copied().filter(|&b| b != a));
        }
    }
    // Greedy colouring: each mask takes the first batch none of its neighbours is in.
    let mut batch = vec![usize::MAX; masks.len()];
    let mut batches = 0;
    for m in 0..masks.len() {
        let mut taken = vec![false; batches];
        for &n in &near[m] {
            if let Some(t) = taken.get_mut(batch[n]) {
                *t = true;
            }
        }
        batch[m] = taken.iter().position(|t| !t).unwrap_or(batches);
        batches = batches.max(batch[m] + 1);
    }
    let mut faces: Vec<Vec<PathData>> = vec![Vec::new(); masks.len()];
    for b in 0..batches {
        let c = top.contours(|w| id(w).is_some_and(|m| batch[m] == b));
        let owner: Option<Vec<usize>> = c
            .contours()
            .map(|ct| {
                let Some(kurbo::PathEl::MoveTo(p)) = ct.path.elements().first() else { return None };
                around.get(&vertex_key(*p))?.iter().copied().find(|&m| batch[m] == b)
            })
            .collect();
        let Some(owner) = owner else {
            // Not expected: every contour starts at a vertex of the arrangement. Fall back to a
            // pass per mask for this batch.
            for (m, mask) in masks.iter().enumerate().filter(|(m, _)| batch[*m] == b) {
                let c = arr.contours(|k| k == mask.as_slice());
                faces[m].extend(c.grouped().into_iter().map(|g| contours_to_path(&c, g, &arr.tidy)));
            }
            continue;
        };
        // A contour belongs under the nearest enclosing contour of the same mask, as in that
        // mask's own pass.
        let mut children = vec![Vec::new(); owner.len()];
        let mut roots = Vec::new();
        for (i, &m) in owner.iter().enumerate() {
            let mut up = c[ContourIdx(i)].parent;
            while let Some(p) = up.filter(|p| owner.get(p.0) != Some(&m)) {
                up = c[p].parent;
            }
            match up {
                Some(p) => children[p.0].push(i),
                None => roots.push(i),
            }
        }
        for r in roots {
            let mut group = Vec::new();
            let mut stack = vec![r];
            while let Some(i) = stack.pop() {
                group.push(ContourIdx(i));
                stack.extend(children[i].iter().rev());
            }
            faces[owner[r]].push(contours_to_path(&c, group, &arr.tidy));
        }
    }
    let mut out = Vec::new();
    for (mask, paths) in masks.iter().zip(faces) {
        let sources: Vec<usize> = mask.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i).collect();
        out.extend(paths.into_iter().filter(|p| !p.is_empty()).map(|path| Region { path, sources: sources.clone() }));
    }
    out
}

/// A point as a map key (the arrangement's vertices are exact).
fn vertex_key(p: Point) -> (u64, u64) {
    (p.x.to_bits(), p.y.to_bits())
}

/// All faces of the planar arrangement of `shapes` (every area covered by at least one shape,
/// split wherever coverage changes).
pub fn regions(shapes: &[Shape]) -> Vec<Region> {
    arrangement(shapes).map(|a| regions_of(&a)).unwrap_or_default()
}

/// The face under `point`, if any (Shape Builder hover/click).
pub fn region_at(shapes: &[Shape], point: Point) -> Option<Region> {
    regions(shapes).into_iter().find(|r| r.contains(point))
}

/// Merge Shape Builder regions into one path (their union).
pub fn merge_regions(regions: &[&Region]) -> PathData {
    let v: Vec<(&PathData, FillRule)> = regions.iter().map(|r| (&r.path, FillRule::NonZero)).collect();
    unite_all(&v)
}

/// Merges faces of the planar arrangement of a set of shapes (as [`regions`] gives them) into one
/// path, from the arrangement itself, made once for every merge of the set: the faces' contours
/// share their edges, so the edges between merged faces vanish (when two faces' copies of an edge
/// differ in the first merge to unite contours, every point within 16 eps of a point kept before
/// it moves onto that point first, in that merge and every later one). Uniting the faces' tidied
/// outlines ([`merge_regions`]) could leave those edges in, as a hairline gap or a spur, where
/// neighbouring faces' outlines had been refit apart. Shapes with open paths (faces lines cut, as
/// [`shape_builder`](crate::shape_builder) gives them) merge in their planar map the same way
/// (#893), and so do faces with a gap closed paths enclose, which no shape covers (#1691). Either
/// falls back to [`merge_regions`] when it can't place a face.
pub struct FaceMerger {
    /// The arrangement (`None`: open paths, or the sweep failed) and the number of shapes.
    arr: Option<Arrangement>,
    shapes: usize,
    /// The shapes, and their planar map once a merge has needed it (open paths, or a gap).
    lines: Option<(Vec<Shape>, std::cell::OnceCell<Option<crate::planar::Map>>)>,
    /// Whether every merge moves the points of the faces' contours together first: the first
    /// merge that unites contours decides it (`unite_faces`).
    snap: std::cell::OnceCell<bool>,
}

impl FaceMerger {
    pub fn new(shapes: &[Shape]) -> Self {
        let has_open = crate::has_open_path(shapes);
        // The Shape Builder's faces come from the planar map only up to its cap.
        let segments: usize = shapes.iter().flat_map(|s| &s.path.subpaths).map(|sp| sp.segment_count()).sum();
        let lines = (segments <= crate::SHAPE_BUILDER_MAX_SEGMENTS).then(|| (shapes.to_vec(), std::cell::OnceCell::new()));
        Self { arr: if has_open { None } else { arrangement(shapes) }, shapes: shapes.len(), lines, snap: std::cell::OnceCell::new() }
    }

    /// The union of `faces`.
    pub fn merge(&self, faces: &[&Region]) -> PathData {
        self.exact(faces).or_else(|| self.planar(faces)).filter(|p| !p.is_empty()).unwrap_or_else(|| merge_regions(faces))
    }

    /// `faces` merged in the planar map of the shapes: each face found again by a point inside
    /// it, and the result checked to cover what they cover.
    fn planar(&self, faces: &[&Region]) -> Option<PathData> {
        let (shapes, map) = self.lines.as_ref()?;
        let map = map.get_or_init(|| crate::planar::map(shapes)).as_ref()?;
        let inside: Vec<Point> = faces.iter().map(|r| crate::planar::interior_point(&r.path)).collect::<Option<_>>()?;
        let merged = map.merge(&inside)?;
        // A face not found as itself (two points in one face of the map) shows in the area.
        let want: f64 = faces.iter().map(|r| crate::area(&r.path, FillRule::NonZero)).sum();
        let outlines: f64 = faces.iter().map(|r| r.path.to_bezpath().perimeter(1e-6)).sum();
        ((crate::area(&merged, FillRule::NonZero) - want).abs() <= 1e-9 * want.max(1.0) + crate::DEFAULT_PRECISION * outlines).then_some(merged)
    }

    fn exact(&self, faces: &[&Region]) -> Option<PathData> {
        // A gap no shape covers is no face of the arrangement: it merges in the planar map.
        if faces.iter().any(|r| r.sources.is_empty()) {
            return None;
        }
        let arr = self.arr.as_ref()?;
        // Each face by its coverage and a point inside it.
        let mut picks: Vec<(Vec<bool>, Point)> = Vec::with_capacity(faces.len());
        for r in faces {
            let mut mask = vec![false; self.shapes];
            for &s in &r.sources {
                *mask.get_mut(s)? = true;
            }
            picks.push((mask, crate::planar::interior_point(&r.path)?));
        }
        let mut raw = BezPath::new();
        let mut done: Vec<&[bool]> = vec![];
        for (mask, _) in &picks {
            if done.contains(&mask.as_slice()) {
                continue;
            }
            done.push(mask);
            let c = arr.contours(|k| k == mask.as_slice());
            for group in c.grouped() {
                let mut face = BezPath::new();
                for i in group {
                    face.extend(c[i].path.iter());
                }
                if picks.iter().any(|(m, p)| m == mask && face.winding(*p) != 0) {
                    raw.extend(face.iter());
                }
            }
        }
        unite_faces(&raw, &arr.tidy, &self.snap)
    }
}

/// The union of the faces whose contours are `raw`, tidied with `tidy`. The points of the
/// contours move together first ([`snap_points`]) when `snap` holds `true`. When `snap` is empty,
/// it is set to whether two copies of an edge in `raw` differ.
fn unite_faces(raw: &BezPath, tidy: &Tidy, snap: &std::cell::OnceCell<bool>) -> Option<PathData> {
    // The planar map joins the ends of copies of an edge within 16 eps too.
    let eps = eps_for([raw]).ok()?;
    let snapped = if snap.get() == Some(&false) {
        None
    } else {
        let (snapped, differ) = snap_points(raw, 16.0 * eps);
        snap.get_or_init(|| differ).then_some(snapped)
    };
    let merged = normalize_bez(snapped.as_ref().unwrap_or(raw), FillRule::NonZero).ok()?;
    Some(all_contours_to_path(&merged, tidy))
}

/// `raw` with the points of its contours gathered onto points more than `tol` apart: each point
/// moves onto a point kept before it within `tol`, or is kept, and the handles next to it move
/// along. A piece left within `tol` of one point is dropped. It also returns whether two copies of
/// an edge differ, before the move or after it ([`copies_differ`]).
///
/// Two faces' contours can carry the edge they share as pieces whose ends lie a few eps apart, the
/// difference left as pieces shorter than eps: linesweeper 0.5 rebuilds a contour's run of pieces
/// of one input curve from that curve and does not move the rebuilt ends onto the sweep's points.
/// Uniting the faces as they are can leave the space between the two copies in, as a hairline that
/// reaches a hole through those short pieces (a spur) or as a seam that splits the outline (#1770).
/// Copies that share their ends leave at most a hairline contour of their own, which
/// [`is_sliver`](crate::boolean::is_sliver) drops.
fn snap_points(raw: &BezPath, tol: f64) -> (BezPath, bool) {
    let cell = |p: Point| ((p.x / tol).floor() as i64, (p.y / tol).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<(usize, Point)>> = HashMap::new();
    let mut snapped: HashMap<(u64, u64), (usize, Point)> = HashMap::new();
    let mut kept: usize = 0;
    let mut snap = |p: Point| {
        *snapped.entry(vertex_key(p)).or_insert_with(|| {
            let (cx, cy) = cell(p);
            let near = (cx - 1..=cx + 1)
                .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
                .filter_map(|k| grid.get(&k))
                .flatten()
                .find(|(_, q)| q.distance(p) <= tol)
                .copied();
            near.unwrap_or_else(|| {
                let v = (kept, p);
                kept += 1;
                grid.entry((cx, cy)).or_default().push(v);
                v
            })
        })
    };
    let mut out = BezPath::new();
    // Each contour's pieces, each with the segment it was and the indices of the two points it runs
    // between.
    let mut contours: Vec<Vec<(PathSeg, PathSeg, [usize; 2])>> = Vec::new();
    for (range, closed) in crate::geom::bez::subpaths(raw) {
        let Some(els) = raw.elements().get(range) else { continue };
        let mut pieces = Vec::new();
        for s in crate::geom::bez::segments(els) {
            let ((i, a), (j, b)) = (snap(s.start()), snap(s.end()));
            let mut seg = to_seg(s);
            crate::planar::pin_ends(std::slice::from_mut(&mut seg), a, b);
            if a == b && seg.c.p1.distance(a) <= tol && seg.c.p2.distance(a) <= tol {
                continue;
            }
            if pieces.is_empty() {
                out.move_to(a);
            }
            let piece = if (a, b) == (s.start(), s.end()) {
                s
            } else if seg.line {
                PathSeg::Line(Line::new(a, b))
            } else {
                PathSeg::Cubic(seg.c)
            };
            out.push(piece.as_path_el());
            pieces.push((piece, s, [i, j]));
        }
        if !pieces.is_empty() && closed {
            out.close_path();
        }
        contours.push(pieces);
    }
    let differ = copies_differ(&contours, kept, tol);
    (out, differ)
}

/// Whether two copies of an edge in `contours` differ. Each piece comes with the segment it was
/// before the move and the indices of the two points it runs between, out of `points`. A copy is a
/// chain of pieces between junctions, the points where other than two piece ends meet. Two chains
/// that run opposite ways between the same two junctions, with the middle and the end of each
/// piece within `tol` of the other chain, are copies of one edge, and they differ when one is not
/// the exact reverse of the other, after the move or before it.
fn copies_differ(contours: &[Vec<(PathSeg, PathSeg, [usize; 2])>], points: usize, tol: f64) -> bool {
    let mut degree = vec![0usize; points];
    for &v in contours.iter().flatten().flat_map(|(_, _, ends)| ends) {
        if let Some(d) = degree.get_mut(v) {
            *d += 1;
        }
    }
    let junction = |v: usize| degree.get(v) != Some(&2);
    // (pieces, the segments they were, first point, last point). A contour through no junction
    // shares no point with another contour.
    let mut chains: Vec<(Vec<PathSeg>, Vec<PathSeg>, usize, usize)> = Vec::new();
    for pieces in contours {
        let Some(first) = pieces.iter().position(|(_, _, [a, _])| junction(*a)) else { continue };
        let (mut cur, mut was): (Vec<PathSeg>, Vec<PathSeg>) = (Vec::new(), Vec::new());
        let (mut from, mut to) = (0, 0);
        for &(s, w, [a, b]) in pieces.iter().cycle().skip(first).take(pieces.len()) {
            if junction(a) && !cur.is_empty() {
                chains.push((std::mem::take(&mut cur), std::mem::take(&mut was), from, to));
            }
            if cur.is_empty() {
                from = a;
            }
            cur.push(s);
            was.push(w);
            to = b;
        }
        chains.push((cur, was, from, to));
    }
    let key = |a: usize, b: usize| (a.min(b), a.max(b));
    let mut by_ends: HashMap<(usize, usize), Vec<usize>> = HashMap::new();
    for (k, (_, _, a, b)) in chains.iter().enumerate() {
        by_ends.entry(key(*a, *b)).or_default().push(k);
    }
    let reversed = |a: &[PathSeg], b: &[PathSeg]| a.iter().rev().map(|s| s.reverse()).eq(b.iter().copied());
    let mut paired = vec![false; chains.len()];
    for (k, (segs, was, a, b)) in chains.iter().enumerate() {
        if paired.get(k) != Some(&false) {
            continue;
        }
        let Some(candidates) = by_ends.get(&key(*a, *b)) else { continue };
        let found = candidates.iter().filter(|&&j| j > k && paired.get(j) == Some(&false)).find_map(|&j| {
            let (other, other_was, c, d) = chains.get(j)?;
            (c == b && d == a && along(segs, other, tol)).then_some((j, other, other_was))
        });
        let Some((j, other, other_was)) = found else { continue };
        for i in [k, j] {
            if let Some(p) = paired.get_mut(i) {
                *p = true;
            }
        }
        if !reversed(segs, other) || !reversed(was, other_was) {
            return true;
        }
    }
    false
}

/// Whether the middle and the end of every piece of `a` lie within `tol` of `b`, and the middle
/// and the end of every piece of `b` lie within `tol` of `a`. `a` and `b` run opposite ways.
fn along(a: &[PathSeg], b: &[PathSeg], tol: f64) -> bool {
    let covers = |from: &[PathSeg], to: &[PathSeg]| {
        // Each point's search goes back along `to` from the piece the point before it was near.
        let mut at = to.len();
        from.iter().flat_map(|s| [s.eval(0.5), s.end()]).all(|p| {
            let mut order = to.iter().enumerate().take(at).rev().chain(to.iter().enumerate().skip(at).rev());
            let Some((i, _)) = order.find(|(_, t)| t.nearest(p, 1e-12).distance_sq.sqrt() <= tol) else { return false };
            at = i + 1;
            true
        })
    };
    covers(a, b) && covers(b, a)
}

/// Pathfinder Outline: split every shape's boundary at junctions of the arrangement.
fn outline_edges(arr: &Arrangement, shapes: &[Shape]) -> Vec<Shape> {
    // Sorted by x (then y), so a segment needs only the junctions inside its control box.
    let junctions = arr.junctions();
    let scale = shapes.iter().filter_map(|s| s.path.control_bounds()).map(|r| r.width().max(r.height())).fold(1.0, f64::max);
    let tol = scale * 1e-7;
    let reach = tol.max(1e-6);
    // (chain, key, source index)
    let mut chains: Vec<(Vec<Seg>, u64)> = Vec::new();
    for s in shapes {
        for sp in &s.path.subpaths {
            if sp.anchors.len() < 2 {
                continue;
            }
            let mut sp = sp.clone();
            sp.closed = true;
            // Split each segment at junctions; record which piece starts are breaks.
            let mut pieces: Vec<(Seg, bool)> = Vec::new();
            for i in 0..sp.segment_count() {
                let c = sp.segment(i);
                let line = sp.segment_is_line(i);
                // A point within `reach` of the curve is within `reach` of its control box (the
                // curve stays inside its control points' hull), so the other junctions can't match.
                let b = kurbo::Shape::bounding_box(&c).union(kurbo::Rect::from_points(c.p1, c.p2)).inflate(reach, reach);
                let lo = junctions.partition_point(|j| j.x < b.x0);
                let mut ts: Vec<f64> = junctions
                    .get(lo..)
                    .unwrap_or_default()
                    .iter()
                    .take_while(|j| j.x <= b.x1)
                    .filter(|j| j.y >= b.y0 && j.y <= b.y1)
                    .filter_map(|&j| {
                        let nr = c.nearest(j, 1e-9);
                        (nr.distance_sq.sqrt() <= reach).then_some(nr.t)
                    })
                    .collect();
                ts.sort_by(f64::total_cmp);
                let start_is_junction = ts.first().is_some_and(|&t| t < 1e-9);
                let mut cuts: Vec<f64> = vec![0.0];
                cuts.extend(ts.into_iter().filter(|&t| t > 1e-9 && t < 1.0 - 1e-9));
                cuts.push(1.0);
                cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
                for (k, w) in cuts.windows(2).enumerate() {
                    let sub = c.subsegment(w[0]..w[1]);
                    let seg = if line { Seg::line(sub.p0, sub.p3) } else { Seg { c: sub, line: false } };
                    pieces.push((seg, if k == 0 { start_is_junction } else { true }));
                }
            }
            // Chain pieces between breaks.
            let first_break = pieces.iter().position(|(_, b)| *b);
            match first_break {
                None => {
                    let segs: Vec<Seg> = pieces.into_iter().map(|(s, _)| s).collect();
                    chains.push((segs, s.key));
                }
                Some(k) => {
                    pieces.rotate_left(k);
                    let mut cur: Vec<Seg> = Vec::new();
                    for (seg, brk) in pieces {
                        if brk && !cur.is_empty() {
                            chains.push((std::mem::take(&mut cur), s.key));
                        }
                        cur.push(seg);
                    }
                    if !cur.is_empty() {
                        chains.push((cur, s.key));
                    }
                }
            }
        }
    }
    // De-duplicate coincident chains, keeping the front-most key (later shapes overwrite). A
    // match has its midpoint within `near` of this one's, so only the chains in the grid cells
    // around it are candidates; the first of them in `out` order is the one a scan would find.
    let mut out: Vec<(Vec<Seg>, u64)> = Vec::new();
    let mut sigs: Vec<(Point, Point, Point)> = Vec::new();
    let sig = |c: &[Seg]| -> (Point, Point, Point) {
        let a = c[0].c.p0;
        let b = c[c.len() - 1].c.p3;
        let mid = mid_point(c);
        (a, b, mid)
    };
    let d = reach * 10.0;
    let near = |p: Point, q: Point| p.distance(q) <= d;
    let cell = |p: Point| ((p.x / d).floor() as i64, (p.y / d).floor() as i64);
    let mut grid: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (chain, key) in chains {
        let (a, b, m) = sig(&chain);
        let (cx, cy) = cell(m);
        let found = (cx - 1..=cx + 1)
            .flat_map(|x| (cy - 1..=cy + 1).map(move |y| (x, y)))
            .filter_map(|k| grid.get(&k))
            .flatten()
            .copied()
            .filter(|&i| {
                let (a2, b2, m2) = sigs[i];
                near(m, m2) && ((near(a, a2) && near(b, b2)) || (near(a, b2) && near(b, a2)))
            })
            .min();
        match found {
            Some(i) => out[i].1 = key,
            None => {
                grid.entry((cx, cy)).or_default().push(out.len());
                sigs.push((a, b, m));
                out.push((chain, key));
            }
        }
    }
    out.into_iter()
        .filter_map(|(chain, key)| {
            // Chain joints are the shapes' own anchors, so there is nothing to re-merge.
            let sp = segs_to_subpath(&chain, false)?;
            Some(Shape::new(PathData::single(sp), FillRule::NonZero, key))
        })
        .collect()
}

fn mid_point(c: &[Seg]) -> Point {
    let lens: Vec<f64> = c.iter().map(|s| kurbo::ParamCurveArclen::arclen(&s.c, 1e-6)).collect();
    let total: f64 = lens.iter().sum();
    let mut acc = 0.0;
    for (s, l) in c.iter().zip(&lens) {
        if acc + l >= total / 2.0 && *l > 0.0 {
            let t = kurbo::ParamCurveArclen::inv_arclen(&s.c, total / 2.0 - acc, 1e-6);
            return s.c.eval(t);
        }
        acc += l;
    }
    c[0].c.p0
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use crate::geom::{Rect, SubPath, shapes};

    /// The faces one contour pass per mask finds: what [`regions_of`] must match exactly.
    fn regions_one_mask_at_a_time(arr: &Arrangement) -> Vec<Region> {
        let mut out = Vec::new();
        for mask in arr.distinct_masks() {
            let c = arr.contours(|m| m == mask.as_slice());
            let sources: Vec<usize> = mask.iter().enumerate().filter(|(_, b)| **b).map(|(i, _)| i).collect();
            for g in c.grouped() {
                let path = contours_to_path(&c, g, &arr.tidy);
                if !path.is_empty() {
                    out.push(Region { path, sources: sources.clone() });
                }
            }
        }
        out
    }

    fn same_as_one_mask_at_a_time(shapes: &[Shape]) {
        let arr = arrangement(shapes).expect("arrangement");
        assert_eq!(regions_of(&arr), regions_one_mask_at_a_time(&arr));
    }

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> PathData {
        shapes::rectangle(Rect::new(x0, y0, x1, y1))
    }

    /// Shapes on a coarse grid, so that edges and corners coincide: rectangles, rectangles with
    /// a hole, ellipses and polygons that may cross themselves, under either fill rule.
    fn arb_shape() -> impl Strategy<Value = Shape> {
        let c = || (0..12i32).prop_map(|v| f64::from(v) * 10.0);
        let r = (c(), c(), 1..6i32, 1..6i32).prop_map(|(x, y, w, h)| (x, y, x + f64::from(w) * 10.0, y + f64::from(h) * 10.0));
        let path = prop_oneof![
            r.clone().prop_map(|(x0, y0, x1, y1)| rect(x0, y0, x1, y1)),
            r.clone().prop_map(|(x0, y0, x1, y1)| {
                let (cx, cy) = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
                let mut p = rect(x0 - 10.0, y0 - 10.0, x1 + 10.0, y1 + 10.0);
                p.subpaths.extend(rect(x0, y0, cx.max(x0 + 5.0), cy.max(y0 + 5.0)).subpaths);
                p
            }),
            r.prop_map(|(x0, y0, x1, y1)| shapes::ellipse(Rect::new(x0, y0, x1, y1))),
            prop::collection::vec((c(), c()), 3..7).prop_map(|pts| {
                let pts: Vec<Point> = pts.into_iter().map(|(x, y)| Point::new(x, y)).collect();
                PathData::single(SubPath::polyline(&pts, true))
            }),
        ];
        (path, any::<bool>(), any::<u64>())
            .prop_map(|(p, even_odd, key)| Shape::new(p, if even_odd { FillRule::EvenOdd } else { FillRule::NonZero }, key))
    }

    #[test]
    fn crossing_strips_match_one_mask_at_a_time() {
        let mut v = Vec::new();
        for i in 0..12 {
            let x = 10.0 + 20.0 * f64::from(i);
            v.push(Shape::new(rect(x, 0.0, x + 4.0, 250.0), FillRule::NonZero, 1));
            v.push(Shape::new(rect(0.0, x, 250.0, x + 4.0), FillRule::NonZero, 2));
        }
        same_as_one_mask_at_a_time(&v);
        // Every crossing, and each strip in 13 pieces.
        assert_eq!(regions(&v).len(), 12 * 12 + 2 * 12 * 13);
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, failure_persistence: None, rng_seed: proptest::test_runner::RngSeed::Fixed(0x5eed_9a7f), ..ProptestConfig::default() })]

        /// Batching the masks changes nothing: the same regions, in the same order, with the same
        /// contours in each.
        #[test]
        fn batched_regions_match_one_mask_at_a_time(v in prop::collection::vec(arb_shape(), 1..8)) {
            same_as_one_mask_at_a_time(&v);
        }
    }

    /// The edge that the two faces of the #1770 tests share.
    fn shared_edge() -> kurbo::CubicBez {
        kurbo::CubicBez::new((100.0, 0.0), (130.0, 30.0), (130.0, 70.0), (100.0, 100.0))
    }

    /// Two faces on either side of [`shared_edge`]. The right face's copy of the edge has its ends
    /// moved along the bottom and top edges by `bottom` and `top`, its handles moved with them, and
    /// its first handle moved by `nudge` more.
    fn faces_apart(bottom: f64, top: f64, nudge: f64) -> BezPath {
        use kurbo::{CubicBez, Vec2};
        let edge = shared_edge();
        let (p0, p3) = (edge.p0 + Vec2::new(bottom, 0.0), edge.p3 + Vec2::new(top, 0.0));
        let copy = CubicBez::new(p0, edge.p1 + (p0 - edge.p0) + Vec2::new(0.0, nudge), edge.p2 + (p3 - edge.p3), p3);
        let mut raw = BezPath::new();
        raw.move_to((0.0, 0.0));
        raw.line_to(edge.p0);
        raw.curve_to(edge.p1, edge.p2, edge.p3);
        raw.line_to((0.0, 100.0));
        raw.close_path();
        raw.move_to(copy.p0);
        raw.line_to((200.0, 0.0));
        raw.line_to((200.0, 100.0));
        raw.line_to(copy.p3);
        raw.curve_to(copy.p2, copy.p1, copy.p0);
        raw.close_path();
        raw
    }

    /// Two faces whose copies of [`shared_edge`] are exact reverses of each other. The left face
    /// has a point 5e-6 pt from another point.
    fn faces_matching() -> BezPath {
        let edge = shared_edge();
        let mut raw = BezPath::new();
        raw.move_to((0.0, 0.0));
        raw.line_to(edge.p0);
        raw.curve_to(edge.p1, edge.p2, edge.p3);
        raw.line_to((0.0, 100.0));
        raw.curve_to((-20.0, 90.0), (-30.0, 70.0), (-30.0, 50.0));
        raw.line_to((-30.0, 50.0 - 5e-6));
        raw.curve_to((-20.0, 40.0), (-20.0, 10.0), (0.0, 0.0));
        raw.close_path();
        raw.move_to(edge.p0);
        raw.line_to((200.0, 0.0));
        raw.line_to((200.0, 100.0));
        raw.line_to(edge.p3);
        raw.curve_to(edge.p2, edge.p1, edge.p0);
        raw.close_path();
        raw
    }

    /// #1770: two faces' contours carry the edge they share as copies whose ends lie a few eps
    /// apart, at one end or at both, with or without a handle a rounding error apart. The faces
    /// unite into one outline, with no hairline left between the copies.
    #[test]
    fn faces_unite_into_one_outline_when_their_copies_of_an_edge_end_a_few_eps_apart() {
        let tidy = Tidy::free(crate::DEFAULT_PRECISION);
        for (bottom, top, nudge) in [(0.0, 3e-6, 4e-7), (3e-6, 3e-6, 4e-7), (0.0, 3e-6, 0.0), (3e-6, 3e-6, 0.0)] {
            let merged = unite_faces(&faces_apart(bottom, top, nudge), &tidy, &std::cell::OnceCell::new()).unwrap();
            assert_eq!(merged.subpaths.len(), 1, "ends apart by {bottom} and {top}, nudge {nudge}: {merged:?}");
            assert!((merged.length() - 600.0).abs() < 1e-3, "ends apart by {bottom} and {top}, nudge {nudge}: {} pt of outline", merged.length());
        }
    }

    /// #1770: when the faces' copies of every edge they share are exact reverses of each other,
    /// the faces unite as they are, and a point 5e-6 pt from another point stays where it is.
    #[test]
    fn faces_whose_copies_of_each_shared_edge_match_unite_as_they_are() {
        let tidy = Tidy::free(crate::DEFAULT_PRECISION);
        let raw = faces_matching();
        let as_they_are = all_contours_to_path(&normalize_bez(&raw, FillRule::NonZero).unwrap(), &tidy);
        assert_eq!(as_they_are.subpaths.len(), 1, "{as_they_are:?}");
        assert_eq!(unite_faces(&raw, &tidy, &std::cell::OnceCell::new()), Some(as_they_are));
    }

    /// #1770: the first union decides for every later union of the same set whether the points
    /// move together. After faces whose copies of an edge end apart, the faces whose copies match
    /// lose their point 5e-6 pt from another point.
    #[test]
    fn later_unions_move_points_as_the_first_union_decided() {
        let tidy = Tidy::free(crate::DEFAULT_PRECISION);
        let snap = std::cell::OnceCell::new();
        let first = unite_faces(&faces_apart(0.0, 3e-6, 0.0), &tidy, &snap).unwrap();
        assert_eq!(first.subpaths.len(), 1, "{first:?}");
        let later = unite_faces(&faces_matching(), &tidy, &snap).unwrap();
        let moved = Point::new(-30.0, 50.0 - 5e-6);
        assert!(later.subpaths.iter().flat_map(|sp| &sp.anchors).all(|a| a.p != moved), "{later:?}");
    }

    #[test]
    fn divide_open_paths_cut_faces_without_taking_the_cutters_paint() {
        let line = |a: (f64, f64), b: (f64, f64)| shapes::line(Point::from(a), Point::from(b));
        let mut cases = vec![
            ("horizontal", vec![line((-10.0, 50.0), (110.0, 50.0))], 2),
            ("reverse", vec![line((110.0, 50.0), (-10.0, 50.0))], 2),
            ("vertical", vec![line((50.0, -10.0), (50.0, 110.0))], 2),
            ("diagonal", vec![line((-10.0, -10.0), (110.0, 110.0))], 2),
            ("crossed", vec![line((-10.0, 50.0), (110.0, 50.0)), line((50.0, -10.0), (50.0, 110.0))], 4),
            ("outside", vec![line((-10.0, 150.0), (110.0, 150.0))], 1),
            ("dangling", vec![line((-10.0, 50.0), (50.0, 50.0))], 1),
            ("boundary", vec![line((-10.0, 0.0), (110.0, 0.0))], 1),
            ("tangent", vec![line((-10.0, 10.0), (10.0, -10.0))], 1),
        ];
        let mut curve = BezPath::new();
        curve.move_to((-10.0, 50.0));
        curve.curve_to((30.0, 15.0), (70.0, 85.0), (110.0, 50.0));
        cases.push(("curved", vec![PathData::from_bezpath(&curve)], 2));
        let mut bent = BezPath::new();
        bent.move_to((-10.0, 25.0));
        bent.line_to((50.0, 75.0));
        bent.line_to((110.0, 25.0));
        cases.push(("bent", vec![PathData::from_bezpath(&bent)], 2));
        for (label, cutters, count) in cases {
            let mut inputs = vec![Shape::new(rect(0.0, 0.0, 100.0, 100.0), FillRule::NonZero, 7)];
            inputs.extend(cutters.into_iter().map(|p| Shape::new(p, FillRule::NonZero, 99)));
            let divided = pathfinder(PathfinderOp::Divide, &inputs);
            assert_eq!(divided.len(), count, "{label}");
            assert!(divided.iter().all(|s| s.key == 7 && s.path.subpaths.iter().all(|sp| sp.closed)), "{label}");
            let total: f64 = divided.iter().map(|s| crate::area(&s.path, s.rule)).sum();
            assert!((total - 10000.0).abs() < 0.01, "{label}: area {total}");
            for p in [Point::new(20.0, 31.0), Point::new(79.0, 37.0), Point::new(81.0, 68.0), Point::new(17.0, 85.0)] {
                assert_eq!(divided.iter().filter(|s| s.path.to_bezpath().winding(p) != 0).count(), 1, "{label}: {p:?}");
            }
        }
    }

    #[test]
    fn divide_open_paths_preserve_frontmost_fills_and_holes() {
        let cutter = || Shape::new(shapes::line(Point::new(-10.0, 50.0), Point::new(160.0, 50.0)), FillRule::NonZero, 99);
        let inputs = [
            Shape::new(rect(0.0, 0.0, 100.0, 100.0), FillRule::NonZero, 7),
            Shape::new(rect(50.0, 0.0, 150.0, 100.0), FillRule::NonZero, 8),
            cutter(),
        ];
        let faces = pathfinder(PathfinderOp::Divide, &inputs);
        assert_eq!(faces.len(), 6);
        assert_eq!(faces.iter().filter(|s| s.key == 7).count(), 2);
        assert_eq!(faces.iter().filter(|s| s.key == 8).count(), 4);
        let total: f64 = faces.iter().map(|s| crate::area(&s.path, s.rule)).sum();
        assert!((total - 15000.0).abs() < 1e-6);
        let mut ring = rect(0.0, 0.0, 100.0, 100.0);
        ring.subpaths.extend(rect(30.0, 30.0, 70.0, 70.0).subpaths);
        let faces = pathfinder(PathfinderOp::Divide, &[Shape::new(ring, FillRule::EvenOdd, 7), cutter()]);
        assert_eq!(faces.len(), 2);
        assert!(faces.iter().all(|s| s.key == 7 && s.path.to_bezpath().winding(Point::new(50.0, 40.0)) == 0));
        let total: f64 = faces.iter().map(|s| crate::area(&s.path, s.rule)).sum();
        assert!((total - 8400.0).abs() < 1e-6);
    }

    #[test]
    fn divide_open_paths_retains_the_existing_segment_cap() {
        let mut inputs = vec![Shape::new(rect(0.0, 0.0, 100.0, 100.0), FillRule::NonZero, 7)];
        inputs.extend((0..4093).map(|_| Shape::new(shapes::line(Point::new(-10.0, 50.0), Point::new(110.0, 50.0)), FillRule::NonZero, 99)));
        let faces = pathfinder(PathfinderOp::Divide, &inputs);
        assert_eq!(faces.len(), 1, "4097 segments fall back to filled areas");
        assert_eq!(faces[0].key, 7);
        assert!((crate::area(&faces[0].path, faces[0].rule) - 10000.0).abs() < 1e-6);
    }
}
