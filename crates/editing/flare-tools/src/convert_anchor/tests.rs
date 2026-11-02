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
fn a_click_makes_a_curve_point_a_corner_and_there_is_nothing_to_do_to_a_corner() {
    let mut f = circle_fixture();
    let commands = convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, None);
    run(&mut f, commands);
    let path = path_of(&f, 0);
    let (PathSegment::CubicTo(_, c2, p), PathSegment::CubicTo(c1, _, _)) = (path[1], path[2])
    else {
        panic!("curves");
    };
    assert_eq!((c2, c1), (p, p), "both handles are pulled in to the point");
    assert!(
        convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, None).is_none(),
        "already a corner"
    );
    let r = rect_fixture();
    assert!(
        convert_anchor(r.session.document(), r.scene, at(&r, 0), 1, None).is_none(),
        "straight edges have no handles"
    );
}

#[test]
fn a_drag_makes_a_corner_smooth_with_mirrored_handles_and_curves_the_lines_beside_it() {
    let mut f = rect_fixture();
    let to = DVec2::new(10.0 + 3.0, 0.0 - 4.0);
    let commands = convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, Some(to));
    run(&mut f, commands);
    let path = path_of(&f, 0);
    let (PathSegment::CubicTo(_, arriving, p), PathSegment::CubicTo(leaving, _, _)) =
        (path[1], path[2])
    else {
        panic!("both sides are curves now: {path:?}");
    };
    let corner = p.as_vec();
    assert_eq!(corner, DVec2::new(10.0, 0.0));
    assert_eq!(leaving.as_vec(), to, "the handle follows the drag");
    assert_eq!(
        arriving.as_vec(),
        corner - (to - corner),
        "the other is its mirror"
    );
    assert!(
        matches!(path[3], PathSegment::LineTo(_)),
        "farther edges stay straight"
    );
    let none = convert_anchor(f.session.document(), f.scene, at(&f, 0), 1, Some(corner));
    assert!(none.is_none(), "a drag of no length changes nothing");
    assert!(f.session.undo());
    assert!(
        path_of(&f, 0)
            .iter()
            .all(|s| !matches!(s, PathSegment::CubicTo(..)))
    );
}

#[test]
fn converting_needs_a_shape_and_a_point() {
    let f = rect_fixture();
    let doc = f.session.document();
    assert!(
        convert_anchor(doc, f.scene, at(&f, 0), 4, Some(DVec2::ONE)).is_none(),
        "Close"
    );
    assert!(convert_anchor(doc, f.scene, at(&f, 0), 99, Some(DVec2::ONE)).is_none());
    assert!(
        convert_anchor(
            doc,
            f.scene,
            SpanRef {
                layer: f.layer,
                start: 7,
                element: 0
            },
            1,
            Some(DVec2::ONE)
        )
        .is_none()
    );
}
