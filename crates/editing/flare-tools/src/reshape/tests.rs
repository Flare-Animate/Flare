#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_core::{DocumentPoint, Rgba8, ShapeId};
use flare_document::{Content, FrameSpan, PathSegment, Shape, Stroke};
use std::num::NonZeroU32;

fn p(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("point")
}

/// A merged line from (0, 100) to (200, 100).
fn line(merged: bool) -> Shape {
    Shape::new(
        ShapeId::new(),
        vec![
            PathSegment::MoveTo(p(0.0, 100.0)),
            PathSegment::LineTo(p(200.0, 100.0)),
        ],
        Rgba8::TRANSPARENT,
    )
    .expect("shape")
    .with_stroke(Stroke::new(2.0, Rgba8::opaque(0, 0, 0)).expect("stroke"))
    .merged(merged)
}

fn with_shape(shape: Shape) -> Fixture {
    let mut f = fixture();
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape)),
    );
    f
}

fn midpoint(shape: &Shape) -> DVec2 {
    let Some(PathSegment::CubicTo(c1, c2, end)) = shape.path().get(1).copied() else {
        panic!("the edge became a curve: {:?}", shape.path());
    };
    let (a, b) = (DVec2::new(0.0, 100.0), end.as_vec());
    (a + c1.as_vec() * 3.0 + c2.as_vec() * 3.0 + b) / 8.0
}

fn shape_at(f: &Fixture) -> Shape {
    match at_element(f) {
        Content::Shape(s) => s,
        other => panic!("{other:?}"),
    }
}

fn at_element(f: &Fixture) -> Content {
    find_element(f.session.document(), f.scene, at(f, 0))
        .cloned()
        .expect("element")
}

#[test]
fn dragging_the_middle_of_a_line_bends_it_through_the_drop_and_keeps_its_ends() {
    let mut f = with_shape(line(true));
    let (grab, commands) = bend_edge_near(
        f.session.document(),
        f.scene,
        0,
        None,
        DVec2::new(100.0, 103.0),
        DVec2::new(100.0, 40.0),
        6.0,
    )
    .expect("an edge");
    assert!((grab.t - 0.5).abs() < 1e-6, "{}", grab.t);
    f.session.apply(commands).expect("valid");
    let bent = shape_at(&f);
    let mid = midpoint(&bent);
    assert!(
        (mid.y - 40.0).abs() < 1e-6,
        "the grabbed point followed: {mid:?}"
    );
    assert!(
        matches!(bent.path()[0], PathSegment::MoveTo(s) if s.as_vec() == DVec2::new(0.0, 100.0))
    );
    assert!(
        matches!(bent.path()[1], PathSegment::CubicTo(_, _, e) if e.as_vec() == DVec2::new(200.0, 100.0))
    );
    assert!(bent.stroke().is_some() && bent.is_merged());
    f.session.undo();
    assert_eq!(shape_at(&f).path().len(), 2, "one undo step");
}

#[test]
fn a_grab_far_from_every_edge_finds_nothing_and_a_drawing_object_is_not_reshaped() {
    let f = with_shape(line(true));
    let d = f.session.document();
    assert!(edge_at(d, f.scene, 0, None, DVec2::new(100.0, 140.0), 6.0).is_none());
    let object = with_shape(line(false));
    assert_eq!(
        bend_edge_near(
            object.session.document(),
            object.scene,
            0,
            None,
            DVec2::new(100.0, 100.0),
            DVec2::new(100.0, 50.0),
            6.0
        )
        .map(|_| ()),
        Err(ReshapeError::NoEdge {
            x: 100.0,
            y: 100.0,
            tolerance: 6.0
        })
    );
}

#[test]
fn a_locked_layer_and_other_layers_are_left_alone() {
    let f = with_shape(line(true));
    let other = LayerId::new();
    assert!(
        edge_at(
            f.session.document(),
            f.scene,
            0,
            Some(other),
            DVec2::new(100.0, 100.0),
            6.0
        )
        .is_none()
    );
    assert!(
        edge_at(
            f.session.document(),
            f.scene,
            0,
            Some(f.layer),
            DVec2::new(100.0, 100.0),
            6.0
        )
        .is_some()
    );
    assert!(
        edge_at(
            f.session.document(),
            f.scene,
            5,
            None,
            DVec2::new(100.0, 100.0),
            6.0
        )
        .is_none()
    );
}

#[test]
fn a_closing_edge_and_a_curve_bend_too() {
    let square = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 100.0, Rgba8::WHITE)
        .expect("rect")
        .merged(true);
    let f = with_shape(square);
    // The left edge is the closing one.
    let (grab, commands) = bend_edge_near(
        f.session.document(),
        f.scene,
        0,
        None,
        DVec2::new(0.0, 50.0),
        DVec2::new(-30.0, 50.0),
        4.0,
    )
    .expect("edge");
    let mut f = f;
    f.session.apply(commands).expect("valid");
    let bent = shape_at(&f);
    assert_eq!(bent.path().len(), 6, "the closing edge gained its curve");
    assert!(matches!(bent.path()[4], PathSegment::CubicTo(_, _, e) if e.as_vec() == DVec2::ZERO));
    assert!(matches!(bent.path()[5], PathSegment::Close));
    assert_eq!(grab.segment, 4);
    // A second pull on the curve it is now.
    let again = bend_edge_near(
        f.session.document(),
        f.scene,
        0,
        None,
        DVec2::new(-30.0, 50.0),
        DVec2::new(-60.0, 50.0),
        6.0,
    )
    .expect("the curve");
    f.session.apply(again.1).expect("valid");
    assert_eq!(shape_at(&f).path().len(), 6, "a curve is bent in place");
}

#[test]
fn bad_numbers_and_a_pull_that_moves_nothing_change_nothing() {
    let f = with_shape(line(true));
    let d = f.session.document();
    let near = DVec2::new(100.0, 100.0);
    assert_eq!(
        bend_edge_near(d, f.scene, 0, None, DVec2::new(f64::NAN, 0.0), near, 6.0).map(|_| ()),
        Err(ReshapeError::NotFinite)
    );
    assert_eq!(
        bend_edge_near(
            d,
            f.scene,
            0,
            None,
            near,
            DVec2::new(f64::INFINITY, 0.0),
            6.0
        )
        .map(|_| ()),
        Err(ReshapeError::NotFinite)
    );
    assert_eq!(
        bend_edge_near(d, f.scene, 0, None, near, near, 6.0).map(|_| ()),
        Err(ReshapeError::Refused)
    );
    assert_eq!(clamp_tolerance(f64::NAN), DEFAULT_TOLERANCE);
    assert_eq!(clamp_tolerance(1e9), MAX_TOLERANCE);
    // An enormous pull is still a valid shape or a refusal, never a panic.
    let wild = bend_edge_near(d, f.scene, 0, None, near, DVec2::new(1e300, -1e300), 6.0);
    match wild {
        Ok((_, commands)) => assert!(flare_commands::apply_all(d, commands).is_ok()),
        Err(error) => assert_eq!(error, ReshapeError::Refused),
    }
}
