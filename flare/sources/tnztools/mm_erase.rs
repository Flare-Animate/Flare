//! Eraser tool geometry: the area a drag sweeps, cut out of shapes' fills and
//! strokes. i_overlay does the polygon booleans.
//!
//! ponytail: the fills and strokes an erase touches are rebuilt from straight
//! lines (curves flattened to within a twip), so they lose their curves there;
//! untouched ones keep theirs. Fit quadratics to the result if files grow too big.

use std::f32::consts::{PI, TAU};

use i_overlay::core::fill_rule::FillRule;
use i_overlay::core::overlay_rule::OverlayRule;
use i_overlay::float::clip::FloatClip;
use i_overlay::float::relate::FloatPredicateOverlay;
use i_overlay::float::simplify::SimplifyShape;
use i_overlay::float::single::SingleFloatOverlay;
use i_overlay::string::clip::ClipRule;

use crate::doc::*;
use crate::hit::distance;

/// Polygons as i_overlay takes them: a contour is a closed run of points, a
/// region an outline with its holes.
type Contour = Vec<[f32; 2]>;
type Region = Vec<Contour>;

/// How far flattened curves and the round eraser's sides stray: a twip, the XFL grid.
const TOLERANCE: f32 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tip {
    Round,
    Square,
}

/// What an erase removes from a shape.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Target {
    /// Fills and strokes (Erase Normal).
    All,
    Fills,
    Strokes,
    /// Only the connected piece of fill under this point (Erase Inside).
    FillAt(Point),
}

/// What a `tip` `size` px across covers dragged along `points`: per step, the
/// convex hull of the tip at both ends; for a single point, the tip. The
/// polygons overlap and run clockwise on screen.
pub fn sweep(points: &[Point], tip: Tip, size: f32) -> Vec<Vec<Point>> {
    let r = size / 2.0;
    let outline: Vec<Point> = match tip {
        Tip::Square => vec![(-r, -r), (r, -r), (r, r), (-r, r)],
        // Corners on the circle, enough that the sides stray under TOLERANCE.
        Tip::Round => {
            let n = (PI / (1.0 - TOLERANCE / r).max(-1.0).acos()).ceil().clamp(8.0, 256.0) as usize;
            (0..n).map(|i| TAU * i as f32 / n as f32).map(|a| (r * a.cos(), r * a.sin())).collect()
        }
    };
    let at = |c: Point| outline.iter().map(move |o| (c.0 + o.0, c.1 + o.1));
    match points {
        [p] => vec![hull(at(*p).collect())],
        _ => points.windows(2).map(|w| hull(at(w[0]).chain(at(w[1])).collect())).collect(),
    }
}

/// What an eraser drag covers: a `sweep`'s overlapping pieces, merged.
// ponytail: one piece per pointer step, so a long drag with a big round tip is slow to
// merge (600 steps at 200 px: ~3.5 s in a debug build). Build round sweeps as one
// stroked outline (i_overlay's StrokeOffset, round caps and joins) if that bites.
pub struct Area(Vec<Region>);

impl Area {
    pub fn new(sweep: &[Vec<Point>]) -> Area {
        let pieces: Vec<Contour> = sweep.iter().map(|poly| poly.iter().copied().map(xy).collect()).collect();
        Area(pieces.simplify_shape(FillRule::NonZero))
    }
}

/// Erases `area`, given in the coordinates `s` is placed in, from `s`. True if
/// anything was erased; a shape erased away has no edges left.
pub fn erase(s: &mut Shape, area: &Area, target: Target) -> bool {
    let Some(to_local) = s.matrix.inverse() else { return false };
    let local = |c: &Contour| c.iter().map(|p| xy(to_local.apply((p[0], p[1])))).collect();
    let mut area: Vec<Region> = area.0.iter().map(|r| r.iter().map(local).collect()).collect();
    let all = 1..=s.fills.len() as u32;
    let (fills, strokes): (Vec<u32>, bool) = match target {
        Target::All => (all.collect(), true),
        Target::Fills => (all.collect(), false),
        Target::Strokes => (Vec::new(), true),
        Target::FillAt(p) => {
            // Fills later in the list are drawn on top.
            let p = to_local.apply(p);
            let Some((id, piece)) = all.rev().find_map(|id| pieces(s, id).into_iter().find(|r| contains(r, p)).map(|r| (id, r)))
            else {
                return false;
            };
            area = area.overlay(&piece, OverlayRule::Intersect, FillRule::EvenOdd);
            (vec![id], false)
        }
    };
    let mut erased = false;
    for id in fills {
        let outline = fill_outline(s, id);
        if FloatPredicateOverlay::with_subj_and_clip_custom(&outline, &area, FillRule::EvenOdd, Default::default())
            .interiors_intersect()
        {
            refill(s, id, outline.overlay(&area, OverlayRule::Difference, FillRule::EvenOdd));
            erased = true;
        }
    }
    if strokes {
        erased |= cut_strokes(s, &area);
    }
    erased
}

