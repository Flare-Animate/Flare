//! Paint Bucket, Ink Bottle and Eyedropper. They see a shape as Animate saves
//! it: a planar graph whose segments meet only at their ends. The faces between
//! the segments are the regions a fill can take, and a region's fill is the
//! style its boundary segments name on the side facing it.

use std::collections::{BTreeSet, HashMap, HashSet};

use crate::display::{display_list_shown, unmasked};
use crate::doc::*;
use crate::hit::{self, ElementRef};

/// What the Eyedropper picked up.
#[derive(Clone, Debug, PartialEq)]
pub enum Picked {
    Fill(Paint),
    Stroke(Stroke),
}

/// Eyedropper: the stroke or fill drawn at `point` on `frame` of `tl`, from
/// inside symbols too. Strokes reach `tolerance` past their edges. A gradient
/// or bitmap fill comes with its matrix in `tl`'s coordinates, to paint on as it lies.
// ponytail: colors come as the shape holds them, before instances' color effects (tint,
// alpha). Carry display.rs's color transforms through `under` if picking tinted symbols matters.
pub fn pick(doc: &Document, tl: &Timeline, frame: u32, point: Point, tolerance: f32) -> Option<Picked> {
    under(doc, tl, frame, point, true).into_iter().find_map(|(_, s, p, scale, m)| {
        let g = Graph::new(s, 0.0);
        let placed = |mut paint: Paint| {
            if let Paint::Gradient(g) = &mut paint {
                g.matrix = m * g.matrix;
            } else if let Paint::Bitmap(b) = &mut paint {
                b.matrix = m * b.matrix;
            }
            paint
        };
        Some(match g.seen(s, p, tolerance * scale)? {
            Seen::Stroke(i) => {
                let stroke = style(&s.strokes, g.pieces[i].styles[STROKE])?;
                Picked::Stroke(Stroke { paint: placed(stroke.paint.clone()), ..stroke.clone() })
            }
            Seen::Fill(face) => Picked::Fill(placed(style(&s.fills, g.fill(&face))?.clone())),
        })
    })
}

/// Paint Bucket: fills the area around `point` on `frame` of `tl` (a scene's
/// or a symbol's timeline) with `paint`. That is the region of the topmost shape
/// there, or an area its strokes enclose, with gaps between loose ends up to `gap`
/// long closed. A region sharing its style with others gets a style of its own.
/// Returns the element to change and its new shape; None if nothing changes. As
/// in Animate, groups and symbols are left alone.
// ponytail: one shape at a time, so strokes of separate drawing objects don't enclose an area
// together (Animate's don't either until Break Apart; its merged raw shapes would).
pub fn bucket(doc: &Document, tl: &Timeline, frame: u32, point: Point, gap: f32, paint: Paint) -> Option<(ElementRef, Shape)> {
    let (r, s, g, face) = target(doc, tl, frame, point, gap)?;
    let sides = face.iter().map(|&h| (h / 2, right(h))).collect();
    let mut changed = s.clone();
    let index = restyle(&mut changed.fills, paint, &g, &sides, &[FILL0, FILL1]);
    g.apply(&mut changed, &sides, index);
    (changed != *s).then_some((r, changed))
}

/// The fill `bucket` would replace at `point`: `Some(None)` for an empty
/// enclosed area, None where it would do nothing.
pub fn area_fill(doc: &Document, tl: &Timeline, frame: u32, point: Point, gap: f32) -> Option<Option<Paint>> {
    let (_, s, g, face) = target(doc, tl, frame, point, gap)?;
    Some(style(&s.fills, g.fill(&face)).cloned())
}

/// The box around the area `bucket` would fill at `point`, in its shape's
/// coordinates, and the shape's matrix: what a gradient poured there spans.
pub fn area_box(doc: &Document, tl: &Timeline, frame: u32, point: Point, gap: f32) -> Option<(hit::Rect, Matrix)> {
    let (_, s, g, face) = target(doc, tl, frame, point, gap)?;
    let points = face.iter().flat_map(|&h| &g.pieces[h / 2].points);
    let b = points.map(|&p| hit::Rect { min: p, max: p }).reduce(hit::Rect::union)?;
    Some((b, s.matrix))
}

/// The Gradient Transform tool's target at `point`: a shape element of `tl`
/// drawn there and the fill style (counted from 1) of its area there. An
/// unfilled area shows what's under it.
pub fn fill_at(doc: &Document, tl: &Timeline, frame: u32, point: Point) -> Option<(ElementRef, u32)> {
    let found = under(doc, tl, frame, point, false).into_iter().find_map(|(r, s, p, _, _)| {
        let g = Graph::new(s, 0.0);
        Some((r, g.fill(&g.face_at(p)?))).filter(|&(_, fill)| fill != 0)
    });
    found.filter(|&(r, _)| is_shape(tl, r))
}

/// The shape element `bucket` acts on at `point`, and the face there.
fn target<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, point: Point, gap: f32) -> Option<(ElementRef, &'a Shape, Graph, Vec<usize>)> {
    let found = under(doc, tl, frame, point, false).into_iter().find_map(|(r, s, p, scale, _)| {
        let g = Graph::new(s, gap * scale);
        let face = g.face_at(p)?;
        Some((r, s, g, face))
    });
    found.filter(|&(r, ..)| is_shape(tl, r))
}

