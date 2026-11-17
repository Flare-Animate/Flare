//! A layer's effect graph through the MCP server: built, read, drawn and undone, and every
//! mistake refused without a change.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::{Value, json};

#[tokio::test]
async fn an_effect_graph_is_built_read_and_undone_one_step_at_a_time() {
    let f = fixture(true, true).await;
    let kinds = f.ok("list_effect_kinds", json!({})).await;
    assert!(
        kinds
            .as_array()
            .expect("kinds")
            .iter()
            .any(|k| k["kind"] == "snow" && k["animated"] == true)
    );
    assert_eq!(f.ok("get_effects", json!({})).await["effects"], Value::Null);

    let before = f.revision().await;
    let added = f
        .ok(
            "add_effect",
            json!({ "kind": "drop-shadow", "settings": { "distance": "8", "color": "FF000080" } }),
        )
        .await;
    assert_eq!(added["node"], 3);
    assert_eq!(added["changed"], true);
    assert_eq!(f.revision().await, before + 1, "one step");
    assert_eq!(
        added["effects"]["nodes"][2]["settings"]["color"],
        "#FF000080"
    );

    f.ok("add_effect", json!({ "kind": "over" })).await;
    f.ok(
        "connect_effects",
        json!({ "from": 1, "to": 4, "input": "over" }),
    )
    .await;
    f.ok("disconnect_effect", json!({ "to": 4, "input": 0 }))
        .await;
    f.ok(
        "move_effect_node",
        json!({ "node": 4, "x": 10.0, "y": -20.0 }),
    )
    .await;
    let set = f
        .ok(
            "set_effect_settings",
            json!({ "node": 3, "settings": { "blur": "0", "opacity": "100" } }),
        )
        .await;
    assert_eq!(set["effects"]["nodes"][2]["settings"]["blur"], 0.0);
    let removed = f.ok("remove_effect", json!({ "node": 3 })).await;
    assert_eq!(removed["effects"]["order"], json!([1, 4, 2]));
    let layers = f.ok("get_info", json!({})).await["scenes"][0]["layers"].clone();
    assert_eq!(layers[0]["effects"], true);

    let cleared = f.ok("clear_effects", json!({})).await;
    assert_eq!(cleared["effects"], Value::Null);
    let again = f.ok("clear_effects", json!({})).await;
    assert_eq!(again["changed"], false, "nothing left: no step");
    f.ok("undo", json!({})).await;
    assert_ne!(f.ok("get_effects", json!({})).await["effects"], Value::Null);
}

#[tokio::test]
async fn mistakes_are_refused_and_change_nothing() {
    let f = fixture(true, false).await;
    f.ok("add_effect", json!({ "kind": "blur" })).await;
    let rev = f.revision().await;
    for (tool, args, said) in [
        ("add_effect", json!({ "kind": "sparkle" }), "unknown effect"),
        (
            "add_effect",
            json!({ "kind": "glow", "settings": { "radius": "-3" } }),
            "radius",
        ),
        (
            "set_effect_settings",
            json!({ "node": 3, "settings": { "glitter": "1" } }),
            "no setting",
        ),
        ("connect_effects", json!({ "from": 3, "to": 3 }), "loop"),
        ("connect_effects", json!({ "from": 2, "to": 3 }), "Output"),
        (
            "remove_effect",
            json!({ "node": 2 }),
            "cannot be added or removed",
        ),
        (
            "disconnect_effect",
            json!({ "to": 3, "input": "under" }),
            "inputs are in",
        ),
        ("remove_effect", json!({ "node": 0 }), "not a node number"),
    ] {
        let (failed, body) = f.tool(tool, args.clone()).await;
        assert!(failed, "{tool} {args} was accepted");
        assert!(body.to_string().contains(said), "{tool}: {body}");
    }
    assert_eq!(f.revision().await, rev);
}

#[tokio::test]
async fn editing_effects_needs_write_access_but_reading_does_not() {
    let f = fixture(false, false).await;
    assert_eq!(f.ok("get_effects", json!({})).await["effects"], Value::Null);
    let (failed, body) = f.tool("add_effect", json!({ "kind": "glow" })).await;
    assert!(failed, "{body}");
}
