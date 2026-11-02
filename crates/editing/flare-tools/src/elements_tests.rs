//! Acting on one element of a keyframe that holds several.
#![allow(clippy::expect_used)]

use crate::ops::*;
use crate::test_support::*;
use emath::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{Rgba8, ShapeId};
use flare_document::{Content, Tween};
use glamx::{DAffine2, DVec2};

fn lefts(f: &Fixture) -> Vec<f32> {
    let span = find_span(f.session.document(), f.scene, at(f, 0)).expect("span");
    (0..span.element_count())
        .map(|i| {
            span_bounds(f.session.document(), f.scene, element(f, 0, i))
                .expect("bounds")
                .left()
        })
        .collect()
}

#[test]
fn a_hit_and_select_all_name_each_element_of_the_keyframe() {
    let mut f = fixture();
    add_three(&mut f);
    let doc = f.session.document();
    assert_eq!(spans_at_frame(doc, f.scene, 2).len(), 3);
    let frame = flare_eval::evaluate_frame(doc, f.scene, 2).expect("frame");
    let found: Vec<usize> = frame
        .shapes
        .iter()
        .map(|d| locate_hit(doc, f.scene, 2, d).expect("located").element)
        .collect();
    assert_eq!(found, vec![0, 1, 2]);
    assert_eq!(
        span_bounds(doc, f.scene, element(&f, 0, 2))
            .expect("b")
            .left(),
        80.0
    );
    assert!(span_bounds(doc, f.scene, element(&f, 0, 3)).is_none());
}

#[test]
fn a_lasso_picks_elements_by_their_own_centres() {
    let mut f = fixture();
    add_three(&mut f);
    let around_middle = [
        Pos2::new(30.0, -5.0),
        Pos2::new(60.0, -5.0),
        Pos2::new(60.0, 20.0),
        Pos2::new(30.0, 20.0),
    ];
    let found = spans_in_polygon(f.session.document(), f.scene, 2, &around_middle);
    assert_eq!(found, vec![element(&f, 0, 1)]);
}

#[test]
fn deleting_an_element_leaves_the_others_and_the_last_one_takes_the_span() {
    let mut f = fixture();
    add_three(&mut f);
    let commands = delete(f.session.document(), f.scene, element(&f, 0, 1));
    f.session.apply(commands).expect("delete");
    assert_eq!(lefts(&f), vec![0.0, 80.0]);
    f.session.undo();
    assert_eq!(lefts(&f), vec![0.0, 40.0, 80.0], "one undo step");

    let both = [element(&f, 0, 0), element(&f, 0, 2)];
    f.session
        .apply(delete_many(f.session.document(), f.scene, &both))
        .expect("delete both");
    assert_eq!(lefts(&f), vec![40.0]);
    let last = delete(f.session.document(), f.scene, element(&f, 0, 0));
    f.session.apply(last).expect("delete last");
    assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
    assert!(delete(f.session.document(), f.scene, element(&f, 0, 0)).is_empty());
}

#[test]
fn moving_one_element_moves_only_that_one_and_several_do_not_undo_each_other() {
    let mut f = fixture();
    add_three(&mut f);
    let moved = translate(
        f.session.document(),
        f.scene,
        element(&f, 0, 1),
        DVec2::new(5.0, 0.0),
    )
    .expect("move");
    f.session.apply(moved).expect("apply");
    assert_eq!(lefts(&f), vec![0.0, 45.0, 80.0]);

    let two = [element(&f, 0, 0), element(&f, 0, 2)];
    let moved = translate_many(f.session.document(), f.scene, &two, DVec2::new(0.0, 7.0))
        .expect("move two");
    f.session.apply(moved).expect("apply");
    let tops: Vec<f32> = (0..3)
        .map(|i| {
            span_bounds(f.session.document(), f.scene, element(&f, 0, i))
                .expect("b")
                .top()
        })
        .collect();
    assert_eq!(tops, vec![7.0, 0.0, 7.0]);
    assert!(translate(f.session.document(), f.scene, element(&f, 0, 9), DVec2::X).is_none());
}

#[test]
fn a_command_for_each_selected_element_applies_in_order() {
    let mut f = fixture();
    add_three(&mut f);
    let all = [element(&f, 0, 0), element(&f, 0, 1), element(&f, 0, 2)];
    let painted = each_element(f.session.document(), &all, |doc, at| {
        restyle(doc, f.scene, at, Some(Rgba8::opaque(1, 2, 3)), None)
    })
    .expect("restyle each");
    f.session.apply(painted).expect("apply");
    for i in 0..3 {
        let Content::Shape(shape) =
            find_element(f.session.document(), f.scene, element(&f, 0, i)).expect("element")
        else {
            panic!("shape");
        };
        assert_eq!(shape.fill(), Rgba8::opaque(1, 2, 3), "element {i}");
    }
    // Deleting them one by one still works because the highest index goes first.
    let gone = each_element(f.session.document(), &all, |doc, at| {
        Some(delete(doc, f.scene, at))
    })
    .expect("delete each");
    f.session.apply(gone).expect("apply");
    assert!(find_span(f.session.document(), f.scene, at(&f, 0)).is_none());
}

