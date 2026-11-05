//! Shortcut sets through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn lists_exports_and_refuses() {
    let f = fixture(false, false).await;
    let flare = f.ok("list_shortcuts", json!({ "export": true })).await;
    assert_eq!(flare["conflicts"], 0);
    let file = flare["file"].to_string();
    let back = f.ok("list_shortcuts", json!({ "file": file })).await;
    assert_eq!(back["shortcuts"], flare["shortcuts"]);
    let tahoma = f.ok("list_shortcuts", json!({ "set": "tahoma" })).await;
    assert_ne!(tahoma["shortcuts"], flare["shortcuts"]);

    let (bad, _) = f.tool("list_shortcuts", json!({ "set": "nope" })).await;
    assert!(bad);
    let conflict = r#"{"schema":1,"set":"flare","bindings":{"Close":"CmdOrCtrl+O"}}"#;
    let (bad, _) = f.tool("list_shortcuts", json!({ "file": conflict })).await;
    assert!(bad);
}
