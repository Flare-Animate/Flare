//! The `bend_edge` tool through the MCP server: the wiring of `flare_tools::reshape`, a dry run
//! that needs no write access, one undo step, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn dragging_an_edge_of_merged_drawing_curves_it_as_one_step() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0, "merge": true }),
    )
    .await;
    let up_there = json!({ "from": [50.0, -40.0], "to": [50.0, -40.0], "dry_run": true });
    let (refused, _) = f.tool("bend_edge", up_there.clone()).await;
    assert!(refused, "nothing is up there yet");
    let plan = f
        .ok(
            "bend_edge",
            json!({ "from": [50.0, 1.0], "to": [50.0, -40.0], "dry_run": true }),
        )
        .await;
    assert_eq!(plan["bent"], false);
    let bent = f
        .ok(
            "bend_edge",
            json!({ "from": [50.0, 1.0], "to": [50.0, -40.0] }),
        )
        .await;
    assert_eq!(bent["bent"], true);
    let after = f.ok("bend_edge", up_there.clone()).await;
    assert_eq!(after["bent"], false, "{after}");
    f.ok("undo", json!({})).await;
    let (refused, _) = f.tool("bend_edge", up_there).await;
    assert!(refused, "one undo step puts the edge back");
}

#[tokio::test]
async fn an_object_a_miss_and_no_write_access_are_refused() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0.0, "y": 0.0, "width": 100.0, "height": 100.0 }),
    )
    .await;
    let (refused, text) = f
        .tool(
            "bend_edge",
            json!({ "from": [50.0, 0.0], "to": [50.0, -40.0] }),
        )
        .await;
    assert!(
        refused && text.to_string().contains("no merged line or shape"),
        "{text}"
    );
    let read_only = fixture(false, false).await;
    let (refused, _) = read_only
        .tool("bend_edge", json!({ "from": [0.0, 0.0], "to": [1.0, 1.0] }))
        .await;
    assert!(refused, "a bend needs write access");
    let (refused, _) = read_only
        .tool(
            "bend_edge",
            json!({ "from": [0.0, 0.0], "to": [1.0, 1.0], "layer": "nope", "dry_run": true }),
        )
        .await;
    assert!(refused, "a bad layer id");
}
