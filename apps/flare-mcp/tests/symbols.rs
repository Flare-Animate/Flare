//! Symbol kind, playback and rigs through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn kind_loop_and_rig() {
    let f = fixture(true, false).await;
    let sym = f.ok("add_symbol", json!({ "name": "Ball" })).await["symbol"]
        .as_str()
        .expect("id")
        .to_string();
    f.ok(
        "place_instance",
        json!({ "symbol": sym, "start": 1, "duration": 4 }),
    )
    .await;
    let before = f.revision().await;
    f.ok(
        "set_symbol_kind",
        json!({ "symbol": sym, "kind": "movie clip" }),
    )
    .await;
    f.ok(
        "set_instance_loop",
        json!({ "at": 1, "mode": "play once", "first_frame": 2 }),
    )
    .await;
    assert_eq!(f.revision().await, before + 2);
    // The command line's spellings are taken too.
    f.ok(
        "set_symbol_kind",
        json!({ "symbol": sym, "kind": "Graphic" }),
    )
    .await;
    f.ok(
        "set_symbol_kind",
        json!({ "symbol": sym, "kind": "movie-clip" }),
    )
    .await;
    f.ok("set_instance_loop", json!({ "at": 1, "mode": "play-once" }))
        .await;
    let rev = f.revision().await;
    let (bad, _) = f
        .tool("set_instance_loop", json!({ "at": 9, "mode": "loop" }))
        .await;
    assert!(bad);
    let (bad, _) = f
        .tool("set_symbol_kind", json!({ "symbol": sym, "kind": "nope" }))
        .await;
    assert!(bad);
    assert_eq!(f.revision().await, rev, "a refusal changes nothing");

    let rig = f.ok("rig_create", json!({ "symbol": sym })).await;
    let rig = f
        .ok(
            "rig_add_bone",
            json!({ "rig": rig, "name": "a", "length": 10.0 }),
        )
        .await;
    let rig = f
        .ok(
            "rig_add_bone",
            json!({ "rig": rig, "name": "b", "parent": 0, "length": 10.0 }),
        )
        .await;
    let rig = f
        .ok(
            "rig_set_pose",
            json!({ "rig": rig, "frame": 0, "ik": [[1, 2, 10.0, 10.0]] }),
        )
        .await;
    let out = f.ok("rig_eval", json!({ "rig": rig, "frame": 0 })).await;
    assert_eq!(out["bones"].as_array().expect("bones").len(), 2);
    let (bad, _) = f
        .tool(
            "rig_add_bone",
            json!({ "rig": rig, "name": "c", "parent": 9, "length": 1.0 }),
        )
        .await;
    assert!(bad);
}