/// The Eraser's Faucet: deletes the stroke within half its width plus
/// `tolerance` of `at`, or else the connected piece of fill under it. False if
/// neither is there.
pub fn faucet(s: &mut Shape, at: Point, tolerance: f32) -> bool {
    let Some(to_local) = s.matrix.inverse() else { return false };
    let p = to_local.apply(at);
    // ponytail: as in hit.rs, the tolerance scales as if the matrix scaled evenly.
    let tolerance = tolerance / (s.matrix.a * s.matrix.d - s.matrix.b * s.matrix.c).abs().sqrt();
    // Strokes are drawn over fills, the last edge's on top.
    for j in (0..s.edges.len()).rev() {
        let e = &s.edges[j];
        let Some(stroke) = (e.stroke as usize).checked_sub(1).and_then(|i| s.strokes.get(i)) else { continue };
        let reach = stroke.width / 2.0 + tolerance;
        let near = |path: &Path| flatten(path).windows(2).any(|w| distance(p, w[0], w[1]) <= reach);
        let Some(k) = e.paths.iter().position(near) else { continue };
        let e = &mut s.edges[j];
        // The path's fills stay.
        let fills = Edge { fill0: e.fill0, fill1: e.fill1, stroke: 0, paths: vec![e.paths.remove(k)] };
        if e.paths.is_empty() {
            s.edges.remove(j);
        }
        if fills.fill0 != 0 || fills.fill1 != 0 {
            s.edges.push(fills);
        }
        return true;
    }
    for id in (1..=s.fills.len() as u32).rev() {
        let mut pieces = pieces(s, id);
        if let Some(i) = pieces.iter().position(|r| contains(r, p)) {
            pieces.remove(i);
            refill(s, id, pieces);
            return true;
        }
    }
    false
}

/// The Magic Wand's cut (trace.rs): what `area` covers of the piece of fill under
/// `at` (both in the coordinates `s` is placed in), taken out of `s` as a shape
/// of its own in that fill. None if `area` misses it.
pub fn split(s: &mut Shape, area: &Area, at: Point) -> Option<Shape> {
    let to_local = s.matrix.inverse()?;
    let local = |c: &Contour| c.iter().map(|p| xy(to_local.apply((p[0], p[1])))).collect();
    let area: Vec<Region> = area.0.iter().map(|r| r.iter().map(local).collect()).collect();
    let p = to_local.apply(at);
    let (id, piece) = (1..=s.fills.len() as u32).rev().find_map(|id| pieces(s, id).into_iter().find(|r| contains(r, p)).map(|r| (id, r)))?;
    let cut = area.overlay(&piece, OverlayRule::Intersect, FillRule::EvenOdd);
    if cut.is_empty() {
        return None;
    }
    let outline = fill_outline(s, id);
    refill(s, id, outline.overlay(&cut, OverlayRule::Difference, FillRule::EvenOdd));
    let fills = vec![s.fills[id as usize - 1].clone()];
    let mut part = Shape { matrix: s.matrix, fills, strokes: Vec::new(), edges: Vec::new(), extra: s.extra.clone() };
    refill(&mut part, 1, cut);
    Some(part)
}

