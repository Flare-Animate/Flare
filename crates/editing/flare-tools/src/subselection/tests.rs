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
fn anchors_are_the_segment_ends_and_moving_one_reshapes_the_outline() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let span = at(&f, 0);
    let Content::Shape(shape) = find_span(f.session.document(), f.scene, span)
        .expect("span")
        .content()
    else {
        panic!("shape");
    };
    let points = anchors(shape);
    assert_eq!(points.len(), 4, "a rectangle has four corners");
    assert_eq!(points[2], (2, Pos2::new(10.0, 10.0)));
    let commands = move_anchor(
        f.session.document(),
        f.scene,
        span,
        2,
        DVec2::new(20.0, 30.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let b = bounds(&f, 0);
    assert_eq!((b.min, b.max), (Pos2::new(0.0, 0.0), Pos2::new(20.0, 30.0)));
    assert!(
        move_anchor(
            f.session.document(),
            f.scene,
            span,
            2,
            DVec2::new(20.0, 30.0)
        )
        .is_none(),
        "no movement"
    );
    assert!(
        move_anchor(f.session.document(), f.scene, span, 4, DVec2::ZERO).is_none(),
        "Close is not an anchor"
    );
    assert!(move_anchor(f.session.document(), f.scene, span, 99, DVec2::ZERO).is_none());
}

#[test]
fn moving_a_curve_anchor_carries_its_handles() {
    let mut f = fixture();
    let ellipse = Shape::ellipse(ShapeId::new(), 10.0, 10.0, 10.0, 10.0, RED).expect("ellipse");
    add(
        &mut f,
        FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(ellipse)),
    );
    let span = at(&f, 0);
    let before = bounds(&f, 0);
    // The first segment is the move to the right-hand point; drag it 6 units to the right.
    let commands = move_anchor(
        f.session.document(),
        f.scene,
        span,
        0,
        DVec2::new(26.0, 10.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let after = bounds(&f, 0);
    assert!(
        after.right() > before.right() + 5.0,
        "{before:?} then {after:?}"
    );
    assert_eq!(after.left(), before.left());
}
