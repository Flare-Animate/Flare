#![allow(clippy::expect_used)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flare-cli"))
        .args(args)
        .output()
        .expect("run cli")
}

fn json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

fn path(dir: &Path) -> String {
    dir.join("t.flare").to_string_lossy().into_owned()
}

#[test]
fn create_edit_reopen_workflow() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Art"])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "place",
        &file,
        &symbol,
        "--start",
        "0",
        "--duration",
        "12",
        "--x",
        "10",
    ]);
    json(&["place", &file, &symbol, "--start", "12", "--duration", "6"]);

    let info = json(&["info", &file]);
    assert_eq!(info["symbols"].as_array().expect("symbols").len(), 1);
    assert_eq!(info["scenes"][0]["layers"][0]["spans"], 2);

    let first = json(&["eval", &file, "--frame", "3"]);
    assert_eq!(first["placed"].as_array().expect("placed").len(), 1);
    assert_eq!(first["placed"][0]["matrix"][4], 10.0);
    assert_eq!(first["placed"][0]["matrix"][5], 0.0);
    let second = json(&["eval", &file, "--frame", "12"]);
    assert_eq!(second["placed"][0]["matrix"][4], 0.0);
    assert!(
        json(&["eval", &file, "--frame", "18"])["placed"]
            .as_array()
            .expect("placed")
            .is_empty()
    );
}

#[test]
fn rejected_edit_leaves_project_unchanged() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Art"])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&["place", &file, &symbol, "--duration", "10"]);
    let before = std::fs::read(&file).expect("read");

    let overlap = run(&["place", &file, &symbol, "--start", "5", "--duration", "3"]);
    assert!(!overlap.status.success());
    let dangling = run(&["place", &file, "00000000-0000-0000-0000-000000000000"]);
    assert!(!dangling.status.success());
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn new_refuses_to_overwrite() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    assert!(!run(&["new", &file]).status.success());
}
