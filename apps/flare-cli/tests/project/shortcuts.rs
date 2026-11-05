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