/// Ink Bottle: gives the stroke at `point` on `frame` of `tl` the style `stroke`,
/// all of it that runs on in the same style, or outlines the filled region there.
/// Strokes reach `tolerance` past their edges. Returns the element to change and
/// its new shape; None if nothing changes. Groups and symbols are left alone. A
/// gradient stroke spans the shape.
pub fn ink(doc: &Document, tl: &Timeline, frame: u32, point: Point, tolerance: f32, mut stroke: Stroke) -> Option<(ElementRef, Shape)> {
    let found = under(doc, tl, frame, point, false).into_iter().find_map(|(r, s, p, scale, _)| {
        let g = Graph::new(s, 0.0);
        let seen = g.seen(s, p, tolerance * scale)?;
        Some((r, s, g, seen))
    });
    let (r, s, g, seen) = found.filter(|&(r, ..)| is_shape(tl, r))?;
    if let (Paint::Gradient(gradient), Some(b)) = (&mut stroke.paint, crate::gradient::edge_box(s)) {
        gradient.fit(b);
    }
    let pieces = match seen {
        Seen::Stroke(i) => g.connected(i),
        Seen::Fill(face) => face.iter().map(|&h| h / 2).collect(),
    };
    let sides = pieces.into_iter().map(|i| (i, STROKE)).collect();
    let mut changed = s.clone();
    let index = restyle(&mut changed.strokes, stroke, &g, &sides, &[STROKE]);
    g.apply(&mut changed, &sides, index);
    (changed != *s).then_some((r, changed))
}

/// The shapes drawn at `frame` of `tl`, topmost first: each with the element of
/// `tl` that put it there, `point` in the shape's coordinates, how long one of
/// `tl`'s units is there, and the matrix from the shape to `tl`. Hidden layers
/// draw none; locked ones are passed over, as the stage can't change them,
/// unless `locked` (the Eyedropper only reads).
fn under<'a>(doc: &'a Document, tl: &'a Timeline, frame: u32, point: Point, locked: bool) -> Vec<(ElementRef, &'a Shape, Point, f32, Matrix)> {
    let mut found = Vec::new();
    let shown = unmasked(display_list_shown(doc, tl, frame, &mut BTreeSet::new()));
    for placed in shown.iter().rev().filter(|p| locked || !tl.layers[p.source.layer].locked()) {
        for (s, m) in hit::shapes(doc, &placed.item, placed.matrix, 0).into_iter().rev() {
            // Text hits as its box, which isn't paint.
            let std::borrow::Cow::Borrowed(s) = s else { continue };
            if let Some(inverse) = m.inverse() {
                found.push((placed.source, s, inverse.apply(point), 1.0 / (m.a * m.d - m.b * m.c).abs().sqrt(), m));
            }
        }
    }
    found
}

/// Whether element `r` of `tl` is a shape itself, not a group or symbol drawing one.
fn is_shape(tl: &Timeline, r: ElementRef) -> bool {
    let key = tl.layers.get(r.layer).and_then(|l| l.frames.get(r.keyframe));
    matches!(key.and_then(|k| k.elements.get(r.element)), Some(Element::Shape(_)))
}

/// The style index `sides` should take to show `value`: their own style, changed
/// in place, if they all have one that nothing else uses; else an equal style,
/// or a new one. `kinds` are the sides (FILL0, FILL1, STROKE) that use `styles`.
fn restyle<T: PartialEq>(styles: &mut Vec<T>, value: T, g: &Graph, sides: &HashSet<(usize, usize)>, kinds: &[usize]) -> u32 {
    let mut current = sides.iter().map(|&(i, k)| g.pieces[i].styles[k]);
    let own = current.next().unwrap_or(0);
    let alone = current.all(|j| j == own)
        && g.pieces.iter().enumerate().all(|(i, piece)| kinds.iter().all(|&k| piece.styles[k] != own || sides.contains(&(i, k))));
    if alone && let Some(slot) = (own as usize).checked_sub(1).and_then(|i| styles.get_mut(i)) {
        *slot = value;
        return own;
    }
    match styles.iter().position(|s| *s == value) {
        Some(i) => i as u32 + 1,
        None => {
            styles.push(value);
            styles.len() as u32
        }
    }
}

/// Indices into a piece's `styles`.
const FILL0: usize = 0;
const FILL1: usize = 1;
const STROKE: usize = 2;
/// Walks enclosing less (in square units) enclose nothing.
const EPS: f64 = 1e-4;

/// A segment of the shape, `edges[e].paths[p].segs[s]`, or a line closing a gap.
struct Piece {
    seg: Option<(usize, usize, usize)>,
    /// fill0, fill1, stroke.
    styles: [u32; 3],
    /// The segment flattened, from start to end.
    points: Vec<Point>,
    /// Its start and end vertex.
    ends: [usize; 2],
}

/// A shape's segments as a planar graph. Half-edge `2i` runs along piece `i`
/// and `2i + 1` back; the face on a half-edge's right sees the piece's fill1 or
/// fill0 respectively.
struct Graph {
    pieces: Vec<Piece>,
    vertices: Vec<Point>,
    /// The half-edges leaving each vertex, by angle.
    out: Vec<Vec<usize>>,
    walks: Vec<Walk>,
}

