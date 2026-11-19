#![allow(clippy::expect_used)]

use glamx::DVec2;

use super::*;

fn map(t: flare_core::Transform2D, x: f64, y: f64) -> DVec2 {
    t.as_affine().transform_point2(DVec2::new(x, y))
}

fn near(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn the_registration_point_goes_to_the_position() {
    let t = placement(100.0, 50.0, 37.0, 2.0, 0.5).expect("placed");
    assert!(near(map(t, 0.0, 0.0), DVec2::new(100.0, 50.0)));
}

#[test]
fn a_positive_angle_turns_clockwise_on_the_stage() {
    // The stage's y runs down: a point to the right turns to below.
    let t = placement(0.0, 0.0, 90.0, 1.0, 1.0).expect("placed");
    assert!(near(map(t, 10.0, 0.0), DVec2::new(0.0, 10.0)));
}

#[test]
fn scale_is_about_the_registration_point_and_before_the_turn() {
    let t = placement(5.0, 5.0, 90.0, 2.0, 3.0).expect("placed");
    // (1, 0) scales to (2, 0), turns to (0, 2), then moves by (5, 5).
    assert!(near(map(t, 1.0, 0.0), DVec2::new(5.0, 7.0)));
    assert!(near(map(t, 0.0, 1.0), DVec2::new(2.0, 5.0)));
}

#[test]
fn bad_numbers_and_a_zero_scale_are_refused() {
    assert_eq!(
        placement(f64::NAN, 0.0, 0.0, 1.0, 1.0),
        Err(PlacementError::NotFinite)
    );
    assert_eq!(
        placement(0.0, 0.0, f64::INFINITY, 1.0, 1.0),
        Err(PlacementError::NotFinite)
    );
    assert_eq!(
        placement(0.0, 0.0, 0.0, 0.0, 1.0),
        Err(PlacementError::ZeroScale)
    );
    assert!(
        placement(0.0, 0.0, 0.0, -1.0, 1.0).is_ok(),
        "a mirror is a scale"
    );
}
