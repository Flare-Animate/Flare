//! Geometry for the Selection tool: which element is under a point, the box
//! around a selection, and moving elements. It asks display.rs what's on stage,
//! so tweens and graphic symbols are hit where they're drawn.

use std::borrow::Cow;
use std::collections::BTreeSet;

use crate::display::{Item, display_list, display_list_shown, unmasked};
use crate::doc::*;
use crate::text;

/// An element of a timeline: `layers[layer].frames[keyframe].elements[element]`.
/// A group is one element.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ElementRef {
    pub layer: usize,
    pub keyframe: usize,
    pub element: usize,
}

/// An axis-aligned box.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub fn union(self, o: Rect) -> Rect {
        Rect {
            min: (self.min.0.min(o.min.0), self.min.1.min(o.min.1)),
            max: (self.max.0.max(o.max.0), self.max.1.max(o.max.1)),
        }
    }
}

/// The topmost element of `tl` drawn under `point` at `frame`. Fills hit
/// inside, strokes within half their width plus `tolerance` (in `tl`'s units).
/// Paint doesn't matter: a transparent fill still hits, as in Animate. Inside a
/// classic tween this is the tween's first keyframe, hit where it's drawn.
/// Hidden and locked layers are passed over, as the stage can't select them.
pub fn hit_test(doc: &Document, tl: &Timeline, frame: u32, point: Point, tolerance: f32) -> Option<ElementRef> {
    unmasked(display_list_shown(doc, tl, frame, &mut BTreeSet::new()))
        .iter()
        .rev()
        .filter(|p| !tl.layers[p.source.layer].locked())
        .find(|p| shapes(doc, &p.item, p.matrix, 0).iter().any(|(s, m)| shape_hit(s, m, point, tolerance)))
        .map(|p| p.source)
}

/// The box around what `elements` draw at `frame`, in `tl`'s coordinates and
/// with strokes included. None if none of them is on stage then: the editor's
/// stage, where guides and unlocked masks show and hidden layers don't.
pub fn bounds(doc: &Document, tl: &Timeline, frame: u32, elements: &[ElementRef]) -> Option<Rect> {
    bounds_through(doc, tl, frame, elements, Matrix::IDENTITY)
}

/// Like [`bounds`], in the coordinates `view` maps `tl`'s into. Through an
/// element's inverse matrix, that is the box around it in its own coordinates.
pub fn bounds_through(doc: &Document, tl: &Timeline, frame: u32, elements: &[ElementRef], view: Matrix) -> Option<Rect> {
    unmasked(display_list_shown(doc, tl, frame, &mut BTreeSet::new()))
        .iter()
        .filter(|p| elements.contains(&p.source))
        .flat_map(|p| shapes(doc, &p.item, view * p.matrix, 0))
        .filter_map(|(s, m)| shape_bounds(&s, &m))
        .reduce(Rect::union)
}

/// Moves `el` by (dx, dy) in its timeline's coordinates.
///
/// Shapes move by their matrix, never their edges: XFL shapes can carry a matrix,
/// the edges stay exactly on their twip grid however often they're dragged, and a
/// dense shape moves in O(1).
pub fn translate(el: &mut Element, dx: f32, dy: f32) {
    match el {
        // ponytail: raw (merge drawing) shapes move by matrix too, so unlike in Animate they don't
        // merge with or cut the shapes they're dropped on. Offset their edges once merging exists.
        Element::Shape(Shape { matrix, .. })
        | Element::Instance(Instance { matrix, .. })
        | Element::Text(Text { matrix, .. }) => {
            matrix.tx += dx;
            matrix.ty += dy;
        }
        Element::Group(members) => members.iter_mut().for_each(|el| translate(el, dx, dy)),
        // ponytail: bitmaps and other unsupported elements are kept as raw XML, so they stay
        // put while the rest of a selection or group moves. Parse their matrix to move them.
        Element::Unsupported { .. } => {}
    }
}

/// What the Selection tool takes hold of in an unselected shape: a corner or
/// end of its outline to move, or a point along an edge to bend it from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Grip {
    /// A point of the outline, in the shape's own coordinates.
    Corner(Point),
    /// Segment `seg` of path `path` of edge `edge`, `t` of the way along it.
    Edge { edge: usize, path: usize, seg: usize, t: f32 },
}

