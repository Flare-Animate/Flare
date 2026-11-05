#![allow(clippy::expect_used)]

use crate::*;
use proptest::prelude::*;

fn view() -> View {
    View::fit(220.0, 220.0, 10.0, (0.0, 1.0), (0.0, 1.0))
}

#[test]
fn the_view_maps_both_ways() {
    let v = view();
    let s = v.to_screen((0.25, 0.75));
    let w = v.to_world(s);
    assert!((w.0 - 0.25).abs() < 1e-9 && (w.1 - 0.75).abs() < 1e-9);
    // y grows upward in the world and downward on screen.
    assert!(v.to_screen((0.0, 1.0)).1 < v.to_screen((0.0, 0.0)).1);
}

#[test]
fn a_degenerate_fit_gives_the_default_view() {
    assert_eq!(
        View::fit(0.0, 100.0, 5.0, (0.0, 1.0), (0.0, 1.0)),
        View::default()
    );
    assert_eq!(
        View::fit(100.0, 100.0, 5.0, (1.0, 1.0), (0.0, 1.0)),
        View::default()
    );
    assert_eq!(
        View::fit(f64::NAN, 100.0, 5.0, (0.0, 1.0), (0.0, 1.0)),
        View::default()
    );
}

#[test]
fn zoom_keeps_the_anchor_still_and_stays_in_limits() {
    let mut v = view();
    let anchor = (100.0, 80.0);
    let before = v.to_world(anchor);
    v.zoom_about(anchor, 2.0, 2.0);
    let after = v.to_world(anchor);
    assert!((before.0 - after.0).abs() < 1e-9 && (before.1 - after.1).abs() < 1e-9);
    for _ in 0..100 {
        v.zoom_about(anchor, 10.0, 10.0);
    }
    assert!(v.scale().0 <= MAX_SCALE && v.scale().1 <= MAX_SCALE);
    for _ in 0..200 {
        v.zoom_about(anchor, 0.1, 0.1);
    }
    assert!(v.scale().0 >= MIN_SCALE);
}

#[test]
fn bad_pan_and_zoom_change_nothing() {
    let mut v = view();
    let was = v;
    v.pan(f64::NAN, 1.0);
    v.zoom_about((1.0, 1.0), -1.0, 1.0);
    v.zoom_about((1.0, 1.0), f64::INFINITY, 1.0);
    v.zoom_about((f64::NAN, 1.0), 2.0, 1.0);
    assert_eq!(v, was);
    v.pan(2.0, -3.0);
    assert_ne!(v, was);
}

#[test]
fn a_pixel_grabs_the_nearest_handle_within_the_radius() {
    let v = view();
    let handles = [0.3, 0.0, -0.3, 0.0];
    let right = v.to_screen((0.3, 0.0));
    assert_eq!(hit_test(&v, handles, right, 6.0), Some(Grab::Right));
    let left = v.to_screen((0.7, 1.0));
    assert_eq!(
        hit_test(&v, handles, (left.0 + 3.0, left.1), 6.0),
        Some(Grab::Left)
    );
    assert_eq!(hit_test(&v, handles, (right.0 + 30.0, right.1), 6.0), None);
    assert_eq!(hit_test(&v, handles, (f64::NAN, 0.0), 6.0), None);
}

#[test]
fn an_overlapping_pair_goes_to_the_nearer_handle() {
    let v = view();
    let handles = [0.5, 0.5, -0.5, -0.5];
    let near_left = v.to_screen((0.5, 0.5));
    assert!(hit_test(&v, handles, near_left, 100.0).is_some());
    let p = v.to_screen((0.5, 0.5));
    assert_eq!(
        hit_test(&v, [0.5, 0.5, -0.4, -0.4], (p.0, p.1), 3.0),
        Some(Grab::Right)
    );
}

#[test]
fn dragging_an_auto_handle_makes_it_aligned_and_moves_the_other_with_it() {
    let d = drag_handle(
        HandleKind::AutoClamped,
        [0.0; 4],
        Grab::Right,
        (0.5, 0.5),
        DragOptions::default(),
    )
    .expect("drag");
    assert_eq!(d.kind, HandleKind::Aligned);
    assert_eq!(&d.handles[..2], &[0.5, 0.5]);
    // The left one points the opposite way along the same line.
    assert!(d.handles[2] < 0.0 && d.handles[3] < 0.0);
    assert!((d.handles[2] - d.handles[3]).abs() < 1e-9);
}

#[test]
fn dragging_a_vector_handle_makes_it_free_and_leaves_the_other() {
    let before = effective_handles(HandleKind::Vector, [0.0; 4]).expect("handles");
    let d = drag_handle(
        HandleKind::Vector,
        [0.0; 4],
        Grab::Left,
        (0.9, 0.2),
        DragOptions::default(),
    )
    .expect("drag");
    assert_eq!(d.kind, HandleKind::Free);
    assert_eq!(&d.handles[..2], &before[..2]);
    assert!((d.handles[2] + 0.1).abs() < 1e-9 && (d.handles[3] + 0.8).abs() < 1e-9);
}

