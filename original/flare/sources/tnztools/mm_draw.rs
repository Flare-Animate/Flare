//! Shape builders for the drawing tools. As in Animate's Object Drawing mode,
//! each drawn shape is self-contained, sits at identity and keeps the pixel
//! coordinates it was drawn in. Closed outlines run clockwise on screen, which
//! puts their inside on the right of every edge: `fill1`.

use std::f32::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use crate::doc::*;

/// Adds `shape` on top of the frame's contents. A shape that draws nothing is dropped.
pub fn add(frame: &mut Frame, shape: Shape) {
    if !shape.edges.is_empty() {
        frame.elements.push(Element::Shape(shape));
    }
}

/// A shape at identity with one edge holding `paths`. Without paths or styles it has no edges.
/// Paths without segments are dropped, as the XFL reader does, and an end within a thousandth
/// of a pixel of its start is pinned onto it, so rounding can't leave a closed outline open.
/// A gradient fill or stroke is fitted to the shape's box.
pub fn shape(mut paths: Vec<Path>, stroke: Option<Stroke>, fill: Option<Paint>) -> Shape {
    paths.retain(|p| !p.segs.is_empty());
    for p in &mut paths {
        if let Some(Seg::Line(end) | Seg::Quad(_, end)) = p.segs.last_mut()
            && near(*end, p.start)
        {
            *end = p.start;
        }
    }
    let visible = !paths.is_empty() && (stroke.is_some() || fill.is_some());
    let edge = Edge { fill0: 0, fill1: fill.is_some().into(), stroke: stroke.is_some().into(), paths };
    let mut shape = Shape {
        matrix: Matrix::IDENTITY,
        fills: fill.into_iter().collect(),
        strokes: stroke.into_iter().collect(),
        edges: if visible { vec![edge] } else { Vec::new() },
        extra: Extra::default(),
    };
    crate::gradient::fit_all(&mut shape);
    shape
}

/// Rectangle Tool: the box with corners `rect`, in either order. A negative
/// `corner_radius` curves the corners inward, as Animate allows.
pub fn rectangle(rect: [Point; 2], corner_radius: f32, stroke: Option<Stroke>, fill: Option<Paint>) -> Shape {
    let (x0, y0, x1, y1) = bounds(rect);
    let r = corner_radius.abs().min((x1 - x0).min(y1 - y0) / 2.0);
    // Starting mid-side, the stroke's two ends meet on a straight run and every corner gets a join.
    let start = ((x0 + x1) / 2.0, y0);
    let mut path = Path { start, segs: Vec::new() };
    // Clockwise from the top right: each corner, and the center of its rounding.
    let corners = [
        ((x1, y0), (x1 - r, y0 + r)),
        ((x1, y1), (x1 - r, y1 - r)),
        ((x0, y1), (x0 + r, y1 - r)),
        ((x0, y0), (x0 + r, y0 + r)),
    ];
    for (k, (corner, inset)) in corners.into_iter().enumerate() {
        let from = (k as f32 - 1.0) * FRAC_PI_2;
        // An inward corner is a quarter circle around the corner itself, turning the other way.
        let (center, from, sweep) =
            if corner_radius < 0.0 { (corner, from - FRAC_PI_2, -FRAC_PI_2) } else { (inset, from, FRAC_PI_2) };
        line_to(&mut path, polar(center, r, from));
        if r > 0.0 {
            path.segs.extend(arc(center, (r, r), from, sweep));
        }
    }
    line_to(&mut path, start);
    shape(vec![path], stroke, fill)
}

/// Oval Tool: the ellipse filling the box with corners `rect`, as eight quadratic curves.
pub fn oval(rect: [Point; 2], stroke: Option<Stroke>, fill: Option<Paint>) -> Shape {
    let (x0, y0, x1, y1) = bounds(rect);
    let center = ((x0 + x1) / 2.0, (y0 + y1) / 2.0);
    let segs = arc(center, ((x1 - x0) / 2.0, (y1 - y0) / 2.0), 0.0, TAU).collect();
    shape(vec![Path { start: (x1, center.1), segs }], stroke, fill)
}

