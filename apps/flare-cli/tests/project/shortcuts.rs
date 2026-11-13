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
