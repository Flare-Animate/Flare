use super::*;

#[test]
fn lists_sets_and_round_trips_a_file() {
    let flare = json(&["shortcuts", "list"]);
    assert_eq!(flare["set"], "flare");
    assert_eq!(flare["conflicts"], 0);
    let ot = json(&["shortcuts", "list", "--set", "opentoonz"]);
    assert_ne!(flare["shortcuts"], ot["shortcuts"]);

    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("keys.json");
    let file = file.to_string_lossy().into_owned();
    json(&["shortcuts", "export", "--set", "tahoma", "--out", &file]);
    let again = json(&["shortcuts", "list", "--file", &file]);
    assert_eq!(again["set"], "tahoma");
    let imported = json(&["shortcuts", "import", &file]);
    assert_eq!(imported["set"], "tahoma");
}

#[test]
fn a_bad_set_or_file_is_refused_and_writes_nothing() {
    assert!(
        !run(&["shortcuts", "list", "--set", "nope"])
            .status
            .success()
    );
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("bad.json");
    std::fs::write(
        &file,
        r#"{"schema":1,"set":"flare","bindings":{"Close":"CmdOrCtrl+O"}}"#,
    )
    .expect("write");
    let out = dir.path().join("out.json");
    let result = run(&[
        "shortcuts",
        "import",
        &file.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
    ]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("already runs"));
    assert!(!out.exists());
}

#[test]
fn a_search_lists_only_what_it_finds() {
    let found = json(&["shortcuts", "list", "--search", "save as"]);
    let rows = found["shortcuts"].as_array().expect("rows");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["shortcut"], "CmdOrCtrl+Shift+S");
    let keys = json(&["shortcuts", "list", "--set", "opentoonz", "--search", "j"]);
    assert_eq!(keys["shortcuts"][0]["name"], "Pencil");
    let none = json(&["shortcuts", "list", "--search", "zzzz"]);
    assert_eq!(none["shortcuts"], serde_json::json!([]));
}

#[test]
fn an_animate_shortcut_file_is_read_onto_the_animate_set_with_a_report() {
    let dir = tempfile::tempdir().expect("dir");
    let xml = dir.path().join("Custom.xml");
    std::fs::write(
        &xml,
        "<keyboardshortcuts>\
         <keycmd><cmd>File.New</cmd><uchar>N</uchar><mod><cmdkey/></mod></keycmd>\
         <keycmd><cmd>Tool.Rotate</cmd><uchar>R</uchar><mod/></keycmd>\
         <keycmd><cmd>Debug.StepIn</cmd><key>12</key><mod><altkey/></mod></keycmd>\
         </keyboardshortcuts>",
    )
    .expect("write");
    let out = dir.path().join("keys.json");
    let report = json(&[
        "shortcuts",
        "import-animate",
        &xml.to_string_lossy(),
        "--out",
        &out.to_string_lossy(),
    ]);
    assert_eq!(report["applied"].as_array().expect("rows").len(), 2);
    assert_eq!(report["skipped"][0]["command"], "Debug.StepIn");
    let kept = json(&[
        "shortcuts",
        "list",
        "--file",
        &out.to_string_lossy(),
        "--search",
        "rotation",
    ]);
    assert_eq!(kept["shortcuts"][0]["shortcut"], "R");

    std::fs::write(&xml, "<keyboardshortcuts><keycmd>").expect("write");
    let bad = dir.path().join("bad.json");
    let result = run(&[
        "shortcuts",
        "import-animate",
        &xml.to_string_lossy(),
        "--out",
        &bad.to_string_lossy(),
    ]);
    assert!(!result.status.success());
    assert!(!bad.exists(), "nothing is written");
}

#[test]
fn edit_gives_a_command_several_keys_takes_one_off_and_refuses_too_many() {
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("keys.json");
    let file = file.to_string_lossy().into_owned();
    let added = json(&[
        "shortcuts",
        "edit",
        "ToolBrush",
        "--add",
        "Shift+B",
        "--out",
        &file,
    ]);
    assert_eq!(added["keys"], serde_json::json!(["B", "Shift+B"]));
    assert_eq!(added["taken_from"], serde_json::Value::Null);
    let listed = json(&["shortcuts", "list", "--file", &file, "--search", "shift+b"]);
    assert_eq!(listed["shortcuts"][0]["action"], "ToolBrush");
    // Keys another command has are taken from it, and it is named.
    let taken = json(&[
        "shortcuts",
        "edit",
        "ToolPen",
        "--file",
        &file,
        "--add",
        "B",
        "--out",
        &file,
    ]);
    assert_eq!(taken["taken_from"], "ToolBrush");
    let removed = json(&[
        "shortcuts",
        "edit",
        "ToolBrush",
        "--file",
        &file,
        "--remove",
        "Shift+B",
    ]);
    assert_eq!(removed["keys"], serde_json::json!([]));
    // A fifth key is refused and the file is left as it was.
    for n in 1..=3 {
        let keys = format!("CmdOrCtrl+Alt+Shift+{n}");
        json(&[
            "shortcuts",
            "edit",
            "Save",
            "--file",
            &file,
            "--add",
            &keys,
            "--out",
            &file,
        ]);
    }
    let before = std::fs::read_to_string(&file).expect("file");
    let refused = run(&[
        "shortcuts",
        "edit",
        "Save",
        "--file",
        &file,
        "--add",
        "CmdOrCtrl+Alt+Shift+4",
        "--out",
        &file,
    ]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("at most 4"));
    assert_eq!(std::fs::read_to_string(&file).expect("file"), before);
    // Nothing asked, or an unknown command: refused.
    assert!(!run(&["shortcuts", "edit", "Save"]).status.success());
    assert!(
        !run(&["shortcuts", "edit", "Nope", "--clear"])
            .status
            .success()
    );
}
