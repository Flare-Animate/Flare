//! Drawing merged shapes into a layer: the same places and refusals as any drawing, with the
//! keyframe's merged drawing cut and joined in the one undo step.
#![allow(clippy::expect_used)]

use std::num::NonZeroU32;

use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, FrameSpan, Shape, Tween};

use super::*;
use crate::test_support::*;

const BLUE: Rgba8 = Rgba8::opaque(0, 0, 255);

fn merged(x: f64, fill: Rgba8) -> Shape {
    Shape::rect(ShapeId::new(), x, 0.0, 20.0, 20.0, fill)
        .expect("rect")
        .merged(true)
}

fn draw(f: &mut Fixture, frame: u32, shapes: Vec<Shape>) -> Result<DrawPlan, DrawRefusal> {
    let plan = draw_shapes(
        f.session.document(),
        f.scene,
        Target::Layer(f.layer),
        frame,
        None,
        shapes,
        "Layer 1",
    )?;
    f.session.apply(plan.commands.clone()).expect("apply");
    Ok(plan)
}

fn span_at(f: &Fixture, frame: u32) -> FrameSpan {
    f.session.document().scenes()[0].timeline().layers()[0]
        .span_at(frame)
        .expect("a keyframe")
        .clone()
}

#[test]
fn a_merged_fill_of_the_same_colour_joins_the_one_there_in_one_undo_step() {
    let mut f = fixture();
    draw(&mut f, 0, vec![merged(0.0, RED)]).expect("first");
    let plan = draw(&mut f, 0, vec![merged(10.0, RED)]).expect("second");
    assert_eq!(span_at(&f, 0).element_count(), 1, "one fill");
    assert_eq!(plan.placed, vec![element(&f, 0, 0)]);
    f.session.undo();
    assert_eq!(
        span_at(&f, 0).element_count(),
        1,
        "the first square, as it was"
    );
    let Some(Content::Shape(first)) = span_at(&f, 0).element(0).cloned() else {
        panic!("a shape");
    };
    assert_eq!(first.path().len(), 5, "untouched by the undone join");
}

#[test]
fn a_merged_fill_of_another_colour_cuts_and_lands_beneath_the_drawing_objects() {
    let mut f = fixture();
    draw(&mut f, 0, vec![merged(0.0, RED)]).expect("merged");
    let object = Shape::rect(ShapeId::new(), 40.0, 0.0, 20.0, 20.0, RED).expect("object");
    draw(&mut f, 0, vec![object.clone()]).expect("object");
    let plan = draw(&mut f, 0, vec![merged(10.0, BLUE)]).expect("cut");
    let span = span_at(&f, 0);
    assert_eq!(span.element_count(), 3);
    assert_eq!(plan.placed, vec![element(&f, 0, 1)], "above the red piece");
    assert_eq!(
        span.element(2),
        Some(&Content::Shape(object)),
        "on top, whole"
    );
}

#[test]
fn drawing_on_a_held_frame_merges_with_the_copy_and_leaves_the_frames_before() {
    let mut f = fixture();
    let first = FrameSpan::new(0, NonZeroU32::new(10).expect("ten"), Content::Empty);
    f.session
        .apply(vec![flare_commands::Command::InsertSpan {
            scene: f.scene,
            layer: f.layer,
            span: first,
        }])
        .expect("keyframe");
    draw(&mut f, 0, vec![merged(0.0, RED)]).expect("first");
    let plan = draw(&mut f, 5, vec![merged(10.0, RED)]).expect("later");
    assert_eq!(span_at(&f, 0).element_count(), 1);
    assert_eq!(span_at(&f, 5).start(), 5, "a keyframe of its own");
    assert_eq!(span_at(&f, 5).element_count(), 1, "joined with the copy");
    assert_eq!(plan.placed, vec![element(&f, 5, 0)], "the joined fill");
}

#[test]
fn a_drawing_object_in_a_batch_with_merged_shapes_stays_an_object() {
    let mut f = fixture();
    draw(&mut f, 0, vec![merged(0.0, RED)]).expect("first");
    let object = Shape::rect(ShapeId::new(), 10.0, 0.0, 20.0, 20.0, RED).expect("object");
    let plan = draw(&mut f, 0, vec![object.clone(), merged(10.0, RED)]).expect("mixed");
    let span = span_at(&f, 0);
    assert_eq!(span.element_count(), 2, "one joined fill, one object");
    assert_eq!(
        span.element(1),
        Some(&Content::Shape(object)),
        "whole, on top"
    );
    assert_eq!(plan.placed, vec![element(&f, 0, 0), element(&f, 0, 1)]);
}

#[test]
fn a_tweened_keyframe_is_refused_as_for_any_drawing() {
    let mut f = fixture();
    draw(&mut f, 0, vec![merged(0.0, RED)]).expect("first");
    let tweened = span_at(&f, 0).with_tween(Tween::Shape);
    f.session
        .apply(vec![
            flare_commands::Command::RemoveSpan {
                scene: f.scene,
                layer: f.layer,
                start: 0,
            },
            flare_commands::Command::InsertSpan {
                scene: f.scene,
                layer: f.layer,
                span: tweened,
            },
        ])
        .expect("tween");
    assert_eq!(
        draw(&mut f, 0, vec![merged(10.0, RED)]),
        Err(DrawRefusal::Held)
    );
}
