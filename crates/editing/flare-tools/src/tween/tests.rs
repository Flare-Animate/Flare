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
fn tweens_toggle_on_instances_only() {
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
    add(
        &mut f,
        FrameSpan::new(
            0,
            NonZeroU32::new(4).expect("nonzero"),
            Content::Instance(instance),
        ),
    );
    add(&mut f, rect_span(4, 2, 0.0, 0.0, 5.0, 5.0));
    let doc = f.session.document();
    assert!(
        set_tween(doc, f.scene, at(&f, 4), Tween::Linear).is_none(),
        "shapes do not tween"
    );
    assert!(
        set_tween(doc, f.scene, at(&f, 0), Tween::None).is_none(),
        "already untweened"
    );
    let commands = set_tween(doc, f.scene, at(&f, 0), Tween::Linear).expect("enable");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!((span.tween(), span.duration().get()), (Tween::Linear, 4));
    let commands =
        set_tween(f.session.document(), f.scene, at(&f, 0), Tween::None).expect("disable");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        find_span(f.session.document(), f.scene, at(&f, 0))
            .expect("span")
            .tween(),
        Tween::None
    );
}

#[test]
fn a_shape_tween_fits_a_shape_and_a_motion_tween_an_instance_and_the_ease_is_kept() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let shape_at = at(&f, 0);
    // A motion tween makes no sense on a shape.
    assert!(set_tween(f.session.document(), f.scene, shape_at, Tween::Linear).is_none());
    let commands =
        set_tween(f.session.document(), f.scene, shape_at, Tween::Shape).expect("a shape tween");
    f.session.apply(commands).expect("apply");
    let commands = set_ease(f.session.document(), f.scene, shape_at, 60).expect("ease");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, shape_at).expect("span");
    assert_eq!((span.tween(), span.ease()), (Tween::Shape, 60));
    // The same setting again is nothing to do, and an ease is kept when the tween is switched off.
    assert!(set_tween(f.session.document(), f.scene, shape_at, Tween::Shape).is_none());
    assert!(set_ease(f.session.document(), f.scene, shape_at, 60).is_none());
    let commands = set_tween(f.session.document(), f.scene, shape_at, Tween::None).expect("off");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, shape_at).expect("span");
    assert_eq!((span.tween(), span.ease()), (Tween::None, 60));
}

#[test]
fn an_ease_is_clamped_and_follows_the_span_through_other_edits() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    let commands =
        set_tween(f.session.document(), f.scene, at(&f, 0), Tween::Shape).expect("tween");
    f.session.apply(commands).expect("apply");
    let commands = set_ease(f.session.document(), f.scene, at(&f, 0), 120).expect("ease");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        find_span(f.session.document(), f.scene, at(&f, 0))
            .expect("span")
            .ease(),
        100
    );
    // Moving and copying keep the tween and its ease.
    let moved = translate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(5.0, 5.0),
    )
    .expect("move");
    f.session.apply(moved).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!((span.tween(), span.ease()), (Tween::Shape, 100));
    let (copy, at_copy) = duplicate(
        f.session.document(),
        f.scene,
        at(&f, 0),
        DVec2::new(1.0, 1.0),
    )
    .expect("copy");
    f.session.apply(copy).expect("apply");
    let copied = find_span(f.session.document(), f.scene, at_copy).expect("span");
    assert_eq!((copied.tween(), copied.ease()), (Tween::Shape, 100));
    let item = copy_span(f.session.document(), f.scene, at(&f, 0)).expect("clip");
    let (pasted, at_paste) = paste(f.session.document(), f.scene, &item, 10).expect("paste");
    f.session.apply(pasted).expect("apply");
    let pasted = find_span(f.session.document(), f.scene, at_paste).expect("span");
    assert_eq!((pasted.tween(), pasted.ease()), (Tween::Shape, 100));
}