/// A closed walk of half-edges keeping one face on its right.
struct Walk {
    halves: Vec<usize>,
    polygon: Vec<Point>,
    /// Positive for a walk running clockwise (on screen) around a face, zero or
    /// negative for one around the outside of a connected set of segments.
    area: f64,
    /// Which connected set of segments it walks.
    component: usize,
}

/// What a shape shows at a point.
enum Seen {
    /// The piece whose stroke is there.
    Stroke(usize),
    /// The half-edges around the filled face there.
    Fill(Vec<usize>),
}

// ponytail: segments are taken to meet only at their ends, as Animate saves shapes. Our own
// drawing tools don't split where a stroke crosses itself (a pencil loop that overshoots,
// brush self-overlap), so such an area has no face. Split crossings when shapes are drawn.
impl Graph {
    /// `gap` > 0 joins loose ends that close.
    fn new(s: &Shape, gap: f32) -> Graph {
        let mut g = Graph { pieces: Vec::new(), vertices: Vec::new(), out: Vec::new(), walks: Vec::new() };
        for (e, edge) in s.edges.iter().enumerate() {
            // Undrawn, with the same fill each side: it bounds nothing.
            if edge.stroke == 0 && edge.fill0 == edge.fill1 {
                continue;
            }
            for (p, path) in edge.paths.iter().enumerate() {
                let mut from = path.start;
                for (i, &seg) in path.segs.iter().enumerate() {
                    let points = hit::polyline(&Path { start: from, segs: vec![seg] });
                    g.add(Some((e, p, i)), [edge.fill0, edge.fill1, edge.stroke], points);
                    from = seg.end();
                }
            }
        }
        if gap > 0.0 {
            g.close_gaps(gap);
        }
        g.link();
        g
    }

    fn add(&mut self, seg: Option<(usize, usize, usize)>, styles: [u32; 3], points: Vec<Point>) {
        let (Some(&a), Some(&b)) = (points.first(), points.last()) else { return };
        // A segment going nowhere has no direction to sort by.
        if points.iter().all(|&q| near(q, a)) {
            return;
        }
        let ends = [self.vertex(a), self.vertex(b)];
        self.pieces.push(Piece { seg, styles, points, ends });
    }

    fn vertex(&mut self, p: Point) -> usize {
        // ponytail: O(n²) search, as render.rs chains fills: a 3000-segment shape takes ~85 ms in
        // a debug build, per click and per pointer move of a Paint Bucket drag. Hash a grid (or
        // keep the graph for the drag) if dense shapes stutter.
        match self.vertices.iter().position(|&v| near(v, p)) {
            Some(i) => i,
            None => {
                self.vertices.push(p);
                self.vertices.len() - 1
            }
        }
    }

    /// Joins each loose end to the nearest other loose end within `gap`, with a
    /// straight piece of no style, as Animate's Close Gaps options do.
    // ponytail: ends only meet ends. An end stopping short of the middle of a segment
    // leaves its area open; splitting the segment there would close it.
    fn close_gaps(&mut self, gap: f32) {
        let mut degree = vec![0; self.vertices.len()];
        for piece in &self.pieces {
            piece.ends.iter().for_each(|&v| degree[v] += 1);
        }
        let loose: Vec<usize> = (0..self.vertices.len()).filter(|&v| degree[v] == 1).collect();
        let mut pairs = BTreeSet::new();
        for &a in &loose {
            let d = |b: usize| dist(self.vertices[a], self.vertices[b]);
            let nearest = loose.iter().copied().filter(|&b| b != a && d(b) <= gap).min_by(|&x, &y| d(x).total_cmp(&d(y)));
            if let Some(b) = nearest {
                pairs.insert((a.min(b), a.max(b)));
            }
        }
        for (a, b) in pairs {
            let points = vec![self.vertices[a], self.vertices[b]];
            self.pieces.push(Piece { seg: None, styles: [0; 3], points, ends: [a, b] });
        }
    }

    /// Sorts the half-edges around each vertex and walks every face.
    fn link(&mut self) {
        let halves = 2 * self.pieces.len();
        let angles: Vec<f32> = (0..halves).map(|h| self.angle(h)).collect();
        let mut out = vec![Vec::new(); self.vertices.len()];
        for h in 0..halves {
            out[self.origin(h)].push(h);
        }
        for around in &mut out {
            around.sort_by(|&a, &b| angles[a].total_cmp(&angles[b]));
        }
        let mut rank = vec![0; halves];
        for around in &out {
            for (i, &h) in around.iter().enumerate() {
                rank[h] = i;
            }
        }
        let mut root: Vec<usize> = (0..self.vertices.len()).collect();
        for piece in &self.pieces {
            let (a, b) = (find(&mut root, piece.ends[0]), find(&mut root, piece.ends[1]));
            root[a] = b;
        }
        let mut seen = vec![false; halves];
        let mut walks = Vec::new();
        for start in 0..halves {
            let (mut h, mut walk) = (start, Vec::new());
            while !seen[h] {
                seen[h] = true;
                walk.push(h);
                // The face on h's right goes on, at h's end, along the half-edge just
                // before h's twin in angle order: the sharpest turn right.
                let around = &out[self.origin(h ^ 1)];
                h = around[(rank[h ^ 1] + around.len() - 1) % around.len()];
            }
            if walk.is_empty() {
                continue;
            }
            let polygon: Vec<Point> = walk.iter().flat_map(|&h| self.points(h).split_last().map(|(_, rest)| rest.to_vec()).unwrap_or_default()).collect();
            let component = find(&mut root, self.origin(start));
            walks.push(Walk { halves: walk, area: area(&polygon), polygon, component });
        }
        (self.out, self.walks) = (out, walks);
    }