/// What `point` (in the shape's timeline) grips of `s`: a corner within
/// `corner_reach`, or else the nearest edge within `edge_reach` of its stroke.
/// Where the outline runs on straight or smoothly there's no corner: a
/// rectangle's outline starts mid-side, an oval's curves meet smoothly.
pub fn grip(s: &Shape, point: Point, corner_reach: f32, edge_reach: f32) -> Option<Grip> {
    let p = s.matrix.inverse()?.apply(point);
    // ponytail: reaches are scaled as if the matrix scaled evenly, as in shape_hit.
    let scale = (s.matrix.a * s.matrix.d - s.matrix.b * s.matrix.c).abs().sqrt();
    let drawn = |e: &&Edge| e.fill0 != 0 || e.fill1 != 0 || stroke_of(s, e).is_some();
    let mut corner = s.edges.iter().filter(drawn).flat_map(|e| &e.paths).flat_map(corners);
    if let Some(c) = corner.find(|&c| (c.0 - p.0).hypot(c.1 - p.1) <= corner_reach / scale) {
        return Some(Grip::Corner(c));
    }
    let mut best: Option<(f32, Grip)> = None;
    for (edge, e) in s.edges.iter().enumerate().filter(|(_, e)| drawn(e)) {
        let reach = stroke_of(s, e).map_or(0.0, |st| st.width / 2.0) + edge_reach / scale;
        for (path, pa) in e.paths.iter().enumerate() {
            let mut from = pa.start;
            for (seg, &sg) in pa.segs.iter().enumerate() {
                // The nearest of 16 chords along a curve, and how far along it that is.
                let chords = if matches!(sg, Seg::Line(_)) { 1 } else { 16 };
                for i in 0..chords {
                    let (t0, t1) = (i as f32 / chords as f32, (i + 1) as f32 / chords as f32);
                    let (d, u) = along(p, at(from, sg, t0), at(from, sg, t1));
                    if d <= reach && best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, Grip::Edge { edge, path, seg, t: t0 + u * (t1 - t0) }));
                    }
                }
                from = sg.end();
            }
        }
    }
    best.map(|(_, g)| g)
}

/// `s` with what `grip` holds dragged by `d` (in the shape's timeline): a
/// corner moves; an edge bends so its gripped point follows, a straight run of
/// lines bending as one curve.
pub fn reshape(s: &Shape, grip: Grip, d: Point) -> Shape {
    let mut s = s.clone();
    let Some(inverse) = s.matrix.inverse() else { return s };
    let (o, q) = (inverse.apply((0.0, 0.0)), inverse.apply(d));
    let d = (q.0 - o.0, q.1 - o.1);
    match grip {
        Grip::Corner(c) => {
            let moved = (c.0 + d.0, c.1 + d.1);
            for path in s.edges.iter_mut().flat_map(|e| &mut e.paths) {
                if near(path.start, c) {
                    path.start = moved;
                }
                for seg in &mut path.segs {
                    if let Seg::Line(to) | Seg::Quad(_, to) = seg
                        && near(*to, c)
                    {
                        *to = moved;
                    }
                }
            }
        }
        Grip::Edge { edge, path, seg, t } => {
            if let Some(path) = s.edges.get_mut(edge).and_then(|e| e.paths.get_mut(path)) {
                bend(path, seg, t, d);
            }
        }
    }
    s
}

