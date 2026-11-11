//! Turning a freehand drag into a clean path: fewer points (Ramer-Douglas-Peucker), joined by
//! smooth cubic curves (Catmull-Rom). Pure functions, so the tools stay thin.

use emath::{Pos2, Vec2};
use flare_core::DocumentPoint;
use flare_document::PathSegment;

/// The points that keep the polyline within `tolerance` of the original. Iterative, so a long
/// drag cannot overflow the stack.
pub fn simplify(points: &[Pos2], tolerance: f32) -> Vec<Pos2> {
    if points.len() < 3 {
        return points.to_vec();
    }
    let mut keep = vec![false; points.len()];
    keep[0] = true;
    keep[points.len() - 1] = true;
    let mut stack = vec![(0, points.len() - 1)];
    while let Some((from, to)) = stack.pop() {
        let (a, b) = (points[from], points[to]);
        let farthest = (from + 1..to)
            .map(|i| (i, distance_to_segment(points[i], a, b)))
            .max_by(|x, y| x.1.total_cmp(&y.1));
        if let Some((index, away)) = farthest
            && away > tolerance
        {
            keep[index] = true;
            stack.push((from, index));
            stack.push((index, to));
        }
    }
    points
        .iter()
        .zip(keep)
        .filter_map(|(p, kept)| kept.then_some(*p))
        .collect()
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let length_sq = ab.length_sq();
    if length_sq <= f32::EPSILON {
        return (p - a).length();
    }
    let t = ((p - a).dot(ab) / length_sq).clamp(0.0, 1.0);
    (p - (a + ab * t)).length()
}

/// An open path through `points`: a line for two points, otherwise a Catmull-Rom spline as
/// cubic segments. `None` for fewer than two points or a non-finite coordinate.
pub fn smooth_path(points: &[Pos2]) -> Option<Vec<PathSegment>> {
    let point = |p: Pos2| DocumentPoint::new(f64::from(p.x), f64::from(p.y)).ok();
    let first = *points.first()?;
    if points.len() < 2 {
        return None;
    }
    let mut out = vec![PathSegment::MoveTo(point(first)?)];
    if points.len() == 2 {
        out.push(PathSegment::LineTo(point(points[1])?));
        return Some(out);
    }
    for i in 0..points.len() - 1 {
        let (c1, c2) = handles(points, i);
        out.push(PathSegment::CubicTo(
            point(c1)?,
            point(c2)?,
            point(points[i + 1])?,
        ));
    }
    Some(out)
}

/// The two control points of the cubic from `points[i]` to `points[i + 1]` in [`smooth_path`].
fn handles(points: &[Pos2], i: usize) -> (Pos2, Pos2) {
    let before = points[i.saturating_sub(1)];
    let (a, b) = (points[i], points[i + 1]);
    let after = points[(i + 2).min(points.len() - 1)];
    let handle = |from: Pos2, to: Pos2| -> Vec2 { (to - from) / 6.0 };
    (a + handle(before, b), b - handle(a, after))
}

/// Points along each cubic of a flattened line.
const FLATTEN_STEPS: usize = 6;

