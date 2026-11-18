#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{FrameSpan, PathSegment, Shape, Stroke};
use std::num::NonZeroU32;

fn p(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("point")
}

/// A line drawn from (0, 100) to (200, 100) with a point every `step`, as the Pencil leaves one.
fn drawn_line(step: f64) -> Shape {
    let count = (200.0 / step) as usize;
    let mut path = vec![PathSegment::MoveTo(p(0.0, 100.0))];
    for i in 1..=count {
        path.push(PathSegment::LineTo(p(i as f64 * step, 100.0)));
    }
    let stroke = Stroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke");
    Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
        .expect("shape")
        .with_stroke(stroke)
}

fn points(shape: &Shape) -> Vec<DVec2> {
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            PathSegment::MoveTo(p) | PathSegment::LineTo(p) | PathSegment::CubicTo(_, _, p) => {
                Some(p.as_vec())
            }
            PathSegment::Close => None,
        })
        .collect()
}

fn pull_of(from: (f64, f64), to: (f64, f64), radius: f64) -> Pull {
    Pull::new(DVec2::new(from.0, from.1), DVec2::new(to.0, to.1), radius).expect("pull")
}

#[test]
fn the_falloff_is_whole_at_the_start_and_nothing_at_the_edge() {
    assert_eq!(falloff(0.0, 10.0), 1.0);
    assert_eq!(falloff(10.0, 10.0), 0.0);
    assert_eq!(falloff(25.0, 10.0), 0.0);
    let half = falloff(5.0, 10.0);
    assert!(half > 0.4 && half < 0.7, "{half}");
    assert!(falloff(3.0, 10.0) > falloff(6.0, 10.0));
    assert_eq!(falloff(f64::NAN, 10.0), 0.0);
    assert_eq!(falloff(1.0, 0.0), 0.0);
}

#[test]
fn a_pull_bends_the_line_near_it_and_leaves_the_rest() {
    let line = drawn_line(5.0);
    let pulled = pull_shape(&line, &pull_of((100.0, 100.0), (100.0, 60.0), 40.0)).expect("bent");
    let moved = points(&pulled);
    let at = |x: f64| {
        moved
            .iter()
            .find(|v| (v.x - x).abs() < 1e-9)
            .copied()
            .expect("a point there")
    };
    assert!(
        (at(100.0).y - 60.0).abs() < 1e-9,
        "the middle follows the pull whole"
    );
    let half_way = at(120.0).y;
    assert!(
        half_way > 60.0 && half_way < 100.0,
        "less further out: {half_way}"
    );
    for x in [0.0, 50.0, 140.0, 200.0] {
        assert_eq!(at(x).y, 100.0, "{x} is outside the circle");
    }
    assert!(pulled.stroke().is_some(), "the paint is kept");
    assert_eq!(pulled.id(), line.id());
}

#[test]
fn a_line_with_two_points_gets_points_to_bend_with() {
    let stroke = Stroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke");
    let straight = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(p(0.0, 0.0)),
            PathSegment::LineTo(p(200.0, 0.0)),
        ],
        Rgba8::TRANSPARENT,
    )
    .expect("shape")
    .with_stroke(stroke);
    let bent = pull_shape(&straight, &pull_of((100.0, 0.0), (100.0, 30.0), 40.0)).expect("bent");
    let moved = points(&bent);
    assert!(moved.len() > 6, "points were added: {}", moved.len());
    assert_eq!(moved.first(), Some(&DVec2::new(0.0, 0.0)), "the ends stay");
    assert_eq!(moved.last(), Some(&DVec2::new(200.0, 0.0)));
    let top = moved.iter().map(|v| v.y).fold(f64::MIN, f64::max);
    assert!((top - 30.0).abs() < 1.0, "the middle follows: {top}");
}

#[test]
fn a_curve_and_a_closed_outline_bend_too() {
    let curve = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(p(0.0, 0.0)),
            PathSegment::CubicTo(p(60.0, 0.0), p(140.0, 0.0), p(200.0, 0.0)),
        ],
        Rgba8::TRANSPARENT,
    )
    .expect("curve")
    .with_stroke(Stroke::new(1.0, Rgba8::opaque(0, 0, 0)).expect("stroke"));
    let bent = pull_shape(&curve, &pull_of((100.0, 0.0), (100.0, -20.0), 30.0)).expect("bent");
    assert!(bent.path().len() > curve.path().len(), "split into pieces");
    let square = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 100.0, RED).expect("rect");
    // The closing edge, from (0, 100) back to (0, 0), is pulled at its middle.
    let dented = pull_shape(&square, &pull_of((0.0, 50.0), (20.0, 50.0), 20.0)).expect("dented");
    assert!(matches!(dented.path().last(), Some(PathSegment::Close)));
    let right = points(&dented)
        .iter()
        .map(|v| v.x)
        .filter(|x| *x < 50.0)
        .fold(0.0, f64::max);
    assert!(
        (right - 20.0).abs() < 1.0,
        "the left side pushed in: {right}"
    );
    assert_eq!(dented.fill(), RED);
}

