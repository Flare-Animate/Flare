//! The camera through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

fn pixel(path: &str, x: usize, y: usize) -> [u8; 4] {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    let at = (y * info.width as usize + x) * 4;
    [buf[at], buf[at + 1], buf[at + 2], buf[at + 3]]
}

#[tokio::test]
async fn set_camera_makes_the_layer_and_a_png_shows_what_it_sees() {
    // The fixture's stage is 40 by 20.
    let f = fixture(true, true).await;
    f.ok(
        "draw_rect",
        json!({ "x": 30, "y": 5, "width": 8, "height": 8, "color": "FF0000", "duration": 6 }),
    )
    .await;
    let out = f
        .ok("export_png", json!({ "frame": 0, "name": "before" }))
        .await;
    assert_eq!(
        pixel(out["path"].as_str().expect("path"), 34, 9),
        [255, 0, 0, 255]
    );

    let before = f.revision().await;
    let set = f
        .ok(
            "set_camera",
            json!({ "frame": 0, "x": 34, "y": 9, "zoom": 2 }),
        )
        .await;
    assert!(set["revision"].as_u64().expect("revision") > before);
    let after = f
        .ok("export_png", json!({ "frame": 0, "name": "after" }))
        .await;
    assert_eq!(
        pixel(after["path"].as_str().expect("path"), 20, 10),
        [255, 0, 0, 255]
    );
    assert_eq!(
        pixel(after["path"].as_str().expect("path"), 34, 9)[..3],
        [255, 255, 255]
    );

    // The camera is a layer above the art, and drawing without naming a layer still reaches the
    // art, not the camera.
    let info = f.ok("get_info", json!({})).await;
    let layers = info["scenes"][0]["layers"].as_array().expect("layers");
    assert_eq!(layers.len(), 2);
    assert_eq!(layers[0]["name"], "Camera");
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 4, "height": 4, "start": 6, "duration": 2 }),
    )
    .await;
}

#[tokio::test]
async fn set_camera_keeps_what_is_not_given_and_refuses_what_cannot_be_a_camera() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 4, "height": 4, "duration": 8 }),
    )
    .await;
    f.ok(
        "set_camera",
        json!({ "frame": 0, "x": 12, "y": 7, "zoom": 3 }),
    )
    .await;
    f.ok(
        "set_camera",
        json!({ "frame": 4, "zoom": 5, "motion": "motion", "ease": 30 }),
    )
    .await;
    let before = f.revision().await;
    for bad in [
        json!({ "frame": 0, "zoom": 0 }),
        json!({ "frame": 0, "zoom": 1000 }),
        json!({ "frame": 70000 }),
        json!({ "frame": 0, "motion": "sideways" }),
        json!({ "frame": 0, "x": 1, "expected_revision": before + 7 }),
    ] {
        let (is_error, message) = f.tool("set_camera", bad.clone()).await;
        assert!(is_error, "{bad}: {message}");
    }
    assert_eq!(
        f.revision().await,
        before,
        "a refused camera changes nothing"
    );
    // Undo takes the whole keyframe away in one step.
    f.ok("undo", json!({})).await;
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(info["can_redo"], true, "the second keyframe was one step");
    assert_eq!(info["can_undo"], true, "and the first is still there");
}

#[tokio::test]
async fn a_read_only_server_refuses_to_move_the_camera() {
    let f = fixture(false, false).await;
    let before = f.revision().await;
    let (is_error, message) = f.tool("set_camera", json!({ "frame": 0, "zoom": 2 })).await;
    assert!(is_error && message.as_str().expect("message").contains("read-only"));
    assert_eq!(f.revision().await, before);
}
