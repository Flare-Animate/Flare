#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn a_rectangle_on_a_later_frame_starts_a_keyframe_beside_the_held_one_and_get_info_counts_them()
 {
    let f = fixture(true, false).await;
    let a = f
        .ok(
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "duration": 8 }),
        )
        .await;
    let b = f
        .ok(
            "draw_rect",
            json!({ "x": 10, "y": 0, "width": 5, "height": 5, "start": 3 }),
        )
        .await;
    assert_eq!(a["layer"], b["layer"]);
    assert_eq!(
        (a["element"].as_u64(), b["element"].as_u64()),
        (Some(0), Some(1))
    );
    let info = f.ok("get_info", json!({})).await;
    let layer = &info["scenes"][0]["layers"][0];
    assert_eq!(
        (layer["spans"].as_u64(), layer["elements"].as_u64()),
        // Frame 3 only showed the first keyframe: a keyframe of its own starts there with a copy
        // of the held rectangle and the new one beside it.
        (Some(2), Some(3))
    );
    assert_eq!(
        info["scenes"][0]["layers"]
            .as_array()
            .expect("layers")
            .len(),
        1
    );

    // One undo step takes the second one away.
    f.ok("undo", json!({})).await;
    assert_eq!(
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][0]["elements"],
        1
    );
}

#[tokio::test]
async fn a_layer_that_takes_no_drawing_and_an_unknown_layer_are_errors_that_change_nothing() {
    let f = fixture(true, false).await;
    f.ok("layer_operation", json!({ "operation": "lock" }))
        .await;
    let before = f.revision().await;
    let (is_error, message) = f
        .tool(
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5 }),
        )
        .await;
    assert!(
        is_error && message.as_str().expect("msg").contains("locked"),
        "{message}"
    );
    let (is_error, _) = f
        .tool(
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5,
                    "layer": "00000000-0000-0000-0000-000000000000" }),
        )
        .await;
    assert!(is_error);
    assert_eq!(f.revision().await, before);
}

#[tokio::test]
async fn get_info_says_how_long_each_scene_runs() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0.0, "y": 0.0, "width": 10.0, "height": 10.0, "duration": 48 }),
    )
    .await;
    let info = f.ok("get_info", json!({})).await;
    let rate = &info["frame_rate"];
    assert_eq!(
        (rate["numerator"].as_u64(), rate["denominator"].as_u64()),
        (Some(24), Some(1))
    );
    assert_eq!(info["scenes"][0]["frames"], 48);
    assert_eq!(info["scenes"][0]["seconds"], 2.0, "48 frames at 24 fps");
}
