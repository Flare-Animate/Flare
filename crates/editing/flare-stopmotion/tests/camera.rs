#![allow(clippy::expect_used)]
use flare_stopmotion::{
    CameraError, CameraSource, MAX_CAPTURE_BYTES, NOT_YET_CAPTURE, NOT_YET_LIVE_VIEW, NoCamera,
    accept_capture,
};

#[test]
fn no_camera_refuses_capture_by_name() {
    let mut camera = NoCamera;
    assert_eq!(camera.name(), "no camera");
    assert_eq!(
        camera.capture(),
        Err(CameraError::NotYetImplemented {
            feature: NOT_YET_CAPTURE
        })
    );
    let message = camera.capture().expect_err("refused").to_string();
    assert!(message.contains("camera capture"), "{message}");
    assert!(message.contains("not yet implemented"), "{message}");
}

#[test]
fn no_camera_refuses_live_view_by_name() {
    let mut camera = NoCamera;
    assert_eq!(
        camera.start_live_view(),
        Err(CameraError::NotYetImplemented {
            feature: NOT_YET_LIVE_VIEW
        })
    );
}

#[test]
fn accepts_a_capture_up_to_the_limit() {
    let bytes = vec![0_u8; MAX_CAPTURE_BYTES];
    assert_eq!(
        accept_capture(bytes).expect("at the limit").len(),
        MAX_CAPTURE_BYTES
    );
}

#[test]
fn refuses_a_capture_over_the_limit_and_names_both_sizes() {
    let bytes = vec![0_u8; MAX_CAPTURE_BYTES + 1];
    assert_eq!(
        accept_capture(bytes),
        Err(CameraError::CaptureTooLarge {
            bytes: MAX_CAPTURE_BYTES + 1,
            max: MAX_CAPTURE_BYTES
        })
    );
}
