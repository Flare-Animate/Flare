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
fn translating_a_shape_moves_it_and_keeps_duration_and_identity() {
    let mut f = fixture();
    add(&mut f, rect_span(3, 10, 0.0, 0.0, 20.0, 10.0));
    let before = f.session.document().clone();
    let commands = translate(&before, f.scene, at(&f, 3), DVec2::new(5.0, -2.0)).expect("move");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 3)).expect("span");
    assert_eq!(span.duration().get(), 10);
    let old = find_span(&before, f.scene, at(&f, 3)).expect("old");
    let (Content::Shape(a), Content::Shape(b)) = (old.content(), span.content()) else {
        panic!("shapes expected");
    };
    assert_eq!(a.id(), b.id());
    assert_eq!(
        bounds(&f, 3),
        Rect::from_min_max(Pos2::new(5.0, -2.0), Pos2::new(25.0, 8.0))
    );
    f.session.undo();
    assert_eq!(f.session.document(), &before);
}

#[test]
fn translating_an_instance_composes_the_translation_and_keeps_tween() {
    let mut f = fixture();
    let symbol = SymbolId::new();
    let rotate =
        Transform2D::new(DAffine2::from_angle(std::f64::consts::FRAC_PI_2)).expect("finite");
    let instance = Instance::new(InstanceId::new(), symbol, rotate);
    let span = FrameSpan::new(
        0,
        NonZeroU32::new(5).expect("nonzero"),
        Content::Instance(instance),
    )
    .with_tween(Tween::Linear);
    f.session
        .apply([Command::AddSymbol(Symbol::new(
            symbol,
            "S",
            Timeline::default(),
        ))])
        .expect("symbol");
    add(&mut f, span);
    let commands = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(10.0, 0.0),
    )
    .expect("move");
    f.session.apply(commands).expect("apply");
    let moved = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!(moved.tween(), Tween::Linear);
    let Content::Instance(instance) = moved.content() else {
        panic!("instance expected")
    };
    let affine = instance.transform().as_affine();
    assert_eq!(affine.translation, DVec2::new(10.0, 0.0));
    assert!(
        (affine.matrix2.x_axis.y - 1.0).abs() < 1e-12,
        "rotation is preserved"
    );
}

#[test]
fn degenerate_translations_do_nothing() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 5, 0.0, 0.0, 5.0, 5.0));
    let doc = f.session.document();
    assert!(translate(doc, f.scene, at(&f, 0), DVec2::ZERO).is_none());
    assert!(translate(doc, f.scene, at(&f, 99), DVec2::ONE).is_none());
    assert!(translate(doc, f.scene, at(&f, 0), DVec2::new(f64::NAN, 0.0)).is_none());
}
