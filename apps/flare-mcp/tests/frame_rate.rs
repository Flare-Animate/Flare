//! Changing a project's frame rate through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn set_stage_changes_the_frame_rate_and_refuses_one_out_of_range() {
    let f = fixture(true, false).await;
    f.ok("set_stage", json!({ "fps": 12.5 })).await;
    let rate = f.ok("get_info", json!({})).await["frame_rate"].clone();
    assert_eq!(rate, json!({ "numerator": 25, "denominator": 2 }));

    let before = f.revision().await;
    for bad in [0.0, -1.0, 241.0] {
        let (is_error, message) = f.tool("set_stage", json!({ "fps": bad })).await;
        assert!(is_error, "{bad}: {message}");
    }
    assert_eq!(f.revision().await, before, "nothing changed");
}