#[test]
fn a_pull_reaching_nothing_changes_nothing() {
    let line = drawn_line(5.0);
    let far = pull_of((100.0, 300.0), (100.0, 250.0), 40.0);
    assert!(!reaches(&line, &far));
    assert!(pull_shape(&line, &far).is_none());
    assert!(reaches(&line, &pull_of((100.0, 130.0), (0.0, 0.0), 40.0)));
}

#[test]
fn a_pull_is_kept_in_range_and_refuses_what_is_not_a_number() {
    let tiny = Pull::new(DVec2::ZERO, DVec2::ONE, 0.0).expect("pull");
    assert_eq!(tiny.radius, MIN_MAGNET_RADIUS);
    let huge = Pull::new(DVec2::ZERO, DVec2::ONE, 1e9).expect("pull");
    assert_eq!(huge.radius, MAX_MAGNET_RADIUS);
    let wild = Pull::new(DVec2::ZERO, DVec2::ONE, f64::NAN).expect("pull");
    assert_eq!(wild.radius, DEFAULT_MAGNET_RADIUS);
    assert_eq!(
        Pull::new(DVec2::new(f64::NAN, 0.0), DVec2::ONE, 10.0),
        Err(MagnetError::NotFinite)
    );
}

#[test]
fn a_dense_line_is_bent_without_passing_the_segment_limit() {
    // A point every unit along a long line, near the limit already: no points are added, but
    // the pull still bends the ones there are.
    let count = flare_document::MAX_SEGMENTS - 2;
    let mut path = vec![PathSegment::MoveTo(p(0.0, 0.0))];
    for i in 1..=count {
        path.push(PathSegment::LineTo(p(i as f64 * 0.01, 0.0)));
    }
    let dense = Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT).expect("shape");
    let bent = pull_shape(&dense, &pull_of((50.0, 0.0), (50.0, 10.0), 30.0)).expect("bent");
    assert!(bent.path().len() <= flare_document::MAX_SEGMENTS);
}

#[test]
fn a_line_just_under_the_limit_is_not_split_past_it() {
    // Far from the pull the line has a point every unit, filling all but ten of the segments a
    // path may have; then one long piece runs through the pull, which would want 27 pieces.
    let max = flare_document::MAX_SEGMENTS;
    let mut path = vec![PathSegment::MoveTo(p(-2000.0, 0.0))];
    while path.len() < max - 10 {
        path.push(PathSegment::LineTo(p(
            -2000.0 + path.len() as f64 * 1e-3,
            0.0,
        )));
    }
    path.push(PathSegment::LineTo(p(200.0, 0.0)));
    let pull = pull_of((100.0, 0.0), (100.0, 10.0), 30.0);
    assert!(
        path::densified(&path, &pull).is_none(),
        "the pieces do not fit"
    );
    let shape = Shape::new(ShapeId::new(), path.clone(), Rgba8::TRANSPARENT).expect("shape");
    let bent = pull_shape(&shape, &pull);
    // Nothing is added, and the points there are stay out of reach, so nothing moves.
    assert!(bent.is_none_or(|b| b.path().len() == path.len()));
    // A line with room left is split.
    let short = [
        PathSegment::MoveTo(p(0.0, 0.0)),
        PathSegment::LineTo(p(200.0, 0.0)),
    ];
    let split = path::densified(&short, &pull).expect("room to split");
    assert_eq!(split.len(), 28);
}

#[test]
fn room_is_counted_up_to_the_limit_and_not_past_it() {
    let max = flare_document::MAX_SEGMENTS;
    let full = vec![PathSegment::Close; max - 1];
    assert!(path::room(&full, 1).is_some(), "the last segment fits");
    assert!(
        path::room(&full, 2).is_none(),
        "one past the limit does not"
    );
    assert!(
        path::room(&full, usize::MAX).is_none(),
        "a count that would overflow does not"
    );
}

#[test]
fn the_document_pull_finds_the_shapes_in_reach_and_is_one_batch() {
    let mut f = fixture();
    let near = FrameSpan::new(
        0,
        NonZeroU32::new(4).expect("nonzero"),
        Content::Shape(drawn_line(10.0)),
    );
    add(&mut f, near);
    let document = f.session.document();
    let pull = pull_of((100.0, 100.0), (100.0, 80.0), 30.0);
    let targets = in_reach(document, f.scene, 0, None, &pull);
    assert_eq!(targets, vec![at(&f, 0)]);
    assert_eq!(
        in_reach(document, f.scene, 9, None, &pull),
        vec![],
        "no span at frame 9"
    );
    let commands = super::pull(document, f.scene, &targets, &pull).expect("pulled");
    f.session.apply(commands).expect("apply");
    let bounds = bounds(&f, 0);
    assert!(bounds.min.y < 85.0, "the line came up: {bounds:?}");
    // Nothing in reach is an error that names where.
    let away = pull_of((900.0, 900.0), (0.0, 0.0), 10.0);
    let refused = super::pull(f.session.document(), f.scene, &[at(&f, 0)], &away);
    assert!(matches!(refused, Err(MagnetError::NothingInReach { .. })));
    assert!(in_reach(f.session.document(), f.scene, 0, None, &away).is_empty());
}
