//! The real binary, over real pipes.
#![allow(clippy::expect_used)]

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};
use serde_json::Value;

fn project(dir: &std::path::Path) -> std::path::PathBuf {
    let path = dir.join("p.flare");
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let document = Document::new(
        DocumentId::new(),
        FrameRate::new(24, 1).expect("rate"),
        vec![scene],
        vec![],
    )
    .expect("document");
    flare_storage::save(&document, &path).expect("save");
    path
}

#[test]
fn stdout_carries_only_protocol_and_the_session_ends_with_its_input() {
    let dir = tempfile::tempdir().expect("dir");
    let path = project(dir.path());
    let mut child = Command::new(env!("CARGO_BIN_EXE_flare-mcp"))
        .arg(&path)
        .arg("--allow-write")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn");
    let mut stdin = child.stdin.take().expect("stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("stdout"));
    let mut send = |message: &str| writeln!(stdin, "{message}").expect("write");
    let mut read = || -> Value {
        let mut line = String::new();
        stdout.read_line(&mut line).expect("read");
        serde_json::from_str(&line).expect("every stdout line is JSON-RPC")
    };
    send(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"t","version":"0"}}}"#,
    );
    let init = read();
    assert!(init["result"]["serverInfo"].is_object(), "{init}");
    send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    send(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"draw_rect","arguments":{"x":0,"y":0,"width":4,"height":4}}}"#,
    );
    assert_eq!(read()["result"]["isError"], false);
    send(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"save","arguments":{}}}"#,
    );
    assert_eq!(read()["result"]["isError"], false);
    drop(stdin); // end of input: the server must exit cleanly
    let status = child.wait().expect("wait");
    assert!(status.success());
    let saved = flare_storage::load(&path).expect("load");
    assert_eq!(
        saved.scenes()[0].timeline().layers()[0].spans().len(),
        1,
        "saved by the agent"
    );
}

#[test]
fn a_missing_project_is_refused_before_any_protocol_is_spoken() {
    let dir = tempfile::tempdir().expect("dir");
    let output = Command::new(env!("CARGO_BIN_EXE_flare-mcp"))
        .arg(dir.path().join("none.flare"))
        .output()
        .expect("run");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty(), "nothing on stdout but protocol");
}
