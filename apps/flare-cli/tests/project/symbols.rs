use super::*;

#[test]
fn set_kind_and_loop_change_the_project() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "100", "--height", "100"]);
    let symbol = json(&["add-symbol", &file, "Ball"])["symbol"]
        .as_str()
        .expect("id")
        .to_string();
    json(&["place", &file, &symbol, "--start", "2", "--duration", "4"]);
    let kind = json(&["symbol", "set-kind", &file, &symbol, "--kind", "movie-clip"]);
    assert_eq!(kind["kind"], "movie clip");
    let set = json(&[
        "symbol",
        "set-loop",
        &file,
        "--at",
        "2",
        "--mode",
        "play-once",
        "--first-frame",
        "3",
    ]);
    assert_eq!(set["mode"], "play once");
    // Every spelling the MCP server takes works here too.
    let spaced = json(&["symbol", "set-kind", &file, &symbol, "--kind", "Movie Clip"]);
    assert_eq!(spaced["kind"], "movie clip");
    let underscored = json(&[
        "symbol",
        "set-loop",
        &file,
        "--at",
        "2",
        "--mode",
        "Play_Once",
    ]);
    assert_eq!(underscored["mode"], "play once");
    assert!(
        !run(&["symbol", "set-kind", &file, &symbol, "--kind", "sprite"])
            .status
            .success()
    );
    // No keyframe at frame 9: refused, nothing changes.
    let bad = run(&["symbol", "set-loop", &file, "--at", "9", "--mode", "loop"]);
    assert!(!bad.status.success());
    assert!(
        !run(&[
            "symbol",
            "set-kind",
            &file,
            &Uuid4::nil(),
            "--kind",
            "button"
        ])
        .status
        .success()
    );
}

struct Uuid4;
impl Uuid4 {
    fn nil() -> String {
        "00000000-0000-0000-0000-000000000000".into()
    }
}

#[test]
fn a_rig_is_built_posed_and_evaluated() {
    let dir = tempfile::tempdir().expect("dir");
    let rig = dir.path().join("a.rig.json");
    let rig = rig.to_str().expect("utf8");
    let sym = "00000000-0000-7000-8000-000000000001";
    json(&[
        "rig",
        "create",
        rig,
        "--symbol",
        sym,
        "--kind",
        "movie-clip",
    ]);
    json(&["rig", "add-bone", rig, "--name", "upper", "--length", "10"]);
    json(&[
        "rig", "add-bone", rig, "--name", "lower", "--parent", "0", "--length", "10",
    ]);
    json(&["rig", "set-pose", rig, "--frame", "0", "--ik", "1,2,10,10"]);
    let out = json(&["rig", "eval", rig, "--frame", "0"]);
    assert_eq!(out["clock"], "Free");
    let lower = &out["bones"][1];
    let tip_x =
        lower["origin"][0].as_f64().expect("x") + 10.0 * lower["angle"].as_f64().expect("a").cos();
    assert!((tip_x - 10.0).abs() < 1e-3);
    assert!(
        !run(&[
            "rig", "add-bone", rig, "--name", "x", "--parent", "7", "--length", "1"
        ])
        .status
        .success()
    );
    assert!(
        !run(&["rig", "set-pose", rig, "--frame", "1", "--ik", "1,5,1,1"])
            .status
            .success()
    );
}

#[test]
fn add_symbol_takes_a_kind_and_refuses_a_name_the_library_has() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "100", "--height", "100"]);
    json(&["add-symbol", &file, "Hero", "--kind", "movie clip"]);
    let info = json(&["info", &file]);
    assert_eq!(info["symbols"][0]["name"], "Hero");
    assert_eq!(info["symbols"][0]["kind"], "movie clip");
    let again = run(&["add-symbol", &file, " Hero "]);
    assert!(!again.status.success());
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("already has a symbol of that name"),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert!(!run(&["add-symbol", &file, "  "]).status.success());
    assert_eq!(
        json(&["info", &file])["symbols"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn convert_makes_a_named_symbol_of_the_shape_with_its_origin_on_the_registration_point() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "200", "--height", "200"]);
    json(&["draw-rect", &file, "--rect", "20", "40", "100", "50"]);
    let made = json(&[
        "symbol",
        "convert",
        &file,
        "--at",
        "0",
        "--name",
        "Box",
        "--kind",
        "button",
        "--registration",
        "center",
    ]);
    assert_eq!(made["name"], "Box");
    assert_eq!(made["kind"], "button");
    assert_eq!(made["registration"], "center");
    let info = json(&["info", &file]);
    assert_eq!(info["symbols"][0]["name"], "Box");
    assert_eq!(info["symbols"][0]["kind"], "button");
    // What is in the keyframe is no longer a shape: a second conversion is refused, and nothing
    // changes.
    let again = run(&["symbol", "convert", &file, "--at", "0"]);
    assert!(!again.status.success());
    assert!(
        String::from_utf8_lossy(&again.stderr).contains("only a drawn shape"),
        "{}",
        String::from_utf8_lossy(&again.stderr)
    );
    assert!(
        !run(&[
            "symbol",
            "convert",
            &file,
            "--at",
            "0",
            "--registration",
            "middle"
        ])
        .status
        .success()
    );
    assert_eq!(
        json(&["info", &file])["symbols"].as_array().map(Vec::len),
        Some(1)
    );
}

#[test]
fn convert_names_the_symbol_itself_when_no_name_is_given() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "200", "--height", "200"]);
    json(&["add-symbol", &file, "Symbol 1"]);
    json(&["draw-rect", &file, "--rect", "0", "0", "10", "10"]);
    let made = json(&["symbol", "convert", &file, "--at", "0"]);
    assert_eq!(made["name"], "Symbol 2", "the first free name");
    assert_eq!(made["kind"], "graphic");
    assert_eq!(made["registration"], "top-left");
}
