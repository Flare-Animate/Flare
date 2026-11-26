//! The rigs a project keeps, through the MCP server: the wiring of `flare_editor::rig_library`,
//! each change one undo step, and the refusals that change nothing.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

async fn symbol(f: &common::Fixture) -> String {
    f.ok("add_symbol", json!({ "name": "Arm" })).await["symbol"]
        .as_str()
        .expect("symbol")
        .to_owned()
}

#[tokio::test]
async fn a_rig_is_kept_in_the_project_edited_evaluated_and_undone() {
    let f = fixture(true, false).await;
    let arm = symbol(&f).await;
    f.ok("project_rig_create", json!({ "symbol": arm })).await;
    f.ok(
        "project_rig_add_bone",
        json!({ "symbol": arm, "name": "upper", "length": 30.0 }),
    )
    .await;
    f.ok(
        "project_rig_set_pose",
        json!({ "symbol": arm, "frame": 4, "rotations": [[0, 0.5]] }),
    )
    .await;
    let listed = f.ok("project_rigs", json!({})).await;
    assert_eq!(listed["rigs"][0]["bones"], 1, "{listed}");
    assert_eq!(listed["rigs"][0]["keys"], 1);
    let eval = f
        .ok("project_rig_eval", json!({ "symbol": arm, "frame": 4 }))
        .await;
    assert_eq!(eval["bones"].as_array().map(Vec::len), Some(1), "{eval}");
    f.ok("undo", json!({})).await;
    let after = f.ok("project_rigs", json!({})).await;
    assert_eq!(
        after["rigs"][0]["keys"], 0,
        "one undo step took the pose back"
    );
    f.ok("project_rig_remove", json!({ "symbol": arm })).await;
    assert_eq!(f.ok("project_rigs", json!({})).await["rigs"], json!([]));
}

#[tokio::test]
async fn refusals_change_nothing_and_a_rig_needs_write_access() {
    let f = fixture(true, false).await;
    let arm = symbol(&f).await;
    let (refused, text) = f
        .tool("project_rig_eval", json!({ "symbol": arm, "frame": 0 }))
        .await;
    assert!(refused && text.to_string().contains("no rig"), "{text}");
    f.ok("project_rig_create", json!({ "symbol": arm })).await;
    let (twice, text) = f.tool("project_rig_create", json!({ "symbol": arm })).await;
    assert!(
        twice && text.to_string().contains("already has a rig"),
        "{text}"
    );
    let (bad, _) = f
        .tool(
            "project_rig_add_bone",
            json!({ "symbol": arm, "name": "x", "parent": 3, "length": 1.0 }),
        )
        .await;
    assert!(bad, "a parent that is not an earlier bone");
    let listed = f.ok("project_rigs", json!({})).await;
    assert_eq!(listed["rigs"][0]["bones"], 0);
    let (nope, _) = f
        .tool("project_rig_create", json!({ "symbol": "nope" }))
        .await;
    assert!(nope, "a bad symbol id");
    let read_only = fixture(false, false).await;
    let (denied, _) = read_only
        .tool("project_rig_create", json!({ "symbol": arm }))
        .await;
    assert!(denied, "a rig needs write access");
}
