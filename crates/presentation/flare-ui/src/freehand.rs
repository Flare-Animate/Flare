//! Turning a freehand drag into a clean path: fewer points (Ramer-Douglas-Peucker), joined by
//! smooth cubic curves (Catmull-Rom). Pure functions, so the tools stay thin.

use egui::{Pos2, Vec2};
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
        let before = points[i.saturating_sub(1)];
        let (a, b) = (points[i], points[i + 1]);
        let after = points[(i + 2).min(points.len() - 1)];
        let handle = |from: Pos2, to: Pos2| -> Vec2 { (to - from) / 6.0 };
        out.push(PathSegment::CubicTo(
            point(a + handle(before, b))?,
            point(b - handle(a, after))?,
            point(b)?,
        ));
    }
    Some(out)
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
