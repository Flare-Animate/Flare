//! The `magnet` tool through the MCP server: the wiring of `flare_tools::magnet`, a dry run that
//! needs no write access, one undo step, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn a_pull_bends_the_edge_near_it_as_one_step() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0 }),
    )
    .await;
    let up_there =
        json!({ "from": [50.0, -35.0], "to": [50.0, -35.0], "radius": 10.0, "dry_run": true });
    let before = f.ok("magnet", up_there.clone()).await;
    assert_eq!(before["reached"], json!([]));
    let pulled = f
        .ok(
            "magnet",
            json!({ "from": [50.0, 0.0], "to": [50.0, -40.0], "radius": 30.0 }),
        )
        .await;
    assert_eq!(pulled["pulled"], true);
    assert_eq!(pulled["reached"].as_array().map(Vec::len), Some(1));
    let after = f.ok("magnet", up_there.clone()).await;
    assert_eq!(
        after["reached"].as_array().map(Vec::len),
        Some(1),
        "{after}"
    );
    // One undo step puts it back.
    f.ok("undo", json!({})).await;
    let undone = f.ok("magnet", up_there).await;
    assert_eq!(undone["reached"], json!([]));
}

#[tokio::test]
async fn nothing_in_reach_or_no_write_access_is_refused() {
    let f = fixture(true, false).await;
    let (refused, text) = f
        .tool(
            "magnet",
            json!({ "from": [400.0, 400.0], "to": [0.0, 0.0], "radius": 10.0 }),
        )
        .await;
    assert!(
        refused && text.to_string().contains("no line or shape"),
        "{text}"
    );
    let read_only = fixture(false, false).await;
    let (refused, _) = read_only
        .tool("magnet", json!({ "from": [0.0, 0.0], "to": [1.0, 1.0] }))
        .await;
    assert!(refused, "a pull needs write access");
    let (refused, _) = read_only
        .tool(
            "magnet",
            json!({ "from": [0.0, 0.0], "to": [1.0, 1.0], "layer": "nope", "dry_run": true }),
        )
        .await;
    assert!(refused, "a bad layer id");
}
