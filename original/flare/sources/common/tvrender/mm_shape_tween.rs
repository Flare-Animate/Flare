//! Shape tweens: from a keyframe of shapes to the next keyframe's, the shapes in
//! between. The two keyframes' fills pair up in order, as do their strokes, and
//! each outline morphs into its partner point for point: both are lined up where
//! they lie nearest each other, cut at the same points (every corner either has,
//! and more, evenly spaced) and blended. A closed stroke morphs as a fill's
//! outline does, so it keeps to the fill it edges. Shape hints pin points of one
//! outline to the other's instead, and an angular blend pins corners to corners:
//! both outlines are then cut into runs between those points.

use tiny_skia::PathSegment;

use crate::Result;
use crate::doc::*;
use crate::hit::polyline;
use crate::render::fill_path;

/// How near an outline, in pixels, a shape hint pins it.
pub const REACH: f32 = 8.0;

/// The in-between `offset` frames into `key`'s shape tween, or None to show the
/// keyframe as it is: at its first frame, or when the tween is broken.
pub fn at(key: &Frame, next: Option<&Frame>, offset: u32) -> Option<Shape> {
    let Tween::Shape { ease, angular, ref hints } = key.tween else { return None };
    if offset == 0 || !valid(key, next) {
        return None;
    }
    let (a, b) = (outlines(&key.elements), outlines(&next?.elements));
    let t = offset as f32 / key.duration as f32;
    Some(morph(&a, &b, t + ease * t * (1.0 - t), angular, hints))
}

/// Whether point `p` lies on the outline of a fill or stroke of `elements`' shapes,
/// within `within`: where a shape hint does its pinning.
pub fn on_outline(elements: &[Element], p: Point, within: f32) -> bool {
    snap(elements, p, within).is_some()
}

/// The point of the outlines of `elements`' fills and strokes nearest `p`, if within
/// `within`: where a shape hint dropped by an outline goes.
pub fn snap(elements: &[Element], p: Point, within: f32) -> Option<Point> {
    let o = outlines(elements);
    let fills = o.fills.iter().flat_map(|f| &f.1).flat_map(|l| (0..l.len()).map(move |i| (l[i], l[(i + 1) % l.len()])));
    let strokes = o.strokes.iter().flat_map(|s| s.1.windows(2).map(|w| (w[0], w[1])));
    let points = fills.chain(strokes).map(|(a, b)| nearest(p, a, b)).filter(|&q| dist2(q, p) <= within * within);
    points.min_by(|&q, &r| dist2(q, p).total_cmp(&dist2(r, p)))
}

/// Shape hints as XFL holds them here: "x y x y" (start, end) per hint, `;` between.
// ponytail: how Animate saves shape hints is unconfirmed, so they go in a `shapeHints`
// attribute of our own, which reads back; Animate's own stay in the frame's `extra`.
pub(crate) fn read_hints(s: &str) -> Result<Vec<(Point, Point)>> {
    let hint = |h: &str| -> Result<(Point, Point)> {
        let v: Vec<f32> = h.split_whitespace().map(str::parse).collect::<std::result::Result<_, _>>()?;
        match v[..] {
            [x0, y0, x1, y1] => Ok(((x0, y0), (x1, y1))),
            _ => Err(format!("shape hint {h:?}").into()),
        }
    };
    s.split(';').filter(|h| !h.trim().is_empty()).map(hint).collect()
}

pub(crate) fn write_hints(hints: &[(Point, Point)]) -> String {
    let hints: Vec<String> = hints.iter().map(|((x0, y0), (x1, y1))| format!("{x0} {y0} {x1} {y1}")).collect();
    hints.join(";")
}

/// Whether `key`'s shape tween plays: the next keyframe starts where it ends, and
/// both hold shapes only. A broken one holds its first keyframe, as in Animate.
pub fn valid(key: &Frame, next: Option<&Frame>) -> bool {
    let shapes = |k: &Frame| !k.elements.is_empty() && k.elements.iter().all(|el| matches!(el, Element::Shape(_)));
    matches!(key.tween, Tween::Shape { .. }) && next.is_some_and(|n| n.index == key.index + key.duration && shapes(n)) && shapes(key)
}

