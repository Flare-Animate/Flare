#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{InstanceId, ShapeId, SymbolId, Transform2D};

use super::*;
use crate::{Camera, Instance, Shape, Stage};

fn span(content: Content) -> FrameSpan {
    FrameSpan::new(0, NonZeroU32::new(24).expect("nonzero"), content)
}

fn shape(x: f64) -> Content {
    let fill = flare_core::Rgba8::opaque(200, 0, 0);
    Content::Shape(
        Shape::rounded_rect(ShapeId::new(), x, 0.0, 10.0, 10.0, 0.0, fill).expect("shape"),
    )
}

fn instance() -> Content {
    Content::Instance(Instance::new(
        InstanceId::new(),
        SymbolId::new(),
        Transform2D::IDENTITY,
    ))
}

#[test]
fn a_span_holds_one_element_unless_given_more() {
    let one = span(shape(0.0));
    assert_eq!(one.element_count(), 1);
    assert!(one.extras().is_empty());
    assert_eq!(one.elements().count(), 1);
}

#[test]
fn extras_are_drawn_above_the_content_in_order() {
    let (a, b, c) = (shape(0.0), shape(20.0), instance());
    let many = span(a.clone())
        .with_extras(vec![b.clone(), c.clone()])
        .expect("several");
    assert_eq!(many.element_count(), 3);
    assert_eq!(many.elements().collect::<Vec<_>>(), vec![&a, &b, &c]);
    assert_eq!(many.element(0), Some(&a));
    assert_eq!(many.element(2), Some(&c));
    assert_eq!(many.element(3), None);
    assert_eq!(many.content(), &a, "content() is still the first element");
}

#[test]
fn only_shapes_and_instances_can_be_several() {
    let camera = Content::Camera(Camera::centered(
        &Stage::new(100, 100, flare_core::Rgba8::opaque(0, 0, 0)).expect("stage"),
    ));
    for (first, more) in [
        (Content::Empty, vec![shape(0.0)]),
        (shape(0.0), vec![Content::Empty]),
        (shape(0.0), vec![camera.clone()]),
        (camera, vec![shape(0.0)]),
    ] {
        assert_eq!(
            span(first).with_extras(more),
            Err(TimelineError::NotAnElement)
        );
    }
}

#[test]
fn a_tween_is_the_first_elements_and_survives_having_others() {
    let many = span(instance())
        .with_tween(Tween::Linear)
        .with_extras(vec![shape(0.0), instance()])
        .expect("several");
    assert_eq!(many.tween(), Tween::Linear);
    assert_eq!(many.element_count(), 3);
    // Taking an element away keeps it while the first is still an instance...
    let fewer = many.without_element(2).expect("one fewer");
    assert_eq!(fewer.tween(), Tween::Linear);
    // ...but not when the first is gone and a shape takes its place.
    let swapped = many.without_element(0).expect("first gone");
    assert_eq!(swapped.tween(), Tween::None);
    assert_eq!(swapped.ease(), 0);
}

#[test]
fn the_number_of_elements_is_bounded() {
    let at_limit = vec![shape(0.0); MAX_ELEMENTS - 1];
    assert!(span(shape(0.0)).with_extras(at_limit).is_ok());
    let over = vec![shape(0.0); MAX_ELEMENTS];
    assert_eq!(
        span(shape(0.0)).with_extras(over),
        Err(TimelineError::TooManyElements(MAX_ELEMENTS + 1))
    );
}

#[test]
fn no_extras_is_a_plain_span_and_equal_to_one() {
    let plain = span(shape(0.0));
    let cleared = plain.clone().with_extras(Vec::new()).expect("none");
    assert_eq!(plain, cleared);
}

#[test]
fn replacing_removing_and_appending_elements_rebuild_the_span() {
    let (a, b, c) = (shape(0.0), shape(20.0), instance());
    let many = span(a.clone()).with_extras(vec![b.clone()]).expect("two");
    let replaced = many.with_element(1, c.clone()).expect("replaced");
    assert_eq!(
        replaced.elements().cloned().collect::<Vec<_>>(),
        vec![a.clone(), c.clone()]
    );
    assert!(many.with_element(2, c.clone()).is_none());
    // A camera cannot be one of several.
    let camera = Content::Camera(
        Camera::new(
            flare_core::DocumentPoint::new(0.0, 0.0).expect("point"),
            1.0,
            0.0,
        )
        .expect("camera"),
    );
    assert!(many.with_element(1, camera).is_none());

    let without_first = many.without_element(0).expect("one left");
    assert_eq!(without_first.element_count(), 1);
    assert_eq!(without_first.content(), &b);
    assert!(without_first.without_element(0).is_none());
    assert!(many.without_element(5).is_none());

    let appended = span(a.clone()).with_appended(c.clone()).expect("appended");
    assert_eq!(
        appended.elements().cloned().collect::<Vec<_>>(),
        vec![a.clone(), c]
    );
    let from_empty = span(Content::Empty)
        .with_appended(b.clone())
        .expect("filled");
    assert_eq!(from_empty.element_count(), 1);
    assert_eq!(from_empty.content(), &b);
}

#[test]
fn a_tweening_span_refuses_another_element_and_stacking_moves_one() {
    let tweening = span(instance()).with_tween(Tween::Linear);
    assert!(tweening.with_appended(shape(0.0)).is_none());

    let (a, b, c) = (shape(0.0), shape(20.0), shape(40.0));
    let many = span(a.clone())
        .with_extras(vec![b.clone(), c.clone()])
        .expect("three");
    let moved = many.with_element_moved(0, 2).expect("moved");
    assert_eq!(moved.elements().cloned().collect::<Vec<_>>(), vec![b, c, a]);
    assert!(many.with_element_moved(0, 3).is_none());
}

#[test]
fn validation_looks_at_every_element_not_only_the_first() {
    use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
    let build = |spans: Vec<FrameSpan>, symbols: Vec<crate::Symbol>| {
        let layer = crate::Layer::new(LayerId::new(), "L", spans).expect("layer");
        let scene = crate::Scene::new(SceneId::new(), "S", crate::Timeline::new(vec![layer]));
        crate::Document::new(
            DocumentId::new(),
            FrameRate::new(24, 1).expect("rate"),
            vec![scene],
            symbols,
        )
    };
    // An extra that places a symbol the document lacks.
    let dangling = span(shape(0.0))
        .with_extras(vec![instance()])
        .expect("extras");
    let errors = build(vec![dangling], vec![]).expect_err("dangling");
    assert!(matches!(
        errors[..],
        [crate::Diagnostic::DanglingSymbol { .. }]
    ));
    // An extra that reuses the identity of the first element's shape.
    let Content::Shape(first) = shape(0.0) else {
        panic!("shape");
    };
    let twin = Content::Shape(first.clone());
    let doubled = span(Content::Shape(first))
        .with_extras(vec![twin])
        .expect("extras");
    let errors = build(vec![doubled], vec![]).expect_err("duplicate");
    assert!(matches!(errors[..], [crate::Diagnostic::DuplicateShape(_)]));
}
