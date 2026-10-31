#![allow(clippy::expect_used)]

use flare_core::Rgba8;
use glamx::DVec2;

use super::*;

fn stage() -> Stage {
    Stage::new(200, 100, Rgba8::WHITE).expect("stage")
}

fn point(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

fn close(a: DVec2, b: DVec2) -> bool {
    (a - b).length() < 1e-9
}

#[test]
fn the_centered_camera_shows_the_stage_as_it_is() {
    let camera = Camera::centered(&stage());
    let view = camera.view(&stage());
    for p in [
        DVec2::ZERO,
        DVec2::new(200.0, 100.0),
        DVec2::new(37.0, 61.0),
    ] {
        assert!(close(view.transform_point2(p), p));
    }
}

#[test]
fn the_cameras_point_is_the_middle_of_the_picture_and_zoom_magnifies_about_it() {
    let camera = Camera::new(point(30.0, 20.0), 2.0, 0.0).expect("camera");
    let view = camera.view(&stage());
    assert!(close(
        view.transform_point2(DVec2::new(30.0, 20.0)),
        DVec2::new(100.0, 50.0)
    ));
    // A point 10 to the right of it is 20 to the right of the middle.
    assert!(close(
        view.transform_point2(DVec2::new(40.0, 20.0)),
        DVec2::new(120.0, 50.0)
    ));
}

#[test]
fn turning_the_camera_clockwise_turns_the_picture_counterclockwise() {
    let camera = Camera::new(point(100.0, 50.0), 1.0, std::f64::consts::FRAC_PI_2).expect("camera");
    let view = camera.view(&stage());
    // 10 to the right of the middle, with the camera turned a quarter clockwise, is now above it.
    assert!(close(
        view.transform_point2(DVec2::new(110.0, 50.0)),
        DVec2::new(100.0, 40.0)
    ));
}

#[test]
fn a_camera_refuses_a_zoom_out_of_range_and_a_rotation_that_is_not_a_number() {
    for zoom in [0.0, -1.0, 0.009, 101.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            Camera::new(point(0.0, 0.0), zoom, 0.0),
            Err(CameraError::Zoom),
            "{zoom}"
        );
    }
    assert_eq!(
        Camera::new(point(0.0, 0.0), 1.0, f64::NAN),
        Err(CameraError::Rotation)
    );
    assert!(Camera::new(point(0.0, 0.0), MIN_ZOOM, 0.0).is_ok());
    assert!(Camera::new(point(0.0, 0.0), MAX_ZOOM, 0.0).is_ok());
}

#[test]
fn between_two_cameras_the_point_and_zoom_go_straight_and_the_rotation_the_short_way() {
    let a = Camera::new(point(0.0, 0.0), 1.0, 0.1).expect("camera");
    let b = Camera::new(point(100.0, 40.0), 3.0, std::f64::consts::TAU - 0.1).expect("camera");
    let half = a.lerp(&b, 0.5);
    assert!(close(half.center().as_vec(), DVec2::new(50.0, 20.0)));
    assert!((half.zoom() - 2.0).abs() < 1e-12);
    // From 0.1 to a turn minus 0.1 is the short way through zero, so halfway is 0.
    assert!(half.rotation().abs() < 1e-9, "{}", half.rotation());
    assert_eq!(a.lerp(&b, 0.0), a);
    assert!((a.lerp(&b, 1.0).zoom() - 3.0).abs() < 1e-12);
    // Outside 0 to 1 is held at the ends.
    assert_eq!(a.lerp(&b, -4.0), a.lerp(&b, 0.0));
}
