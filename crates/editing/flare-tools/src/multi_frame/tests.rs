#![allow(clippy::expect_used)]

use super::*;
use crate::test_support::*;
use flare_commands::LayerChange;

/// Frames 0-3, 4-5 and 6-9 on the fixture's layer, the rectangle moving right; a second layer
/// holds one rectangle over frames 0-9.
fn walk() -> (Fixture, SpanRef) {
    let mut f = fixture();
    add(&mut f, rect_span(0, 4, 0.0, 0.0, 10.0, 10.0));
    add(&mut f, rect_span(4, 2, 20.0, 0.0, 10.0, 10.0));
    add(&mut f, rect_span(6, 4, 40.0, 0.0, 10.0, 10.0));
    let other = add_on_new_layer(&mut f, rect_span(0, 10, 0.0, 50.0, 10.0, 10.0));
    (f, other)
}

#[test]
fn the_frames_shown_are_where_each_keyframe_first_shows_in_the_range() {
    let (f, _) = walk();
    let doc = f.session.document();
    assert_eq!(frames_shown(doc, f.scene, 0, 9), vec![0, 4, 6]);
    // A keyframe that began before the range shows at the range's first frame.
    assert_eq!(frames_shown(doc, f.scene, 5, 7), vec![5, 6]);
    assert_eq!(
        frames_shown(doc, f.scene, 7, 5),
        vec![5, 6],
        "ends in any order"
    );
    assert!(frames_shown(doc, f.scene, 20, 30).is_empty());
}

#[test]
fn select_all_in_a_range_takes_every_keyframe_but_not_locked_or_hidden_layers() {
    let (mut f, other) = walk();
    let starts = |f: &Fixture, from, to| -> Vec<(bool, u32)> {
        spans_in_range(f.session.document(), f.scene, from, to)
            .iter()
            .map(|s| (s.layer == other.layer, s.start))
            .collect()
    };
    let mut all = starts(&f, 0, 9);
    all.sort_unstable();
    assert_eq!(all, vec![(false, 0), (false, 4), (false, 6), (true, 0)]);
    let mut part = starts(&f, 5, 5);
    part.sort_unstable();
    assert_eq!(part, vec![(false, 4), (true, 0)]);
    f.session
        .apply([Command::UpdateLayer {
            scene: f.scene,
            layer: other.layer,
            change: LayerChange::Locked(true),
        }])
        .expect("lock");
    assert!(
        starts(&f, 0, 9).iter().all(|(o, _)| !o),
        "locked layer left out"
    );
    f.session
        .apply([Command::UpdateLayer {
            scene: f.scene,
            layer: f.layer,
            change: LayerChange::Visible(false),
        }])
        .expect("hide");
    assert!(starts(&f, 0, 9).is_empty(), "hidden layer left out");
}

#[test]
fn a_lasso_takes_what_it_surrounds_in_any_frame_shown() {
    let (f, _) = walk();
    // Around the rectangles of frames 4 and 6 (x 20 to 50), not those at x 0.
    let polygon = [
        Pos2::new(15.0, -5.0),
        Pos2::new(55.0, -5.0),
        Pos2::new(55.0, 15.0),
        Pos2::new(15.0, 15.0),
    ];
    let found = spans_in_polygon_range(f.session.document(), f.scene, 0, 9, &polygon);
    let mut starts: Vec<u32> = found.iter().map(|s| s.start).collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![4, 6]);
    // Only the frames in the range count.
    let found = spans_in_polygon_range(f.session.document(), f.scene, 0, 3, &polygon);
    assert!(found.is_empty());
}

#[test]
fn transforming_a_range_moves_every_keyframe_in_it_as_one_step() {
    let (mut f, other) = walk();
    let before = |f: &Fixture, start| bounds(f, start);
    let (a, b, c) = (before(&f, 0), before(&f, 4), before(&f, 6));
    let moved = transform_range(
        f.session.document(),
        f.scene,
        0,
        5,
        RangeTransform {
            dx: 100.0,
            ..RangeTransform::default()
        },
    );
    run(&mut f, moved);
    assert_eq!(bounds(&f, 0).min.x, a.min.x + 100.0);
    assert_eq!(bounds(&f, 4).min.x, b.min.x + 100.0);
    assert_eq!(bounds(&f, 6), c, "outside the range: untouched");
    let other_now =
        span_bounds(f.session.document(), f.scene, other).expect("other layer's rectangle");
    assert_eq!(other_now.min.x, 100.0, "every layer in the range moves");
    f.session.undo();
    assert_eq!(bounds(&f, 0), a, "one undo step");
}

#[test]
fn scaling_a_range_is_about_the_middle_of_all_of_it() {
    let (mut f, _) = walk();
    // Everything spans x 0 to 50 and y 0 to 60: the middle is (25, 30).
    let doubled = transform_range(
        f.session.document(),
        f.scene,
        0,
        9,
        RangeTransform {
            scale_x: 2.0,
            scale_y: 2.0,
            ..RangeTransform::default()
        },
    );
    run(&mut f, doubled);
    let first = bounds(&f, 0);
    assert!((first.min.x - -25.0).abs() < 1e-3, "{first:?}");
    assert!((first.width() - 20.0).abs() < 1e-3, "{first:?}");
    // Nothing to do, or nothing to move: no commands.
    let doc = f.session.document();
    assert!(transform_range(doc, f.scene, 0, 9, RangeTransform::default()).is_none());
    assert!(
        transform_range(
            doc,
            f.scene,
            40,
            50,
            RangeTransform {
                dx: 1.0,
                ..RangeTransform::default()
            }
        )
        .is_none()
    );
    let zero = RangeTransform {
        scale_x: 0.0,
        ..RangeTransform::default()
    };
    assert!(
        transform_range(doc, f.scene, 0, 9, zero).is_none(),
        "a zero scale destroys"
    );
}
