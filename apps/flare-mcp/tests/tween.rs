//! Interpolation through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn lists_evaluates_sets_and_bakes() {
    let f = fixture(true, false).await;
    let list = f.ok("list_easings", json!({})).await;
    assert!(list["families"].as_array().expect("families").len() >= 13);
    let v = f
        .ok(
            "eval_curve",
            json!({ "family": "quad", "easing": "in", "t": 0.5 }),
        )
        .await;
    assert!(v["value"].as_f64().expect("value") < 0.5);
    let (bad, _) = f
        .tool("eval_curve", json!({ "family": "nope", "t": 0.5 }))
        .await;
    assert!(bad);

    for (start, x, duration) in [(0, 0, 4), (4, 20, 2)] {
        f.ok(
            "draw_rect",
            json!({ "x": x, "y": 2, "width": 5, "height": 5, "color": "FF0000", "start": start, "duration": duration }),
        )
        .await;
    }
    let before = f.revision().await;
    f.ok(
        "set_interpolation",
        json!({ "at": 0, "family": "bounce", "easing": "out" }),
    )
    .await;
    assert_eq!(f.revision().await, before + 1);
    let (dup, _) = f
        .tool(
            "set_interpolation",
            json!({ "at": 0, "family": "bounce", "easing": "out" }),
        )
        .await;
    assert!(dup, "already set");
    let rev = f.revision().await;
    let (err, _) = f
        .tool("create_inbetweens", json!({ "at": 0, "count": 0 }))
        .await;
    assert!(err);
    assert_eq!(f.revision().await, rev, "a refusal changes nothing");
    f.ok("create_inbetweens", json!({ "at": 0, "count": 3 }))
        .await;
    assert_eq!(f.revision().await, rev + 1);
    f.ok("undo", json!({})).await;
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(info["can_redo"], true, "the in-betweens were one step");
}
