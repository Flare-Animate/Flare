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
fn deleting_a_corner_joins_its_neighbours_and_a_shape_keeps_its_least_points() {
    let mut f = rect_fixture();
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 2);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 3, "a triangle");
    assert!(
        delete_anchor(f.session.document(), f.scene, at(&f, 0), 1).is_none(),
        "a closed shape keeps three points"
    );
    assert!(f.session.undo());
    assert_eq!(anchor_count(&f), 4);
    // The first point goes too, and the outline is what is left.
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 0);
    run(&mut f, commands);
    let b = bounds(&f, 0);
    assert_eq!((b.min, b.max), (Pos2::new(0.0, 0.0), Pos2::new(10.0, 10.0)));
    assert_eq!(anchor_count(&f), 3);
    // Close is not a point, a missing segment is not either.
    let f = rect_fixture();
    assert!(delete_anchor(f.session.document(), f.scene, at(&f, 0), 4).is_none());
    assert!(delete_anchor(f.session.document(), f.scene, at(&f, 0), 99).is_none());
}

#[test]
fn deleting_a_point_of_an_open_line_shortens_it_down_to_two_points() {
    let mut f = fixture();
    let stroke = flare_document::Stroke::new(2.0, RED).expect("stroke");
    let p = |x, y| flare_core::DocumentPoint::new(x, y).expect("point");
    let path = vec![
        PathSegment::MoveTo(p(0.0, 0.0)),
        PathSegment::LineTo(p(5.0, 5.0)),
        PathSegment::LineTo(p(10.0, 0.0)),
    ];
    let shape = Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
        .expect("shape")
        .with_stroke(stroke);
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape)),
    );
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 2);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 2);
    assert!(delete_anchor(f.session.document(), f.scene, at(&f, 0), 1).is_none());
}

#[test]
fn deleting_a_curve_point_makes_one_curve_where_there_were_two() {
    let mut f = circle_fixture();
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 2);
    run(&mut f, commands);
    let path = path_of(&f, 0);
    assert_eq!(anchor_count(&f), 4);
    assert!(matches!(path[2], PathSegment::CubicTo(..)), "still a curve");
    // The first point of a path that comes back to it by a segment of its own.
    let commands = delete_anchor(f.session.document(), f.scene, at(&f, 0), 0);
    run(&mut f, commands);
    assert_eq!(anchor_count(&f), 3);
    assert!(f.session.undo() && f.session.undo());
    assert_eq!(anchor_count(&f), 5, "the circle again, one undo for each");
}
