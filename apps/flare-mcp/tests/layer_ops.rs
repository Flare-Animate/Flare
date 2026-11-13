//! Show All, Lock Others, Hide Others and Duplicate Layer through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::{Fixture, fixture};
use serde_json::{Value, json};

async fn layers(f: &Fixture) -> Vec<Value> {
    f.ok("get_info", json!({})).await["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .clone()
}

async fn flag(f: &Fixture, key: &str) -> Vec<bool> {
    layers(f)
        .await
        .iter()
        .map(|l| l[key].as_bool().expect("flag"))
        .collect()
}

#[tokio::test]
async fn layer_ops_change_the_project_as_one_step_each() {
    let f = fixture(true, false).await;
    f.ok("add_layer", json!({ "name": "B" })).await;
    f.ok("add_layer", json!({ "name": "C" })).await;
    let ids: Vec<String> = layers(&f)
        .await
        .iter()
        .map(|l| l["id"].as_str().expect("id").to_owned())
        .collect();
    assert_eq!(ids.len(), 3);

    let before = f.revision().await;
    f.ok("hide_other_layers", json!({ "layer": ids[1] })).await;
    assert_eq!(flag(&f, "visible").await, vec![false, true, false]);
    assert_eq!(f.revision().await, before + 1, "one step");
    let again = f.ok("hide_other_layers", json!({ "layer": ids[1] })).await;
    assert_eq!(again["changed"], false);
    assert_eq!(f.revision().await, before + 1, "nothing to change");

    f.ok("show_all_layers", json!({})).await;
    assert_eq!(flag(&f, "visible").await, vec![true, true, true]);
    f.ok("lock_other_layers", json!({ "layer": ids[0] })).await;
    assert_eq!(flag(&f, "locked").await, vec![false, true, true]);

    let out = f.ok("duplicate_layer", json!({ "layer": ids[2] })).await;
    let copy = out["copy"].as_str().expect("the copy's id").to_owned();
    let after = layers(&f).await;
    assert_eq!(after.len(), 4);
    assert_eq!(after[2]["id"], copy.as_str());
    assert_eq!(after[2]["name"], "C copy");

    let rev = f.revision().await;
    let (bad, _) = f
        .tool(
            "lock_other_layers",
            json!({ "layer": "00000000-0000-0000-0000-000000000000" }),
        )
        .await;
    assert!(bad, "an unknown layer is refused");
    assert_eq!(f.revision().await, rev, "a refusal changes nothing");
}

#[tokio::test]
async fn layers_become_a_symbol_and_a_taken_name_is_refused() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 10, "y": 10, "width": 8, "height": 8, "duration": 6 }),
    )
    .await;
    f.ok("add_layer", json!({ "name": "Top" })).await;
    let ids: Vec<String> = layers(&f)
        .await
        .iter()
        .map(|l| l["id"].as_str().expect("id").to_owned())
        .collect();
    let out = f
        .ok(
            "convert_layers_to_symbol",
            json!({ "layers": [ids[0]], "name": "Flower", "kind": "movie-clip" }),
        )
        .await;
    let after = layers(&f).await;
    assert_eq!(after.len(), 2);
    assert!(after.iter().any(|l| l["id"] == out["layer"]));
    let info = f.ok("get_info", json!({})).await;
    assert!(
        info["symbols"]
            .as_array()
            .expect("symbols")
            .iter()
            .any(|s| s["name"] == "Flower")
    );
    let rev = f.revision().await;
    let (bad, _) = f
        .tool(
            "convert_layers_to_symbol",
            json!({ "layers": [ids[1]], "name": "Flower" }),
        )
        .await;
    assert!(bad, "the name is taken");
    let (bad, _) = f
        .tool(
            "convert_layers_to_symbol",
            json!({ "layers": [], "name": "Empty" }),
        )
        .await;
    assert!(bad, "no layers");
    assert_eq!(f.revision().await, rev, "a refusal changes nothing");
}

#[tokio::test]
async fn set_layer_opacity_changes_the_view_and_refuses_too_much() {
    let f = fixture(true, false).await;
    assert_eq!(layers(&f).await[0]["opacity"], 100);
    let before = f.revision().await;
    f.ok("set_layer_opacity", json!({ "percent": 50 })).await;
    assert_eq!(layers(&f).await[0]["opacity"], 50);
    assert_eq!(f.revision().await, before + 1);
    let same = f.ok("set_layer_opacity", json!({ "percent": 50 })).await;
    assert_eq!(same["changed"], false);
    let (bad, _) = f.tool("set_layer_opacity", json!({ "percent": 150 })).await;
    assert!(bad);
    assert_eq!(f.revision().await, before + 1, "a refusal changes nothing");
}