/// Bends segment `k` of `path`, or the straight run of lines it's in, into one
/// quadratic curve whose point `t` along `k` moves by `d`; a run is cut where
/// its points were, so the path keeps its segments.
fn bend(path: &mut Path, k: usize, t: f32, d: Point) {
    let n = path.segs.len();
    if k >= n {
        return;
    }
    let closed = near(path.segs[n - 1].end(), path.start);
    let start = |path: &Path, i: usize| if i == 0 { path.start } else { path.segs[i - 1].end() };
    let straight = |path: &Path, i: usize, j: usize| {
        let dir = |i: usize| (path.segs[i].end().0 - start(path, i).0, path.segs[i].end().1 - start(path, i).1);
        matches!((path.segs[i], path.segs[j]), (Seg::Line(_), Seg::Line(_))) && smooth(dir(i), dir(j))
    };
    let (mut first, mut last, mut count) = (k, k, 1);
    while count < n {
        let prev = match first {
            0 if closed => n - 1,
            0 => break,
            i => i - 1,
        };
        if !straight(path, prev, first) {
            break;
        }
        (first, count) = (prev, count + 1);
    }
    while count < n {
        let next = if last + 1 < n { last + 1 } else if closed { 0 } else { break };
        if !straight(path, last, next) {
            break;
        }
        (last, count) = (next, count + 1);
    }
    let run: Vec<usize> = (0..count).map(|i| (first + i) % n).collect();
    let (a, b) = (start(path, first), path.segs[last].end());
    let lengths: Vec<f32> = run.iter().map(|&i| (path.segs[i].end().0 - start(path, i).0).hypot(path.segs[i].end().1 - start(path, i).1)).collect();
    let total = lengths.iter().sum::<f32>().max(1e-6);
    let before: f32 = run.iter().zip(&lengths).take_while(|(i, _)| **i != k).map(|(_, l)| l).sum();
    // A curve alone keeps its control point; a straight run starts as a flat curve.
    let (c0, u) = match path.segs[k] {
        Seg::Quad(c, _) => (c, t),
        Seg::Line(_) => (((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), (before + t * lengths[run.iter().position(|&i| i == k).unwrap_or(0)]) / total),
    };
    // The curve's point at u moves by 2u(1 - u) times its control point's move.
    let u = u.clamp(0.1, 0.9);
    let w = 2.0 * u * (1.0 - u);
    let c = (c0.0 + d.0 / w, c0.1 + d.1 / w);
    // Curve a-c-b between parameters x and y: its control point (x ≠ y) or point (x = y).
    let blossom = |x: f32, y: f32| {
        let (p, q, r) = ((1.0 - x) * (1.0 - y), (1.0 - x) * y + x * (1.0 - y), x * y);
        (p * a.0 + q * c.0 + r * b.0, p * a.1 + q * c.1 + r * b.1)
    };
    let mut s0 = 0.0;
    for (j, &i) in run.iter().enumerate() {
        let s1 = if j + 1 == count { 1.0 } else { s0 + lengths[j] / total };
        path.segs[i] = Seg::Quad(blossom(s0, s1), blossom(s1, s1));
        if j > 0 && i == 0 {
            path.start = blossom(s0, s0);
        }
        s0 = s1;
    }
}

/// The points of `path` where it ends or turns a corner.
fn corners(path: &Path) -> Vec<Point> {
    let n = path.segs.len();
    let closed = n > 0 && near(path.segs[n - 1].end(), path.start);
    // Each segment's heading leaving its start and arriving at its end.
    let mut from = path.start;
    let heads: Vec<(Point, Point)> = path
        .segs
        .iter()
        .map(|&seg| {
            let line = (seg.end().0 - from.0, seg.end().1 - from.1);
            let h = match seg {
                Seg::Quad(c, to) if !near(c, from) && !near(c, to) => ((c.0 - from.0, c.1 - from.1), (to.0 - c.0, to.1 - c.1)),
                _ => (line, line),
            };
            from = seg.end();
            h
        })
        .collect();
    (0..=n)
        .filter(|&i| !(closed && i == n))
        .filter_map(|i| {
            let incoming = if i > 0 { Some(heads[i - 1].1) } else if closed { Some(heads[n - 1].1) } else { None };
            let outgoing = if i < n { Some(heads[i].0) } else if closed { Some(heads[0].0) } else { None };
            let at = if i == 0 { path.start } else { path.segs[i - 1].end() };
            match (incoming, outgoing) {
                (Some(a), Some(b)) if smooth(a, b) => None,
                _ => Some(at),
            }
        })
        .collect()
}

/// Headings within about 6° of each other.
fn smooth(a: Point, b: Point) -> bool {
    let (cross, dot) = (a.0 * b.1 - a.1 * b.0, a.0 * b.0 + a.1 * b.1);
    dot > 0.0 && cross.abs() <= 0.1 * a.0.hypot(a.1) * b.0.hypot(b.1)
}

fn near(a: Point, b: Point) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

/// The point `t` along a segment from `from`.
fn at(from: Point, seg: Seg, t: f32) -> Point {
    match seg {
        Seg::Line(to) => (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t),
        Seg::Quad(c, to) => {
            let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
            (u * from.0 + v * c.0 + w * to.0, u * from.1 + v * c.1 + w * to.1)
        }
    }
}

/// The shapes `item` draws, with their matrices. Movie clips and buttons show
/// their first frame, as on Animate's stage; text is its box, as Animate hits it.
pub(crate) fn shapes<'a>(doc: &'a Document, item: &Item<'a>, m: Matrix, depth: u32) -> Vec<(Cow<'a, Shape>, Matrix)> {
    match *item {
        Item::Shape(s) => vec![(Cow::Borrowed(s), m)],
        Item::Text(t) => vec![(Cow::Owned(text::box_shape(t)), m)],
        Item::Clip(sym, _) if depth < 32 => display_list(doc, &sym.timeline, 0, &mut BTreeSet::new())
            .into_iter()
            .flat_map(|p| shapes(doc, &p.item, m * p.matrix, depth + 1))
            .collect(),
        Item::Clip(..) => Vec::new(),
        Item::Morph(ref s) => vec![(Cow::Owned((**s).clone()), m)],
        // ponytail: what a mask hides still hits, and counts toward bounds. The stage locks masked
        // layers while it shows the masking, so only clips and graphics holding masks are off.
        Item::Masked { ref content, .. } => content.iter().flat_map(|p| shapes(doc, &p.item, m * p.matrix, depth)).collect(),
    }
}