pub fn line(a: Point, b: Point, stroke: Stroke) -> Shape {
    shape(vec![Path { start: a, segs: vec![Seg::Line(b)] }], Some(stroke), None)
}

/// PolyStar Tool: a regular polygon with 3 to 32 `sides`, or with `star` a star
/// with that many points, its inner corners at `star` × `radius` (0..=1). The
/// first corner lies at `rotation` radians clockwise from +x, so aiming it at
/// the pointer follows the drag.
pub fn polystar(
    center: Point,
    radius: f32,
    sides: u32,
    star: Option<f32>,
    rotation: f32,
    stroke: Option<Stroke>,
    fill: Option<Paint>,
) -> Shape {
    let (per_side, inner) = match star {
        Some(ratio) => (2, radius * ratio.clamp(0.0, 1.0)),
        None => (1, radius),
    };
    let n = sides.clamp(3, 32) as usize * per_side;
    let corners: Vec<Point> = (0..n)
        .map(|i| polar(center, if i % 2 == 0 { radius } else { inner }, rotation + TAU * i as f32 / n as f32))
        .collect();
    // Starting mid-side, as rectangles do.
    let start = mid(corners[n - 1], corners[0]);
    let segs = corners.into_iter().chain([start]).map(Seg::Line).collect();
    shape(vec![Path { start, segs }], stroke, fill)
}

/// Pencil Tool, Smooth mode. `points` are the raw pointer samples: those within
/// `smoothing` pixels of the simplified line are dropped, and quadratic curves
/// join the rest. A single sample makes a dot.
pub fn pencil(points: &[Point], stroke: Stroke, smoothing: f32) -> Shape {
    let pts = simplify(points, smoothing);
    let path = pts.first().map(|&start| Path { start, segs: smooth(&pts) });
    shape(path.into_iter().collect(), Some(stroke), None)
}

/// Brush Tool: what a round brush `width` across covers along the raw pointer
/// samples `points`, as a fill: the smoothed center line offset to either side,
/// with round caps joining the two sides.
pub fn brush(points: &[Point], width: f32, fill: Paint) -> Shape {
    let half = width / 2.0;
    // The outline sits `half` out along each sample's normal, which pointer jitter would tilt.
    let pts = simplify(points, width / 10.0);
    let n = pts.len();
    match pts[..] {
        [] => return shape(Vec::new(), None, Some(fill)),
        [(x, y)] => return oval([(x - half, y - half), (x + half, y + half)], None, Some(fill)),
        _ => {}
    }
    let dirs: Vec<Point> = pts.windows(2).map(|w| unit(w[0], w[1])).collect();
    // The tangent at each sample bisects the segments either side of it.
    let tangents: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (dirs[i.saturating_sub(1)], dirs[i.min(n - 2)]);
            (a.1 + b.1).atan2(a.0 + b.0)
        })
        .collect();
    let side = |turn: f32| -> Vec<Point> {
        pts.iter().zip(&tangents).map(|(&p, &a)| polar(p, half, a + turn)).collect()
    };
    let (left, mut right) = (side(-FRAC_PI_2), side(FRAC_PI_2));
    right.reverse();
    // Clockwise: along the left side, around the end, back along the right side, around the start.
    let mut segs = smooth(&left);
    segs.extend(arc(pts[n - 1], (half, half), tangents[n - 1] - FRAC_PI_2, PI));
    segs.extend(smooth(&right));
    segs.extend(arc(pts[0], (half, half), tangents[0] + FRAC_PI_2, PI));
    // ponytail: the outline isn't merged with itself, so where a stroke overlaps itself (a
    // crossing, a loop closed on its start, a back-and-forth dab, a pen-lift hook longer than
    // width/10, any turn tighter than the brush) the overlap renders as a hole. Fix: split
    // the edges where they cross and keep those between filled and empty, as Animate stores shapes.
    shape(vec![Path { start: left[0], segs }], None, Some(fill))
}