#[test]
fn converting_and_grouping_act_on_the_chosen_elements_only() {
    let mut f = fixture();
    add_three(&mut f);
    let (commands, symbol) =
        convert_to_symbol(f.session.document(), f.scene, element(&f, 0, 1)).expect("convert");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!(span.element_count(), 3);
    assert!(matches!(span.element(1), Some(Content::Instance(i)) if i.symbol() == symbol));
    assert!(matches!(span.element(0), Some(Content::Shape(_))));

    let pair = [element(&f, 0, 0), element(&f, 0, 2)];
    let (commands, group_symbol) = group(f.session.document(), f.scene, &pair).expect("group");
    f.session.apply(commands).expect("apply");
    let span = find_span(f.session.document(), f.scene, at(&f, 0)).expect("span");
    assert_eq!(
        span.element_count(),
        2,
        "the two shapes became one instance"
    );
    // The group stands where its topmost element stood.
    assert!(matches!(span.element(1), Some(Content::Instance(i)) if i.symbol() == group_symbol));
    // An instance among the chosen ones cannot be grouped.
    let mixed = [element(&f, 0, 0), element(&f, 0, 1)];
    assert!(group(f.session.document(), f.scene, &mixed).is_none());
}

#[test]
fn a_duplicate_joins_the_keyframe_and_a_tweening_keyframe_gets_a_layer() {
    let mut f = fixture();
    add_three(&mut f);
    let (commands, copy) = duplicate(
        f.session.document(),
        f.scene,
        element(&f, 0, 0),
        DVec2::new(0.0, 30.0),
    )
    .expect("duplicate");
    f.session.apply(commands).expect("apply");
    assert_eq!(copy.element, 3);
    assert_eq!(lefts(&f).len(), 4);
    let ids: std::collections::HashSet<ShapeId> = (0..4)
        .filter_map(
            |i| match find_element(f.session.document(), f.scene, element(&f, 0, i)) {
                Some(Content::Shape(s)) => Some(s.id()),
                _ => None,
            },
        )
        .collect();
    assert_eq!(ids.len(), 4, "the copy has an identity of its own");

    let mut g = fixture();
    add(
        &mut g,
        rect_span(0, 6, 0.0, 0.0, 10.0, 10.0).with_tween(Tween::Shape),
    );
    let (commands, copy) = duplicate(
        g.session.document(),
        g.scene,
        at(&g, 0),
        DVec2::new(5.0, 0.0),
    )
    .expect("copy");
    g.session.apply(commands).expect("apply");
    assert_ne!(
        copy.layer, g.layer,
        "a tween holds one object, so the copy has a layer"
    );
}

#[test]
fn a_keyframe_of_several_elements_takes_a_tween_on_its_first_and_copies_one_element() {
    let mut f = fixture();
    add_three(&mut f);
    // The first element is a shape, so a shape tween fits; a motion tween does not.
    assert!(set_tween(f.session.document(), f.scene, at(&f, 0), Tween::Linear).is_none());
    let commands = set_tween(f.session.document(), f.scene, at(&f, 0), Tween::Shape)
        .expect("a shape tween on the first element");
    f.session.apply(commands).expect("apply");
    let span = &f.session.document().scenes()[0].timeline().layers()[0].spans()[0];
    assert_eq!(span.tween(), Tween::Shape);
    assert_eq!(span.element_count(), 3, "the others are kept, held");
    f.session.undo();
    let item = copy_span(f.session.document(), f.scene, element(&f, 0, 2)).expect("copy");
    let (commands, pasted) = paste(f.session.document(), f.scene, &item, 10).expect("paste");
    f.session.apply(commands).expect("apply");
    assert_eq!(
        span_bounds(f.session.document(), f.scene, pasted)
            .expect("b")
            .left(),
        80.0
    );
}

#[test]
fn breaking_an_instance_apart_puts_its_parts_in_its_own_place_in_the_stack() {
    let mut f = fixture();
    add_three(&mut f);
    let pair = [element(&f, 0, 0), element(&f, 0, 1)];
    let (commands, _) = group(f.session.document(), f.scene, &pair).expect("group");
    f.session.apply(commands).expect("apply");
    // Now: the group at 0 and the plain shape (x 80) at 1.
    let (commands, parts) =
        break_apart(f.session.document(), f.scene, element(&f, 0, 0)).expect("break");
    f.session.apply(commands).expect("apply");
    assert_eq!(parts.len(), 2);
    assert_eq!(
        lefts(&f),
        vec![0.0, 40.0, 80.0],
        "back where they were, in order"
    );
}

#[test]
fn path_edits_and_paint_reach_the_named_element() {
    let mut f = fixture();
    add_three(&mut f);
    let target = element(&f, 0, 2);
    let near = DVec2::new(85.0, 0.0);
    let added = add_anchor(f.session.document(), f.scene, target, near, 2.0).expect("anchor");
    f.session.apply(added).expect("apply");
    let count = |i: usize| match find_element(f.session.document(), f.scene, element(&f, 0, i)) {
        Some(Content::Shape(s)) => s.path().len(),
        _ => 0,
    };
    assert!(count(2) > count(0));
    let restyled = restyle(
        f.session.document(),
        f.scene,
        element(&f, 0, 1),
        Some(Rgba8::WHITE),
        None,
    )
    .expect("paint");
    f.session.apply(restyled).expect("apply");
    let fill = |i: usize| match find_element(f.session.document(), f.scene, element(&f, 0, i)) {
        Some(Content::Shape(s)) => s.fill(),
        _ => Rgba8::TRANSPARENT,
    };
    assert_eq!(fill(1), Rgba8::WHITE);
    assert_eq!(fill(0), RED);
    let _ = (
        Command::RemoveSpan {
            scene: f.scene,
            layer: f.layer,
            start: 0,
        },
        Rect::NOTHING,
        DAffine2::IDENTITY,
    );
}
