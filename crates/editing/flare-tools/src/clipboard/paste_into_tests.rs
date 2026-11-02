#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{InstanceId, ShapeId, SymbolId, Transform2D};
use flare_document::{Content, FrameSpan, Instance, Shape};

use super::*;
use crate::ops::{ClipItem, copy_span};
use crate::test_support::*;

fn item(x: f64) -> ClipItem {
    ClipItem {
        name: "Layer 1".to_owned(),
        duration: NonZeroU32::new(4).expect("nz"),
        tween: flare_document::Tween::None,
        ease: 0,
        content: Content::Shape(
            Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, RED).expect("rect"),
        ),
    }
}

fn paste(
    f: &mut Fixture,
    target: Target,
    frame: u32,
    items: &[ClipItem],
) -> Result<DrawPlan, PasteRefusal> {
    let plan = paste_into(
        f.session.document(),
        f.scene,
        target,
        items,
        frame,
        DVec2::ZERO,
        "Layer 1",
    )?;
    f.session.apply(plan.commands.clone()).expect("apply");
    Ok(plan)
}

fn layers(f: &Fixture) -> usize {
    f.session.document().scenes()[0].timeline().layers().len()
}

#[test]
fn pasted_things_join_the_keyframe_of_the_layer_and_make_no_layer() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 10, 0.0, 0.0, 5.0, 5.0));
    let before = layers(&f);
    let target = Target::Layer(f.layer);
    let plan = paste(&mut f, target, 3, &[item(20.0), item(40.0)]).expect("paste");
    assert_eq!(layers(&f), before, "no new layer");
    assert_eq!(plan.placed.len(), 2);
    let span = &f.session.document().scenes()[0].timeline().layers()[0].spans()[0];
    assert_eq!(span.element_count(), 3);
    f.session.undo();
    let span = &f.session.document().scenes()[0].timeline().layers()[0].spans()[0];
    assert_eq!(span.element_count(), 1, "one undo step");
}

#[test]
fn pasting_into_a_blank_frame_starts_a_keyframe_there() {
    let mut f = fixture();
    let target = Target::Layer(f.layer);
    let plan = paste(&mut f, target, 5, &[item(0.0)]).expect("paste");
    assert_eq!(plan.placed.len(), 1);
    let spans = f.session.document().scenes()[0].timeline().layers()[0]
        .spans()
        .to_vec();
    assert_eq!(spans.last().map(FrameSpan::start), Some(5));
}

#[test]
fn what_is_pasted_is_new_and_moved() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 10, 0.0, 0.0, 5.0, 5.0));
    let original = copy_span(f.session.document(), f.scene, at(&f, 0)).expect("copy");
    let plan = paste_into(
        f.session.document(),
        f.scene,
        Target::Layer(f.layer),
        std::slice::from_ref(&original),
        0,
        DVec2::new(30.0, 0.0),
        "L",
    )
    .expect("paste");
    f.session.apply(plan.commands).expect("apply");
    let span = &f.session.document().scenes()[0].timeline().layers()[0].spans()[0];
    let (Content::Shape(a), Content::Shape(b)) = (span.content(), &span.extras()[0]) else {
        panic!("two shapes");
    };
    assert_ne!(a.id(), b.id(), "a fresh identity");
    assert_ne!(a.path(), b.path(), "moved by the delta");
}

#[test]
fn an_instance_of_a_symbol_the_document_lacks_is_refused() {
    let mut f = fixture();
    let missing = ClipItem {
        content: Content::Instance(Instance::new(
            InstanceId::new(),
            SymbolId::new(),
            Transform2D::IDENTITY,
        )),
        ..item(0.0)
    };
    let target = Target::Layer(f.layer);
    assert_eq!(
        paste(&mut f, target, 0, &[missing]),
        Err(PasteRefusal::MissingSymbol)
    );
}

#[test]
fn a_locked_hidden_or_held_layer_refuses_as_a_drawing_would() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 10, 0.0, 0.0, 5.0, 5.0));
    f.session
        .apply(flare_commands::layers::set_locked(f.scene, f.layer, true))
        .expect("lock");
    let locked = Target::Layer(f.layer);
    assert_eq!(
        paste(&mut f, locked, 0, &[item(0.0)]),
        Err(PasteRefusal::Layer(DrawRefusal::Locked))
    );
    assert_eq!(
        paste(
            &mut f,
            Target::Layer(flare_core::LayerId::new()),
            0,
            &[item(0.0)]
        ),
        Err(PasteRefusal::Layer(DrawRefusal::NoSuchLayer))
    );
}