impl Layer {
    /// Create Shape Tween on the span holding `f`; one already there keeps its settings.
    /// A motion tween it replaces leaves its target as its first frame shows it.
    pub fn create_shape_tween(&mut self, f: u32) {
        if let Some(i) = self.keyframe_at(f)
            && !matches!(self.frames[i].tween, Tween::Shape { .. })
        {
            self.remove_tween(f);
            self.frames[i].tween = Tween::Shape { ease: 0.0, angular: false, hints: Vec::new() };
        }
    }
}

/// A keyframe's shapes flattened, in the timeline's coordinates, bottom first:
/// each fill with its outlines as closed loops of points, each stroke's paths
/// joined into runs of them (a closed one coming back to its start).
#[derive(Default)]
struct Outlines {
    fills: Vec<(Paint, Vec<Vec<Point>>)>,
    strokes: Vec<(Stroke, Vec<Point>)>,
}

fn outlines(elements: &[Element]) -> Outlines {
    let mut out = Outlines::default();
    for el in elements {
        let Element::Shape(s) = el else { continue };
        let m = s.matrix;
        let to_stage = |points: Vec<Point>| points.into_iter().map(|p| m.apply(p)).collect::<Vec<_>>();
        for (i, paint) in s.fills.iter().enumerate() {
            let loops = fill_path(&s.edges, i as u32 + 1).map(|p| loops(&p)).unwrap_or_default();
            if !loops.is_empty() {
                out.fills.push((paint.clone(), loops.into_iter().map(to_stage).collect()));
            }
        }
        // A stroke scales with its shape, as render.rs draws it.
        let scale = (m.a * m.d - m.b * m.c).abs().sqrt();
        for (i, run) in stroke_runs(&s.edges) {
            let Some(style) = (i as usize).checked_sub(1).and_then(|i| s.strokes.get(i)) else { continue };
            out.strokes.push((Stroke { width: style.width * scale, ..style.clone() }, to_stage(run)));
        }
    }
    out
}

/// The stroked paths of `edges` joined end to end, one stroke style each, so an
/// outline stroked in pieces (as XFL often has it) morphs whole, like its fill.
fn stroke_runs(edges: &[Edge]) -> Vec<(u32, Vec<Point>)> {
    let mut pieces: Vec<(u32, Vec<Point>)> = edges.iter().filter(|e| e.stroke > 0).flat_map(|e| e.paths.iter().map(|p| (e.stroke, polyline(p)))).collect();
    let mut runs = Vec::new();
    while !pieces.is_empty() {
        let (style, mut run) = pieces.remove(0);
        // Grown at its end, then turned round and grown at its start, until it closes.
        for _ in 0..2 {
            while closed(&run).is_none() {
                let end = run[run.len() - 1];
                let Some(i) = pieces.iter().position(|(s, p)| *s == style && (near(p[0], end) || near(p[p.len() - 1], end))) else { break };
                let mut p = pieces.remove(i).1;
                if !near(p[0], end) {
                    p.reverse();
                }
                run.extend_from_slice(&p[1..]);
            }
            run.reverse();
        }
        runs.push((style, run));
    }
    runs
}

/// A run that comes back to its start, as a loop without the point closing it.
fn closed(run: &[Point]) -> Option<&[Point]> {
    let mut l = run;
    while l.len() > 1 && near(l[0], l[l.len() - 1]) {
        l = &l[..l.len() - 1];
    }
    (l.len() > 2 && l.len() < run.len()).then_some(l)
}

/// A fill's path as loops of points, each curve cut into 16 chords.
fn loops(path: &tiny_skia::Path) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = Vec::new();
    let mut from = (0.0, 0.0);
    for seg in path.segments() {
        let points: Vec<Point> = match seg {
            PathSegment::MoveTo(p) => {
                out.push(Vec::new());
                vec![(p.x, p.y)]
            }
            PathSegment::LineTo(p) => vec![(p.x, p.y)],
            PathSegment::QuadTo(c, p) => (1..=16)
                .map(|i| {
                    let t = i as f32 / 16.0;
                    let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
                    (u * from.0 + v * c.x + w * p.x, u * from.1 + v * c.y + w * p.y)
                })
                .collect(),
            // fill_path makes no cubic curves.
            PathSegment::CubicTo(_, _, p) => vec![(p.x, p.y)],
            PathSegment::Close => continue,
        };
        from = points.last().copied().unwrap_or(from);
        if let Some(l) = out.last_mut() {
            l.extend(points);
        }
    }
    for l in &mut out {
        // Loops come back to where they started; the blend closes them itself.
        while l.len() > 1 && near(l[0], l[l.len() - 1]) {
            l.pop();
        }
    }
    out.retain(|l| l.len() > 2);
    out
}