fn shape_hit(s: &Shape, m: &Matrix, point: Point, tolerance: f32) -> bool {
    let Some(inverse) = m.inverse() else { return false };
    let p = inverse.apply(point);
    // ponytail: the tolerance is scaled as if the matrix scaled evenly, so it stretches under
    // non-uniform scale or skew; and strokes are hit as if their caps and joins were round.
    let tolerance = tolerance / (m.a * m.d - m.b * m.c).abs().sqrt();
    // Even-odd per fill, as render.rs fills: a crossing flips the fills on both sides.
    // render.rs chains the edges into loops first, which doesn't change any parity.
    let mut inside = vec![false; s.fills.len() + 1];
    for e in &s.edges {
        let reach = stroke_of(s, e).map(|st| st.width / 2.0 + tolerance);
        for path in &e.paths {
            for line in polyline(path).windows(2) {
                let (a, b) = (line[0], line[1]);
                if reach.is_some_and(|r| distance(p, a, b) <= r) {
                    return true;
                }
                // Does a ray from p to the right cross ab? Half-open in y, so shared ends count once.
                if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                    for f in [e.fill0, e.fill1] {
                        if let Some(v) = inside.get_mut(f as usize) {
                            *v = !*v;
                        }
                    }
                }
            }
        }
    }
    inside[1..].contains(&true)
}

/// The box around `s` drawn through `m`, half of each stroke included.
fn shape_bounds(s: &Shape, m: &Matrix) -> Option<Rect> {
    // ponytail: boxed as if caps and joins were round, so square caps and miter joins can
    // poke out by up to a stroke width. Add their corners if a selection box visibly clips.
    let drawn = s.edges.iter().filter(|e| e.fill0 != 0 || e.fill1 != 0 || stroke_of(s, e).is_some());
    let boxes = drawn.flat_map(|e| {
        // How far a circle as wide as the stroke reaches each way, through the matrix.
        let half = stroke_of(s, e).map_or(0.0, |st| st.width / 2.0);
        let pad = (half * m.a.hypot(m.c), half * m.b.hypot(m.d));
        e.paths.iter().flat_map(polyline).map(move |q| {
            let (x, y) = m.apply(q);
            Rect { min: (x - pad.0, y - pad.1), max: (x + pad.0, y + pad.1) }
        })
    });
    boxes.reduce(Rect::union)
}

/// The path's points, each curve cut into 16 chords: they stray from it by at
/// most 1/1024 of |start - 2·control + end|, a fraction of a pixel.
pub(crate) fn polyline(path: &Path) -> Vec<Point> {
    let (mut from, mut points) = (path.start, vec![path.start]);
    for &seg in &path.segs {
        if let Seg::Quad(c, to) = seg {
            points.extend((1..16).map(|i| {
                let t = i as f32 / 16.0;
                let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
                (u * from.0 + v * c.0 + w * to.0, u * from.1 + v * c.1 + w * to.1)
            }));
        }
        from = seg.end();
        points.push(from);
    }
    points
}

/// From `p` to the segment `ab`.
pub(crate) fn distance(p: Point, a: Point, b: Point) -> f32 {
    along(p, a, b).0
}