/// Ramer-Douglas-Peucker: every sample dropped lies within `tolerance` of the
/// line through the kept ones. Repeated samples go too.
fn simplify(points: &[Point], tolerance: f32) -> Vec<Point> {
    let Some(last) = points.len().checked_sub(1) else { return Vec::new() };
    let mut keep = vec![false; points.len()];
    (keep[0], keep[last]) = (true, true);
    let mut spans = vec![(0, last)];
    while let Some((a, b)) = spans.pop() {
        let far = (a + 1..b).map(|i| (i, off_segment(points[i], points[a], points[b])));
        if let Some((i, d)) = far.max_by(|x, y| x.1.total_cmp(&y.1))
            && d > tolerance
        {
            keep[i] = true;
            spans.extend([(a, i), (i, b)]);
        }
    }
    let mut kept: Vec<Point> = points.iter().zip(keep).filter_map(|(&p, k)| k.then_some(p)).collect();
    kept.dedup();
    kept
}

/// How far `p` is from the segment `a`-`b`.
fn off_segment(p: Point, a: Point, b: Point) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    let t = if len2 > 0.0 { (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p.0 - a.0 - t * dx).hypot(p.1 - a.1 - t * dy)
}

/// Midpoint smoothing, as segments from `pts[0]` to the last point: each inner
/// point becomes the control of a curve between the midpoints of its two sides.
// ponytail: sharp turns round off like gentle ones, so a quick V loses its tip.
// Keep turns past some angle as corners if Animate's Smooth mode turns out to.
fn smooth(pts: &[Point]) -> Vec<Seg> {
    let Some(&last) = pts.last() else { return Vec::new() };
    if pts.len() <= 2 {
        return vec![Seg::Line(last)];
    }
    let half_way = |i: usize| mid(pts[i], pts[i + 1]);
    let mut segs = vec![Seg::Line(half_way(0))];
    segs.extend((1..pts.len() - 1).map(|i| Seg::Quad(pts[i], half_way(i))));
    segs.push(Seg::Line(last));
    segs
}

/// The arc of the ellipse around `c` with radii `r`, from angle `from` turning
/// by `sweep` radians (positive is clockwise on screen), as quadratic curves of
/// at most 45°. Each control point is where the tangents at its ends meet, which
/// keeps the curve within 0.32% of the radius outside the ellipse.
fn arc(c: Point, r: (f32, f32), from: f32, sweep: f32) -> impl Iterator<Item = Seg> {
    let n = (sweep.abs() / FRAC_PI_4).ceil().max(1.0);
    let step = sweep / n;
    let at = move |angle: f32, scale: f32| (c.0 + scale * r.0 * angle.cos(), c.1 + scale * r.1 * angle.sin());
    (1..=n as u32).map(move |i| {
        let to = from + step * i as f32;
        Seg::Quad(at(to - step / 2.0, 1.0 / (step / 2.0).cos()), at(to, 1.0))
    })
}

/// Appends a line to `p`, unless it would be shorter than a thousandth of a pixel.
fn line_to(path: &mut Path, p: Point) {
    let end = path.segs.last().map_or(path.start, |s| s.end());
    if !near(p, end) {
        path.segs.push(Seg::Line(p));
    }
}

fn near(a: Point, b: Point) -> bool {
    (a.0 - b.0).hypot(a.1 - b.1) <= 1e-3
}

/// Left, top, right, bottom of the box with corners `a` and `b`.
fn bounds([a, b]: [Point; 2]) -> (f32, f32, f32, f32) {
    (a.0.min(b.0), a.1.min(b.1), a.0.max(b.0), a.1.max(b.1))
}

fn polar(c: Point, r: f32, angle: f32) -> Point {
    (c.0 + r * angle.cos(), c.1 + r * angle.sin())
}

