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