    fn origin(&self, h: usize) -> usize {
        self.pieces[h / 2].ends[h % 2]
    }

    /// Half-edge `h` flattened, from its origin.
    fn points(&self, h: usize) -> Vec<Point> {
        let mut points = self.pieces[h / 2].points.clone();
        if h % 2 == 1 {
            points.reverse();
        }
        points
    }

    /// The direction `h` leaves its origin in, as an angle clockwise from +x.
    fn angle(&self, h: usize) -> f32 {
        let points = self.points(h);
        let o = points[0];
        let q = points.iter().copied().find(|&q| !near(q, o)).unwrap_or(o);
        (q.1 - o.1).atan2(q.0 - o.0)
    }

    /// The half-edges around the face around `p`: the smallest walk surrounding
    /// it, and those around the segments inside that walk that don't touch it
    /// (the face's holes). None outside every face.
    fn face_at(&self, p: Point) -> Option<Vec<usize>> {
        let outer = self.around(p, None)?;
        let mut halves = self.walks[outer].halves.clone();
        for w in &self.walks {
            let q = w.polygon[0];
            if w.area <= EPS && surrounds(&self.walks[outer].polygon, q) && self.around(q, Some(w.component)) == Some(outer) {
                halves.extend(&w.halves);
            }
        }
        Some(halves)
    }

    /// The smallest face walk surrounding `p`, other than `skip`'s.
    // ponytail: every walk is tested, so finding holes is quadratic in walks. Fine for
    // drawn shapes; bound the walks by box first if dense imported art gets slow.
    fn around(&self, p: Point, skip: Option<usize>) -> Option<usize> {
        let walks = self.walks.iter().enumerate();
        let surrounding = walks.filter(|(_, w)| w.area > EPS && Some(w.component) != skip && surrounds(&w.polygon, p));
        surrounding.min_by(|a, b| a.1.area.total_cmp(&b.1.area)).map(|(i, _)| i)
    }

    /// The fill a face shows: what its first segment of the shape names on its side.
    fn fill(&self, face: &[usize]) -> u32 {
        let real = face.iter().find(|&&h| self.pieces[h / 2].seg.is_some());
        real.map_or(0, |&h| self.pieces[h / 2].styles[right(h)])
    }

    /// The stroked piece nearest `p`, if its stroke reaches `p` give or take `tolerance`.
    fn stroke_at(&self, s: &Shape, p: Point, tolerance: f32) -> Option<usize> {
        let reaching = self.pieces.iter().enumerate().filter_map(|(i, piece)| {
            let width = style(&s.strokes, piece.styles[STROKE])?.width;
            let d = piece.points.windows(2).map(|w| hit::distance(p, w[0], w[1])).fold(f32::INFINITY, f32::min);
            (d <= width / 2.0 + tolerance).then_some((d, i))
        });
        reaching.min_by(|a, b| a.0.total_cmp(&b.0)).map(|(_, i)| i)
    }

    /// What `s` shows at `p`: a stroke, else a filled face.
    fn seen(&self, s: &Shape, p: Point, tolerance: f32) -> Option<Seen> {
        if let Some(i) = self.stroke_at(s, p, tolerance) {
            return Some(Seen::Stroke(i));
        }
        let face = self.face_at(p)?;
        style(&s.fills, self.fill(&face)).map(|_| Seen::Fill(face))
    }

    /// Piece `start` and the pieces of the same stroke joined to it end to end.
    fn connected(&self, start: usize) -> Vec<usize> {
        let stroke = self.pieces[start].styles[STROKE];
        let mut seen = vec![false; self.pieces.len()];
        seen[start] = true;
        let (mut found, mut next) = (vec![start], 0);
        while let Some(&i) = found.get(next) {
            next += 1;
            for h in self.pieces[i].ends.iter().flat_map(|&v| &self.out[v]) {
                let j = h / 2;
                if !seen[j] && self.pieces[j].styles[STROKE] == stroke {
                    seen[j] = true;
                    found.push(j);
                }
            }
        }
        found
    }