fn morph(a: &Outlines, b: &Outlines, t: f32, angular: bool, hints: &[(Point, Point)]) -> Shape {
    let mut s = Shape { matrix: Matrix::IDENTITY, fills: Vec::new(), strokes: Vec::new(), edges: Vec::new(), extra: Extra::default() };
    for i in 0..a.fills.len().max(b.fills.len()) {
        // A fill the other keyframe lacks shrinks away, or grows from nothing.
        let (paint, loops) = match (a.fills.get(i), b.fills.get(i)) {
            (Some((pa, la)), Some((pb, lb))) => (blend_paint(pa, pb, t), blend_loops(la, lb, t, angular, hints)),
            (Some((p, l)), None) => (p.clone(), l.iter().map(|l| shrink(l, t)).collect()),
            (None, Some((p, l))) => (p.clone(), l.iter().map(|l| shrink(l, 1.0 - t)).collect()),
            (None, None) => continue,
        };
        s.fills.push(paint);
        let paths = loops.iter().filter_map(|l| path(l, true)).collect();
        s.edges.push(Edge { fill0: 0, fill1: s.fills.len() as u32, stroke: 0, paths });
    }
    for i in 0..a.strokes.len().max(b.strokes.len()) {
        let (style, run, round) = match (a.strokes.get(i), b.strokes.get(i)) {
            (Some((sa, ra)), Some((sb, rb))) => {
                let style = Stroke { width: sa.width + (sb.width - sa.width) * t, paint: blend_paint(&sa.paint, &sb.paint, t), ..sa.clone() };
                // Closed both ends, it morphs as a fill's outline, so it keeps to the fill it edges.
                match (closed(ra), closed(rb)) {
                    (Some(la), Some(lb)) => (style, blend_loop(la, lb, t, angular, hints), true),
                    _ => (style, blend_run(ra, rb, t), false),
                }
            }
            (Some((st, r)), None) => (st.clone(), shrink(r, t), false),
            (None, Some((st, r))) => (st.clone(), shrink(r, 1.0 - t), false),
            (None, None) => continue,
        };
        s.strokes.push(style);
        let paths = path(&run, round).into_iter().collect();
        s.edges.push(Edge { fill0: 0, fill1: 0, stroke: s.strokes.len() as u32, paths });
    }
    s
}

/// Solid colors blend; other paints switch halfway.
fn blend_paint(a: &Paint, b: &Paint, t: f32) -> Paint {
    match (a, b) {
        (Paint::Solid(x), Paint::Solid(y)) => Paint::Solid(std::array::from_fn(|i| x[i] + (y[i] - x[i]) * t)),
        _ if t < 0.5 => a.clone(),
        _ => b.clone(),
    }
}

/// A fill's loops, the biggest of each keyframe paired up (outer outlines before holes).
fn blend_loops(a: &[Vec<Point>], b: &[Vec<Point>], t: f32, angular: bool, hints: &[(Point, Point)]) -> Vec<Vec<Point>> {
    fn by_size(loops: &[Vec<Point>]) -> Vec<&Vec<Point>> {
        let mut v: Vec<&Vec<Point>> = loops.iter().collect();
        v.sort_by(|p, q| area(q).abs().total_cmp(&area(p).abs()));
        v
    }
    let (a, b) = (by_size(a), by_size(b));
    (0..a.len().max(b.len()))
        .map(|i| match (a.get(i), b.get(i)) {
            (Some(p), Some(q)) => blend_loop(p, q, t, angular, hints),
            (Some(p), None) => shrink(p, t),
            (None, Some(q)) => shrink(q, 1.0 - t),
            (None, None) => Vec::new(),
        })
        .collect()
}

