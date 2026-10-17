//! Pen Tool paths. Anchors carry cubic Bézier handles, as Animate's Pen Tool
//! draws them, but shapes store only quadratic curves, so each cubic segment
//! becomes as many quadratics as keep within a twip of it.

use crate::doc::*;
use crate::draw;

/// A point of a Pen path and the handles shaping the segments either side of
/// it, all three in document pixels. A corner's handles sit on its point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub point: Point,
    pub handle_in: Point,
    pub handle_out: Point,
}

impl Anchor {
    pub fn corner(point: Point) -> Anchor {
        Anchor { point, handle_in: point, handle_out: point }
    }

    /// A smooth point: the path leaves towards `handle_out` and arrives from
    /// its mirror image, as a Pen drag makes.
    pub fn smooth(point: Point, handle_out: Point) -> Anchor {
        let handle_in = (2.0 * point.0 - handle_out.0, 2.0 * point.1 - handle_out.1);
        Anchor { point, handle_in, handle_out }
    }
}

/// How far the quadratics may stray from the cubics: a twip, XFL's finest step.
const TOLERANCE: f32 = 0.05;

/// The shape a Pen path draws. A closed path runs on from the last anchor back
/// to the first and takes `fill` inside; an open one takes no fill. Fewer than
/// two anchors draw nothing.
pub fn pen_shape(anchors: &[Anchor], closed: bool, stroke: Option<Stroke>, fill: Option<Paint>) -> Shape {
    let fill = fill.filter(|_| closed);
    if anchors.len() < 2 {
        return draw::shape(Vec::new(), stroke, fill);
    }
    let ends = anchors[1..].iter().chain(closed.then_some(&anchors[0]));
    let segs = anchors.iter().zip(ends).flat_map(|(a, b)| segment(a, b)).collect();
    let path = Path { start: anchors[0].point, segs };
    let clockwise = twice_area(&path) >= 0.0;
    let mut shape = draw::shape(vec![path], stroke, fill);
    if !clockwise {
        // draw::shape fills the right side, which is the inside only of a clockwise path.
        shape.edges.iter_mut().for_each(|e| std::mem::swap(&mut e.fill0, &mut e.fill1));
    }
    // ponytail: a path crossing itself keeps one fill side for all its loops, so the loops
    // turning the other way get it wrong for Animate (our renderer fills even-odd and doesn't
    // care). Fix: split the edges where they cross, as Animate stores shapes.
    shape
}

/// The cubic from `a` to `b` as quadratics, or a line between two corners.
fn segment(a: &Anchor, b: &Anchor) -> Vec<Seg> {
    let p = [a.point, a.handle_out, b.handle_in, b.point];
    if p[1] == p[0] && p[2] == p[3] {
        return vec![Seg::Line(p[3])];
    }
    // A cubic's midpoint quadratic, with the control below, strays from it by at most
    // √3/36·|p3 - 3·p2 + 3·p1 - p0|; cutting the cubic into n equal pieces divides that by n³.
    let (dx, dy) = mix([-1.0, 3.0, -3.0, 1.0], p);
    let n = (dx.hypot(dy) * 3f32.sqrt() / 36.0 / TOLERANCE).cbrt().ceil().max(1.0);
    // The point at t, and the tangent there (d/dt).
    let at = |t: f32| {
        let u = 1.0 - t;
        let point = mix([u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t], p);
        let tangent = mix([-3.0 * u * u, 3.0 * u * u - 6.0 * u * t, 6.0 * u * t - 3.0 * t * t, 3.0 * t * t], p);
        (point, tangent)
    };
    (1..=n as u32)
        .map(|i| {
            let (t0, t1) = ((i - 1) as f32 / n, i as f32 / n);
            let ((q0, d0), (q3, d3)) = (at(t0), at(t1));
            // The piece's own cubic has controls q0 + d0·h/3 and q3 - d3·h/3; the midpoint
            // control (3·(q1 + q2) - q0 - q3) / 4 works out as below.
            let k = (t1 - t0) / 4.0;
            Seg::Quad(mix([0.5, 0.5, k, -k], [q0, q3, d0, d3]), q3)
        })
        .collect()
}

/// Σ wᵢ·pᵢ. Weights of exactly 0 and 1 give back the points exactly.
fn mix(w: [f32; 4], p: [Point; 4]) -> Point {
    (0..4).fold((0.0, 0.0), |(x, y), i| (x + w[i] * p[i].0, y + w[i] * p[i].1))
}

