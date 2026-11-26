//! Moving a layer to any place in the stacking order through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn move_puts_a_layer_above_another_or_at_the_bottom_and_refuses_a_plain_folder() {
    let f = fixture(true, false).await;
    let mut ids = Vec::new();
    for name in ["B", "C"] {
        let added = f.ok("add_layer", json!({ "name": name })).await;
        ids.push(added["layer"].as_str().expect("id").to_owned());
    }
    let names = || async {
        f.ok("get_info", json!({})).await["scenes"][0]["layers"]
            .as_array()
            .expect("layers")
            .iter()
            .map(|l| l["name"].as_str().expect("name").to_owned())
            .collect::<Vec<_>>()
    };
    let first = names().await[0].clone();
    f.ok(
        "layer_operation",
        json!({ "operation": "move", "layer": ids[1], "before": ids[0] }),
    )
    .await;
    assert_eq!(names().await, [first.as_str(), "C", "B"]);
    f.ok(
        "layer_operation",
        json!({ "operation": "move", "layer": ids[1] }),
    )
    .await;
    assert_eq!(names().await, [first.as_str(), "B", "C"], "to the bottom");

    let before = f.revision().await;
    for refused in [
        json!({ "operation": "move", "layer": ids[1] }),
        json!({ "operation": "move", "layer": ids[1], "folder": ids[0] }),
    ] {
        let (bad, message) = f.tool("layer_operation", refused).await;
        assert!(bad, "{message}");
    }
    assert_eq!(f.revision().await, before, "nothing changed");
}

#[tokio::test]
async fn move_keyframe_replaces_what_it_lands_on_on_its_layer_or_another() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 5, "height": 5, "duration": 10 }),
    )
    .await;
    f.ok(
        "frame_operation",
        json!({ "operation": "insert_blank_keyframe", "at": 4 }),
    )
    .await;
    let f = &f;
    let spans = |i: usize| async move {
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][i]["spans"].as_u64()
    };
    f.ok(
        "frame_operation",
        json!({ "operation": "move_keyframe", "at": 1, "to": 6 }),
    )
    .await;
    assert_eq!(spans(0).await, Some(2));

    let second = f.ok("add_layer", json!({ "name": "Second" })).await["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    f.ok(
        "frame_operation",
        json!({ "operation": "move_keyframe", "at": 6, "to": 0, "onto": second }),
    )
    .await;
    assert_eq!((spans(0).await, spans(1).await), (Some(1), Some(1)));

    let before = f.revision().await;
    for refused in [
        json!({ "operation": "move_keyframe", "at": 0 }),
        json!({ "operation": "move_keyframe", "at": 40, "to": 1 }),
    ] {
        let (bad, message) = f.tool("frame_operation", refused).await;
        assert!(bad, "{message}");
    }
    assert_eq!(f.revision().await, before);
}

#[tokio::test]
async fn move_frames_and_copy_frames_take_a_run_of_frames_to_dest() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 5, "height": 5, "duration": 2 }),
    )
    .await;
    let f = &f;
    let spans = |i: usize| async move {
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][i]["spans"].as_u64()
    };
    f.ok(
        "frame_operation",
        json!({ "operation": "copy_frames", "at": 0, "to": 1, "dest": 4 }),
    )
    .await;
    assert_eq!(spans(0).await, Some(2), "the original and the copy");
    // The empty frames between them go, and the copy moves up.
    f.ok(
        "frame_operation",
        json!({ "operation": "remove_frames", "at": 2, "to": 3 }),
    )
    .await;
    f.ok(
        "frame_operation",
        json!({ "operation": "move_frames", "at": 0, "to": 3, "dest": 6 }),
    )
    .await;
    assert_eq!(spans(0).await, Some(2), "both moved together");

    let second = f.ok("add_layer", json!({ "name": "Second" })).await["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    f.ok(
        "frame_operation",
        json!({ "operation": "move_frames", "at": 6, "to": 7, "dest": 0, "onto": second }),
    )
    .await;
    assert_eq!((spans(0).await, spans(1).await), (Some(1), Some(1)));

    let before = f.revision().await;
    for refused in [
        json!({ "operation": "move_frames", "at": 8, "to": 9 }),
        json!({ "operation": "copy_frames", "at": 0, "to": 5, "dest": 9 }),
        json!({ "operation": "move_frames", "at": 8, "to": 9, "dest": 8 }),
    ] {
        let (bad, message) = f.tool("frame_operation", refused).await;
        assert!(bad, "{message}");
    }
    assert_eq!(f.revision().await, before);
}

#[tokio::test]
async fn add_layer_under_puts_the_new_layer_just_under_the_one_named() {
    let f = fixture(true, false).await;
    f.ok("add_layer", json!({ "name": "B" })).await;
    let info = f.ok("get_info", json!({})).await;
    let first = info["scenes"][0]["layers"][0]["id"]
        .as_str()
        .expect("id")
        .to_owned();
    let first_name = info["scenes"][0]["layers"][0]["name"]
        .as_str()
        .expect("name")
        .to_owned();
    f.ok("add_layer", json!({ "name": "Under", "under": first }))
        .await;
    let names: Vec<String> = f.ok("get_info", json!({})).await["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["name"].as_str().expect("name").to_owned())
        .collect();
    assert_eq!(names, [first_name.as_str(), "Under", "B"]);

    let before = f.revision().await;
    let (bad, message) = f
        .tool(
            "add_layer",
            json!({ "name": "Lost", "under": "00000000-0000-0000-0000-000000000000" }),
        )
        .await;
    assert!(bad, "{message}");
    assert_eq!(f.revision().await, before, "nothing changed");
}