fn blend_loop(a: &[Point], b: &[Point], t: f32, angular: bool, hints: &[(Point, Point)]) -> Vec<Point> {
    // However each was drawn, so a fill's outline and the stroke round it line up alike.
    let (mut a, mut b) = (canonical(a), canonical(b));
    let pins = pins(&mut a, &mut b, hints);
    if !pins.is_empty() {
        return blend_runs(&a, &b, &pins, t);
    }
    if angular && let Some(corners) = by_corners(&a, &b) {
        return blend_runs(&a, &b, &corners, t);
    }
    let n = a.len().max(b.len()).clamp(64, 512);
    let (pa, pb) = (resample(&a, n, true), resample(&b, n, true));
    // `b` started where the two lie nearest each other all round, or where it starts if that's
    // as near: rounding mustn't turn a box a quarter round.
    // ponytail: O(n²) in the points (512 at most a loop); line up a coarser sampling first if it shows.
    let cost: Vec<f32> = (0..n).map(|k| (0..n).map(|i| dist2(pa[i], pb[(i + k) % n])).sum()).collect();
    let best = cost.iter().copied().fold(f32::INFINITY, f32::min);
    let k = if cost[0] <= best * 1.0001 { 0 } else { cost.iter().position(|&c| c == best).unwrap_or(0) };
    let (pa, pb) = matched(&a, &starting(&b, k as f32 / n as f32), n, true);
    blend(&pa, &pb, t)
}

/// Loop `l` running clockwise from its top left point, whichever way round and
/// from wherever it was drawn.
fn canonical(l: &[Point]) -> Vec<Point> {
    let mut l = l.to_vec();
    if area(&l) < 0.0 {
        l.reverse();
    }
    let first = (0..l.len()).min_by(|&i, &j| l[i].1.total_cmp(&l[j].1).then(l[i].0.total_cmp(&l[j].0)));
    l.rotate_left(first.unwrap_or(0));
    l
}

/// Loop `l` from the point `s` of the way round it, by length.
fn starting(l: &[Point], s: f32) -> Vec<Point> {
    let j = fractions(l, true).partition_point(|&f| f <= s);
    let mut out: Vec<Point> = along(l, true, &[s]).into_iter().chain(l[j..].iter().copied()).chain(l[..j].iter().copied()).collect();
    out.dedup_by(|p, q| near(*p, *q));
    while out.len() > 1 && near(out[0], out[out.len() - 1]) {
        out.pop();
    }
    out
}

/// The shape hints pinning a point of loop `a` to one of `b` (each within `REACH` of its
/// loop), those points put into the loops, as their indices, in `a`'s order. Should `b`
/// have them in another order, just the first holds.
fn pins(a: &mut Vec<Point>, b: &mut Vec<Point>, hints: &[(Point, Point)]) -> Vec<(usize, usize)> {
    let put: Vec<(Point, Point)> = hints.iter().filter_map(|&(p, q)| Some((put(a, p)?, put(b, q)?))).collect();
    let index = |l: &[Point], p: Point| l.iter().position(|&q| q == p);
    let mut pins: Vec<(usize, usize)> = put.into_iter().filter_map(|(p, q)| Some((index(a, p)?, index(b, q)?))).collect();
    pins.sort();
    pins.dedup_by_key(|p| p.0);
    // Going round `a` in order, `b`'s go round once in order too: one step back, at most.
    let backs = (0..pins.len()).filter(|&i| pins[(i + 1) % pins.len()].1 <= pins[i].1).count();
    if backs > 1 {
        pins.truncate(1);
    }
    pins
}

/// The point of loop `l` nearest `p`, if within `REACH`: one of its own, or put in as one.
fn put(l: &mut Vec<Point>, p: Point) -> Option<Point> {
    let n = l.len();
    let (i, q) = (0..n).map(|i| (i, nearest(p, l[i], l[(i + 1) % n]))).min_by(|x, y| dist2(x.1, p).total_cmp(&dist2(y.1, p)))?;
    if dist2(q, p) > REACH * REACH {
        return None;
    }
    match [l[i], l[(i + 1) % n]].into_iter().find(|&v| near(v, q)) {
        Some(v) => Some(v),
        None => {
            l.insert(i + 1, q);
            Some(q)
        }
    }
}

/// The point of segment `ab` nearest `p`.
fn nearest(p: Point, a: Point, b: Point) -> Point {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (a.0 + dx * t, a.1 + dy * t)
}

/// Angular blend: the loops' corners lined up with corners, as pins. None unless both have
/// as many (three or more).
fn by_corners(a: &[Point], b: &[Point]) -> Option<Vec<(usize, usize)>> {
    let (ca, cb) = (corners(a), corners(b));
    let m = ca.len();
    if m < 3 || cb.len() != m {
        return None;
    }
    let cost = |s: usize| (0..m).map(|j| dist2(a[ca[j]], b[cb[(j + s) % m]])).sum::<f32>();
    let shift = (0..m).min_by(|&s, &r| cost(s).total_cmp(&cost(r)))?;
    Some((0..m).map(|j| (ca[j], cb[(j + shift) % m])).collect())
}

