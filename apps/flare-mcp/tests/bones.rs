//! Bones through the MCP server: the same armature the Bone tool makes, posed by an agent.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::{Value, json};

/// Two instances of a 4 px square, at (10, 10) and (20, 10), each on a layer of its own.
async fn two_parts(f: &common::Fixture) -> (String, String) {
    let symbol = f
        .ok(
            "add_symbol",
            json!({ "name": "Part", "x": -2, "y": -2, "width": 4, "height": 4, "color": "FF0000" }),
        )
        .await["symbol"]
        .as_str()
        .expect("symbol")
        .to_owned();
    let second = f.ok("add_layer", json!({ "name": "Second" })).await["layer"]
        .as_str()
        .expect("layer")
        .to_owned();
    let place = |x: f64, layer: Option<String>| {
        let mut args = json!({ "symbol": symbol, "x": x, "y": 10, "duration": 8 });
        if let Some(layer) = layer {
            args["layer"] = json!(layer);
        }
        args
    };
    let first = f.ok("place_instance", place(10.0, None)).await;
    let other = f.ok("place_instance", place(20.0, Some(second))).await;
    let id = |v: &Value| v["instance"].as_str().expect("instance").to_owned();
    (id(&first), id(&other))
}

#[tokio::test]
async fn an_agent_links_poses_and_releases_an_armature() {
    let f = fixture(true, true).await;
    let (shoulder, hand) = two_parts(&f).await;
    let made = f
        .ok("add_bone", json!({ "from": shoulder, "to": hand }))
        .await;
    assert_eq!(made["armatures"][0]["joints"][1]["parent"], 0, "{made}");
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(info["scenes"][0]["layers"][0]["armature"], true, "{info}");

    let posed = f
        .ok("pose_bone", json!({ "joint": 0, "frame": 4, "turn": 90 }))
        .await;
    let hand_at = &posed["armatures"][0]["joints"][1];
    assert!(
        (hand_at["x"].as_f64().expect("x") - 10.0).abs() < 1e-9,
        "{posed}"
    );
    assert!(
        (hand_at["y"].as_f64().expect("y") - 20.0).abs() < 1e-9,
        "{posed}"
    );
    let listed = f.ok("list_bones", json!({ "frame": 0 })).await;
    assert_eq!(
        listed["armatures"][0]["joints"][1]["x"], 20.0,
        "the pose before is kept"
    );

    f.ok("limit_joint", json!({ "joint": 1, "min": -10, "max": 10 }))
        .await;
    f.ok("detach_bone", json!({ "joint": 1 })).await;
    f.ok("release_armature", json!({})).await;
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(info["scenes"][0]["layers"][0]["armature"], false, "{info}");
}

#[tokio::test]
async fn bones_refuse_with_a_message_that_names_them() {
    let f = fixture(true, false).await;
    let (shoulder, _) = two_parts(&f).await;
    let (is_error, message) = f
        .tool("add_bone", json!({ "from": shoulder, "to": shoulder }))
        .await;
    assert!(is_error);
    assert!(message.to_string().contains("bones"), "{message}");
    let (is_error, message) = f.tool("pose_bone", json!({ "joint": 0, "turn": 5 })).await;
    assert!(is_error);
    assert!(message.to_string().contains("no armature"), "{message}");
    let read_only = fixture(false, false).await;
    let (is_error, _) = read_only
        .tool("add_bone", json!({ "from": shoulder, "to": shoulder }))
        .await;
    assert!(is_error, "writing needs write access");
}
