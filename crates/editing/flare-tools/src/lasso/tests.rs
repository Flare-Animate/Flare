#![allow(clippy::expect_used)]

use super::*;
use crate::common::SpanRef;
use emath::Pos2;
use flare_core::LayerId;

fn p(x: f32, y: f32) -> Pos2 {
    Pos2::new(x, y)
}

fn span(start: u32) -> SpanRef {
    SpanRef {
        layer: LayerId::new(),
        start,
        element: 0,
    }
}

#[test]
fn a_dragged_loop_takes_points_a_few_pixels_apart_and_no_more_than_its_limit() {
    let mut points = Vec::new();
    assert!(freehand::extend(&mut points, p(0.0, 0.0), 1.0));
    assert!(
        !freehand::extend(&mut points, p(0.5, 0.0), 1.0),
        "too close to see"
    );
    assert!(freehand::extend(&mut points, p(2.5, 0.0), 1.0));
    // Zoomed in, the same screen distance is a smaller stage distance.
    assert!(freehand::extend(&mut points, p(2.75, 0.0), 10.0));
    let mut full = vec![p(0.0, 0.0); freehand::MAX_POINTS];
    assert!(!freehand::extend(&mut full, p(100.0, 100.0), 1.0));
    assert_eq!(full.len(), freehand::MAX_POINTS);
}

#[test]
fn a_loop_replaces_the_selection_or_adds_to_it_without_repeats() {
    let (a, b, c) = (span(0), span(1), span(2));
    assert_eq!(
        freehand::selection_after(&[a], vec![b, c], false),
        vec![b, c]
    );
    assert_eq!(
        freehand::selection_after(&[a, b], vec![b, c], true),
        vec![a, b, c]
    );
    assert!(freehand::selection_after(&[a], Vec::new(), false).is_empty());
}

#[test]
fn clicks_add_corners_close_on_the_first_and_ignore_a_repeat() {
    let mut corners = Vec::new();
    for (at, want) in [
        (p(0.0, 0.0), polygon::Click::Corner),
        (p(100.0, 0.0), polygon::Click::Corner),
        (p(100.0, 0.0), polygon::Click::Ignored),
        (p(0.0, 0.0), polygon::Click::Corner),
    ] {
        // Only two distinct corners are in when the first is clicked again: it is not a loop yet.
        assert_eq!(polygon::click(&mut corners, at, 1.0), want);
    }
    corners.push(p(50.0, 80.0));
    assert_eq!(
        polygon::click(&mut corners, p(3.0, 4.0), 1.0),
        polygon::Click::Close
    );
    assert_eq!(corners.len(), 4, "closing adds nothing");
    // The close radius is in screen pixels: zoomed in, it is a smaller stage distance.
    assert_eq!(
        polygon::click(&mut corners, p(3.0, 4.0), 10.0),
        polygon::Click::Corner
    );
}

#[test]
fn a_loop_has_a_limit_and_fewer_than_three_corners_enclose_nothing() {
    let mut corners: Vec<Pos2> = (0..polygon::MAX_CORNERS)
        .map(|i| p(i as f32 * 20.0, 0.0))
        .collect();
    assert_eq!(
        polygon::click(&mut corners, p(1.0e6, 1.0e6), 1.0),
        polygon::Click::Ignored
    );
    let mut two = vec![p(0.0, 0.0), p(10.0, 0.0)];
    assert!(!polygon::can_close(&mut two));
    assert!(two.is_empty(), "dropped, so the next loop starts clean");
    let mut three = vec![p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0)];
    assert!(polygon::can_close(&mut three));
    assert_eq!(three.len(), 3);
    assert!(!polygon::can_close(&mut Vec::new()));
}

#[test]
fn a_point_is_inside_a_loop_by_the_even_odd_rule() {
    let square = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    assert!(polygon_contains(&square, p(5.0, 5.0)));
    assert!(!polygon_contains(&square, p(15.0, 5.0)));
    assert!(!polygon_contains(&[], p(0.0, 0.0)));
}

#[test]
fn a_loop_takes_a_stroke_it_touches_though_the_center_of_its_bounds_is_outside() {
    use crate::test_support::*;
    use flare_core::{DocumentPoint, ShapeId};
    use flare_document::{Content, FrameSpan, PathSegment, Shape, Stroke};
    use std::num::NonZeroU32;
    let pt = |x: f64, y: f64| DocumentPoint::new(x, y).expect("point");
    // A pencil "L": its bounds center (50, 50) is empty space.
    let path = vec![
        PathSegment::MoveTo(pt(0.0, 0.0)),
        PathSegment::LineTo(pt(0.0, 100.0)),
        PathSegment::LineTo(pt(100.0, 100.0)),
    ];
    let shape = Shape::new(ShapeId::new(), path, flare_core::Rgba8::TRANSPARENT)
        .expect("shape")
        .with_stroke(Stroke::new(2.0, RED).expect("stroke"));
    let mut f = fixture();
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    add(&mut f, span);
    let around = |x0: f32, y0: f32, x1: f32, y1: f32| [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
    let hit = spans_in_polygon(
        f.session.document(),
        f.scene,
        0,
        &around(-5.0, -5.0, 10.0, 30.0),
    );
    assert_eq!(hit, vec![at(&f, 0)]);
    let miss = spans_in_polygon(
        f.session.document(),
        f.scene,
        0,
        &around(30.0, 30.0, 70.0, 70.0),
    );
    assert!(miss.is_empty(), "a loop in the empty middle takes nothing");
    let bounds = bounds(&f, 0);
    assert_eq!(
        (bounds.min.x, bounds.max.y),
        (-1.0, 101.0),
        "includes half the width"
    );
}