/// Loops `a` and `b` blended run by run between their `pins`, each run cut at the same
/// points as its partner, so pinned points meet.
fn blend_runs(a: &[Point], b: &[Point], pins: &[(usize, usize)], t: f32) -> Vec<Point> {
    let (mut pa, mut pb) = (Vec::new(), Vec::new());
    for (j, &(a0, b0)) in pins.iter().enumerate() {
        let (a1, b1) = pins[(j + 1) % pins.len()];
        let (ra, rb) = (run(a, a0, a1), run(b, b0, b1));
        let (ra, rb) = matched(&ra, &rb, ra.len().max(rb.len()).max(8), false);
        // Each run's end starts the next.
        pa.extend_from_slice(&ra[..ra.len() - 1]);
        pb.extend_from_slice(&rb[..rb.len() - 1]);
    }
    blend(&pa, &pb, t)
}

/// Where a loop turns by more than 25°.
fn corners(l: &[Point]) -> Vec<usize> {
    let n = l.len();
    (0..n)
        .filter(|&i| {
            let (p, q, r) = (l[(i + n - 1) % n], l[i], l[(i + 1) % n]);
            let (u, v) = ((q.0 - p.0, q.1 - p.1), (r.0 - q.0, r.1 - q.1));
            let (cross, dot) = (u.0 * v.1 - u.1 * v.0, u.0 * v.0 + u.1 * v.1);
            cross.atan2(dot).abs() > 25f32.to_radians()
        })
        .collect()
}

/// The points of loop `l` from `i` round to `j`, both included.
fn run(l: &[Point], i: usize, j: usize) -> Vec<Point> {
    let n = l.len();
    let steps = (j + n - i) % n;
    (0..=if steps == 0 { n } else { steps }).map(|k| l[(i + k) % n]).collect()
}

/// A stroked path into another, both cut at the same points, its ends paired the nearer way.
fn blend_run(a: &[Point], b: &[Point], t: f32) -> Vec<Point> {
    let mut b = b.to_vec();
    let (a0, a1, b0, b1) = (a[0], a[a.len() - 1], b[0], b[b.len() - 1]);
    if dist2(a0, b1) + dist2(a1, b0) < dist2(a0, b0) + dist2(a1, b1) {
        b.reverse();
    }
    let (pa, pb) = matched(a, &b, a.len().max(b.len()).clamp(16, 256), false);
    blend(&pa, &pb, t)
}

fn blend(a: &[Point], b: &[Point], t: f32) -> Vec<Point> {
    a.iter().zip(b).map(|(p, q)| (p.0 + (q.0 - p.0) * t, p.1 + (q.1 - p.1) * t)).collect()
}

/// `n` points evenly spaced along `points` (round the loop when `closed`, else from end to end).
fn resample(points: &[Point], n: usize, closed: bool) -> Vec<Point> {
    let steps = if closed { n } else { n - 1 }.max(1) as f32;
    along(points, closed, &(0..n).map(|i| i as f32 / steps).collect::<Vec<_>>())
}

/// `a` and `b` cut at the same fractions of their length: at every point either has,
/// and at `n` evenly spaced, so corners as far along both meet as corners.
fn matched(a: &[Point], b: &[Point], n: usize, closed: bool) -> (Vec<Point>, Vec<Point>) {
    let steps = if closed { n } else { n - 1 }.max(1);
    let even = (0..=steps).map(|i| i as f32 / steps as f32).filter(|&s| !closed || s < 1.0);
    let mut at: Vec<f32> = fractions(a, closed).into_iter().chain(fractions(b, closed)).chain(even).collect();
    at.sort_by(f32::total_cmp);
    at.dedup_by(|s, r| (*s - *r).abs() < 1e-6);
    (along(a, closed, &at), along(b, closed, &at))
}

/// The points `at` (fractions 0..=1, in order) of the way along `points` by length.
fn along(points: &[Point], closed: bool, at: &[f32]) -> Vec<Point> {
    let Some(&first) = points.first() else { return Vec::new() };
    let pts: Vec<Point> = points.iter().copied().chain(closed.then_some(first)).collect();
    if pts.len() < 2 {
        return vec![first; at.len()];
    }
    let lengths = lengths(&pts);
    let total = lengths[lengths.len() - 1];
    at.iter()
        .map(|&s| {
            let want = total * s;
            let j = lengths.partition_point(|&l| l <= want).clamp(1, pts.len() - 1);
            let run = lengths[j] - lengths[j - 1];
            let f = if run > 0.0 { ((want - lengths[j - 1]) / run).min(1.0) } else { 0.0 };
            (pts[j - 1].0 + (pts[j].0 - pts[j - 1].0) * f, pts[j - 1].1 + (pts[j].1 - pts[j - 1].1) * f)
        })
        .collect()
}

