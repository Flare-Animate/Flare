use super::*;

/// A new project with one entity, `Hero`, given a transform at x = 3.
fn project(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "200", "--height", "100"]);
    let entity = json(&["game", "add-entity", &file, "Hero"])["entity"].as_u64();
    assert_eq!(entity, Some(1));
    json(&[
        "game",
        "set-component",
        &file,
        "--entity",
        "1",
        "transform",
        "x=3",
    ]);
    file
}

#[test]
fn a_game_is_built_and_saved_from_the_command_line() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    json(&["game", "gravity", &file, "--x", "0", "--y", "-980"]);
    json(&[
        "game",
        "set-component",
        &file,
        "--entity",
        "1",
        "sprite",
        "frames=4",
        "fps=8",
    ]);
    // A second process reads what the first one saved.
    let shown = json(&["game", "show", &file]);
    assert_eq!(shown["gravity"], serde_json::json!([0.0, -980.0]));
    assert_eq!(shown["entities"]["1"]["name"], "Hero");
    assert_eq!(
        shown["entities"]["1"]["components"]
            .as_object()
            .map(|m| m.len()),
        Some(2)
    );

    json(&["game", "remove-component", &file, "--entity", "1", "sprite"]);
    let shown = json(&["game", "show", &file]);
    assert_eq!(
        shown["entities"]["1"]["components"]
            .as_object()
            .map(|m| m.len()),
        Some(1)
    );
    json(&["game", "remove-entity", &file, "--entity", "1"]);
    assert_eq!(
        json(&["game", "show", &file]),
        serde_json::json!({
            "gravity": [0.0, -980.0],
            "entities": {}
        })
    );
}

#[test]
fn a_bad_edit_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    let before = json(&["game", "show", &file]);
    let f = file.as_str();
    for args in [
        vec![
            "game",
            "set-component",
            f,
            "--entity",
            "1",
            "sprite",
            "fsp=8",
        ],
        vec!["game", "set-component", f, "--entity", "1", "nonsense"],
        vec![
            "game",
            "set-component",
            f,
            "--entity",
            "9",
            "transform",
            "x=1",
        ],
        vec![
            "game",
            "set-component",
            f,
            "--entity",
            "0",
            "transform",
            "x=1",
        ],
        vec![
            "game",
            "set-component",
            f,
            "--entity",
            "1",
            "transform",
            "x",
        ],
        vec!["game", "remove-entity", f, "--entity", "9"],
        vec!["game", "gravity", f, "--x", "0", "--y", "nan"],
        vec!["game", "run", f, "--frames", "5", "--script", "missing.txt"],
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "{args:?} must be refused");
    }
    assert_eq!(json(&["game", "show", &file]), before);
}

#[test]
fn a_run_reports_its_digest_and_repeats_exactly() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    let script = dir.path().join("script.txt");
    std::fs::write(&script, "right;right,space;").expect("script");
    let script = script.to_string_lossy().into_owned();
    let args = [
        "game",
        "run",
        file.as_str(),
        "--frames",
        "30",
        "--fps",
        "60",
        "--script",
        script.as_str(),
    ];
    let first = json(&args);
    let second = json(&args);
    assert_eq!(first, second);
    assert_eq!(first["frames"], 30);
    assert_eq!(first["digest"].as_str().map(str::len), Some(16));
    assert!(first["events"].is_array());

    // A run never changes the project.
    let shown = json(&["game", "show", &file]);
    assert_eq!(shown["entities"]["1"]["name"], "Hero");
}

#[test]
fn the_game_exports_its_description_and_sheet_and_never_overwrites() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    let out = dir.path().join("out");
    let out_text = out.to_string_lossy().into_owned();
    let exported = json(&[
        "game", "export", &file, "--dir", &out_text, "--name", "hero",
    ]);
    assert_eq!(exported["frames"], 1);
    let description: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(out.join("hero.json")).expect("description"))
            .expect("json");
    assert_eq!(description["format"], "flare-game");
    assert_eq!(description["sheet"]["image"], "hero.png");
    assert_eq!(description["game"]["entities"]["1"]["name"], "Hero");
    let png = std::fs::read(out.join("hero.png")).expect("png");
    assert!(png.starts_with(&[0x89, b'P', b'N', b'G']));
    assert!(out.join("hero.sheet.json").exists());

    let again = run(&[
        "game", "export", &file, "--dir", &out_text, "--name", "hero",
    ]);
    assert!(!again.status.success());
    let other = run(&[
        "game", "export", &file, "--dir", &out_text, "--name", "../x",
    ]);
    assert!(!other.status.success());
}