/// Cuts `area` out of the strokes it crosses; true if it crossed any.
fn cut_strokes(s: &mut Shape, area: &[Region]) -> bool {
    let outside = ClipRule { invert: true, boundary_included: false };
    let inside = ClipRule { invert: false, boundary_included: false };
    let mut cut = false;
    let mut edges = Vec::with_capacity(s.edges.len());
    for e in std::mem::take(&mut s.edges) {
        if e.stroke == 0 {
            edges.push(e);
            continue;
        }
        let lines = e.paths.iter().map(|p| flatten(p).into_iter().map(xy).collect::<Contour>());
        // i_overlay drops lines that go nowhere, like a Pencil click's dot: a dot goes if the area covers it.
        let (dots, lines): (Vec<Contour>, Vec<Contour>) = lines.partition(|l| l.iter().all(|&q| near(q, l[0])));
        let covered = |dot: &Contour| area.iter().any(|r| contains(r, (dot[0][0], dot[0][1])));
        if !dots.iter().any(covered) && lines.clip_by(&area, FillRule::EvenOdd, inside).is_empty() {
            edges.push(e);
            continue;
        }
        cut = true;
        let stroke = e.stroke;
        let kept = lines.clip_by(&area, FillRule::EvenOdd, outside).into_iter().filter(|l| l.len() > 1).collect();
        let dots = dots.iter().filter(|d| !covered(d));
        let kept: Vec<Path> = join(kept).iter().chain(dots).map(|l| polyline(l)).collect();
        if e.fill0 != 0 || e.fill1 != 0 {
            edges.push(Edge { stroke: 0, ..e });
        }
        if !kept.is_empty() {
            edges.push(Edge { fill0: 0, fill1: 0, stroke, paths: kept });
        }
    }
    s.edges = edges;
    cut
}

/// Joins lines that continue one another, so their strokes join there instead
/// of ending in caps: i_overlay splits a line where it starts tracing it.
fn join(mut lines: Vec<Contour>) -> Vec<Contour> {
    let mut joined = Vec::with_capacity(lines.len());
    while let Some(mut line) = lines.pop() {
        // Onto one end, then the other.
        for _ in 0..2 {
            while let Some(i) = lines.iter().position(|l| near(l[0], line[line.len() - 1]) || near(l[l.len() - 1], line[line.len() - 1])) {
                let mut next = lines.swap_remove(i);
                if !near(next[0], line[line.len() - 1]) {
                    next.reverse();
                }
                line.extend_from_slice(&next[1..]);
            }
            line.reverse();
        }
        joined.push(line);
    }
    joined
}

/// Makes `region` the whole of fill `id`.
fn refill(s: &mut Shape, id: u32, region: Vec<Region>) {
    for e in &mut s.edges {
        for side in [&mut e.fill0, &mut e.fill1] {
            if *side == id {
                *side = 0;
            }
        }
    }
    s.edges.retain(|e| e.fill0 != 0 || e.fill1 != 0 || e.stroke != 0);
    // i_overlay's outlines run clockwise on screen and its holes counterclockwise,
    // so the fill is on their right.
    let paths: Vec<Path> = region.iter().flatten().filter(|c| c.len() > 2).map(|c| polyline(&[&c[..], &c[..1]].concat())).collect();
    if !paths.is_empty() {
        s.edges.push(Edge { fill0: 0, fill1: id, stroke: 0, paths });
    }
}

/// The connected pieces of fill `id`, each an outline with its holes.
fn pieces(s: &Shape, id: u32) -> Vec<Region> {
    fill_outline(s, id).simplify_shape(FillRule::EvenOdd)
}

/// Fill `id`'s outline as closed contours: its edges flattened, each turned to
/// keep the fill on its right, and chained end to start. They are read even-odd,
/// as render.rs fills them.
fn fill_outline(s: &Shape, id: u32) -> Vec<Contour> {
    let mut pieces: Vec<Contour> = Vec::new();
    for e in &s.edges {
        for path in &e.paths {
            let line: Contour = flatten(path).into_iter().map(xy).collect();
            if e.fill0 == id {
                pieces.push(line.iter().rev().copied().collect());
            }
            if e.fill1 == id {
                pieces.push(line);
            }
        }
    }
    let mut contours = Vec::new();
    // ponytail: O(n²) endpoint search, as in render.rs.
    while let Some(mut c) = pieces.pop() {
        while !near(c[0], c[c.len() - 1])
            && let Some(i) = pieces.iter().position(|q| near(q[0], c[c.len() - 1]))
        {
            c.extend_from_slice(&pieces.swap_remove(i)[1..]);
        }
        contours.push(c);
    }
    contours
}

