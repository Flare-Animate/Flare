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
fn grouping_shapes_makes_one_symbol_instance_and_breaking_it_apart_restores_the_parts() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 6, 0.0, 0.0, 10.0, 10.0));
    let first = at(&f, 0);
    let second = add_on_new_layer(&mut f, rect_span(0, 6, 40.0, 0.0, 10.0, 10.0));
    assert!(
        group(f.session.document(), f.scene, &[first]).is_none(),
        "one shape is not a group"
    );
    let (commands, symbol) = group(f.session.document(), f.scene, &[first, second]).expect("group");
    f.session.apply(commands).expect("apply");
    let document = f.session.document();
    assert_eq!(
        document
            .symbol(symbol)
            .expect("symbol")
            .timeline()
            .layers()
            .len(),
        2
    );
    assert!(
        find_span(document, f.scene, second).is_none(),
        "the other span is gone"
    );
    assert!(matches!(
        find_span(document, f.scene, first)
            .expect("instance")
            .content(),
        Content::Instance(_)
    ));
    // The group moves as one, and breaking it apart puts each part back where it now is.
    let moved = translate(document, f.scene, first, DVec2::new(100.0, 0.0)).expect("move");
    f.session.apply(moved).expect("apply");
    let (commands, parts) = break_apart(f.session.document(), f.scene, first).expect("break apart");
    assert_eq!(parts.len(), 2);
    f.session.apply(commands).expect("apply");
    let document = f.session.document();
    assert!(
        find_span(document, f.scene, first).is_none(),
        "the instance is gone"
    );
    let mut lefts: Vec<f32> = parts
        .iter()
        .map(|p| span_bounds(document, f.scene, *p).expect("bounds").left())
        .collect();
    lefts.sort_by(f32::total_cmp);
    assert_eq!(lefts, vec![100.0, 140.0]);
    assert!(
        break_apart(document, f.scene, parts[0]).is_none(),
        "a shape cannot be broken apart"
    );
}