/// Twice the area inside the closed `path`, positive if it runs clockwise on screen.
fn twice_area(path: &Path) -> f32 {
    let cross = |a: Point, b: Point| a.0 * b.1 - a.1 * b.0;
    let mut from = path.start;
    path.segs
        .iter()
        .map(|&seg| {
            let twice = match seg {
                Seg::Line(to) => cross(from, to),
                // ∮ x dy - y dx along a quadratic.
                Seg::Quad(c, to) => (2.0 * cross(from, c) + 2.0 * cross(c, to) + cross(from, to)) / 3.0,
            };
            from = seg.end();
            twice
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ink() -> Stroke {
        Stroke { width: 1.0, paint: Paint::Solid([0.0, 0.0, 0.0, 1.0]), cap: Cap::Round, join: Join::Round }
    }

    fn red() -> Paint {
        Paint::Solid([1.0, 0.0, 0.0, 1.0])
    }

    fn sides(s: &Shape) -> (u32, u32) {
        (s.edges[0].fill0, s.edges[0].fill1)
    }

    #[test]
    fn cubics_become_quadratics_within_a_twip() {
        // An S, and a cubic that loops over itself.
        let curves = [
            [Anchor::smooth((0.0, 0.0), (100.0, -150.0)), Anchor::smooth((300.0, 0.0), (400.0, -150.0))],
            [Anchor::smooth((0.0, 0.0), (300.0, -200.0)), Anchor::smooth((200.0, 0.0), (500.0, 200.0))],
        ];
        for [a, b] in curves {
            let s = pen_shape(&[a, b], false, Some(ink()), None);
            let path = &s.edges[0].paths[0];
            let n = path.segs.len();
            assert!((2..=12).contains(&n), "{n} pieces");
            // Piece i follows the cubic from t = i/n to (i + 1)/n.
            let cubic = [a.point, a.handle_out, b.handle_in, b.point];
            let (mut from, mut worst) = (path.start, 0.0f32);
            for (i, &seg) in path.segs.iter().enumerate() {
                let Seg::Quad(c, to) = seg else { panic!("{seg:?}") };
                for k in 0..=32 {
                    let (s, t) = (k as f32 / 32.0, (i as f32 + k as f32 / 32.0) / n as f32);
                    let (u, v) = (1.0 - s, 1.0 - t);
                    let q = mix([u * u, 2.0 * u * s, s * s, 0.0], [from, c, to, to]);
                    let p = mix([v * v * v, 3.0 * v * v * t, 3.0 * v * t * t, t * t * t], cubic);
                    worst = worst.max((q.0 - p.0).hypot(q.1 - p.1));
                }
                from = to;
            }
            assert!(worst <= TOLERANCE, "strays {worst}");
            assert_eq!((path.start, from), (a.point, b.point), "the ends are exact");
        }
    }

    #[test]
    fn corners_join_with_lines_and_open_paths_take_no_fill() {
        let s = pen_shape(&[Anchor::corner((0.0, 0.0)), Anchor::corner((10.0, 5.0))], false, Some(ink()), Some(red()));
        assert_eq!(s.edges[0].paths[0].segs, [Seg::Line((10.0, 5.0))]);
        assert!(s.fills.is_empty());
        assert_eq!((sides(&s), s.edges[0].stroke), ((0, 0), 1));
        assert!(pen_shape(&[Anchor::corner((0.0, 0.0))], true, Some(ink()), Some(red())).edges.is_empty());
    }

    #[test]
    fn closed_paths_fill_their_inside() {
        // Clockwise on screen: along the top, down to the tip, back up.
        let mut triangle = [(10.0, 10.0), (90.0, 10.0), (50.0, 80.0)].map(Anchor::corner);
        let s = pen_shape(&triangle, true, Some(ink()), Some(red()));
        assert_eq!(sides(&s), (0, 1));
        assert_eq!(s.edges[0].paths[0].segs.last(), Some(&Seg::Line((10.0, 10.0))), "back to the first anchor");
        triangle.reverse();
        assert_eq!(sides(&pen_shape(&triangle, true, None, Some(red()))), (1, 0));

        // Two smooth points, left and right, leaving up and down (clockwise) or down and up.
        // The anchors alone enclose nothing, so the side comes from the curves.
        let lens = |up: f32| [Anchor::smooth((0.0, 50.0), (0.0, 50.0 - up)), Anchor::smooth((100.0, 50.0), (100.0, 50.0 + up))];
        assert_eq!(sides(&pen_shape(&lens(50.0), true, None, Some(red()))), (0, 1));
        assert_eq!(sides(&pen_shape(&lens(-50.0), true, None, Some(red()))), (1, 0));
    }
}