    /// Sets each of `sides` (piece, FILL0 / FILL1 / STROKE) to style `index` and
    /// rewrites `s`'s edges to match: an edge whose segments now differ splits,
    /// and gap closers become edges.
    fn apply(&self, s: &mut Shape, sides: &HashSet<(usize, usize)>, index: u32) {
        let mut styles: HashMap<usize, [u32; 3]> = HashMap::new();
        for &(i, side) in sides {
            styles.entry(i).or_insert(self.pieces[i].styles)[side] = index;
        }
        let by_seg: HashMap<_, _> = styles.iter().filter_map(|(&i, &st)| Some((self.pieces[i].seg?, st))).collect();
        let mut edges = Vec::new();
        for (e, edge) in s.edges.iter().enumerate() {
            let own = [edge.fill0, edge.fill1, edge.stroke];
            let mut split = Vec::new();
            for (p, path) in edge.paths.iter().enumerate() {
                let (mut from, mut prev) = (path.start, None);
                for (i, &seg) in path.segs.iter().enumerate() {
                    let st = by_seg.get(&(e, p, i)).copied().unwrap_or(own);
                    let target = edge_with(&mut split, st);
                    match target.paths.last_mut() {
                        // Styled as the segment before: its run goes on.
                        Some(run) if prev == Some(st) => run.segs.push(seg),
                        _ => target.paths.push(Path { start: from, segs: vec![seg] }),
                    }
                    (from, prev) = (seg.end(), Some(st));
                }
            }
            edges.extend(split);
        }
        let mut closers: Vec<_> = styles.into_iter().filter(|&(i, _)| self.pieces[i].seg.is_none()).collect();
        closers.sort_by_key(|&(i, _)| i);
        for (i, [fill0, fill1, stroke]) in closers {
            let points = &self.pieces[i].points;
            let path = Path { start: points[0], segs: vec![Seg::Line(points[points.len() - 1])] };
            edges.push(Edge { fill0, fill1, stroke, paths: vec![path] });
        }
        s.edges = edges;
    }
}

/// The edge of `edges` with these styles, added if missing.
fn edge_with(edges: &mut Vec<Edge>, [fill0, fill1, stroke]: [u32; 3]) -> &mut Edge {
    let i = match edges.iter().position(|e| [e.fill0, e.fill1, e.stroke] == [fill0, fill1, stroke]) {
        Some(i) => i,
        None => {
            edges.push(Edge { fill0, fill1, stroke, paths: Vec::new() });
            edges.len() - 1
        }
    };
    &mut edges[i]
}

/// Which of its piece's fills half-edge `h` has on its right.
fn right(h: usize) -> usize {
    if h % 2 == 0 { FILL1 } else { FILL0 }
}

/// Style `index` of `styles`, counting from 1 as edges do; 0 is none.
fn style<T>(styles: &[T], index: u32) -> Option<&T> {
    (index as usize).checked_sub(1).and_then(|i| styles.get(i))
}

fn find(root: &mut [usize], mut v: usize) -> usize {
    while root[v] != v {
        root[v] = root[root[v]];
        v = root[v];
    }
    v
}

fn near(a: Point, b: Point) -> bool {
    (a.0 - b.0).abs() < 1e-3 && (a.1 - b.1).abs() < 1e-3
}

fn dist(a: Point, b: Point) -> f32 {
    (a.0 - b.0).hypot(a.1 - b.1)
}

/// Even-odd: is `p` inside the closed `polygon`?
fn surrounds(polygon: &[Point], p: Point) -> bool {
    let mut inside = false;
    for (i, &a) in polygon.iter().enumerate() {
        let b = polygon[(i + 1) % polygon.len()];
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < a.0 + (p.1 - a.1) / (b.1 - a.1) * (b.0 - a.0) {
            inside = !inside;
        }
    }
    inside
}

