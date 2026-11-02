#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_document::Content;

fn xs(f: &Fixture) -> Vec<f32> {
    let span = find_span(f.session.document(), f.scene, at(f, 0)).expect("span");
    (0..span.element_count())
        .map(|i| {
            span_bounds(f.session.document(), f.scene, element(f, 0, i))
                .expect("bounds")
                .left()
        })
        .collect()
}

use crate::ops::{find_span, span_bounds};

fn arrange(f: &mut Fixture, picks: &[usize], how: Stack) -> Option<Vec<SpanRef>> {
    let refs: Vec<SpanRef> = picks.iter().map(|i| element(f, 0, *i)).collect();
    let (commands, now) = arrange_elements(f.session.document(), f.scene, &refs, how)?;
    f.session.apply(commands).expect("apply");
    Some(now)
}

#[test]
fn front_and_back_move_the_chosen_to_the_ends_keeping_their_order() {
    let mut f = fixture();
    add_three(&mut f);
    let now = arrange(&mut f, &[0], Stack::Front).expect("front");
    assert_eq!(xs(&f), vec![40.0, 80.0, 0.0]);
    assert_eq!(now, vec![element(&f, 0, 2)]);
    arrange(&mut f, &[0, 1], Stack::Back)
        .is_none()
        .then_some(())
        .expect("already at the back");
    arrange(&mut f, &[1, 2], Stack::Back).expect("back");
    assert_eq!(xs(&f), vec![80.0, 0.0, 40.0]);
}

#[test]
fn forward_and_backward_pass_one_unchosen_element_and_stop_at_the_ends() {
    let mut f = fixture();
    add_three(&mut f);
    arrange(&mut f, &[0], Stack::Forward).expect("forward");
    assert_eq!(xs(&f), vec![40.0, 0.0, 80.0]);
    arrange(&mut f, &[2], Stack::Backward).expect("backward");
    assert_eq!(xs(&f), vec![40.0, 80.0, 0.0]);
    assert!(
        arrange(&mut f, &[2], Stack::Forward).is_none(),
        "already on top"
    );
    assert!(
        arrange(&mut f, &[0], Stack::Backward).is_none(),
        "already at the bottom"
    );
    assert!(
        arrange(&mut f, &[7], Stack::Front).is_none(),
        "no such element"
    );
}

#[test]
fn arranging_is_one_undo_step_and_never_loses_an_element() {
    let mut f = fixture();
    add_three(&mut f);
    arrange(&mut f, &[0, 2], Stack::Forward).expect("forward");
    assert_eq!(xs(&f).len(), 3);
    f.session.undo();
    assert_eq!(xs(&f), vec![0.0, 40.0, 80.0]);
    let _ = Content::Empty;
}