/// Close enough to be one point, as render.rs chains outlines.
fn near(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3
}

/// The path's points, each curve cut into chords that stray under TOLERANCE from it.
fn flatten(path: &Path) -> Vec<Point> {
    let mut points = vec![path.start];
    for &seg in &path.segs {
        let from = points[points.len() - 1];
        if let Seg::Quad(c, to) = seg {
            // A chord across 1/n of a quadratic strays at most |from - 2c + to| / 4n² from it.
            let bend = (from.0 - 2.0 * c.0 + to.0).hypot(from.1 - 2.0 * c.1 + to.1);
            let n = (bend / (4.0 * TOLERANCE)).sqrt().ceil().clamp(1.0, 64.0) as u32;
            points.extend((1..n).map(|i| {
                let t = i as f32 / n as f32;
                let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
                (u * from.0 + v * c.0 + w * to.0, u * from.1 + v * c.1 + w * to.1)
            }));
        }
        points.push(seg.end());
    }
    points
}

/// Even-odd: is `p` inside `contours`?
fn contains(contours: &[Contour], (x, y): Point) -> bool {
    let mut inside = false;
    for c in contours {
        for (a, b) in c.iter().zip(c.iter().cycle().skip(1)) {
            if (a[1] > y) != (b[1] > y) && x < a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]) {
                inside = !inside;
            }
        }
    }
    inside
}

/// Andrew's monotone chain: the convex hull, clockwise on screen.
fn hull(mut points: Vec<Point>) -> Vec<Point> {
    points.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
    points.dedup();
    if points.len() < 3 {
        return points;
    }
    let turn = |o: Point, a: Point, b: Point| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut h: Vec<Point> = Vec::with_capacity(points.len() + 1);
    // The top side left to right, then the bottom side back.
    for &p in &points {
        while h.len() >= 2 && turn(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
            h.pop();
        }
        h.push(p);
    }
    let top = h.len() + 1;
    for &p in points.iter().rev().skip(1) {
        while h.len() >= top && turn(h[h.len() - 2], h[h.len() - 1], p) <= 0.0 {
            h.pop();
        }
        h.push(p);
    }
    h.pop();
    h
}

fn polyline(points: &[[f32; 2]]) -> Path {
    let pt = |p: &[f32; 2]| (p[0], p[1]);
    Path { start: pt(&points[0]), segs: points[1..].iter().map(|p| Seg::Line(pt(p))).collect() }
}