/// How far along `points` each of them lies, as a fraction of the whole length.
fn fractions(points: &[Point], closed: bool) -> Vec<f32> {
    let Some(&first) = points.first() else { return Vec::new() };
    let lengths = lengths(&points.iter().copied().chain(closed.then_some(first)).collect::<Vec<_>>());
    let total = lengths[lengths.len() - 1];
    lengths[..points.len()].iter().map(|l| if total > 0.0 { l / total } else { 0.0 }).collect()
}

/// The length of `points` up to each of them.
fn lengths(points: &[Point]) -> Vec<f32> {
    let mut lengths = vec![0.0f32];
    for w in points.windows(2) {
        lengths.push(lengths[lengths.len() - 1] + dist2(w[0], w[1]).sqrt());
    }
    lengths
}

/// `points` shrunk toward their middle, `t` of the way to nothing.
fn shrink(points: &[Point], t: f32) -> Vec<Point> {
    let n = points.len().max(1) as f32;
    let c = points.iter().fold((0.0, 0.0), |c, p| (c.0 + p.0 / n, c.1 + p.1 / n));
    points.iter().map(|p| (p.0 + (c.0 - p.0) * t, p.1 + (c.1 - p.1) * t)).collect()
}

/// Twice the signed area of a loop: positive clockwise on screen.
fn area(l: &[Point]) -> f32 {
    (0..l.len()).map(|i| {
        let (p, q) = (l[i], l[(i + 1) % l.len()]);
        p.0 * q.1 - q.0 * p.1
    }).sum()
}

/// The points as a path of lines, closed back to its start when `closed`.
fn path(points: &[Point], closed: bool) -> Option<Path> {
    let (&start, rest) = points.split_first()?;
    let segs: Vec<Seg> = rest.iter().copied().chain(closed.then_some(start)).map(Seg::Line).collect();
    (!segs.is_empty()).then_some(Path { start, segs })
}

fn dist2(a: Point, b: Point) -> f32 {
    (a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)
}

