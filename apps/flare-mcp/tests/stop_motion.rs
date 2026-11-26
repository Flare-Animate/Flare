//! The stop-motion session through the MCP server: frames from PNG files in the export directory,
//! each change one undo step, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

fn png(shade: u8) -> Vec<u8> {
    let mut out = Vec::new();
    let mut encoder = png::Encoder::new(&mut out, 2, 2);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().expect("header");
    writer.write_image_data(&[shade; 16]).expect("data");
    writer.finish().expect("finish");
    out
}

#[tokio::test]
async fn a_session_takes_frames_from_files_and_lays_them_on_a_layer() {
    let f = fixture(true, true).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("exports");
    std::fs::write(exports.join("a.png"), png(5)).expect("a");
    std::fs::write(exports.join("b.png"), png(200)).expect("b");
    f.ok("stop_motion_start", json!({ "fps": 12, "hold": 2 }))
        .await;
    f.ok("stop_motion_add_frame", json!({ "source": "a.png" }))
        .await;
    let added = f
        .ok(
            "stop_motion_add_frame",
            json!({ "source": "b.png", "hold": 3 }),
        )
        .await;
    assert_eq!(added["frames"], 2, "{added}");
    let listed = f.ok("stop_motion_list", json!({})).await;
    assert_eq!(listed["duration"], 5, "{listed}");
    f.ok("stop_motion_set_hold", json!({ "index": 0, "hold": 4 }))
        .await;
    assert_eq!(f.ok("stop_motion_list", json!({})).await["duration"], 7);
    f.ok("undo", json!({})).await;
    assert_eq!(f.ok("stop_motion_list", json!({})).await["duration"], 5);
    let laid = f.ok("stop_motion_lay", json!({})).await;
    assert!(laid["layer"].is_string(), "{laid}");
    f.ok("stop_motion_remove_frame", json!({ "index": 1 }))
        .await;
    f.ok("stop_motion_end", json!({})).await;
    let (refused, _) = f.tool("stop_motion_list", json!({})).await;
    assert!(refused, "the session ended");
}

#[tokio::test]
async fn refusals_change_nothing_and_a_session_needs_write_access() {
    let f = fixture(true, true).await;
    let exports = f.dir.path().join("exports");
    std::fs::create_dir_all(&exports).expect("exports");
    std::fs::write(exports.join("bad.png"), b"not a png").expect("bad");
    let (early, text) = f
        .tool("stop_motion_add_frame", json!({ "source": "bad.png" }))
        .await;
    assert!(
        early && text.to_string().contains("no stop-motion session"),
        "{text}"
    );
    let (rate, _) = f.tool("stop_motion_start", json!({ "fps": 0 })).await;
    assert!(rate, "a rate of 0");
    f.ok("stop_motion_start", json!({})).await;
    let (bad, _) = f
        .tool("stop_motion_add_frame", json!({ "source": "bad.png" }))
        .await;
    assert!(bad, "not a PNG");
    let (escape, _) = f
        .tool("stop_motion_add_frame", json!({ "source": "../x.png" }))
        .await;
    assert!(escape, "a name that leaves the export directory");
    let (none, text) = f
        .tool("stop_motion_set_hold", json!({ "index": 9, "hold": 2 }))
        .await;
    assert!(none && text.to_string().contains("no frame 9"), "{text}");
    assert_eq!(
        f.ok("stop_motion_list", json!({})).await["frames"],
        json!([])
    );
    let read_only = fixture(false, true).await;
    let (denied, _) = read_only.tool("stop_motion_start", json!({})).await;
    assert!(denied);
}
