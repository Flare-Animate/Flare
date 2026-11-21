//! A drawing holds across the frames after it: drawing on one of them starts a keyframe there
//! with the held drawing kept and the new strokes beside it, and the frames before are as they
//! were.
#![allow(clippy::expect_used)]

use flare_core::ShapeId;
use flare_document::{Content, Shape, Tween};

use super::*;
use crate::test_support::*;

fn shape(x: f64) -> Shape {
    Shape::rect(ShapeId::new(), x, 0.0, 10.0, 10.0, RED).expect("rect")
}

fn draw(f: &mut Fixture, frame: u32, x: f64) -> DrawPlan {
    let plan = draw_shapes(
        f.session.document(),
        f.scene,
        Target::Layer(f.layer),
        frame,
        None,
        vec![shape(x)],
        "Layer 1",
    )
    .expect("draw");
    f.session.apply(plan.commands.clone()).expect("apply");
    plan
}

fn spans(f: &Fixture) -> Vec<(u32, u32, usize)> {
    f.session.document().scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.element_count()))
        .collect()
}

fn holding(f: &mut Fixture) {
    f.session
        .apply([flare_commands::Command::UpdateLayer {
            scene: f.scene,
            layer: f.layer,
            change: flare_commands::LayerChange::Hold(true),
        }])
        .expect("hold");
}

#[test]
fn drawing_on_a_later_frame_keeps_the_held_drawing_and_adds_beside_it() {
    let mut f = fixture();
    holding(&mut f);
    draw(&mut f, 0, 0.0);
    // Frame 4 shows the held drawing; a stroke there starts a keyframe of its own.
    let plan = draw(&mut f, 4, 30.0);
    assert_eq!(
        spans(&f),
        vec![(0, 4, 1), (4, 1, 2)],
        "frames 0 to 3 are as they were; frame 4 has the held shape and the new one"
    );
    assert_eq!(plan.placed.len(), 1, "only the new stroke is placed");
    assert_eq!(plan.placed[0].element, 1);
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    let first = layer.span_at(0).expect("first");
    let second = layer.span_at(4).expect("second");
    let (Content::Shape(a), Content::Shape(b)) = (first.content(), second.content()) else {
        panic!("shapes expected");
    };
    assert_ne!(
        a.id(),
        b.id(),
        "the copy is its own element, not the same identity"
    );
    assert_eq!(a.path(), b.path(), "and the same artwork");
    // The next frames still show it: nothing is drawn there, so the new keyframe is held.
    assert_eq!(layer.span_at(9).expect("held").start(), 4);
}

#[test]
fn one_undo_takes_the_copy_on_write_back() {
    let mut f = fixture();
    holding(&mut f);
    draw(&mut f, 0, 0.0);
    let before = spans(&f);
    draw(&mut f, 3, 30.0);
    assert_ne!(spans(&f), before);
    f.session.undo();
    assert_eq!(spans(&f), before);
}

#[test]
fn drawing_inside_a_longer_hold_splits_it_and_keeps_the_tail() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 10, 0.0, 0.0, 5.0, 5.0));
    draw(&mut f, 6, 30.0);
    assert_eq!(spans(&f), vec![(0, 6, 1), (6, 4, 2)]);
}

#[test]
fn a_tween_cannot_share_itself_so_drawing_inside_it_is_refused() {
    let mut f = fixture();
    let tweened = rect_span(0, 10, 0.0, 0.0, 5.0, 5.0).with_tween(Tween::Linear);
    add(&mut f, tweened);
    let refused = draw_shapes(
        f.session.document(),
        f.scene,
        Target::Layer(f.layer),
        5,
        None,
        vec![shape(30.0)],
        "Layer 1",
    );
    assert_eq!(refused.expect_err("refused"), DrawRefusal::Held);
}

#[test]
fn a_new_layer_made_by_drawing_holds_and_a_blank_keyframe_ends_the_hold() {
    let mut f = fixture();
    f.session
        .apply([flare_commands::Command::RemoveLayer {
            scene: f.scene,
            layer: f.layer,
        }])
        .expect("remove");
    let plan = draw_shapes(
        f.session.document(),
        f.scene,
        Target::Default,
        0,
        None,
        vec![shape(0.0)],
        "Layer 1",
    )
    .expect("draw");
    f.session.apply(plan.commands).expect("apply");
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    assert!(layer.is_hold());
    assert!(layer.span_at(50).is_some(), "still on show long after");

    let blank = flare_document::FrameSpan::new(5, NonZeroU32::MIN, Content::Empty);
    let ended = layer
        .spans()
        .last()
        .expect("span")
        .retimed(0, NonZeroU32::new(5).expect("nz"));
    let layer = layer.with_spans(vec![ended, blank]).expect("layer");
    assert!(
        layer.span_at(4).is_some(),
        "the drawing shows before the blank keyframe"
    );
    assert!(
        layer.span_at(50).is_none(),
        "after a blank keyframe nothing is held: it is how a drawing is made to stop"
    );
}

#[test]
fn a_layer_that_does_not_hold_shows_nothing_past_its_last_keyframe() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 3, 0.0, 0.0, 5.0, 5.0));
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    assert!(!layer.is_hold());
    assert!(layer.span_at(2).is_some());
    assert!(layer.span_at(3).is_none(), "as Animate's layers end");
}
