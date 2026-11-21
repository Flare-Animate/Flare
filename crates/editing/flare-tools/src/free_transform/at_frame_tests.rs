//! Moving what a held drawing shows starts a keyframe there; the frames before keep their drawing.
#![allow(clippy::expect_used)]

use flare_commands::{Command, LayerChange};
use glamx::{DAffine2, DVec2};

use crate::ops::*;
use crate::test_support::*;

fn holding(f: &mut Fixture) {
    f.session
        .apply([Command::UpdateLayer {
            scene: f.scene,
            layer: f.layer,
            change: LayerChange::Hold(true),
        }])
        .expect("hold");
}

fn spans(f: &Fixture) -> Vec<(u32, u32, usize)> {
    f.session.document().scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .map(|s| (s.start(), s.duration().get(), s.element_count()))
        .collect()
}

#[test]
fn moving_on_a_held_frame_copies_the_drawing_to_a_keyframe_and_moves_the_copy() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    holding(&mut f);
    let before = bounds(&f, 0);
    let (commands, refs) = translate_many_at(
        f.session.document(),
        f.scene,
        &[at(&f, 0)],
        9,
        DVec2::new(30.0, 0.0),
    )
    .expect("move");
    assert_eq!(
        refs[0].start, 9,
        "the selection follows to the new keyframe"
    );
    f.session.apply(commands).expect("apply");
    let layer = &f.session.document().scenes()[0].timeline().layers()[0];
    assert_eq!(layer.spans().len(), 2);
    assert_eq!(layer.spans()[0].start(), 0);
    assert_eq!(
        layer.spans()[0].duration().get(),
        9,
        "frames 0 to 8 keep it"
    );
    assert_eq!(bounds(&f, 0), before, "earlier frames are unchanged");
    let moved = bounds(&f, 9);
    assert!((moved.min.x - before.min.x - 30.0).abs() < 1e-4);
    assert_eq!(layer.span_at(20).expect("held on").start(), 9);
}

#[test]
fn a_transform_on_a_held_frame_leaves_the_earlier_frames_and_is_one_undo() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    holding(&mut f);
    let before = spans(&f);
    let (commands, _) = transform_many_at(
        f.session.document(),
        f.scene,
        &[at(&f, 0)],
        6,
        DAffine2::from_scale(DVec2::new(2.0, 2.0)),
    )
    .expect("scale");
    f.session.apply(commands).expect("apply");
    assert_eq!(spans(&f).len(), 2);
    f.session.undo();
    assert_eq!(spans(&f), before, "one undo takes the whole copy back");
}

#[test]
fn on_a_frame_a_keyframe_covers_the_edit_is_the_plain_one() {
    let mut f = fixture();
    add(&mut f, rect_span(0, 10, 0.0, 0.0, 10.0, 10.0));
    holding(&mut f);
    let (commands, refs) = translate_many_at(
        f.session.document(),
        f.scene,
        &[at(&f, 0)],
        5,
        DVec2::new(3.0, 0.0),
    )
    .expect("move");
    assert_eq!(refs[0].start, 0);
    f.session.apply(commands).expect("apply");
    assert_eq!(spans(&f), vec![(0, 10, 1)]);
}

#[test]
fn a_move_by_nothing_changes_nothing_and_starts_no_keyframe() {
    let f = fixture();
    assert!(translate_many_at(f.session.document(), f.scene, &[], 4, DVec2::ZERO).is_none());
}
