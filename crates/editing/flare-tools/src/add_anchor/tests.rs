#![allow(clippy::expect_used, unused_imports)]

use crate::ops::*;
use crate::test_support::*;
use emath::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{
    DocumentId, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId, SymbolId, Transform2D,
};
use flare_document::{
    Content, Document, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Symbol, Timeline,
    Tween,
};
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};
use std::num::NonZeroU32;

#[test]
fn an_anchor_added_on_an_edge_splits_it_and_leaves_the_outline_as_it_was() {
    let mut f = rect_fixture();
    let before = bounds(&f, 0);
    let top = add_anchor(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(5.0, 0.4),
        2.0,
    );
    run(&mut f, top);
    assert_eq!(anchor_count(&f), 5);
    assert_eq!(bounds(&f, 0), before);
    let points = path_of(&f, 0);
    assert!(
        points
            .iter()
            .any(|s| matches!(s, PathSegment::LineTo(p) if p.as_vec() == DVec2::new(5.0, 0.0))),
        "the new point is on the edge: {points:?}"
    );
    // The edge Close draws back to the start can be split too (the left edge).
    let left = add_anchor(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(0.0, 5.0),
        2.0,
    );
    run(&mut f, left);
    assert_eq!(anchor_count(&f), 6);
    assert!(matches!(path_of(&f, 0).last(), Some(PathSegment::Close)));
    assert!(f.session.undo() && f.session.undo());
    assert_eq!(anchor_count(&f), 4, "each was one undo step");
}

#[test]
fn nothing_is_added_away_from_the_outline_at_a_corner_or_on_something_that_is_not_a_shape() {
    let f = rect_fixture();
    let doc = f.session.document();
    let near = |x, y, tol| add_anchor(doc, f.scene, at(&f, 0), DVec2::new(x, y), tol);
    assert!(near(5.0, 5.0, 2.0).is_none(), "inside, away from any edge");
    assert!(
        near(10.0, 0.0, 2.0).is_none(),
        "at a corner there is a point already"
    );
    assert!(near(5.0, 0.0, 2.0).is_some());
    assert!(near(5.0, 1.0, 0.5).is_none(), "outside the tolerance");
    assert!(near(f64::NAN, 0.0, 2.0).is_none(), "not a number");
    assert!(
        add_anchor(
            doc,
            f.scene,
            SpanRef {
                layer: f.layer,
                start: 99
            },
            DVec2::ZERO,
            2.0
        )
        .is_none()
    );
}

#[test]
fn an_anchor_added_on_a_curve_lies_on_it_and_the_two_halves_are_the_same_curve() {
    let mut f = circle_fixture();
    // The point of the circle at 45 degrees.
    let r = 20.0 * std::f64::consts::FRAC_1_SQRT_2;
    let near = DVec2::new(50.0 + r + 0.5, 50.0 + r + 0.5);
    let commands = add_anchor(f.session.document(), f.scene, at(&f, 0), near, 3.0);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 6);
    let path = path_of(&f, 0);
    let PathSegment::CubicTo(_, _, p) = path[2] else {
        panic!("the split curve");
    };
    let on_circle = (p.as_vec() - DVec2::new(50.0, 50.0)).length();
    assert!(
        (on_circle - 20.0).abs() < 0.05,
        "{on_circle} is off the circle"
    );
}
