//! The game description through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn a_game_is_edited_run_and_exported() {
    let f = fixture(true, true).await;
    assert_eq!(f.ok("get_game", json!({})).await, json!(null));

    let added = f.ok("add_game_entity", json!({ "name": "Hero" })).await;
    assert_eq!(added["entity"], 1);
    f.ok(
        "set_game_component",
        json!({ "entity": 1, "kind": "transform", "fields": { "x": "3" } }),
    )
    .await;
    f.ok(
        "set_game_component",
        json!({ "entity": 1, "kind": "sprite", "fields": { "frames": "4", "fps": "8" } }),
    )
    .await;
    f.ok("set_game_gravity", json!({ "x": 0, "y": 980 })).await;
    let game = f.ok("get_game", json!({})).await;
    assert_eq!(game["entities"]["1"]["name"], "Hero");
    assert_eq!(game["gravity"], json!([0.0, 980.0]));

    let run = f
        .ok("run_game", json!({ "frames": 30, "script": "right;" }))
        .await;
    assert_eq!(run["frames"], 30);
    assert_eq!(run["digest"].as_str().map(str::len), Some(16));
    let again = f
        .ok("run_game", json!({ "frames": 30, "script": "right;" }))
        .await;
    assert_eq!(run, again);

    let exported = f.ok("export_game", json!({ "name": "hero" })).await;
    assert_eq!(exported["frames"], 1);
    assert!(f.dir.path().join("exports").join("hero.json").exists());
    assert!(f.dir.path().join("exports").join("hero.png").exists());
    assert!(
        f.dir
            .path()
            .join("exports")
            .join("hero.sheet.json")
            .exists()
    );
}

#[tokio::test]
async fn bad_edits_and_exports_are_refused_and_change_nothing() {
    let f = fixture(true, true).await;
    f.ok("add_game_entity", json!({ "name": "Hero" })).await;
    f.ok(
        "set_game_component",
        json!({ "entity": 1, "kind": "transform", "fields": { "x": "3" } }),
    )
    .await;
    let before = f.ok("get_game", json!({})).await;

    let refused = [
        (
            "set_game_component",
            json!({ "entity": 1, "kind": "sprite", "fields": { "fsp": "8" } }),
        ),
        (
            "set_game_component",
            json!({ "entity": 1, "kind": "nonsense", "fields": {} }),
        ),
        (
            "set_game_component",
            json!({ "entity": 1, "kind": "sprite", "fields": { "a=b": "1" } }),
        ),
        (
            "set_game_component",
            json!({ "entity": 0, "kind": "transform", "fields": {} }),
        ),
        ("remove_game_entity", json!({ "entity": 9 })),
        (
            "add_game_entity",
            json!({ "name": "Ghost", "symbol": "not-a-uuid" }),
        ),
        (
            "add_game_entity",
            json!({ "name": "Ghost", "expected_revision": 999 }),
        ),
        ("export_game", json!({ "name": "../escape" })),
        (
            "run_game",
            json!({ "frames": 5, "script": "left;left;left;left;left;left" }),
        ),
    ];
    for (tool, arguments) in refused {
        let (is_error, value) = f.tool(tool, arguments.clone()).await;
        assert!(is_error, "{tool} {arguments} must be refused: {value}");
    }
    assert_eq!(f.ok("get_game", json!({})).await, before);

    let (is_error, _) = f.tool("export_game", json!({ "name": "hero" })).await;
    assert!(!is_error);
    let (is_error, _) = f.tool("export_game", json!({ "name": "hero" })).await;
    assert!(is_error, "an export never overwrites");
}

#[tokio::test]
async fn a_read_only_server_refuses_edits_and_no_export_directory_refuses_exports() {
    let read_only = fixture(false, true).await;
    let (is_error, _) = read_only
        .tool("add_game_entity", json!({ "name": "Hero" }))
        .await;
    assert!(is_error);

    let no_exports = fixture(true, false).await;
    no_exports
        .ok("add_game_entity", json!({ "name": "Hero" }))
        .await;
    let (is_error, _) = no_exports.tool("export_game", json!({})).await;
    assert!(is_error);
}
