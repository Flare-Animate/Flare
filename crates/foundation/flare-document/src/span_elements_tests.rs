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
fn a_tweening_span_cannot_take_extras_and_one_with_extras_cannot_tween() {
    let tweening = span(instance()).with_tween(Tween::Linear);
    assert_eq!(
        tweening.with_extras(vec![instance()]),
        Err(TimelineError::ManyElementsTween)
    );
    let many = span(instance())
        .with_extras(vec![instance()])
        .expect("several")
        .with_tween(Tween::Linear);
    assert_eq!(many.tween(), Tween::None, "the request is ignored");
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
