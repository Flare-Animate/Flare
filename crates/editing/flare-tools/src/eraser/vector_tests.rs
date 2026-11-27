#![allow(clippy::expect_used)]

use crate::eraser::vector::{
    MAX_LOOP_POINTS, Take, VectorEraserType, erase_with, freehand_loop, rectangle,
};
use crate::test_support::*;
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{Content, FrameSpan, PathSegment, Shape, Stroke};
use glamx::DVec2;
use std::num::NonZeroU32;

/// The paths' first points of the shapes on the first layer at `frame`, to tell them apart.
fn shapes_at(f: &Fixture, frame: u32) -> Vec<(f64, f64)> {
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    let Some(span) = layer.span_at(frame) else {
        return Vec::new();
    };
    span.elements()
        .filter_map(|c| match c {
            Content::Shape(shape) => match shape.path().first() {
                Some(PathSegment::MoveTo(p)) => Some((p.as_vec().x, p.as_vec().y)),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

/// A span of `duration` frames holding a 10 by 10 square at (0, 0), one at (30, 0), and a
/// stroked line from (0, 30) to (60, 30).
fn three_shapes(f: &mut Fixture, duration: u32) {
    let square = |x: f64| {
        Content::Shape(Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, RED).expect("square"))
    };
    let line = Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(DocumentPoint::new(0.0, 30.0).expect("point")),
            PathSegment::LineTo(DocumentPoint::new(60.0, 30.0).expect("point")),
        ],
        Rgba8::TRANSPARENT,
    )
    .expect("line")
    .with_stroke(Stroke::new(2.0, RED).expect("stroke"));
    let span = FrameSpan::new(0, NonZeroU32::new(duration).expect("nonzero"), square(0.0))
        .with_elements(vec![square(0.0), square(30.0), Content::Shape(line)])
        .expect("elements")
        .expect("not empty");
    add(f, span);
}

fn erase(f: &mut Fixture, frame: u32, area: Vec<DVec2>, touched: bool) -> usize {
    let take = if touched { Take::Touched } else { Take::Inside };
    let commands = erase_with(f.session.document(), f.scene, frame, &[area], take, None);
    let count = commands.len();
    if count > 0 {
        f.session.apply(commands).expect("erase");
    }
    count
}

#[test]
fn whole_line_takes_every_shape_it_touches_whole() {
    let mut f = fixture();
    three_shapes(&mut f, 1);
    // A box over a corner of the first square and the middle of the line, missing the second.
    let tip = rectangle(DVec2::new(8.0, 8.0), DVec2::new(20.0, 35.0));
    assert!(erase(&mut f, 0, tip, true) > 0);
    assert_eq!(
        shapes_at(&f, 0),
        vec![(30.0, 0.0)],
        "the square and the line it touched went whole; the other square stayed whole"
    );
    assert_eq!(
        f.session.undo_labels().count(),
        2,
        "the shapes, then one step"
    );
}

#[test]
fn rectangular_and_freehand_take_only_what_lies_wholly_inside() {
    let mut f = fixture();
    three_shapes(&mut f, 1);
    // Round the first square, and across the line without holding all of it.
    let area = rectangle(DVec2::new(-5.0, -5.0), DVec2::new(15.0, 40.0));
    assert!(erase(&mut f, 0, area, false) > 0);
    assert_eq!(
        shapes_at(&f, 0),
        vec![(30.0, 0.0), (0.0, 30.0)],
        "the square inside went; the line it crossed stayed, whole"
    );
    let around = freehand_loop(&[
        DVec2::new(-5.0, 20.0),
        DVec2::new(70.0, 20.0),
        DVec2::new(70.0, 40.0),
        DVec2::new(-5.0, 40.0),
    ]);
    assert!(erase(&mut f, 0, around, false) > 0);
    assert_eq!(
        shapes_at(&f, 0),
        vec![(30.0, 0.0)],
        "the loop held the line"
    );
}

#[test]
fn nothing_taken_makes_no_step_and_a_held_frame_changes_alone() {
    let mut f = fixture();
    three_shapes(&mut f, 3);
    let far = rectangle(DVec2::new(100.0, 100.0), DVec2::new(120.0, 120.0));
    assert_eq!(erase(&mut f, 1, far.clone(), true), 0, "touched nothing");
    assert_eq!(erase(&mut f, 1, far, false), 0, "held nothing");
    let all = rectangle(DVec2::new(-5.0, -5.0), DVec2::new(70.0, 40.0));
    assert!(erase(&mut f, 1, all, false) > 0);
    assert!(shapes_at(&f, 1).is_empty(), "frame 1 is blank");
    assert_eq!(shapes_at(&f, 0).len(), 3, "the keyframe keeps its drawing");
    assert_eq!(shapes_at(&f, 2).len(), 3, "and so do the frames after");
}

#[test]
fn areas_are_bounded_and_refuse_what_is_not_one() {
    assert!(
        rectangle(DVec2::ZERO, DVec2::new(0.0, 5.0)).is_empty(),
        "no width"
    );
    assert!(rectangle(DVec2::ZERO, DVec2::new(f64::NAN, 5.0)).is_empty());
    assert_eq!(rectangle(DVec2::ZERO, DVec2::ONE).len(), 4);
    assert!(
        freehand_loop(&[DVec2::ZERO, DVec2::ONE]).is_empty(),
        "two points"
    );
    let bad = [DVec2::ZERO, DVec2::X, DVec2::splat(f64::INFINITY), DVec2::Y];
    assert_eq!(
        freehand_loop(&bad).len(),
        3,
        "the point that is not finite is left out"
    );
    let many: Vec<DVec2> = (0..MAX_LOOP_POINTS + 50)
        .map(|i| DVec2::from_angle(i as f64 * 0.001))
        .collect();
    assert_eq!(freehand_loop(&many).len(), MAX_LOOP_POINTS);
}

#[test]
fn types_are_named_and_parsed() {
    for kind in VectorEraserType::ALL {
        assert_eq!(VectorEraserType::parse(kind.name()), Some(kind));
    }
    assert_eq!(
        VectorEraserType::parse(" Rectangular "),
        Some(VectorEraserType::Rectangular)
    );
    assert_eq!(VectorEraserType::parse("polyline"), None, "not built");
    assert!(VectorEraserType::Segment.uses_tip() && VectorEraserType::Whole.uses_tip());
    assert!(!VectorEraserType::Freehand.uses_tip());
    assert_eq!(VectorEraserType::default(), VectorEraserType::Segment);
    assert_eq!(VectorEraserType::Segment.take(), Take::Segments);
    assert_eq!(VectorEraserType::Rectangular.take(), Take::Inside);
}

#[test]
fn an_area_from_points_is_checked_for_its_type() {
    use crate::eraser::vector::{VectorEraseError, area};
    let (a, b) = (DVec2::ZERO, DVec2::new(10.0, 10.0));
    assert_eq!(
        area(VectorEraserType::Rectangular, &[a, b], 0.0).map(|o| o.len()),
        Ok(4)
    );
    assert!(matches!(
        area(VectorEraserType::Rectangular, &[a], 20.0),
        Err(VectorEraseError::WrongPoints { given: 1, .. })
    ));
    assert!(
        area(VectorEraserType::Rectangular, &[a, a], 20.0).is_err(),
        "no area"
    );
    assert!(area(VectorEraserType::Freehand, &[a, b], 20.0).is_err());
    assert!(
        area(VectorEraserType::Segment, &[a], 20.0).is_ok_and(|o| o.len() >= 3),
        "a dot"
    );
    assert_eq!(
        area(VectorEraserType::Segment, &[a, b], 0.5),
        Err(VectorEraseError::BadSize)
    );
    assert_eq!(
        area(VectorEraserType::Segment, &[a, b], f64::NAN),
        Err(VectorEraseError::BadSize)
    );
    assert!(area(VectorEraserType::Segment, &[], 20.0).is_err());
    assert_eq!(
        area(
            VectorEraserType::Freehand,
            &[a, b, DVec2::splat(f64::NAN)],
            20.0
        ),
        Err(VectorEraseError::NotFinite)
    );
}
