//! Properties of the tool geometry over many generated inputs: whatever a hand does, the result
//! is a valid path or a refusal, never a panic and never a corrupt shape.
#![allow(clippy::expect_used)]

use emath::{Pos2, Rect, Vec2};
use flare_core::{Rgba8, ShapeId};
use flare_document::{PathSegment, Shape};
use flare_input::{InputSample, PointerKind, PressureCurve};
use flare_tools::ops::{Align, align_deltas, distribute_deltas, polygon_contains};
use flare_tools::{brush, freehand};
use proptest::prelude::*;

fn coordinate() -> impl Strategy<Value = f32> {
    -5000.0_f32..5000.0
}

fn point() -> impl Strategy<Value = Pos2> {
    (coordinate(), coordinate()).prop_map(|(x, y)| Pos2::new(x, y))
}

fn rect() -> impl Strategy<Value = Rect> {
    (point(), 1.0_f32..800.0, 1.0_f32..800.0)
        .prop_map(|(min, w, h)| Rect::from_min_size(min, Vec2::new(w, h)))
}

proptest! {
    #[test]
    fn simplifying_keeps_a_subsequence_that_starts_and_ends_where_the_drag_did(
        points in prop::collection::vec(point(), 2..200), tolerance in 0.0_f32..50.0,
    ) {
        let kept = freehand::simplify(&points, tolerance);
        prop_assert!(kept.len() >= 2 && kept.len() <= points.len());
        prop_assert_eq!(kept.first(), points.first());
        prop_assert_eq!(kept.last(), points.last());
        let mut rest = points.iter();
        for k in &kept {
            prop_assert!(rest.any(|p| p == k), "a kept point is one of the originals, in order");
        }
    }

    #[test]
    fn a_smoothed_line_is_a_valid_shape_through_the_ends(
        points in prop::collection::vec(point(), 2..80),
    ) {
        let path = freehand::smooth_path(&points).expect("two or more finite points");
        let shape = Shape::new(ShapeId::new(), path.clone(), Rgba8::TRANSPARENT);
        prop_assert!(shape.is_ok());
        let PathSegment::MoveTo(first) = path[0] else { panic!("starts with a move") };
        prop_assert!((first.as_vec().x - f64::from(points[0].x)).abs() < 1e-3);
        let end = match path.last().expect("segments") {
            PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => p.as_vec(),
            other => panic!("unexpected end {other:?}"),
        };
        let last = points.last().expect("points");
        prop_assert!((end.x - f64::from(last.x)).abs() < 1e-3 && (end.y - f64::from(last.y)).abs() < 1e-3);
    }

    #[test]
    fn a_brush_outline_is_a_closed_valid_shape_or_nothing_for_any_pressure(
        raw in prop::collection::vec((coordinate(), coordinate(), 0.0_f32..=1.0), 0..120),
        width in 0.1_f64..300.0, min in 0.0_f32..1.0, tolerance in 0.0_f64..20.0,
    ) {
        let samples: Vec<InputSample> = raw
            .iter()
            .filter_map(|(x, y, p)| InputSample::new(f64::from(*x), f64::from(*y), *p, PointerKind::Pen))
            .collect();
        match brush::outline(&samples, width, PressureCurve::new(min, 1.0), tolerance) {
            None => {}
            Some(path) => {
                prop_assert!(matches!(path.last(), Some(PathSegment::Close)));
                prop_assert!(Shape::new(ShapeId::new(), path, Rgba8::opaque(0, 0, 0)).is_ok());
            }
        }
    }

    #[test]
    fn aligning_puts_every_shape_on_the_same_line_whatever_they_were(
        rects in prop::collection::vec(rect(), 1..12), target in rect(),
    ) {
        for how in [Align::Left, Align::CenterH, Align::Right, Align::Top, Align::CenterV, Align::Bottom] {
            let moved: Vec<Rect> = rects
                .iter()
                .zip(align_deltas(&rects, how, target))
                .map(|(r, d)| r.translate(d))
                .collect();
            for r in &moved {
                let (got, want) = match how {
                    Align::Left => (r.left(), target.left()),
                    Align::CenterH => (r.center().x, target.center().x),
                    Align::Right => (r.right(), target.right()),
                    Align::Top => (r.top(), target.top()),
                    Align::CenterV => (r.center().y, target.center().y),
                    Align::Bottom => (r.bottom(), target.bottom()),
                };
                prop_assert!((got - want).abs() < 0.01, "{how:?}: {got} vs {want}");
            }
        }
    }

    #[test]
    fn distributing_leaves_the_outer_two_and_equal_gaps_between_the_rest(
        rects in prop::collection::vec(rect(), 0..10), horizontal in any::<bool>(),
    ) {
        let deltas = distribute_deltas(&rects, horizontal);
        prop_assert_eq!(deltas.len(), rects.len());
        if rects.len() < 3 {
            prop_assert!(deltas.iter().all(|d| *d == Vec2::ZERO));
            return Ok(());
        }
        let axis = |r: &Rect| if horizontal { (r.left(), r.right()) } else { (r.top(), r.bottom()) };
        // Nothing moves across the other axis.
        for d in &deltas {
            let off_axis = if horizontal { d.y } else { d.x };
            prop_assert_eq!(off_axis, 0.0);
        }
        let first = rects.iter().map(|r| axis(r).0).fold(f32::MAX, f32::min);
        let last = rects.iter().map(|r| axis(r).1).fold(f32::MIN, f32::max);
        let total: f32 = rects.iter().map(|r| axis(r).1 - axis(r).0).sum();
        let slack = last - first - total;
        let moved: Vec<(f32, f32)> =
            rects.iter().zip(&deltas).map(|(r, d)| axis(&r.translate(*d))).collect();
        if slack > 1.0 {
            // Room to spare: equal gaps between neighbours.
            let mut sorted = moved.clone();
            sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
            let gaps: Vec<f32> = sorted.windows(2).map(|w| w[1].0 - w[0].1).collect();
            for g in &gaps {
                prop_assert!((g - gaps[0]).abs() < 0.05, "gaps {gaps:?}");
            }
        } else if slack < -1.0 {
            // Too big for that: centers are evenly spaced, in the order they were in.
            let center = |a: (f32, f32)| (a.0 + a.1) / 2.0;
            let mut order: Vec<usize> = (0..rects.len()).collect();
            order.sort_by(|a, b| center(axis(&rects[*a])).total_cmp(&center(axis(&rects[*b]))));
            let centers: Vec<f32> = order.iter().map(|i| center(moved[*i])).collect();
            let step = centers[1] - centers[0];
            for w in centers.windows(2) {
                prop_assert!((w[1] - w[0] - step).abs() < 0.05, "centers {centers:?}");
            }
            prop_assert!(step >= -0.05, "the order is kept");
        }
    }

    #[test]
    fn a_point_is_in_an_axis_aligned_box_polygon_exactly_when_it_is_in_the_box(
        r in rect(), p in point(),
    ) {
        let corners = [r.left_top(), r.right_top(), r.right_bottom(), r.left_bottom()];
        let inside = p.x > r.left() && p.x < r.right() && p.y > r.top() && p.y < r.bottom();
        let outside = p.x < r.left() || p.x > r.right() || p.y < r.top() || p.y > r.bottom();
        let answer = polygon_contains(&corners, p);
        if inside { prop_assert!(answer); }
        if outside { prop_assert!(!answer); }
    }
}
