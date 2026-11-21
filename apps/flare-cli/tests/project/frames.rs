use super::*;

fn two_keyframes(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "40", "--height", "20"]);
    for (x, start, duration) in [("0", "0", "5"), ("10", "5", "3")] {
        json(&[
            "draw-rect",
            &file,
            "--rect",
            x,
            "0",
            "5",
            "5",
            "--start",
            start,
            "--duration",
            duration,
        ]);
    }
    file
}

#[test]
fn frames_lists_the_cells_and_goto_finds_keyframes_and_refuses_what_is_not_there() {
    let dir = tempfile::tempdir().expect("dir");
    let file = two_keyframes(dir.path());
    let listed = json(&["frames", &file, "--first", "4", "--count", "3"]);
    assert_eq!(listed["frames"], 8);
    let cells = listed["cells"].as_array().expect("cells");
    assert_eq!(cells.len(), 3);
    assert_eq!(cells[1]["frame"], 5);
    assert_eq!(cells[1]["keyframes"], 1);

    let next = json(&["goto", &file, "next-keyframe", "--from", "2"]);
    assert_eq!(next["frame"], 5);
    assert_eq!(json(&["goto", &file, "last"])["frame"], 7);
    assert_eq!(json(&["goto", &file, "3"])["shows_art"], true);

    for bad in [vec!["goto", &file, "99"], vec!["goto", &file, "soon"]] {
        let output = run(&bad);
        assert!(!output.status.success(), "{bad:?}");
    }
    let output = run(&["goto", &file, "previous-keyframe", "--from", "0"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no frame or keyframe"));
}
