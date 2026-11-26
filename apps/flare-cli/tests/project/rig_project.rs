//! `flare rig project`: the rigs a project keeps. The rules are tested in
//! `flare_editor::rig_library`; here the file, the options and the refusals are checked, and that
//! a rig is in the project (not beside it) after the command ends.

use super::*;

fn project_with_symbol() -> (tempfile::TempDir, String, String) {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Arm"])["symbol"]
        .as_str()
        .expect("symbol")
        .to_owned();
    (dir, file, symbol)
}

#[test]
fn a_rig_is_kept_in_the_project_and_survives_reopening() {
    let (_dir, file, symbol) = project_with_symbol();
    json(&["rig", "project", "create", &file, "--symbol", &symbol]);
    json(&[
        "rig", "project", "add-bone", &file, "--symbol", &symbol, "--name", "upper", "--length",
        "30",
    ]);
    let added = json(&[
        "rig", "project", "add-bone", &file, "--symbol", &symbol, "--name", "lower", "--parent",
        "0", "--length", "20",
    ]);
    assert_eq!(added["bone"], 1);
    json(&[
        "rig", "project", "set-pose", &file, "--symbol", &symbol, "--frame", "4", "--rotate",
        "0=0.5",
    ]);
    let listed = json(&["rig", "project", "list", &file]);
    assert_eq!(listed["rigs"][0]["bones"], 2, "{listed}");
    assert_eq!(listed["rigs"][0]["keys"], 1);
    let eval = json(&[
        "rig", "project", "eval", &file, "--symbol", &symbol, "--frame", "4",
    ]);
    assert_eq!(eval["bones"].as_array().map(Vec::len), Some(2), "{eval}");
    // The rig is in the project: export it, remove it, import it back.
    let dir = tempfile::tempdir().expect("dir");
    let out = dir
        .path()
        .join("arm.rig.json")
        .to_string_lossy()
        .into_owned();
    json(&["rig", "project", "export", &file, "--symbol", &symbol, &out]);
    json(&["rig", "project", "remove", &file, "--symbol", &symbol]);
    assert_eq!(
        json(&["rig", "project", "list", &file])["rigs"],
        serde_json::json!([])
    );
    json(&["rig", "project", "import", &file, &out]);
    assert_eq!(
        json(&["rig", "project", "list", &file])["rigs"][0]["bones"],
        2
    );
}

#[test]
fn refusals_name_the_reason_and_leave_the_project_alone() {
    let (_dir, file, symbol) = project_with_symbol();
    let saved = std::fs::read(&file).expect("file");
    let no_rig = run(&["rig", "project", "eval", &file, "--symbol", &symbol]);
    assert!(!no_rig.status.success());
    assert!(String::from_utf8_lossy(&no_rig.stderr).contains("no rig"));
    let no_symbol = run(&[
        "rig",
        "project",
        "create",
        &file,
        "--symbol",
        "00000000-0000-0000-0000-000000000001",
    ]);
    assert!(String::from_utf8_lossy(&no_symbol.stderr).contains("no such symbol"));
    json(&["rig", "project", "create", &file, "--symbol", &symbol]);
    let twice = run(&["rig", "project", "create", &file, "--symbol", &symbol]);
    assert!(String::from_utf8_lossy(&twice.stderr).contains("already has a rig"));
    let bad_parent = run(&[
        "rig", "project", "add-bone", &file, "--symbol", &symbol, "--name", "x", "--parent", "3",
        "--length", "1",
    ]);
    assert!(!bad_parent.status.success());
    assert_eq!(
        json(&["rig", "project", "list", &file])["rigs"][0]["bones"],
        0,
        "a refused bone changed nothing"
    );
    assert_ne!(std::fs::read(&file).expect("file"), saved);
}
