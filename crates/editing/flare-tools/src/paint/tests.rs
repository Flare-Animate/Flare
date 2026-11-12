#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_core::ShapeId;

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn shape_at(f: &Fixture) -> Shape {
    match find_element(f.session.document(), f.scene, at(f, 0)) {
        Some(Content::Shape(shape)) => shape.clone(),
        other => panic!("shape expected, got {other:?}"),
    }
}

fn pour(f: &mut Fixture, paint: Fill) -> bool {
    match fill(f.session.document(), f.scene, at(f, 0), paint) {
        Some(commands) => {
            f.session.apply(commands).expect("fill");
            true
        }
        None => false,
    }
}

#[test]
fn a_solid_fill_replaces_the_color_and_a_repeat_changes_nothing() {
    let mut f = rect_fixture();
    assert!(pour(&mut f, Fill::Solid(BLUE)));
    assert_eq!(shape_at(&f).fill(), BLUE);
    assert!(!pour(&mut f, Fill::Solid(BLUE)), "already that color");
}

#[test]
fn a_gradient_fill_spans_the_outline_across_or_down() {
    let mut f = fixture();
    let stroke = Stroke::new(4.0, RED).expect("stroke");
    let shape = Shape::rect(ShapeId::new(), 10.0, 20.0, 100.0, 50.0, RED)
        .expect("rect")
        .with_stroke(stroke);
    add(
        &mut f,
        flare_document::FrameSpan::new(
            0,
            std::num::NonZeroU32::new(1).expect("nonzero"),
            Content::Shape(shape),
        ),
    );
    let across = Fill::Linear {
        start: BLUE,
        end: RED,
        vertical: false,
    };
    assert!(pour(&mut f, across));
    let painted = shape_at(&f);
    let gradient = painted.gradient().expect("gradient");
    let ends = |g: Gradient| (g.from().as_vec(), g.to().as_vec());
    let (from, to) = ends(gradient);
    assert_eq!((from.x, from.y, to.x, to.y), (10.0, 20.0, 110.0, 20.0));
    assert_eq!((gradient.start(), gradient.end()), (BLUE, RED));
    assert!(!runs_vertically(&gradient));
    assert_eq!(painted.stroke(), Some(stroke), "the outline stays");
    assert!(
        !pour(&mut f, across),
        "the same gradient again changes nothing"
    );
    let down = Fill::Linear {
        start: BLUE,
        end: RED,
        vertical: true,
    };
    assert!(pour(&mut f, down));
    let gradient = shape_at(&f).gradient().expect("gradient");
    let (from, to) = ends(gradient);
    assert_eq!((from.x, from.y, to.x, to.y), (10.0, 20.0, 10.0, 70.0));
    assert!(runs_vertically(&gradient));
    // A solid fill afterwards drops the gradient.
    assert!(pour(&mut f, Fill::Solid(RED)));
    assert_eq!(shape_at(&f).gradient(), None);
}

#[test]
fn a_gradient_over_a_shape_with_no_width_falls_back_to_its_start_color() {
    let mut f = fixture();
    let line = Shape::line(
        ShapeId::new(),
        (5.0, 0.0),
        (5.0, 40.0),
        Stroke::new(1.0, RED).expect("stroke"),
    )
    .expect("line");
    add(
        &mut f,
        flare_document::FrameSpan::new(
            0,
            std::num::NonZeroU32::new(1).expect("nonzero"),
            Content::Shape(line),
        ),
    );
    let paint = Fill::Linear {
        start: BLUE,
        end: RED,
        vertical: false,
    };
    assert!(pour(&mut f, paint));
    let painted = shape_at(&f);
    assert_eq!((painted.fill(), painted.gradient()), (BLUE, None));
}

#[test]
fn filling_what_is_not_a_shape_or_not_there_is_refused() {
    let f = fixture();
    assert_eq!(
        fill(f.session.document(), f.scene, at(&f, 0), Fill::Solid(BLUE)),
        None
    );
}

#[test]
fn the_eyedropper_takes_the_stroke_on_the_line_and_the_fill_inside() {
    let stroke = Stroke::new(3.0, BLUE).expect("stroke");
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, RED)
        .expect("rect")
        .with_stroke(stroke);
    assert_eq!(
        sample(&shape, true, true, false),
        Some(Sample::Stroke(stroke))
    );
    assert_eq!(
        sample(&shape, false, true, false),
        Some(Sample::Fill(RED, None))
    );
    assert_eq!(
        sample(&shape, false, true, true),
        Some(Sample::Stroke(stroke)),
        "Shift takes the stroke wherever the shape is clicked"
    );
    assert_eq!(sample(&shape, false, false, false), None);
}

#[test]
fn the_eyedropper_has_nothing_to_take_from_an_empty_inside_or_a_missing_stroke() {
    let outline = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, Rgba8::TRANSPARENT)
        .expect("rect")
        .with_stroke(Stroke::new(1.0, BLUE).expect("stroke"));
    // Inside an outline with no fill the click is not on a fill that shows.
    assert_eq!(sample(&outline, false, false, false), None);
    let plain = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, RED).expect("rect");
    assert_eq!(sample(&plain, false, true, true), None, "no stroke to take");
}

#[test]
fn the_eyedropper_takes_a_gradient_whole() {
    let from = DocumentPoint::new(0.0, 0.0).expect("finite");
    let to = DocumentPoint::new(0.0, 10.0).expect("finite");
    let gradient = Gradient::linear(from, to, RED, BLUE).expect("gradient");
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, RED)
        .expect("rect")
        .with_gradient(gradient);
    assert_eq!(
        sample(&shape, false, true, false),
        Some(Sample::Fill(RED, Some(gradient)))
    );
    assert!(runs_vertically(&gradient));
}