fn xy((x, y): Point) -> [f32; 2] {
    [x, y]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw;
    use crate::render::render;

    const WHITE: [u8; 3] = [255, 255, 255];
    const RED: [u8; 3] = [255, 0, 0];
    const BLACK: [u8; 3] = [0, 0, 0];

    fn red() -> Paint {
        Paint::Solid([1.0, 0.0, 0.0, 1.0])
    }

    fn ink(width: f32) -> Stroke {
        Stroke { width, paint: Paint::Solid([0.0, 0.0, 0.0, 1.0]), cap: Cap::Round, join: Join::Round }
    }

    /// Renders `shapes` on a white 100×100 stage and checks `pixels` as (x, y, color).
    fn check(shapes: &[Shape], pixels: &[(u32, u32, [u8; 3])]) {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        for s in shapes {
            draw::add(&mut doc.scenes[0].layers[0].frames[0], s.clone());
        }
        let (pixmap, _) = render(&doc, 0, 1.0).unwrap();
        for &(x, y, want) in pixels {
            let c = pixmap.pixel(x, y).unwrap().demultiply();
            assert_eq!([c.red(), c.green(), c.blue()], want, "pixel {x}, {y}");
        }
    }

    fn area(points: &[Point], tip: Tip, size: f32) -> Area {
        Area::new(&sweep(points, tip, size))
    }

    /// Twice the area inside the shape's fill paths: positive if they run clockwise on screen.
    fn twice_area(s: &Shape) -> f32 {
        let fills = s.edges.iter().filter(|e| e.fill1 != 0 || e.fill0 != 0);
        let points = fills.flat_map(|e| &e.paths).map(flatten);
        points.map(|p| p.iter().zip(p.iter().cycle().skip(1)).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum::<f32>()).sum()
    }

    #[test]
    fn erases_a_hole_in_a_fill() {
        let mut s = draw::rectangle([(10.0, 10.0), (90.0, 90.0)], 0.0, None, Some(red()));
        assert!(erase(&mut s, &area(&[(50.0, 50.0)], Tip::Round, 20.0), Target::All));
        check(&[s.clone()], &[(50, 50, WHITE), (50, 42, WHITE), (50, 38, RED), (20, 20, RED), (85, 85, RED), (5, 5, WHITE)]);
        // The outline runs clockwise and the hole the other way: the fill is on their right.
        assert!(s.edges.iter().all(|e| (e.fill0, e.fill1) == (0, 1)));
        assert!((twice_area(&s) / 2.0 - (6400.0 - PI * 100.0)).abs() < 4.0, "{}", twice_area(&s) / 2.0);

        // Missing the shape changes nothing.
        let before = s.clone();
        assert!(!erase(&mut s, &area(&[(95.0, 5.0), (95.0, 95.0)], Tip::Square, 6.0), Target::All));
        assert_eq!(s, before);
    }

    #[test]
    fn a_sweep_cuts_a_line_in_two() {
        let mut s = draw::line((10.0, 50.0), (90.0, 50.0), ink(4.0));
        assert!(erase(&mut s, &area(&[(50.0, 20.0), (50.0, 80.0)], Tip::Square, 10.0), Target::All));
        assert_eq!(s.edges.iter().map(|e| e.paths.len()).sum::<usize>(), 2);
        // Round caps reach 2 px past the cuts at 45 and 55.
        check(&[s], &[(30, 50, BLACK), (70, 50, BLACK), (50, 50, WHITE), (48, 50, WHITE), (52, 50, WHITE)]);

        // An oval's outline, cut on both sides, is left as two arcs.
        let mut oval = draw::oval([(10.0, 10.0), (90.0, 90.0)], Some(ink(2.0)), None);
        assert!(erase(&mut oval, &area(&[(50.0, 0.0), (50.0, 100.0)], Tip::Round, 10.0), Target::All));
        assert_eq!(oval.edges.iter().map(|e| e.paths.len()).sum::<usize>(), 2);
    }

    #[test]
    fn dots_go_when_covered_and_stay_otherwise() {
        // A click with the Pencil leaves a dot: a path that goes nowhere.
        let mut dot = draw::pencil(&[(50.0, 50.0)], ink(6.0), 1.0);
        assert!(erase(&mut dot, &area(&[(50.0, 50.0)], Tip::Round, 10.0), Target::All));
        assert!(dot.edges.is_empty());

        // One in the same edge as a line the eraser cuts stays where it was.
        let mut s = draw::line((10.0, 50.0), (90.0, 50.0), ink(4.0));
        s.edges[0].paths.push(Path { start: (50.0, 80.0), segs: vec![Seg::Line((50.0, 80.0))] });
        assert!(erase(&mut s, &area(&[(50.0, 40.0), (50.0, 60.0)], Tip::Square, 10.0), Target::All));
        check(&[s], &[(50, 80, BLACK), (50, 50, WHITE), (30, 50, BLACK)]);
    }

    #[test]
    fn modes_pick_fills_or_strokes() {
        let square = draw::rectangle([(10.0, 10.0), (90.0, 90.0)], 0.0, Some(ink(2.0)), Some(red()));
        // Across the middle, from beyond one side to beyond the other.
        let stripe = area(&[(0.0, 50.0), (100.0, 50.0)], Tip::Square, 10.0);

        let mut fills = square.clone();
        assert!(erase(&mut fills, &stripe, Target::Fills));
        check(&[fills], &[(50, 50, WHITE), (50, 30, RED), (10, 50, BLACK), (90, 50, BLACK), (5, 50, WHITE)]);

        let mut lines = square.clone();
        assert!(erase(&mut lines, &stripe, Target::Strokes));
        let strokes: Vec<&Path> = lines.edges.iter().filter(|e| e.stroke != 0).flat_map(|e| &e.paths).collect();
        assert_eq!(strokes.len(), 2, "the outline cut on both sides: its top and bottom halves");
        check(&[lines], &[(50, 50, RED), (9, 50, WHITE), (90, 50, WHITE), (10, 30, BLACK)]);

        let mut all = square;
        assert!(erase(&mut all, &stripe, Target::All));
        check(&[all], &[(50, 50, WHITE), (10, 50, WHITE), (50, 30, RED), (10, 30, BLACK)]);
    }

    #[test]
    fn erase_inside_keeps_to_the_fill_it_starts_in() {
        // Two squares in one shape; the sweep starts in the left one and crosses both.
        let mut s = draw::rectangle([(10.0, 10.0), (40.0, 90.0)], 0.0, None, Some(red()));
        s.edges.extend(draw::rectangle([(60.0, 10.0), (90.0, 90.0)], 0.0, None, Some(red())).edges);
        let stripe = area(&[(20.0, 50.0), (80.0, 50.0)], Tip::Round, 10.0);
        assert!(erase(&mut s, &stripe, Target::FillAt((20.0, 50.0))));
        check(&[s.clone()], &[(30, 50, WHITE), (30, 30, RED), (70, 50, RED)]);
        // Starting outside any fill erases nothing.
        assert!(!erase(&mut s, &stripe, Target::FillAt((50.0, 50.0))));
    }

    #[test]
    fn faucet_deletes_a_whole_stroke_or_fill() {
        let mut s = draw::rectangle([(10.0, 10.0), (90.0, 90.0)], 0.0, Some(ink(2.0)), Some(red()));
        // A stripe splits the fill in two.
        assert!(erase(&mut s, &area(&[(0.0, 50.0), (100.0, 50.0)], Tip::Square, 10.0), Target::Fills));
        assert!(faucet(&mut s, (50.0, 30.0), 0.0), "the top half");
        check(&[s.clone()], &[(50, 30, WHITE), (50, 70, RED), (10, 30, BLACK)]);
        assert!(faucet(&mut s, (10.5, 30.0), 0.0), "the outline");
        check(&[s.clone()], &[(10, 30, WHITE), (50, 70, RED)]);
        assert!(!faucet(&mut s, (50.0, 30.0), 0.0), "nothing left there");
        assert!(faucet(&mut s, (50.0, 70.0), 0.0));
        assert!(s.edges.is_empty());
    }

    #[test]
    fn works_in_the_shapes_placement() {
        // Drawn at the origin, then moved to (50, 50).
        let mut s = draw::rectangle([(0.0, 0.0), (40.0, 40.0)], 0.0, None, Some(red()));
        s.matrix.tx = 50.0;
        s.matrix.ty = 50.0;
        assert!(!erase(&mut s, &area(&[(20.0, 20.0)], Tip::Round, 10.0), Target::All), "where it was drawn");
        assert!(erase(&mut s, &area(&[(70.0, 70.0)], Tip::Round, 10.0), Target::All));
        check(&[s.clone()], &[(70, 70, WHITE), (60, 60, RED)]);
        let mut gone = s.clone();
        assert!(erase(&mut gone, &area(&[(40.0, 70.0), (100.0, 70.0)], Tip::Square, 50.0), Target::All));
        assert!(gone.edges.is_empty(), "erased away");
        assert!(faucet(&mut s, (60.0, 60.0), 0.0));
        assert!(s.edges.is_empty());
    }

    #[test]
    fn sweep_covers_the_path() {
        let capsule = sweep(&[(0.0, 0.0), (10.0, 0.0)], Tip::Round, 4.0);
        assert_eq!(capsule.len(), 1);
        let inside = |p: Point| contains(&[capsule[0].iter().map(|&q| xy(q)).collect()], p);
        assert!(inside((5.0, 1.9)) && inside((-1.9, 0.0)) && inside((11.9, 0.0)));
        assert!(!inside((5.0, 2.1)) && !inside((12.1, 0.0)) && !inside((11.5, 1.5)));
        let square = sweep(&[(0.0, 0.0)], Tip::Square, 4.0);
        assert_eq!(square, [vec![(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)]], "clockwise on screen");
    }
}