fn mid(a: Point, b: Point) -> Point {
    ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0)
}

fn unit(a: Point, b: Point) -> Point {
    let (x, y) = (b.0 - a.0, b.1 - a.1);
    let len = x.hypot(y);
    (x / len, y / len)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Renders `shapes` on a white 100×100 stage and checks `pixels` as (x, y, color);
    /// pixel (x, y) covers x..x+1, y..y+1.
    fn check(shapes: impl IntoIterator<Item = Shape>, pixels: &[(u32, u32, [u8; 3])]) {
        let mut doc = Document::new(100.0, 100.0, 24.0);
        for s in shapes {
            add(&mut doc.scenes[0].layers[0].frames[0], s);
        }
        let (pixmap, skipped) = render(&doc, 0, 1.0).unwrap();
        assert!(skipped.is_empty(), "{skipped:?}");
        for &(x, y, want) in pixels {
            let c = pixmap.pixel(x, y).unwrap().demultiply();
            assert_eq!([c.red(), c.green(), c.blue()], want, "pixel {x}, {y}");
        }
    }

    fn quad(a: Point, c: Point, b: Point, t: f32) -> Point {
        let u = 1.0 - t;
        (u * u * a.0 + 2.0 * u * t * c.0 + t * t * b.0, u * u * a.1 + 2.0 * u * t * c.1 + t * t * b.1)
    }

    /// The area inside the shape's paths, curves flattened: positive if they run clockwise on screen.
    fn area(s: &Shape) -> f32 {
        let mut twice = 0.0;
        for path in s.edges.iter().flat_map(|e| &e.paths) {
            let mut pts = vec![path.start];
            for &seg in &path.segs {
                let from = pts[pts.len() - 1];
                match seg {
                    Seg::Line(to) => pts.push(to),
                    Seg::Quad(c, to) => pts.extend((1..=16).map(|i| quad(from, c, to, i as f32 / 16.0))),
                }
            }
            twice += pts.iter().zip(pts.iter().cycle().skip(1)).map(|(a, b)| a.0 * b.1 - b.0 * a.1).sum::<f32>();
        }
        twice / 2.0
    }

    #[test]
    fn rectangles() {
        check(
            [
                rectangle([(60.0, 40.0), (10.0, 10.0)], 0.0, Some(ink(2.0)), Some(red())),
                rectangle([(10.0, 50.0), (60.0, 90.0)], 10.0, None, Some(red())),
                rectangle([(70.0, 50.0), (95.0, 90.0)], -10.0, None, Some(red())),
            ],
            &[
                (35, 25, RED),
                (35, 10, BLACK),
                (10, 25, BLACK),
                (10, 10, BLACK),
                (7, 25, WHITE),
                (5, 5, WHITE),
                // Rounded: the corner itself is cut off.
                (11, 51, WHITE),
                (14, 54, RED),
                (35, 70, RED),
                // Inward: a quarter circle around the corner is cut out.
                (71, 51, WHITE),
                (75, 55, WHITE),
                (82, 62, RED),
                (93, 70, RED),
            ],
        );
    }

    #[test]
    fn oval_line_and_polystar() {
        check(
            [oval([(10.0, 10.0), (90.0, 50.0)], Some(ink(2.0)), Some(red()))],
            &[
                (50, 30, RED),
                (50, 13, RED),
                (50, 10, BLACK),
                (10, 30, BLACK),
                (89, 30, BLACK),
                (12, 12, WHITE),
                (88, 48, WHITE),
            ],
        );
        let line = line((10.0, 10.0), (90.0, 10.0), ink(4.0));
        check([line], &[(50, 10, BLACK), (9, 10, BLACK), (50, 13, WHITE), (6, 10, WHITE)]);

        // Pointing up; probe between the first two points, at 0.7 of the radius.
        let (center, up) = ((50.0, 60.0), -FRAC_PI_2);
        let (x, y) = polar(center, 21.0, up + PI / 5.0);
        let between = (x as u32, y as u32);
        let pentagon = polystar(center, 30.0, 5, None, up, None, Some(red()));
        check([pentagon], &[(50, 60, RED), (50, 33, RED), (between.0, between.1, RED), (50, 28, WHITE)]);
        let star = polystar(center, 30.0, 5, Some(0.5), up, None, Some(red()));
        check([star], &[(50, 60, RED), (50, 33, RED), (between.0, between.1, WHITE)]);
    }

    #[test]
    fn closed_shapes_run_clockwise() {
        let assert_area = |s: Shape, want: f32| {
            let got = area(&s);
            assert!((got - want).abs() < want * 0.01, "area {got}, want {want}");
        };
        let fill = Some(red());
        assert_area(rectangle([(60.0, 40.0), (10.0, 10.0)], 0.0, None, fill.clone()), 50.0 * 30.0);
        assert_area(rectangle([(10.0, 50.0), (60.0, 90.0)], 10.0, None, fill.clone()), 2000.0 - (4.0 - PI) * 100.0);
        assert_area(rectangle([(70.0, 50.0), (95.0, 90.0)], -10.0, None, fill.clone()), 25.0 * 40.0 - PI * 100.0);
        assert_area(oval([(10.0, 10.0), (90.0, 50.0)], None, fill.clone()), PI * 40.0 * 20.0);
        assert_area(polystar((50.0, 50.0), 30.0, 5, None, 0.0, None, fill.clone()), 2.5 * 900.0 * (TAU / 5.0).sin());
        assert_area(polystar((50.0, 50.0), 30.0, 5, Some(0.5), 0.0, None, fill), 5.0 * 30.0 * 15.0 * (PI / 5.0).sin());
        let samples: Vec<Point> = (20..=80).map(|x| (x as f32, 50.0)).collect();
        assert_area(brush(&samples, 20.0, red()), 60.0 * 20.0 + PI * 100.0);
    }

    #[test]
    fn closed_outlines_end_exactly_where_they_start() {
        // A hair's gap is enough for twip rounding to leave the fill open in SWF.
        let fill = Some(red());
        for s in [
            // Radius past half the width: a pill, its last arc ending at the start.
            rectangle([(0.3, 0.7), (20.3, 40.9)], 50.0, None, fill.clone()),
            rectangle([(0.3, 0.7), (20.3, 40.9)], -50.0, None, fill.clone()),
            oval([(-40.0, -20.0), (40.0, 20.0)], None, fill.clone()),
            brush(&[(20.0, 50.0), (50.0, 60.0), (80.0, 50.0)], 20.0, red()),
        ] {
            let path = &s.edges[0].paths[0];
            assert_eq!(path.segs.last().map(|s| s.end()), Some(path.start));
        }
    }

    #[test]
    fn oval_strays_under_half_a_percent() {
        let s = oval([(-40.0, -20.0), (40.0, 20.0)], Some(ink(1.0)), None);
        let path = &s.edges[0].paths[0];
        assert_eq!(path.segs.len(), 8);
        let (mut from, mut worst) = (path.start, 0.0f32);
        for &seg in &path.segs {
            let Seg::Quad(c, to) = seg else { panic!("{seg:?}") };
            for i in 0..=32 {
                let (x, y) = quad(from, c, to, i as f32 / 32.0);
                worst = worst.max(((x / 40.0).hypot(y / 20.0) - 1.0).abs());
            }
            from = to;
        }
        assert!(worst < 0.005, "{worst}");
        assert!((from.0 - path.start.0).hypot(from.1 - path.start.1) < 1e-4, "closed");
    }

    #[test]
    fn simplify_drops_jitter_and_keeps_corners() {
        let jitter = [(0.0, 0.0), (1.0, 0.3), (2.0, -0.3), (3.0, 0.3), (4.0, 0.0)];
        assert_eq!(simplify(&jitter, 0.5), [(0.0, 0.0), (4.0, 0.0)]);
        let corner = [(0.0, 0.0), (5.0, 0.2), (10.0, 0.0), (10.2, 5.0), (10.0, 10.0)];
        assert_eq!(simplify(&corner, 0.5), [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]);
        // Back to the start: the span's ends coincide.
        let lap = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)];
        assert_eq!(simplify(&lap, 0.5), lap);
        assert_eq!(simplify(&[(0.0, 0.0), (1.0, 0.01), (2.0, 0.0)], 0.0).len(), 3);
        assert_eq!(simplify(&[(1.0, 1.0); 3], 0.5), [(1.0, 1.0)]);
        assert!(simplify(&[], 0.5).is_empty());
    }

    #[test]
    fn pencil_smooths_a_hand_drawn_v() {
        // One sample per pixel down to a tip at (50, 90) and back up, wobbling 0.3 px.
        let wobble = |x: i32| if x % 2 == 0 { 0.3 } else { -0.3 };
        let v = |x: i32| (x as f32, 90.0 - 2.0 * (x - 50).abs() as f32 + wobble(x));
        let samples: Vec<Point> = (10..=90).map(v).collect();
        let s = pencil(&samples, ink(4.0), 1.0);
        // Three samples are left: a line, a curve around the tip, a line.
        let segs = &s.edges[0].paths[0].segs;
        assert!(matches!(segs[..], [Seg::Line(_), Seg::Quad(..), Seg::Line(_)]), "{segs:?}");
        // The curve passes halfway between the tip and the arms' midpoints.
        check([s], &[(20, 30, BLACK), (50, 70, BLACK), (50, 60, WHITE), (50, 85, WHITE)]);

        check([pencil(&[(50.0, 50.0)], ink(6.0), 1.0)], &[(50, 50, BLACK), (50, 55, WHITE)]);
    }

    #[test]
    fn brush_covers_its_path_with_round_caps() {
        let wobble = |x: i32| if x % 2 == 0 { 0.4 } else { -0.4 };
        let straight: Vec<Point> = (20..=80).map(|x| (x as f32, 50.0 + wobble(x))).collect();
        check(
            [brush(&straight, 20.0, red())],
            &[
                (50, 50, RED),
                (50, 57, RED),
                (50, 61, WHITE),
                (50, 39, WHITE),
                // Round caps: past the ends on the center line, but not at the corners.
                (12, 50, RED),
                (11, 42, WHITE),
                (88, 50, RED),
                (91, 50, WHITE),
            ],
        );

        // Over the top of a circle of radius 40 around (50, 90).
        let on_circle = |deg: i32| polar((50.0, 90.0), 40.0, (deg as f32).to_radians());
        let over_the_top: Vec<Point> = (180..=360).map(on_circle).collect();
        check(
            [brush(&over_the_top, 10.0, red())],
            &[(50, 50, RED), (50, 53, RED), (21, 61, RED), (50, 43, WHITE), (50, 57, WHITE), (50, 90, WHITE)],
        );

        check([brush(&[(50.0, 50.0)], 10.0, red())], &[(50, 50, RED), (53, 50, RED), (50, 56, WHITE)]);
    }

    #[test]
    fn nothing_to_draw_adds_nothing() {
        let mut frame = Frame { index: 0, duration: 1, tween: Tween::None, elements: Vec::new(), extra: Extra::default() };
        add(&mut frame, brush(&[], 10.0, red()));
        add(&mut frame, pencil(&[], ink(1.0), 1.0));
        add(&mut frame, rectangle([(0.0, 0.0), (5.0, 5.0)], 0.0, None, None));
        // A click without a drag: every side is too short to draw.
        add(&mut frame, rectangle([(5.0, 5.0), (5.0, 5.0)], 0.0, Some(ink(1.0)), Some(red())));
        assert!(frame.elements.is_empty(), "{:?}", frame.elements);
    }
}
