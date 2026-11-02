#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn drawn_rectangles_join_the_keyframe_of_the_layer_and_get_info_counts_them() {
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
        (Some(1), Some(2))
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
