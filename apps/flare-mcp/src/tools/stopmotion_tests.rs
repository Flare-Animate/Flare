use super::stopmotion::{StartStopMotion, capture_refusal, session_settings};

fn args(fps: u32, hold: u32, before: u8, after: u8, opacity: u8) -> StartStopMotion {
    StartStopMotion {
        fps,
        hold,
        onion_before: before,
        onion_after: after,
        onion_opacity: opacity,
    }
}

#[test]
fn valid_settings_come_back_checked() {
    let value = session_settings(args(12, 2, 1, 1, 40)).expect("valid");
    assert_eq!(value["fps"], 12);
    assert_eq!(value["default_hold"], 2);
    assert_eq!(value["onion_skin"]["enabled"], true);
    assert_eq!(value["frames"].as_array().map(Vec::len), Some(0));
}

#[test]
fn out_of_range_settings_are_refused_by_the_library_message() {
    let error = session_settings(args(0, 1, 0, 0, 50)).expect_err("refused");
    assert!(error.contains("frame rate"), "{error}");
    let error = session_settings(args(24, 1, 9, 0, 50)).expect_err("refused");
    assert!(error.contains("0 to 5"), "{error}");
}

#[test]
fn capture_answers_with_the_named_error() {
    let error = capture_refusal().expect_err("no camera is built");
    assert!(error.contains("camera capture"), "{error}");
    assert!(error.contains("not yet implemented"), "{error}");
}
