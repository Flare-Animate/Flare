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

#[tokio::test]
async fn a_search_lists_only_what_it_finds() {
    let f = fixture(false, false).await;
    let found = f.ok("list_shortcuts", json!({ "search": "cmd+s" })).await;
    let rows = found["shortcuts"].as_array().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["name"], "Save");
    let tools = f
        .ok("list_shortcuts", json!({ "search": "rotation" }))
        .await;
    assert_eq!(tools["shortcuts"][0]["shortcut"], "D");
    let none = f.ok("list_shortcuts", json!({ "search": "zzzz" })).await;
    assert_eq!(none["shortcuts"], json!([]));
}

#[tokio::test]
async fn an_animate_shortcut_file_is_read_with_a_report_and_a_bad_one_refused() {
    let f = fixture(false, false).await;
    let xml = "<keyboardshortcuts>\
        <keycmd><cmd>File.New</cmd><uchar>N</uchar><mod><cmdkey/></mod></keycmd>\
        <keycmd><cmd>Window.Output</cmd><key>8</key><mod/></keycmd>\
        <keycmd><cmd>Debug.StepIn</cmd><key>12</key><mod><altkey/></mod></keycmd>\
        </keyboardshortcuts>";
    let out = f
        .ok("import_animate_shortcuts", json!({ "xml": xml }))
        .await;
    assert_eq!(out["set"], "animate");
    assert_eq!(out["applied"][1]["keys"], "F2");
    assert_eq!(out["skipped"][0]["command"], "Debug.StepIn");
    let file = out["file"].to_string();
    let kept = f
        .ok("list_shortcuts", json!({ "file": file, "search": "f2" }))
        .await;
    assert_eq!(kept["shortcuts"][0]["action"], "ShowOutput");
    let (bad, _) = f
        .tool("import_animate_shortcuts", json!({ "xml": "<a></b>" }))
        .await;
    assert!(bad);
}
