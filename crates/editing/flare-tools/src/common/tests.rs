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
fn stroke_width_is_part_of_the_bounds_and_survives_moves() {
    use flare_document::Stroke;
    let mut f = fixture();
    let stroke = Stroke::new(6.0, RED).expect("stroke");
    let shape = Shape::rect(ShapeId::new(), 10.0, 10.0, 20.0, 20.0, RED)
        .expect("rect")
        .with_stroke(stroke);
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Shape(shape));
    add(&mut f, span);
    assert_eq!(
        bounds(&f, 0),
        Rect::from_min_max(Pos2::new(7.0, 7.0), Pos2::new(33.0, 33.0))
    );
    let commands = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(1.0, 1.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let moved = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    let Content::Shape(shape) = moved.content() else {
        panic!("shape expected")
    };
    assert_eq!(shape.stroke(), Some(stroke), "moving keeps the stroke");
}

#[test]
fn symbols_made_only_of_other_symbols_have_bounds_through_their_transforms() {
    let (inner, outer) = (SymbolId::new(), SymbolId::new());
    let tl = |spans: Vec<FrameSpan>| {
        Timeline::new(vec![Layer::new(LayerId::new(), "L", spans).expect("layer")])
    };
    let shift =
        Transform2D::new(DAffine2::from_translation(DVec2::new(100.0, 20.0))).expect("finite");
    let placed = FrameSpan::new(
        0,
        NonZeroU32::new(1).expect("nonzero"),
        Content::Instance(Instance::new(InstanceId::new(), inner, shift)),
    );
    let symbols = vec![
        Symbol::new(
            inner,
            "Inner",
            tl(vec![rect_span(0, 1, 0.0, 0.0, 10.0, 5.0)]),
        ),
        Symbol::new(outer, "Outer", tl(vec![placed])),
    ];
    let rate = FrameRate::new(24, 1).expect("rate");
    let scene = Scene::new(SceneId::new(), "S", Timeline::default());
    let doc = Document::new(DocumentId::new(), rate, vec![scene], symbols).expect("doc");
    let bounds = symbol_bounds(&doc, outer).expect("nested bounds");
    assert_eq!(
        (bounds.min, bounds.max),
        (Pos2::new(100.0, 20.0), Pos2::new(110.0, 25.0))
    );
    assert!(
        symbol_bounds(&doc, SymbolId::new()).is_none(),
        "unknown symbol"
    );
}

#[test]
fn many_span_bounds_match_the_one_at_a_time_answers_and_skip_missing_spans() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 10.0, 10.0));
    let layer = f.layer;
    let present = SpanRef {
        layer,
        start: 0,
        element: 0,
    };
    let missing = SpanRef {
        layer,
        start: 9,
        element: 0,
    };
    let gone = SpanRef {
        layer: LayerId::new(),
        start: 0,
        element: 0,
    };
    let many = spans_bounds(f.session.document(), f.scene, &[present, missing, gone]);
    let one = span_bounds(f.session.document(), f.scene, present).expect("bounds");
    assert_eq!(many, vec![one]);
    assert!(spans_bounds(f.session.document(), SceneId::new(), &[present]).is_empty());
}