/// The signed area inside the closed `polygon`, positive if it runs clockwise
/// on screen. In f64, so a walk out and back along the same segments cancels.
fn area(polygon: &[Point]) -> f64 {
    let n = polygon.len();
    let twice: f64 = (0..n)
        .map(|i| {
            let (a, b) = (polygon[i], polygon[(i + 1) % n]);
            a.0 as f64 * b.1 as f64 - b.0 as f64 * a.1 as f64
        })
        .sum();
    twice / 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw;
    use crate::render::render;

    const WHITE: [u8; 3] = [255, 255, 255];
    const BLACK: [u8; 3] = [0, 0, 0];
    const RED: [u8; 3] = [255, 0, 0];
    const GREEN: [u8; 3] = [0, 255, 0];
    const BLUE: [u8; 3] = [0, 0, 255];

    fn solid([r, g, b]: [u8; 3]) -> Paint {
        Paint::Solid([r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0])
    }

    fn line(width: f32, color: [u8; 3]) -> Stroke {
        Stroke { width, paint: solid(color), cap: Cap::Round, join: Join::Round }
    }

    /// A 100×100 document showing `elements` on its one frame.
    fn doc(elements: Vec<Element>) -> Document {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        doc.scenes[0].layers[0].frames[0].elements = elements;
        doc
    }

    fn shapes(shapes: Vec<Shape>) -> Document {
        doc(shapes.into_iter().map(Element::Shape).collect())
    }

    fn shape(d: &Document, i: usize) -> &Shape {
        let Element::Shape(s) = &d.scenes[0].layers[0].frames[0].elements[i] else { panic!("not a shape") };
        s
    }

    /// Checks (x, y, color) pixels of frame 1; pixel (x, y) covers x..x+1, y..y+1.
    fn check(d: &Document, pixels: &[(u32, u32, [u8; 3])]) {
        let (pixmap, skipped) = render(d, 0, 1.0).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        for &(x, y, want) in pixels {
            let c = pixmap.pixel(x, y).unwrap().demultiply();
            assert_eq!([c.red(), c.green(), c.blue()], want, "pixel {x}, {y}");
        }
    }

    fn lines(points: &[Point]) -> Path {
        Path { start: points[0], segs: points[1..].iter().map(|&p| Seg::Line(p)).collect() }
    }

    /// Clockwise on screen, so the inside is on the right: fill1.
    fn square(x: f32, y: f32, side: f32) -> Vec<Point> {
        vec![(x, y), (x + side, y), (x + side, y + side), (x, y + side), (x, y)]
    }

    fn raw(fills: Vec<Paint>, strokes: Vec<Stroke>, edges: Vec<Edge>) -> Shape {
        Shape { matrix: Matrix::IDENTITY, fills, strokes, edges, extra: Extra::default() }
    }

    /// Paint Bucket on the scene's first frame, kept: whether it changed anything.
    fn fill(d: &mut Document, at: Point, gap: f32, paint: Paint) -> bool {
        let change = bucket(d, &d.scenes[0], 0, at, gap, paint);
        keep(d, change)
    }

    /// Ink Bottle on the scene's first frame, kept: whether it changed anything.
    fn restroke(d: &mut Document, at: Point, tolerance: f32, stroke: Stroke) -> bool {
        let change = ink(d, &d.scenes[0], 0, at, tolerance, stroke);
        keep(d, change)
    }

    fn keep(d: &mut Document, change: Option<(ElementRef, Shape)>) -> bool {
        let Some((r, s)) = change else { return false };
        d.scenes[0].layers[r.layer].frames[r.keyframe].elements[r.element] = Element::Shape(s);
        true
    }

    #[test]
    fn bucket_recolors_a_region_or_fills_an_outline() {
        let filled = draw::rectangle([(10.0, 10.0), (40.0, 40.0)], 0.0, Some(line(2.0, BLACK)), Some(solid(RED)));
        let outline = draw::rectangle([(60.0, 10.0), (90.0, 40.0)], 0.0, Some(line(2.0, BLACK)), None);
        // A pencil loop that ends where it starts.
        let lap = draw::pencil(&[(10.0, 60.0), (40.0, 60.0), (40.0, 90.0), (10.0, 90.0), (10.0, 60.0)], line(2.0, BLACK), 0.5);
        let mut d = shapes(vec![filled, outline, lap]);
        let blue = |d: &mut Document, x, y| fill(d, (x, y), 0.0, solid(BLUE));
        assert!(blue(&mut d, 25.0, 25.0));
        assert!(blue(&mut d, 75.0, 25.0));
        assert!(blue(&mut d, 25.0, 75.0));
        assert!(!blue(&mut d, 50.0, 50.0), "nothing there");
        assert!(!blue(&mut d, 25.0, 25.0), "already blue");
        check(&d, &[(25, 25, BLUE), (10, 25, BLACK), (75, 25, BLUE), (60, 25, BLACK), (95, 25, WHITE), (25, 75, BLUE), (50, 50, WHITE)]);
        assert_eq!(shape(&d, 0).fills, [solid(BLUE)], "a style no other region uses changes in place");
    }

    #[test]
    fn area_fill_is_what_the_bucket_would_replace() {
        let filled = draw::rectangle([(10.0, 10.0), (40.0, 40.0)], 0.0, Some(line(2.0, BLACK)), Some(solid(RED)));
        let outline = draw::rectangle([(60.0, 10.0), (90.0, 40.0)], 0.0, Some(line(2.0, BLACK)), None);
        let d = shapes(vec![filled, outline]);
        let at = |x, y| area_fill(&d, &d.scenes[0], 0, (x, y), 0.0);
        assert_eq!(at(25.0, 25.0), Some(Some(solid(RED))));
        assert_eq!(at(75.0, 25.0), Some(None), "an empty enclosed area");
        assert_eq!(at(50.0, 50.0), None, "nothing to fill");
    }

    #[test]
    fn bucket_splits_off_a_region_sharing_its_style() {
        // One red style for a donut (its hole runs counterclockwise, so the red is
        // outside the hole) and a square apart from it.
        let mut hole = square(30.0, 30.0, 20.0);
        hole.reverse();
        let paths = vec![lines(&square(10.0, 10.0, 60.0)), lines(&hole), lines(&square(80.0, 80.0, 15.0))];
        let mut d = shapes(vec![raw(vec![solid(RED)], vec![], vec![Edge { fill0: 0, fill1: 1, stroke: 0, paths }])]);
        check(&d, &[(20, 20, RED), (40, 40, WHITE), (85, 85, RED)]);

        assert!(fill(&mut d, (20.0, 20.0), 0.0, solid(BLUE)));
        check(&d, &[(20, 20, BLUE), (60, 60, BLUE), (40, 40, WHITE), (85, 85, RED)]);
        // The hole is a face of its own.
        assert!(fill(&mut d, (40.0, 40.0), 0.0, solid(GREEN)));
        check(&d, &[(40, 40, GREEN), (20, 20, BLUE), (85, 85, RED)]);
        assert_eq!(shape(&d, 0).fills, [solid(RED), solid(BLUE), solid(GREEN)]);
    }

    #[test]
    fn a_stroke_across_a_fill_divides_it() {
        // A red square, with vertices where a black line crosses it, red on both sides.
        let outline = lines(&[(10.0, 10.0), (50.0, 10.0), (90.0, 10.0), (90.0, 90.0), (50.0, 90.0), (10.0, 90.0), (10.0, 10.0)]);
        let divider = lines(&[(50.0, 10.0), (50.0, 90.0)]);
        let edges = vec![
            Edge { fill0: 0, fill1: 1, stroke: 0, paths: vec![outline] },
            Edge { fill0: 1, fill1: 1, stroke: 1, paths: vec![divider] },
        ];
        let mut d = shapes(vec![raw(vec![solid(RED)], vec![line(2.0, BLACK)], edges)]);
        assert!(fill(&mut d, (30.0, 50.0), 0.0, solid(BLUE)));
        check(&d, &[(30, 50, BLUE), (70, 50, RED), (50, 50, BLACK)]);
    }

    #[test]
    fn close_gaps_joins_loose_ends() {
        // A pencil square whose end stops 3 px short of its start.
        let open = draw::pencil(&[(10.0, 10.0), (90.0, 10.0), (90.0, 90.0), (10.0, 90.0), (10.0, 13.0)], line(1.0, BLACK), 0.5);
        let mut d = shapes(vec![open]);
        assert!(!fill(&mut d, (50.0, 50.0), 2.0, solid(BLUE)), "the gap is wider");
        assert!(fill(&mut d, (50.0, 50.0), 4.0, solid(BLUE)));
        check(&d, &[(50, 50, BLUE), (11, 11, BLUE), (95, 50, WHITE), (5, 50, WHITE)]);
    }

    #[test]
    fn ink_restyles_a_stroke_or_outlines_a_fill() {
        let stroked = draw::rectangle([(10.0, 10.0), (40.0, 40.0)], 0.0, Some(line(2.0, BLACK)), Some(solid(RED)));
        let bare = draw::rectangle([(60.0, 10.0), (90.0, 40.0)], 0.0, None, Some(solid(RED)));
        // Two outlines in one style.
        let pair = raw(vec![], vec![line(2.0, BLACK)], vec![Edge { fill0: 0, fill1: 0, stroke: 1, paths: vec![lines(&square(10.0, 60.0, 30.0)), lines(&square(60.0, 60.0, 30.0))] }]);
        let mut d = shapes(vec![stroked, bare, pair]);
        check(&d, &[(8, 25, WHITE), (59, 25, WHITE), (8, 75, WHITE)]);

        let wide = line(6.0, GREEN);
        let mut apply = |x, y| restroke(&mut d, (x, y), 0.0, wide.clone());
        assert!(apply(10.0, 25.0), "on the stroke");
        assert!(apply(75.0, 25.0), "inside a fill without one");
        assert!(apply(10.0, 75.0), "one of the two outlines");
        assert!(!apply(50.0, 50.0), "nothing there");
        assert!(!apply(10.0, 25.0), "already like that");
        check(&d, &[(8, 25, GREEN), (40, 40, GREEN), (25, 25, RED), (58, 25, GREEN), (62, 25, GREEN), (75, 25, RED)]);
        check(&d, &[(8, 75, GREEN), (60, 75, BLACK), (58, 75, WHITE)]);
        assert_eq!(shape(&d, 0).strokes, [wide.clone()], "a style no other stroke uses changes in place");
        assert_eq!(shape(&d, 2).strokes, [line(2.0, BLACK), wide]);
    }

    #[test]
    fn eyedropper_picks_fills_and_strokes_inside_symbols_too() {
        let filled = draw::rectangle([(10.0, 10.0), (40.0, 40.0)], 0.0, Some(line(2.0, BLACK)), Some(solid(RED)));
        let outline = draw::rectangle([(60.0, 10.0), (90.0, 40.0)], 0.0, Some(line(2.0, BLACK)), None);
        let mut box_doc = shapes(vec![draw::rectangle([(0.0, 0.0), (10.0, 10.0)], 0.0, None, Some(solid(BLUE)))]);
        // A movie clip of a blue 10 px square at double size, and the same in a group.
        let instance = Instance {
            symbol: "Box".into(),
            kind: SymbolKind::MovieClip,
            matrix: Matrix { a: 2.0, d: 2.0, tx: 10.0, ty: 60.0, ..Matrix::IDENTITY },
            pivot: (0.0, 0.0),
            color: ColorTransform::IDENTITY,
            looping: Loop::Loop,
            first_frame: 0,
            extra: Extra::default(),
        };
        let group = Element::Group(vec![Element::Shape(draw::rectangle([(60.0, 60.0), (90.0, 90.0)], 0.0, None, Some(solid(GREEN))))]);
        let mut d = doc(vec![Element::Shape(filled), Element::Shape(outline), Element::Instance(instance), group]);
        d.library.insert("Box".into(), Symbol { kind: SymbolKind::MovieClip, timeline: box_doc.scenes.remove(0), extra: Extra::default() });

        let at = |x, y| pick(&d, &d.scenes[0], 0, (x, y), 1.0);
        assert_eq!(at(25.0, 25.0), Some(Picked::Fill(solid(RED))));
        assert_eq!(at(11.5, 25.0), Some(Picked::Stroke(line(2.0, BLACK))), "within tolerance");
        assert_eq!(at(75.0, 25.0), None, "inside an unfilled outline");
        assert_eq!(at(20.0, 70.0), Some(Picked::Fill(solid(BLUE))), "inside the clip");
        assert_eq!(at(75.0, 75.0), Some(Picked::Fill(solid(GREEN))), "inside the group");
        assert_eq!(at(50.0, 50.0), None);

        // The bucket and ink bottle leave symbols and groups alone.
        let before = d.clone();
        assert!(!fill(&mut d, (20.0, 70.0), 0.0, solid(RED)));
        assert!(!fill(&mut d, (75.0, 75.0), 0.0, solid(RED)));
        assert!(!restroke(&mut d, (20.0, 70.0), 1.0, line(4.0, RED)));
        assert_eq!(d, before);
        // Editing the symbol, its timeline's own shape takes paint, in its own coordinates.
        let (r, s) = bucket(&d, &d.library["Box"].timeline, 0, (5.0, 5.0), 0.0, solid(RED)).expect("the symbol's shape");
        assert_eq!((r, s.fills), (ElementRef { layer: 0, keyframe: 0, element: 0 }, vec![solid(RED)]));
    }

    #[test]
    fn gradients_span_the_shape_or_area_they_go_to() {
        let linear = Paint::Gradient(Gradient::new(GradientKind::Linear));
        let mut square = draw::rectangle([(10.0, 10.0), (50.0, 30.0)], 0.0, None, Some(linear.clone()));
        let Paint::Gradient(fitted) = &square.fills[0] else { panic!("not a gradient") };
        assert_eq!(fitted.matrix, Matrix { a: 40.0 / 1638.4, d: 20.0 / 1638.4, tx: 30.0, ty: 20.0, ..Matrix::IDENTITY }, "drawn: across its box");
        square.matrix = Matrix::translate(5.0, 0.0);
        let mut d = shapes(vec![square.clone()]);
        let r = ElementRef { layer: 0, keyframe: 0, element: 0 };
        assert_eq!(fill_at(&d, &d.scenes[0], 0, (40.0, 20.0)), Some((r, 1)));
        assert_eq!(fill_at(&d, &d.scenes[0], 0, (12.0, 20.0)), None, "outside, the shape having moved");
        // The Eyedropper hands the gradient over where it lies on stage.
        let Some(Picked::Fill(Paint::Gradient(picked))) = pick(&d, &d.scenes[0], 0, (40.0, 20.0), 0.0) else { panic!("no gradient") };
        assert_eq!(picked.matrix, Matrix::translate(5.0, 0.0) * fitted.matrix);
        // The bucket's area, in the shape's own coordinates.
        let (b, m) = area_box(&d, &d.scenes[0], 0, (40.0, 20.0), 0.0).unwrap();
        assert_eq!((b, m), (hit::Rect { min: (10.0, 10.0), max: (50.0, 30.0) }, square.matrix));
        // The Ink Bottle's gradient spans the shape it outlines.
        let stroke = Stroke { paint: linear, ..line(2.0, BLACK) };
        assert!(restroke(&mut d, (40.0, 20.0), 0.0, stroke));
        assert_eq!(gradient_of(&shape(&d, 0).strokes[0].paint).matrix, fitted.matrix);
        // An outline drawn over it, unfilled, leaves its fill to take.
        let outline = draw::rectangle([(20.0, 12.0), (48.0, 28.0)], 0.0, Some(line(1.0, BLACK)), None);
        d.scenes[0].layers[0].frames[0].elements.push(Element::Shape(outline));
        assert_eq!(fill_at(&d, &d.scenes[0], 0, (40.0, 20.0)), Some((r, 1)));
    }

    fn gradient_of(p: &Paint) -> &Gradient {
        let Paint::Gradient(g) = p else { panic!("{p:?}") };
        g
    }

    #[test]
    fn shapes_read_from_xfl() {
        // A hand-written fixture: the ball's region is four edges, two with the fill on their left.
        let mut d = crate::xfl::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tween")).unwrap();
        let light_blue = Paint::Solid([0xCC as f32 / 255.0, 0xE5 as f32 / 255.0, 1.0, 1.0]);
        let at = |d: &Document, x, y| pick(d, &d.scenes[0], 0, (x, y), 0.0);
        assert_eq!(at(&d, 100.0, 200.0), Some(Picked::Fill(solid(RED))), "the ball");
        assert_eq!(at(&d, 77.0, 200.0), Some(Picked::Fill(solid(RED))), "inside its curved side");
        assert_eq!(at(&d, 72.0, 200.0), Some(Picked::Fill(light_blue)), "the background, outside it");
        assert_eq!(at(&d, 49.0, 200.0), Some(Picked::Stroke(line(4.0, BLACK))));
        assert_eq!(at(&d, 520.0, 30.0), Some(Picked::Fill(solid(GREEN))), "a movie clip");

        check(&d, &[(300, 300, [0xCC, 0xE5, 0xFF]), (48, 300, BLACK)]);
        assert!(fill(&mut d, (300.0, 300.0), 0.0, solid(BLUE)));
        assert!(restroke(&mut d, (50.0, 100.0), 0.0, line(2.0, RED)));
        check(&d, &[(300, 300, BLUE), (100, 200, RED), (50, 300, RED), (48, 300, WHITE)]);
    }
}
