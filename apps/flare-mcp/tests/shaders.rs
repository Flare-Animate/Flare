//! Shader graph library tools through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

const VALID: &str = r#"{"version":1,"nodes":[{"op":"constant","args":0.0},{"op":"constant","args":0.0},{"op":"constant","args":0.0},{"op":"constant","args":1.0},{"op":"color","args":[0,1,2,3]}],"output":4}"#;

#[tokio::test]
async fn shader_graphs_are_listed_edited_and_validated() {
    let f = fixture(true, true).await;
    assert_eq!(
        f.ok("list_shader_graphs", json!({})).await["shader_graphs"],
        json!([])
    );
    f.ok(
        "set_shader_graph",
        json!({"name":"Wavy","graph_json":VALID,"expected_revision":0}),
    )
    .await;
    let listed = f.ok("list_shader_graphs", json!({})).await;
    assert_eq!(listed["shader_graphs"][0]["name"], "Wavy");
    assert_eq!(listed["shader_graphs"][0]["nodes"], 5);
    let fetched = f.ok("get_shader_graph", json!({"name":"Wavy"})).await;
    assert_eq!(
        flare_import::import_shader_graph(
            fetched["graph_json"]
                .as_str()
                .expect("graph JSON")
                .as_bytes()
        )
        .expect("valid returned graph")
        .nodes()
        .len(),
        5
    );

    let (error, _) = f
        .tool(
            "set_shader_graph",
            json!({"name":"Broken","graph_json":"{\"version\":1,\"nodes\":[{\"op\":\"sin\",\"args\":0}],\"output\":0}"}),
        )
        .await;
    assert!(error);
    assert_eq!(f.ok("list_shader_graphs", json!({})).await, listed);
}

#[tokio::test]
async fn shader_graph_writes_obey_permissions_and_revision_checks() {
    let read_only = fixture(false, true).await;
    assert!(
        read_only
            .tool(
                "set_shader_graph",
                json!({"name":"Wavy","graph_json":VALID}),
            )
            .await
            .0
    );

    let f = fixture(true, true).await;
    assert!(
        f.tool(
            "set_shader_graph",
            json!({"name":"Wavy","graph_json":VALID,"expected_revision":1}),
        )
        .await
        .0
    );
    assert_eq!(
        f.ok("list_shader_graphs", json!({})).await["shader_graphs"],
        json!([])
    );
}

#[tokio::test]
async fn png_export_applies_named_shader_graphs_in_order() {
    let f = fixture(true, true).await;
    let red = r#"{"version":1,"nodes":[{"op":"constant","args":1.0},{"op":"constant","args":0.0},{"op":"constant","args":0.0},{"op":"constant","args":1.0},{"op":"color","args":[0,1,2,3]}],"output":4}"#;
    f.ok(
        "set_shader_graph",
        json!({"name":"Red","graph_json":red,"expected_revision":0}),
    )
    .await;

    let exported = f
        .ok(
            "export_png",
            json!({"frame":0,"name":"shader-render","width":1,"height":1,"shaders":["Red"]}),
        )
        .await;
    let png = std::fs::read(exported["path"].as_str().expect("path")).expect("read PNG");
    let image = flare_import::read_still("shader.png", &png, 1).expect("decode PNG");
    assert_eq!(image.rgba, vec![255, 0, 0, 255]);
}
