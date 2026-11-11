use emath::{Pos2, Rect};

use super::handles::Handle;
use super::{Grab, GrabAxis, grab_at};

/// A frame from (10, 10) to (110, 60), handles 4 units across.
fn frame() -> Rect {
    Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(110.0, 60.0))
}

#[test]
fn each_handle_scales_and_says_which_way_it_drags() {
    let at = |x, y| grab_at(frame(), Pos2::new(x, y), 4.0, false);
    let cases = [
        ((10.0, 10.0), Handle::TopLeft, GrabAxis::Falling),
        ((110.0, 60.0), Handle::BottomRight, GrabAxis::Falling),
        ((110.0, 10.0), Handle::TopRight, GrabAxis::Rising),
        ((10.0, 60.0), Handle::BottomLeft, GrabAxis::Rising),
        ((10.0, 35.0), Handle::Left, GrabAxis::Horizontal),
        ((111.0, 36.0), Handle::Right, GrabAxis::Horizontal),
        ((60.0, 10.0), Handle::Top, GrabAxis::Vertical),
        ((60.0, 62.0), Handle::Bottom, GrabAxis::Vertical),
    ];
    for ((x, y), handle, axis) in cases {
        assert_eq!(at(x, y), Some(Grab::Scale(handle)), "{handle:?}");
        assert_eq!(Grab::Scale(handle).axis(), Some(axis), "{handle:?}");
    }
}

#[test]
fn alt_skews_edges_along_themselves_and_leaves_corners_scaling() {
    let at = |x, y| grab_at(frame(), Pos2::new(x, y), 4.0, true);
    assert_eq!(at(60.0, 10.0), Some(Grab::Skew(Handle::Top)));
    assert_eq!(
        at(60.0, 10.0).and_then(Grab::axis),
        Some(GrabAxis::Horizontal)
    );
    assert_eq!(at(110.0, 35.0), Some(Grab::Skew(Handle::Right)));
    assert_eq!(
        at(110.0, 35.0).and_then(Grab::axis),
        Some(GrabAxis::Vertical)
    );
    assert_eq!(at(10.0, 10.0), Some(Grab::Scale(Handle::TopLeft)));
}

#[test]
fn inside_moves_just_outside_a_corner_rotates_and_far_off_is_nothing() {
    let at = |x, y| grab_at(frame(), Pos2::new(x, y), 4.0, false);
    assert_eq!(at(60.0, 35.0), Some(Grab::Move), "the middle");
    assert_eq!(
        at(30.0, 20.0),
        Some(Grab::Move),
        "off any ink, still inside"
    );
    assert_eq!(at(118.0, 68.0), Some(Grab::Rotate));
    assert_eq!(Grab::Rotate.axis(), None);
    assert_eq!(at(200.0, 200.0), None);
    assert_eq!(
        at(60.0, 75.0),
        None,
        "below the bottom edge, past its handle"
    );
    assert_eq!(grab_at(frame(), Pos2::new(f32::NAN, 1.0), 4.0, false), None);
}