fn near(a: Point, b: Point) -> bool {
    dist2(a, b) < 1e-6
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw;
    use crate::hit::{self, ElementRef, Rect, distance};

    fn red() -> Paint {
        Paint::Solid([1.0, 0.0, 0.0, 1.0])
    }

    /// A layer tweening `from` to `to` over 10 frames.
    fn tween(from: Vec<Element>, to: Vec<Element>, angular: bool) -> Document {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        let frames = vec![
            Frame { index: 0, duration: 10, tween: Tween::Shape { ease: 0.0, angular, hints: Vec::new() }, elements: from, extra: Extra::default() },
            Frame { index: 10, duration: 1, tween: Tween::None, elements: to, extra: Extra::default() },
        ];
        doc.scenes[0].layers[0].frames = frames;
        doc
    }

    fn bounds(doc: &Document, frame: u32) -> Rect {
        hit::bounds(doc, &doc.scenes[0], frame, &[ElementRef { layer: 0, keyframe: frame.min(10) as usize / 10, element: 0 }]).unwrap()
    }

    fn close(a: Rect, b: Rect, within: f32) -> bool {
        [a.min.0 - b.min.0, a.min.1 - b.min.1, a.max.0 - b.max.0, a.max.1 - b.max.1].iter().all(|d| d.abs() < within)
    }

    #[test]
    fn a_square_grows_into_a_wide_blue_bar() {
        let square = draw::rectangle([(0.0, 0.0), (100.0, 100.0)], 0.0, None, Some(red()));
        let mut bar = draw::rectangle([(0.0, 0.0), (200.0, 50.0)], 0.0, None, Some(Paint::Solid([0.0, 0.0, 1.0, 1.0])));
        bar.matrix = Matrix::translate(100.0, 200.0);
        for angular in [false, true] {
            let doc = tween(vec![Element::Shape(square.clone())], vec![Element::Shape(bar.clone())], angular);
            let key = &doc.scenes[0].layers[0].frames[0];
            let next = doc.scenes[0].layers[0].frames.get(1);
            assert!(valid(key, next));
            assert!(at(key, next, 0).is_none(), "the keyframe shows as it is");
            // Halfway: between the two in place, size and color; a smooth blend cuts the corners a little.
            let mid = at(key, next, 5).unwrap();
            let half = Rect { min: (50.0, 100.0), max: (200.0, 175.0) };
            assert_eq!(mid.fills, [Paint::Solid([0.5, 0.0, 0.5, 1.0])]);
            assert!(close(bounds(&doc, 5), half, if angular { 0.01 } else { 2.0 }), "{angular}: {:?}", bounds(&doc, 5));
            assert!(close(bounds(&doc, 0), Rect { min: (0.0, 0.0), max: (100.0, 100.0) }, 0.01));
            if angular {
                // Corner to corner, the sides stay straight: every point lies on the halfway box.
                let b = half;
                let on_edge = |p: &Point| [p.0 - b.min.0, b.max.0 - p.0, p.1 - b.min.1, b.max.1 - p.1].iter().any(|d| d.abs() < 0.01);
                assert!(mid.edges[0].paths[0].segs.iter().all(|s| on_edge(&s.end())));
            }
        }
    }

    #[test]
    fn holes_strokes_and_extra_fills() {
        let stroke = Stroke { width: 2.0, paint: Paint::Solid([0.0, 0.0, 0.0, 1.0]), cap: Cap::Round, join: Join::Round };
        let ring = draw::shape(
            vec![hit_rect(0.0, 0.0, 100.0, 100.0), hit_rect(25.0, 25.0, 75.0, 75.0)],
            Some(stroke.clone()),
            Some(red()),
        );
        let dot = draw::oval([(200.0, 200.0), (220.0, 220.0)], None, Some(red()));
        let thick = Stroke { width: 6.0, ..stroke };
        let line = draw::line((0.0, 300.0), (100.0, 300.0), thick);
        let doc = tween(vec![Element::Shape(ring)], vec![Element::Shape(dot), Element::Shape(line)], false);
        let layer = &doc.scenes[0].layers[0];
        let mid = at(&layer.frames[0], layer.frames.get(1), 5).unwrap();
        // The ring's fill into the dot's (its hole shrinking away); its stroke into the line.
        assert_eq!((mid.fills.len(), mid.edges[0].paths.len()), (1, 2));
        assert_eq!(mid.strokes.iter().map(|s| s.width).collect::<Vec<_>>(), [4.0, 2.0], "the line's and the ring's second outline's");
        // A tween to text, or with nothing after it, is broken: it holds the keyframe.
        let mut broken = doc.clone();
        broken.scenes[0].layers[0].frames.truncate(1);
        assert!(!valid(&broken.scenes[0].layers[0].frames[0], None));
        assert!(close(bounds(&broken, 5), Rect { min: (-1.0, -1.0), max: (101.0, 101.0) }, 0.01));
    }

    #[test]
    fn hints_pin_points_to_points() {
        let square = |x| draw::rectangle([(x, 0.0), (x + 100.0, 100.0)], 0.0, None, Some(red()));
        let mut doc = tween(vec![Element::Shape(square(0.0))], vec![Element::Shape(square(200.0))], false);
        let passes = |doc: &Document, p: Point| {
            let layer = &doc.scenes[0].layers[0];
            let mid = at(&layer.frames[0], layer.frames.get(1), 5).unwrap();
            mid.edges[0].paths[0].segs.iter().any(|s| (s.end().0 - p.0).hypot(s.end().1 - p.1) < 0.5)
        };
        assert!(passes(&doc, (100.0, 0.0)) && !passes(&doc, (150.0, 50.0)), "unhinted, corners meet their like");
        // Hint a: the first square's top left corner to the second's bottom right.
        if let Tween::Shape { hints, .. } = &mut doc.scenes[0].layers[0].frames[0].tween {
            hints.push(((0.0, 0.0), (300.0, 100.0)));
        }
        assert!(passes(&doc, (150.0, 50.0)), "the hinted points meet halfway");
        let elements = &doc.scenes[0].layers[0].frames[0].elements;
        assert!(on_outline(elements, (50.0, 0.0), 0.5) && !on_outline(elements, (50.0, 50.0), 0.5));
        let hints = vec![((0.0, 0.0), (300.0, 100.0)), ((1.5, -2.0), (3.25, 4.0))];
        assert_eq!(read_hints(&write_hints(&hints)).unwrap(), hints);
    }

    #[test]
    fn hints_pin_exactly_on_big_outlines() {
        // A 400×300 box into the same box 500 px right: a hint from its top right corner to the
        // other's bottom right, and one from a point of its top side to one of the other's bottom.
        let boxed = |x: f32| draw::rectangle([(x, 0.0), (x + 400.0, 300.0)], 0.0, None, Some(red()));
        for (hint, meet) in [(((400.0, 0.0), (900.0, 300.0)), (650.0, 150.0)), (((207.0, 0.0), (707.0, 300.0)), (457.0, 150.0))] {
            let mut doc = tween(vec![Element::Shape(boxed(0.0))], vec![Element::Shape(boxed(500.0))], false);
            if let Tween::Shape { hints, .. } = &mut doc.scenes[0].layers[0].frames[0].tween {
                hints.push(hint);
            }
            let layer = &doc.scenes[0].layers[0];
            let mid = at(&layer.frames[0], layer.frames.get(1), 5).unwrap();
            let d = polyline(&mid.edges[0].paths[0]).iter().map(|p| (p.0 - meet.0).hypot(p.1 - meet.1)).fold(f32::INFINITY, f32::min);
            assert!(d < 0.01, "{hint:?}: the pinned points meet {d} px from {meet:?}");
        }
    }

    #[test]
    fn strokes_keep_to_their_fills_and_corners_stay_corners() {
        // A stroked 200×100 box into a stroked circle (drawn from its right), and into the same box
        // twice the size with its stroke in pieces: halfway, the stroke runs along the fill's edge.
        let ink = Stroke { width: 2.0, paint: Paint::Solid([0.0, 0.0, 0.0, 1.0]), cap: Cap::Round, join: Join::Round };
        let boxed = |x0: f32, y0: f32, x1: f32, y1: f32| draw::rectangle([(x0, y0), (x1, y1)], 0.0, Some(ink.clone()), Some(red()));
        let circle = draw::oval([(300.0, 100.0), (400.0, 200.0)], Some(ink.clone()), Some(red()));
        let mut pieces = boxed(100.0, 100.0, 500.0, 300.0);
        let whole = pieces.edges.remove(0);
        let mut from = whole.paths[0].start;
        for &seg in &whole.paths[0].segs {
            pieces.edges.insert(0, Edge { paths: vec![Path { start: from, segs: vec![seg] }], ..whole.clone() });
            from = seg.end();
        }
        for (to, angular) in [(circle.clone(), false), (circle, true), (pieces, false)] {
            let doc = tween(vec![Element::Shape(boxed(0.0, 0.0, 200.0, 100.0))], vec![Element::Shape(to)], angular);
            let layer = &doc.scenes[0].layers[0];
            let mid = at(&layer.frames[0], layer.frames.get(1), 5).unwrap();
            let fill = polyline(&mid.edges[0].paths[0]);
            let on_fill = |p: &Point| fill.windows(2).any(|w| distance(*p, w[0], w[1]) < 0.01);
            assert_eq!(mid.strokes.len(), 1, "one outline, one stroke");
            assert!(mid.edges[1].paths.iter().flat_map(polyline).all(|p| on_fill(&p)), "angular {angular}: the stroke strays off the fill");
        }
        // The box into its double: every point of the one between lies on its sides, corners kept.
        let doc = tween(vec![Element::Shape(boxed(0.0, 0.0, 200.0, 100.0))], vec![Element::Shape(boxed(100.0, 100.0, 500.0, 300.0))], false);
        let layer = &doc.scenes[0].layers[0];
        let mid = at(&layer.frames[0], layer.frames.get(1), 5).unwrap();
        let b = Rect { min: (50.0, 50.0), max: (350.0, 200.0) };
        let on_edge = |p: &Point| [p.0 - b.min.0, b.max.0 - p.0, p.1 - b.min.1, b.max.1 - p.1].iter().any(|d| d.abs() < 0.01);
        assert!(mid.edges.iter().flat_map(|e| e.paths.iter().flat_map(polyline)).all(|p| on_edge(&p)));
    }

    fn hit_rect(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
        Path { start: (x0, y0), segs: vec![Seg::Line((x1, y0)), Seg::Line((x1, y1)), Seg::Line((x0, y1)), Seg::Line((x0, y0))] }
    }
}
