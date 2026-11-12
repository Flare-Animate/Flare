#![allow(clippy::expect_used)]

use super::*;
use crate::drawing::Target;
use crate::test_support::*;
use flare_document::PathSegment;

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn square() -> Vec<area::Outline> {
    vec![vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)]]
}

fn elements(f: &Fixture) -> Vec<Content> {
    find_span(f.session.document(), f.scene, at(f, 0))
        .expect("span")
        .elements()
        .cloned()
        .collect()
}

#[test]
fn an_area_becomes_a_closed_filled_shape_on_the_stage() {
    let shape = area_shape(
        &square(),
        |(x, y)| (x * 2.0 + 1.0, y * 2.0),
        Fill::Solid(BLUE),
    )
    .expect("shape");
    assert_eq!(shape.fill(), BLUE);
    assert_eq!(shape.path().len(), 5, "a move, three lines and a close");
    assert!(matches!(shape.path()[4], PathSegment::Close));
    let gradient = Fill::Linear {
        start: BLUE,
        end: RED,
        vertical: false,
    };
    let shaded = area_shape(&square(), |p| p, gradient).expect("shape");
    let g = shaded.gradient().expect("gradient");
    assert_eq!((g.from().as_vec().x, g.to().as_vec().x), (0.0, 10.0));
    assert!(area_shape(&square(), |_| (f64::NAN, 0.0), Fill::Solid(BLUE)).is_none());
    assert!(area_shape(&[], |p| p, Fill::Solid(BLUE)).is_none());
}

#[test]
fn an_area_over_a_shape_goes_just_above_it() {
    let mut f = rect_fixture();
    add_extra(&mut f);
    let shape = area_shape(&square(), |p| p, Fill::Solid(BLUE)).expect("shape");
    let id = shape.id();
    let commands = pour_above(f.session.document(), f.scene, at(&f, 0), shape).expect("poured");
    f.session.apply(commands).expect("apply");
    let all = elements(&f);
    assert_eq!(all.len(), 3);
    assert!(
        matches!(&all[1], Content::Shape(s) if s.id() == id),
        "between the two"
    );
    let missing = SpanRef {
        element: 9,
        ..at(&f, 0)
    };
    let again = area_shape(&square(), |p| p, Fill::Solid(BLUE)).expect("shape");
    assert_eq!(
        pour_above(f.session.document(), f.scene, missing, again),
        None
    );
}

#[test]
fn an_area_on_empty_stage_goes_under_everything_in_the_layer() {
    let mut f = rect_fixture();
    let shape = area_shape(&square(), |p| p, Fill::Solid(BLUE)).expect("shape");
    let id = shape.id();
    let plan = pour_under(
        f.session.document(),
        f.scene,
        Target::Layer(f.layer),
        0,
        shape,
        "Layer",
    )
    .expect("poured");
    assert_eq!(plan.placed, vec![at(&f, 0)]);
    f.session.apply(plan.commands).expect("apply");
    let all = elements(&f);
    assert_eq!(all.len(), 2);
    assert!(
        matches!(&all[0], Content::Shape(s) if s.id() == id),
        "at the bottom"
    );
}

/// A second rectangle in the fixture's keyframe, over the first.
fn add_extra(f: &mut Fixture) {
    let other = flare_document::Shape::rect(flare_core::ShapeId::new(), 2.0, 2.0, 3.0, 3.0, RED)
        .expect("rect");
    let plan = crate::drawing::draw_shapes(
        f.session.document(),
        f.scene,
        Target::Layer(f.layer),
        0,
        None,
        vec![other],
        "Layer",
    )
    .expect("drawn");
    f.session.apply(plan.commands).expect("apply");
}