/// The line the Pencil commits for a drag through `points` (simplified to within `tolerance`,
/// then smoothed as [`smooth_path`] does), as points close enough together to draw as straight
/// pieces. A live preview draws these, so it shows the curve that will be kept.
pub fn smoothed_points(points: &[Pos2], tolerance: f32) -> Vec<Pos2> {
    let kept = simplify(points, tolerance);
    if kept.len() < 3 {
        return kept;
    }
    let mut out = Vec::with_capacity((kept.len() - 1) * FLATTEN_STEPS + 1);
    out.push(kept[0]);
    for i in 0..kept.len() - 1 {
        let (a, (c1, c2), b) = (kept[i], handles(&kept, i), kept[i + 1]);
        for s in 1..=FLATTEN_STEPS {
            let t = s as f32 / FLATTEN_STEPS as f32;
            let u = 1.0 - t;
            let at = a.to_vec2() * (u * u * u)
                + c1.to_vec2() * (3.0 * t * u * u)
                + c2.to_vec2() * (3.0 * t * t * u)
                + b.to_vec2() * (t * t * t);
            out.push(at.to_pos2());
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn p(x: f32, y: f32) -> Pos2 {
        Pos2::new(x, y)
    }

    #[test]
    fn collinear_points_collapse_to_the_ends_but_corners_stay() {
        let line: Vec<Pos2> = (0..50).map(|i| p(i as f32, 2.0 * i as f32)).collect();
        assert_eq!(simplify(&line, 0.5), vec![line[0], line[49]]);
        let corner = [
            p(0.0, 0.0),
            p(10.0, 0.0),
            p(20.0, 0.0),
            p(20.0, 10.0),
            p(20.0, 20.0),
        ];
        assert_eq!(
            simplify(&corner, 0.5),
            vec![p(0.0, 0.0), p(20.0, 0.0), p(20.0, 20.0)]
        );
    }

    #[test]
    fn tiny_inputs_pass_through_and_a_long_drag_does_not_recurse() {
        assert_eq!(simplify(&[p(1.0, 1.0)], 1.0), vec![p(1.0, 1.0)]);
        let wave: Vec<Pos2> = (0..100_000)
            .map(|i| p(i as f32, (i as f32 * 0.01).sin() * 50.0))
            .collect();
        let simple = simplify(&wave, 1.0);
        assert!(simple.len() < wave.len() / 10);
        assert_eq!(simple[0], wave[0]);
    }

    #[test]
    fn the_smoothed_points_follow_the_committed_curve_through_the_kept_points() {
        let drag = [p(0.0, 0.0), p(10.0, 8.0), p(20.0, 0.0), p(30.0, 8.0)];
        let flat = smoothed_points(&drag, 0.1);
        assert_eq!(flat.len(), 3 * FLATTEN_STEPS + 1);
        for (k, kept) in drag.iter().enumerate() {
            assert!(
                (flat[k * FLATTEN_STEPS] - *kept).length() < 1e-4,
                "passes through {k}"
            );
        }
        // Between two kept points it lies on the cubic the commit keeps, not on a straight line.
        let path = smooth_path(&drag).expect("path");
        let mut from = drag[0];
        for (segment, piece) in path[1..].iter().enumerate() {
            let PathSegment::CubicTo(c1, c2, to) = piece else {
                panic!("a cubic")
            };
            let (c1, c2, to) = (c1.as_vec(), c2.as_vec(), to.as_vec());
            for s in 1..=FLATTEN_STEPS {
                let t = s as f64 / FLATTEN_STEPS as f64;
                let u = 1.0 - t;
                let x = u * u * u * f64::from(from.x)
                    + 3.0 * t * u * u * c1.x
                    + 3.0 * t * t * u * c2.x
                    + t * t * t * to.x;
                let y = u * u * u * f64::from(from.y)
                    + 3.0 * t * u * u * c1.y
                    + 3.0 * t * t * u * c2.y
                    + t * t * t * to.y;
                let got = flat[segment * FLATTEN_STEPS + s];
                assert!((f64::from(got.x) - x).abs() < 1e-3 && (f64::from(got.y) - y).abs() < 1e-3);
            }
            from = Pos2::new(to.x as f32, to.y as f32);
        }
        let quarter = flat[FLATTEN_STEPS / 2];
        assert!(
            (quarter - Pos2::new(5.0, 4.0)).length() > 0.3,
            "bent: {quarter:?}"
        );
        assert_eq!(smoothed_points(&[p(0.0, 0.0), p(5.0, 5.0)], 1.0).len(), 2);
    }

    #[test]
    fn a_smooth_path_passes_through_every_point() {
        assert!(smooth_path(&[p(0.0, 0.0)]).is_none());
        assert_eq!(
            smooth_path(&[p(0.0, 0.0), p(5.0, 5.0)])
                .expect("line")
                .len(),
            2
        );
        let path =
            smooth_path(&[p(0.0, 0.0), p(10.0, 5.0), p(20.0, 0.0), p(30.0, 5.0)]).expect("path");
        // A move, then one cubic per gap, each ending on the next point.
        assert_eq!(path.len(), 4);
        let PathSegment::CubicTo(_, _, end) = path[3] else {
            panic!("cubic expected");
        };
        assert!((end.as_vec() - glamx::DVec2::new(30.0, 5.0)).length() < 1e-6);
        assert!(smooth_path(&[p(0.0, 0.0), p(f32::NAN, 1.0), p(2.0, 2.0)]).is_none());
    }
}
