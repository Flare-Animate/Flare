//! Stop motion through the command line: settings, the simulated capture, the timeline
//! layout and the onion ghosts. No camera backend is built, so the real capture is refused
//! by name, and sessions are not saved, which every reply says.

use super::*;

#[test]
fn new_session_prints_its_settings() {
    let reply = json(&[
        "stop-motion-new",
        "--fps",
        "24",
        "--hold",
        "3",
        "--onion-before",
        "2",
        "--onion-after",
        "1",
        "--onion-opacity",
        "40",
    ]);
    assert_eq!(reply["fps"], 24);
    assert_eq!(reply["default_hold"], 3);
    assert_eq!(reply["onion_skin"]["before"], 2);
    assert_eq!(reply["onion_skin"]["opacity"], 40);
    assert_eq!(reply["onion_skin"]["enabled"], serde_json::json!(true));
    assert_eq!(reply["camera"], "no camera");
}

#[test]
fn a_capture_without_a_camera_is_refused_by_name() {
    let output = run(&["stop-motion-capture"]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("camera capture is not yet implemented in Flare"),
        "{}",
        stderr
    );
}

#[test]
fn a_simulated_capture_says_what_stood_in() {
    let reply = json(&["stop-motion-capture", "--simulated"]);
    assert_eq!(reply["captured"], serde_json::json!(true));
    assert_eq!(reply["camera"], "simulated camera");
    assert_eq!(reply["pattern_width"], 64);
    assert_eq!(reply["pattern_height"], 48);
    assert_eq!(reply["frame_id"], 0);
    assert_eq!(reply["session_saved"], serde_json::json!(false));
}

#[test]
fn the_timeline_lays_holds_end_to_end() {
    let reply = json(&[
        "stop-motion-timeline",
        "--fps",
        "12",
        "--hold",
        "2",
        "--frames",
        "3",
    ]);
    assert_eq!(reply["duration"], 6);
    let slots = reply["slots"].as_array().expect("slots");
    let starts: Vec<u64> = slots
        .iter()
        .map(|s| s["start"].as_u64().expect("start"))
        .collect();
    assert_eq!(starts, [0, 2, 4]);
    assert_eq!(slots[0]["length"], 2);
}

#[test]
fn the_onion_names_the_ghosts_around_the_live_frame() {
    let reply = json(&[
        "stop-motion-onion",
        "--before",
        "2",
        "--after",
        "1",
        "--frames",
        "5",
        "--frame",
        "3",
    ]);
    assert_eq!(reply["ghosts"], serde_json::json!([2, 1, 4]));
    // At the first frame there is nothing earlier to show.
    let reply = json(&[
        "stop-motion-onion",
        "--before",
        "2",
        "--after",
        "1",
        "--frames",
        "5",
        "--frame",
        "1",
    ]);
    assert_eq!(reply["ghosts"], serde_json::json!([2]));
}

#[test]
fn an_onion_frame_past_the_end_is_refused() {
    let output = run(&[
        "stop-motion-onion",
        "--before",
        "1",
        "--frames",
        "3",
        "--frame",
        "4",
    ]);
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("past the 3 frames"), "{}", stderr);
}
