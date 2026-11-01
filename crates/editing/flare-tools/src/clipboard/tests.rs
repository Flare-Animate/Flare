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
fn convert_to_symbol_replaces_the_shape_with_an_equivalent_instance() {
    let mut f = fixture();
    add(&mut f, rect_span(2, 8, 30.0, 40.0, 10.0, 10.0));
    let before = flare_eval::evaluate_frame(f.session.document(), f.scene, 4).expect("eval");
    let before_id = before.shapes[0].shape.id();
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, at(&f, 2)).expect("convert");
    f.session.apply(commands).expect("apply");

    let doc = f.session.document();
    assert_eq!(doc.symbols().len(), 1);
    assert_eq!(doc.symbols()[0].name(), "Symbol 1");
    let span = find_span(doc, f.scene, at(&f, 2)).expect("span");
    assert_eq!(span.duration().get(), 8);
    assert!(matches!(span.content(), Content::Instance(i) if i.symbol() == symbol));
    // Same artwork on stage: same shape, identity transform, one nesting level.
    let after = flare_eval::evaluate_frame(doc, f.scene, 4).expect("eval");
    assert_eq!(after.shapes.len(), 1);
    assert_eq!(after.shapes[0].shape.id(), before_id);
    assert_eq!(after.shapes[0].transform, DAffine2::IDENTITY);
    assert_eq!(after.shapes[0].path.len(), 1);
    // Undo restores the original in one step.
    f.session.undo();
    assert_eq!(f.session.document().symbols().len(), 0);
}

#[test]
fn only_shapes_can_be_converted() {
    let mut f = fixture();
    let symbol = SymbolId::new();
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            symbol,
            "S",
            Timeline::default(),
        ))])
        .expect("symbol");
    let instance = Instance::new(InstanceId::new(), symbol, Transform2D::IDENTITY);
    let span = FrameSpan::new(0, NonZeroU32::MIN, Content::Instance(instance));
    add(&mut f, span);
    assert!(convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).is_none());
    assert!(convert_to_symbol(f.session.document(), f.scene, at(&f, 5)).is_none());
}

#[test]
fn placing_an_instance_centers_the_symbol_on_the_target_point() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 24, 0.0, 0.0, 10.0, 10.0));
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, at(&f, 0)).expect("convert");
    f.session.apply(commands).expect("convert");
    let commands = place_instance(
        f.session.document(),
        f.scene,
        symbol,
        DVec2::new(100.0, 60.0),
    )
    .expect("place");
    f.session.apply(commands).expect("place");
    let scene = &f.session.document().scenes()[0];
    let new_layer = scene.timeline().layers().last().expect("layer").id();
    let placed = span_bounds(
        f.session.document(),
        f.scene,
        SpanRef {
            layer: new_layer,
            start: 0,
        },
    )
    .expect("bounds");
    assert_eq!(placed.center(), Pos2::new(100.0, 60.0));
    assert_eq!(scene.timeline().layers().len(), 2);
}

#[test]
fn a_mirror_copy_is_a_new_layer_reflected_across_the_axis_and_the_original_stays() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 10.0, 0.0, 20.0, 10.0));
    let before = f.session.document().clone();
    let mirror = DAffine2::from_translation(DVec2::new(100.0, 0.0))
        * DAffine2::from_scale(DVec2::new(-1.0, 1.0))
        * DAffine2::from_translation(DVec2::new(-100.0, 0.0));
    let (commands, copy) =
        duplicate_through(&before, f.scene, at(&f, 0), mirror, "mirror").expect("mirror copy");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        span_bounds(f.session.document(), f.scene, copy).expect("copy bounds"),
        Rect::from_min_max(Pos2::new(170.0, 0.0), Pos2::new(190.0, 10.0))
    );
    assert_eq!(
        bounds(&f, 0),
        span_bounds(&before, f.scene, at(&f, 0)).expect("old")
    );
    let names: Vec<&str> = f.session.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .map(|l| l.name())
        .collect();
    assert_eq!(names, ["L", "L mirror"]);
    f.session.undo();
    assert_eq!(
        f.session.document(),
        &before,
        "one undo step removes the copy"
    );
}

#[test]
fn a_mirror_copy_has_its_own_identity_and_a_collapsing_matrix_is_refused() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 10.0, 0.0, 20.0, 10.0));
    let document = f.session.document().clone();
    let flip = DAffine2::from_scale(DVec2::new(-1.0, 1.0));
    let (commands, copy) =
        duplicate_through(&document, f.scene, at(&f, 0), flip, "mirror").expect("copy");
    f.session.apply(commands).expect("apply");
    let ids: Vec<ShapeId> = [at(&f, 0), copy]
        .into_iter()
        .filter_map(|s| find_span(f.session.document(), f.scene, s))
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.id()),
            _ => None,
        })
        .collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1]);
    let flat = DAffine2::from_scale(DVec2::new(0.0, 1.0));
    assert!(duplicate_through(&document, f.scene, at(&f, 0), flat, "x").is_none());
    assert!(duplicate_through(&document, f.scene, at(&f, 9), flip, "x").is_none());
}

#[test]
fn converting_can_make_a_button_or_movie_clip_and_defaults_to_a_graphic() {
    use flare_document::SymbolKind;
    for (kind, convert) in [
        (SymbolKind::Graphic, None),
        (SymbolKind::Button, Some(SymbolKind::Button)),
        (SymbolKind::MovieClip, Some(SymbolKind::MovieClip)),
    ] {
        let mut f = fixture();
        add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
        let (commands, symbol) = match convert {
            Some(kind) => convert_to_symbol_as(f.session.document(), f.scene, at(&f, 0), kind),
            None => convert_to_symbol(f.session.document(), f.scene, at(&f, 0)),
        }
        .expect("convertible");
        f.session.apply(commands).expect("apply");
        let made = f.session.document().symbol(symbol).expect("symbol");
        assert_eq!(made.kind(), kind);
    }
}
