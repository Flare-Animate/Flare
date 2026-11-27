//! `flare vector-erase`: the wiring of `flare_tools::eraser::vector` to the command line. What is
//! taken is tested in the library; here the file, the types and the refusals are checked.

use super::*;

/// How many elements the project's layers hold, from `flare info`.
fn shapes(file: &str) -> usize {
    let info = json(&["info", file]);
    info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["elements"].as_u64().unwrap_or(0) as usize)
        .sum()
}

#[test]
fn each_type_takes_whole_shapes_and_writes_the_project() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&["draw-rect", &file, "--rect", "0", "0", "40", "40"]);
    json(&["draw-rect", &file, "--rect", "100", "0", "40", "40"]);
    json(&["draw-rect", &file, "--rect", "200", "0", "40", "40"]);
    assert_eq!(shapes(&file), 3, "three rectangles to start");
    // A box that holds the first rectangle and crosses the second.
    let boxed = json(&[
        "vector-erase",
        &file,
        "--type",
        "rectangular",
        "--point",
        "-5",
        "-5",
        "--point",
        "120",
        "50",
    ]);
    assert_eq!(boxed["erased"], true);
    assert_eq!(shapes(&file), 2, "only the one wholly inside went");
    // A tip touching the corner of the second takes it whole.
    json(&["vector-erase", &file, "--point", "100", "0", "--size", "4"]);
    assert_eq!(shapes(&file), 1);
    json(&[
        "vector-erase",
        &file,
        "--type",
        "freehand",
        "--point",
        "190",
        "-10",
        "--point",
        "250",
        "-10",
        "--point",
        "250",
        "50",
        "--point",
        "190",
        "50",
    ]);
    assert_eq!(shapes(&file), 0, "the loop held the last");
}

#[test]
fn nothing_taken_or_a_wrong_request_is_refused_and_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&["draw-rect", &file, "--rect", "0", "0", "40", "40"]);
    let saved = std::fs::read(&file).expect("file");
    let refused = |args: &[&str], says: &str| {
        let out = run(&[&["vector-erase", &file][..], args].concat());
        assert!(!out.status.success(), "{args:?}");
        let text = String::from_utf8_lossy(&out.stderr);
        assert!(text.contains(says), "{args:?}: {text}");
    };
    refused(&["--point", "400", "400"], "no shape is taken");
    refused(
        &[
            "--type",
            "rectangular",
            "--point",
            "-5",
            "-5",
            "--point",
            "20",
            "20",
        ],
        "no shape is taken",
    );
    refused(&["--type", "rectangular", "--point", "0", "0"], "needs 2");
    refused(
        &["--type", "polyline", "--point", "0", "0"],
        "no type 'polyline'",
    );
    refused(&["--point", "0", "0", "--size", "0"], "size");
    assert_eq!(std::fs::read(&file).expect("file"), saved);
}

#[test]
fn segment_takes_a_stroke_out_to_where_another_crosses_it() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    // Two thin strokes, as the Brush draws them: one across, one down through it at x = 48..52.
    json(&["draw-rect", &file, "--rect", "0", "-2", "100", "4"]);
    json(&["draw-rect", &file, "--rect", "48", "-50", "4", "100"]);
    let boxed = [
        "vector-erase",
        &file,
        "--type",
        "rectangular",
        "--point",
        "-5",
        "-5",
        "--point",
        "55",
        "5",
    ];
    assert!(
        !run(&boxed).status.success(),
        "the stroke across runs out of the box"
    );
    let taken = json(&["vector-erase", &file, "--point", "80", "0", "--size", "4"]);
    assert_eq!(taken["type"], "segment", "segment is the default");
    assert_eq!(shapes(&file), 2, "both strokes are still there");
    json(&boxed);
    assert_eq!(
        shapes(&file),
        1,
        "the stroke across now ends at the far side of the other"
    );
}