#[test]
fn free_and_aligned_keep_their_kind() {
    let free = drag_handle(
        HandleKind::Free,
        [0.2, 0.0, -0.2, 0.0],
        Grab::Right,
        (0.4, 0.9),
        DragOptions::default(),
    )
    .expect("drag");
    assert_eq!(free.kind, HandleKind::Free);
    assert_eq!(&free.handles[2..], &[-0.2, 0.0]);
    let aligned = drag_handle(
        HandleKind::Aligned,
        [0.2, 0.0, -0.2, 0.0],
        Grab::Left,
        (0.8, 0.8),
        DragOptions::default(),
    )
    .expect("drag");
    assert_eq!(aligned.kind, HandleKind::Aligned);
    // The right handle is opposite the left one, with its own length.
    assert!(aligned.handles[0] > 0.0 && aligned.handles[1] > 0.0);
}

#[test]
fn a_drag_is_clamped_and_snapped() {
    let d = drag_handle(
        HandleKind::Free,
        [0.0; 4],
        Grab::Right,
        (7.0, 100.0),
        DragOptions::default(),
    )
    .expect("drag");
    assert_eq!(d.handles[0], 1.0);
    assert_eq!(d.handles[1], MAX_HANDLE_VALUE);
    let left = drag_handle(
        HandleKind::Free,
        [0.0; 4],
        Grab::Left,
        (-7.0, -100.0),
        DragOptions::default(),
    )
    .expect("drag");
    assert_eq!(left.handles[2], -1.0);
    let s = drag_handle(
        HandleKind::Free,
        [0.0; 4],
        Grab::Right,
        (0.33, 0.62),
        DragOptions { snap: Some(0.25) },
    )
    .expect("drag");
    assert_eq!(&s.handles[..2], &[0.25, 0.5]);
}

#[test]
fn bad_input_is_refused_or_ignored() {
    assert!(effective_handles(HandleKind::Free, [f64::NAN, 0.0, 0.0, 0.0]).is_err());
    assert!(
        drag_handle(
            HandleKind::Free,
            [f64::INFINITY, 0.0, 0.0, 0.0],
            Grab::Right,
            (0.5, 0.5),
            DragOptions::default()
        )
        .is_err()
    );
    let same = drag_handle(
        HandleKind::Free,
        [0.1, 0.2, -0.1, 0.0],
        Grab::Right,
        (f64::NAN, 0.5),
        DragOptions::default(),
    )
    .expect("ignored");
    assert_eq!(same.handles, [0.1, 0.2, -0.1, 0.0]);
}

#[test]
fn sampling_is_bounded_reuses_the_vector_and_hides_non_finite_values() {
    let mut out = Vec::new();
    sample_into(|t| t, 10, &mut out);
    assert_eq!(out.len(), 11);
    assert_eq!((out[0], out[10]), (0.0, 1.0));
    let cap = out.capacity();
    sample_into(|t| t * t, 8, &mut out);
    assert_eq!(out.len(), 9);
    assert_eq!(out.capacity(), cap);
    sample_into(|_| 0.0, usize::MAX, &mut out);
    assert_eq!(out.len(), MAX_SAMPLES + 1);
    sample_into(|_| f64::NAN, 3, &mut out);
    assert!(out.iter().all(|v| *v == 0.0));
    sample_into(|t| t, 0, &mut out);
    assert_eq!(out.len(), 2);
}

proptest! {
    #[test]
    fn drags_always_give_valid_in_range_handles(
        kind in 0usize..5,
        grab_right in any::<bool>(),
        h in proptest::array::uniform4(-3.0f64..3.0),
        tx in -50.0f64..50.0,
        ty in -50.0f64..50.0,
        snap in proptest::option::of(0.01f64..1.0),
    ) {
        let kind = HandleKind::ALL[kind];
        let grab = if grab_right { Grab::Right } else { Grab::Left };
        let d = drag_handle(kind, h, grab, (tx, ty), DragOptions { snap }).expect("finite");
        prop_assert!((0.0..=1.0).contains(&d.handles[0]));
        prop_assert!((-1.0..=0.0).contains(&d.handles[2]));
        for v in d.handles {
            prop_assert!(v.is_finite() && v.abs() <= MAX_HANDLE_VALUE + 1e-9);
        }
        // The result is a curve flare-tween accepts and evaluates to finite values.
        let mut a = Key::new(0.0, 0.0);
        a.handle_kind = d.kind;
        a.right = Handle { dt: d.handles[0], dv: d.handles[1] };
        let mut b = Key::new(1.0, 1.0);
        b.left = Handle { dt: d.handles[2], dv: d.handles[3] };
        let c = Curve::new(vec![a, b], Extrapolation::Constant, Params::default()).expect("valid");
        prop_assert!(c.evaluate(0.5).expect("finite").is_finite());
    }

    #[test]
    fn aligned_drags_keep_the_handles_on_one_line(
        h in proptest::array::uniform4(-1.0f64..1.0),
        tx in 0.05f64..1.0,
        ty in -2.0f64..2.0,
    ) {
        let d = drag_handle(HandleKind::Aligned, h, Grab::Right, (tx, ty), DragOptions::default()).expect("ok");
        let cross = d.handles[0] * d.handles[3] - d.handles[1] * d.handles[2];
        prop_assert!(cross.abs() < 1e-6, "cross {cross}");
    }

    #[test]
    fn view_round_trips(px in -1000.0f64..1000.0, py in -1000.0f64..1000.0, f in 0.2f64..5.0) {
        let mut v = view();
        v.zoom_about((px, py), f, f);
        let w = v.to_world((px, py));
        let s = v.to_screen(w);
        prop_assert!((s.0 - px).abs() < 1e-6 && (s.1 - py).abs() < 1e-6);
    }
}