/// From `p` to the segment `ab`, and how far along `ab` (0..=1) its nearest point is.
pub(crate) fn along(p: Point, a: Point, b: Point) -> (f32, f32) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    ((a.0 + t * dx - p.0).hypot(a.1 + t * dy - p.1), t)
}

fn stroke_of<'a>(s: &'a Shape, e: &Edge) -> Option<&'a Stroke> {
    (e.stroke as usize).checked_sub(1).and_then(|i| s.strokes.get(i))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::xfl;

    fn el(layer: usize, keyframe: usize, element: usize) -> ElementRef {
        ElementRef { layer, keyframe, element }
    }

    fn bbox(x0: f32, y0: f32, x1: f32, y1: f32) -> Option<Rect> {
        Some(Rect { min: (x0, y0), max: (x1, y1) })
    }

    fn rect_path(x0: f32, y0: f32, x1: f32, y1: f32) -> Path {
        let segs = vec![Seg::Line((x1, y0)), Seg::Line((x1, y1)), Seg::Line((x0, y1)), Seg::Line((x0, y0))];
        Path { start: (x0, y0), segs }
    }

    /// One edge, with fill 1 on its right if `fill` and a 4 px stroke if `stroke`.
    fn shape(fill: bool, stroke: bool, paths: Vec<Path>) -> Shape {
        let black = Paint::Solid([0.0, 0.0, 0.0, 1.0]);
        Shape {
            matrix: Matrix::IDENTITY,
            fills: vec![black.clone()],
            strokes: vec![Stroke { width: 4.0, paint: black, cap: Cap::Round, join: Join::Round }],
            edges: vec![Edge { fill0: 0, fill1: fill as u32, stroke: stroke as u32, paths }],
            extra: Extra::default(),
        }
    }

    fn instance(symbol: &str, kind: SymbolKind, matrix: Matrix) -> Element {
        Element::Instance(Instance {
            symbol: symbol.into(),
            kind,
            matrix,
            pivot: (0.0, 0.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra: Extra::default(),
        })
    }

    /// One frame long, with these layers, top first.
    fn timeline(layers: Vec<Vec<Element>>) -> Timeline {
        let layer = |elements| Layer {
            name: String::new(),
            kind: LayerKind::Normal,
            frames: vec![Frame { index: 0, duration: 1, tween: Tween::None, elements, extra: Extra::default() }],
            extra: Extra::default(),
        };
        Timeline { name: String::new(), layers: layers.into_iter().map(layer).collect(), extra: Extra::default() }
    }

    fn doc(layers: Vec<Vec<Element>>) -> Document {
        let mut doc = Document::new(550.0, 400.0, 24.0);
        doc.scenes[0] = timeline(layers);
        doc
    }

    #[test]
    fn fills_strokes_and_misses() {
        // A 100 px square with a 20 px hole, and an unfilled outline.
        let donut = shape(true, false, vec![rect_path(0.0, 0.0, 100.0, 100.0), rect_path(40.0, 40.0, 60.0, 60.0)]);
        let outline = shape(false, true, vec![rect_path(200.0, 0.0, 300.0, 100.0)]);
        let d = doc(vec![vec![Element::Shape(donut), Element::Shape(outline)]]);
        let hit = |x, y, tolerance| hit_test(&d, &d.scenes[0], 0, (x, y), tolerance);
        assert_eq!(hit(20.0, 20.0, 0.0), Some(el(0, 0, 0)), "fill");
        assert_eq!(hit(50.0, 50.0, 0.0), None, "even-odd hole");
        assert_eq!(hit(150.0, 50.0, 0.0), None, "between");
        assert_eq!(hit(250.0, 50.0, 0.0), None, "inside an unfilled outline");
        assert_eq!(hit(201.5, 50.0, 0.0), Some(el(0, 0, 1)), "on the stroke");
        assert_eq!(hit(197.0, 50.0, 0.0), None, "past half the stroke width");
        assert_eq!(hit(197.0, 50.0, 1.5), Some(el(0, 0, 1)), "within tolerance");
    }

    #[test]
    fn topmost_element_wins() {
        let square = |x: f32| Element::Shape(shape(true, false, vec![rect_path(x, 0.0, x + 100.0, 100.0)]));
        let mut d = doc(vec![vec![square(50.0)], vec![square(0.0), square(20.0)]]);
        let hit = |d: &Document, x| hit_test(d, &d.scenes[0], 0, (x, 50.0), 0.0);
        assert_eq!(hit(&d, 10.0), Some(el(1, 0, 0)));
        assert_eq!(hit(&d, 30.0), Some(el(1, 0, 1)), "later elements are on top");
        assert_eq!(hit(&d, 60.0), Some(el(0, 0, 0)), "upper layers are on top");
        // A guide is on the stage while its timeline is edited (and so can be picked), but never publishes.
        d.scenes[0].layers[0].kind = LayerKind::Guide;
        assert_eq!(hit(&d, 60.0), Some(el(0, 0, 0)), "guides are on the stage being edited");
        assert!(display_list(&d, &d.scenes[0], 0, &mut BTreeSet::new()).iter().all(|p| p.source.layer != 0), "but don't publish");
    }

    #[test]
    fn hidden_and_locked_layers_are_passed_over() {
        let square = || Element::Shape(shape(true, false, vec![rect_path(0.0, 0.0, 100.0, 100.0)]));
        let mut d = doc(vec![vec![square()], vec![square()], vec![square()]]);
        d.scenes[0].layers[0].set_visible(false);
        d.scenes[0].layers[1].set_locked(true);
        assert_eq!(hit_test(&d, &d.scenes[0], 0, (50.0, 50.0), 0.0), Some(el(2, 0, 0)));
    }

    #[test]
    fn grips_bend_edges_and_move_corners() {
        // As the Rectangle tool draws it: its outline starts mid-way along the top.
        let black = Paint::Solid([0.0, 0.0, 0.0, 1.0]);
        let stroke = Stroke { width: 2.0, paint: black.clone(), cap: Cap::Round, join: Join::Round };
        let mut rect = crate::draw::rectangle([(0.0, 0.0), (100.0, 50.0)], 0.0, Some(stroke), Some(black));
        rect.matrix = Matrix::translate(10.0, 20.0);
        let at = |x, y| grip(&rect, (x, y), 4.0, 3.0);
        assert_eq!(at(112.0, 21.0), Some(Grip::Corner((100.0, 0.0))));
        assert!(matches!(at(60.0, 21.0), Some(Grip::Edge { .. })), "mid-side is no corner");
        assert_eq!(at(60.0, 45.0), None, "inside, away from the edges");

        // Pulling the middle of the top up bends the whole side, its corners staying put.
        let bent = reshape(&rect, at(60.0, 20.0).unwrap(), (0.0, -20.0));
        let d = doc(vec![vec![Element::Shape(bent)]]);
        let hit = |x, y| hit_test(&d, &d.scenes[0], 0, (x, y), 0.0).is_some();
        assert!(hit(60.0, 1.0) && hit(30.0, 12.0) && hit(90.0, 12.0), "the side bulges up along its length");
        assert!(!hit(60.0, -5.0) && !hit(12.0, 5.0) && !hit(108.0, 5.0));

        // Dragging a corner moves that point alone.
        let moved = reshape(&rect, Grip::Corner((100.0, 50.0)), (30.0, 30.0));
        let d = doc(vec![vec![Element::Shape(moved)]]);
        assert_eq!(bounds(&d, &d.scenes[0], 0, &[el(0, 0, 0)]), bbox(9.0, 19.0, 141.0, 101.0));
    }

    #[test]
    fn instances_and_groups_follow_their_matrices() {
        // A 10x5 movie clip turned a quarter clockwise, doubled and moved to (100, 100): 90..100 x 100..120.
        let turned = Matrix { a: 0.0, b: 2.0, c: -2.0, d: 0.0, tx: 100.0, ty: 100.0 };
        // A 10 px graphic line with a 4 px stroke at half size, grouped with a square.
        let half = Matrix { a: 0.5, d: 0.5, tx: 300.0, ty: 300.0, ..Matrix::IDENTITY };
        let square = Element::Shape(shape(true, false, vec![rect_path(200.0, 0.0, 220.0, 20.0)]));
        let group = Element::Group(vec![square, instance("Line", SymbolKind::Graphic, half)]);
        // The line again, turned a quarter, 2x along it and 0.5x across: (400, 300) to (400, 320), 2 px wide.
        let squashed = Matrix { a: 0.0, b: 2.0, c: -0.5, d: 0.0, tx: 400.0, ty: 300.0 };
        let mut d = doc(vec![vec![
            instance("Box", SymbolKind::MovieClip, turned),
            group,
            instance("Line", SymbolKind::Graphic, squashed),
        ]]);
        let line = Path { start: (0.0, 0.0), segs: vec![Seg::Line((10.0, 0.0))] };
        let symbols = [
            ("Box", SymbolKind::MovieClip, shape(true, false, vec![rect_path(0.0, 0.0, 10.0, 5.0)])),
            ("Line", SymbolKind::Graphic, shape(false, true, vec![line])),
        ];
        for (name, kind, s) in symbols {
            d.library.insert(name.into(), Symbol { kind, timeline: timeline(vec![vec![Element::Shape(s)]]), extra: Extra::default() });
        }

        let tl = &d.scenes[0];
        let hit = |x, y, tolerance| hit_test(&d, tl, 0, (x, y), tolerance);
        assert_eq!(hit(95.0, 110.0, 0.0), Some(el(0, 0, 0)));
        assert_eq!(hit(5.0, 2.0, 0.0), None, "where the clip would be untransformed");
        assert_eq!(hit(105.0, 110.0, 0.0), None);
        assert_eq!(hit(210.0, 10.0, 0.0), Some(el(0, 0, 1)), "a group member");
        // At half size the line's stroke is 2 px wide.
        assert_eq!(hit(302.0, 300.9, 0.0), Some(el(0, 0, 1)));
        assert_eq!(hit(302.0, 301.5, 0.0), None);
        assert_eq!(hit(302.0, 301.5, 0.75), Some(el(0, 0, 1)), "tolerance is in stage pixels");
        assert_eq!(hit(305.5, 300.0, 0.0), Some(el(0, 0, 1)), "the line's round end");
        assert_eq!(hit(306.5, 300.0, 0.0), None, "past its end, though on its line");

        assert_eq!(bounds(&d, tl, 0, &[el(0, 0, 0)]), bbox(90.0, 100.0, 100.0, 120.0));
        assert_eq!(bounds(&d, tl, 0, &[el(0, 0, 1)]), bbox(200.0, 0.0, 306.0, 301.0), "half the scaled stroke");
        assert_eq!(bounds(&d, tl, 0, &[el(0, 0, 2)]), bbox(399.0, 296.0, 401.0, 324.0), "the stroke's ends stretch");
        assert_eq!(bounds(&d, tl, 0, &[el(0, 0, 0), el(0, 0, 1)]), bbox(90.0, 0.0, 306.0, 301.0));
        assert_eq!(bounds(&d, tl, 0, &[el(5, 0, 0)]), None);
    }

    #[test]
    fn nested_symbols_show_their_current_frame() {
        let square = shape(true, false, vec![rect_path(0.0, 0.0, 10.0, 10.0)]);
        let moved = |tx| Element::Shape(Shape { matrix: Matrix { tx, ..Matrix::IDENTITY }, ..square.clone() });
        let mut d = doc(vec![vec![
            instance("Outer", SymbolKind::MovieClip, Matrix { a: 2.0, d: 2.0, tx: 100.0, ..Matrix::IDENTITY }),
            instance("Steps", SymbolKind::Graphic, Matrix { ty: 50.0, ..Matrix::IDENTITY }),
        ]]);
        d.scenes[0].layers[0].frames[0].duration = 2;
        let mut steps = timeline(vec![vec![moved(0.0)]]);
        steps.layers[0].frames.push(Frame { index: 1, duration: 1, tween: Tween::None, elements: vec![moved(20.0)], extra: Extra::default() });
        let inner = instance("Inner", SymbolKind::MovieClip, Matrix { tx: 10.0, ..Matrix::IDENTITY });
        let symbols = [
            // The square 10 px right inside a clip doubled at (100, 0): 120..140 x 0..20.
            ("Outer", SymbolKind::MovieClip, timeline(vec![vec![inner]])),
            ("Inner", SymbolKind::MovieClip, timeline(vec![vec![moved(0.0)]])),
            // The square at 0..10, then at 20..30.
            ("Steps", SymbolKind::Graphic, steps),
        ];
        for (name, kind, timeline) in symbols {
            d.library.insert(name.into(), Symbol { kind, timeline, extra: Extra::default() });
        }

        let tl = &d.scenes[0];
        let hit = |frame, x, y| hit_test(&d, tl, frame, (x, y), 0.0);
        assert_eq!(hit(0, 135.0, 10.0), Some(el(0, 0, 0)));
        assert_eq!(hit(0, 115.0, 10.0), None, "the inner clip's offset is doubled too");
        assert_eq!(bounds(&d, tl, 0, &[el(0, 0, 0)]), bbox(120.0, 0.0, 140.0, 20.0));
        assert_eq!(hit(0, 5.0, 55.0), Some(el(0, 0, 1)));
        assert_eq!(hit(1, 5.0, 55.0), None, "the graphic moved on to its second frame");
        assert_eq!(hit(1, 25.0, 55.0), Some(el(0, 0, 1)));
    }

    #[test]
    fn tween_fixture() {
        let d = xfl::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")).unwrap();
        let tl = &d.scenes[0];
        let hit = |frame, x, y| hit_test(&d, tl, frame, (x, y), 0.0);
        // Layers top down: Blink, Ball, Background.
        let (blink, ball, background) = (el(0, 0, 0), el(1, 0, 0), el(2, 0, 0));
        assert_eq!(hit(0, 100.0, 200.0), Some(ball), "the ball, above the background");
        assert_eq!(hit(0, 77.0, 200.0), Some(ball), "inside the ball's curved side");
        assert_eq!(hit(0, 72.0, 200.0), Some(background), "outside it");
        assert_eq!(hit(0, 49.0, 200.0), Some(background), "the background's stroke");
        assert_eq!(hit(0, 10.0, 10.0), None);
        assert_eq!(hit(1, 520.0, 30.0), Some(blink), "movie clips show their first frame");
        // Halfway through the tween the ball is drawn at x = 250, and is still the first keyframe's.
        assert_eq!(hit(5, 100.0, 200.0), Some(background));
        assert_eq!(hit(5, 250.0, 200.0), Some(ball));
        assert_eq!(hit(10, 400.0, 200.0), Some(el(1, 1, 0)));

        assert_eq!(bounds(&d, tl, 0, &[ball]), bbox(75.0, 180.0, 120.0, 220.0));
        assert_eq!(bounds(&d, tl, 5, &[ball]), bbox(225.0, 180.0, 270.0, 220.0));
        assert_eq!(bounds(&d, tl, 10, &[ball]), None, "the second keyframe is showing");
        assert_eq!(bounds(&d, tl, 0, &[background]), bbox(48.0, 48.0, 502.0, 352.0));
        assert_eq!(bounds(&d, tl, 0, &[ball, blink]), bbox(75.0, 20.0, 530.0, 220.0));
    }

    #[test]
    fn translate_moves_matrices_not_edges() {
        let square = shape(true, false, vec![rect_path(0.0, 0.0, 10.0, 10.0)]);
        let right = Matrix { tx: 20.0, ..Matrix::IDENTITY };
        let group = Element::Group(vec![Element::Shape(square.clone()), instance("Square", SymbolKind::Graphic, right)]);
        let mut d = doc(vec![vec![group]]);
        let timeline = timeline(vec![vec![Element::Shape(square.clone())]]);
        d.library.insert("Square".into(), Symbol { kind: SymbolKind::Graphic, timeline, extra: Extra::default() });

        translate(&mut d.scenes[0].layers[0].frames[0].elements[0], 10.0, -5.0);
        let Element::Group(members) = &d.scenes[0].layers[0].frames[0].elements[0] else { panic!("not a group") };
        let moved = Matrix { tx: 10.0, ty: -5.0, ..Matrix::IDENTITY };
        assert_eq!(members[0], Element::Shape(Shape { matrix: moved, ..square }));
        let Element::Instance(inst) = &members[1] else { panic!("not an instance") };
        assert_eq!((inst.matrix.tx, inst.matrix.ty), (30.0, -5.0));

        let tl = &d.scenes[0];
        assert_eq!(bounds(&d, tl, 0, &[el(0, 0, 0)]), bbox(10.0, -5.0, 40.0, 5.0));
        assert_eq!(hit_test(&d, tl, 0, (12.0, 0.0), 0.0), Some(el(0, 0, 0)));
        assert_eq!(hit_test(&d, tl, 0, (5.0, 7.0), 0.0), None, "where it was");
    }
}
