//! `flare brush`: the wiring of `flare_tools::brush::lay_strokes` to the command line. What a
//! stroke's outline is, is tested in the library; here the options, the strokes file, the saved
//! project, the picture it renders and the refusals.

use super::*;

fn stage(file: &str) {
    json(&["new", file]);
    json(&[
        "set-stage",
        file,
        "--width",
        "100",
        "--height",
        "60",
        "--background",
        "ffffff",
    ]);
}

fn element_count(file: &str) -> usize {
    let info = json(&["info", file]);
    info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["elements"].as_u64().unwrap_or(0) as usize)
        .sum()
}

#[test]
fn a_stroke_along_points_is_painted_in_its_color() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let out = json(&[
        "brush",
        &file,
        "--along",
        "10",
        "30",
        "--along",
        "50",
        "30",
        "--along",
        "90",
        "30",
        "--size",
        "10",
        "--color",
        "ff0000",
        "--stabilizer",
        "0",
    ]);
    assert_eq!(out["drawn"], 1);
    assert_eq!(out["skipped"], serde_json::json!([]));
    let png = dir.path().join("t.png").to_string_lossy().into_owned();
    json(&["export", &file, &png]);
    let image = read_png(&png);
    assert_eq!(pixel(&image, 50, 30), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 50, 10), [255, 255, 255, 255]);
    // Half the width is 5, so the paint stops short of 8 below the line.
    assert_eq!(pixel(&image, 50, 38), [255, 255, 255, 255]);
}

#[test]
fn light_pressure_paints_a_thinner_line() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    json(&[
        "brush",
        &file,
        "--along",
        "10",
        "30",
        "--along",
        "50",
        "30",
        "--along",
        "90",
        "30",
        "--pressure",
        "0.2",
        "0.2",
        "0.2",
        "--size",
        "20",
        "--stabilizer",
        "0",
    ]);
    let png = dir.path().join("t.png").to_string_lossy().into_owned();
    json(&["export", &file, &png]);
    let image = read_png(&png);
    assert_eq!(pixel(&image, 50, 30), [0, 0, 0, 255]);
    // At full pressure the line reaches 10 below its centre; at a light touch, nowhere near.
    assert_eq!(pixel(&image, 50, 37), [255, 255, 255, 255]);
}

#[test]
fn a_strokes_file_lays_every_stroke_in_one_go() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let strokes = dir.path().join("strokes.json");
    std::fs::write(
        &strokes,
        "[[[10, 10], [90, 10]], [[10, 50, 0.5], [90, 50, 1.0]], [[5, 5], [5, 5]]]",
    )
    .expect("write");
    let out = json(&["brush", &file, "--strokes", &strokes.to_string_lossy()]);
    assert_eq!(out["drawn"], 2);
    assert_eq!(out["skipped"], serde_json::json!([2]));
    assert_eq!(element_count(&file), 2);
}

#[test]
fn refusals_name_the_problem_and_change_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    stage(&file);
    let before = std::fs::read(&file).expect("read");
    let strokes = dir.path().join("strokes.json");
    std::fs::write(&strokes, "[[[10, 10], [90]]]").expect("write");
    let cases: [(&[&str], &str); 6] = [
        (&["--along", "5", "5", "--along", "5", "5"], "draws nothing"),
        (
            &[
                "--along",
                "1",
                "1",
                "--along",
                "9",
                "9",
                "--profile",
                "airbrush",
            ],
            "no brush profile",
        ),
        (
            &["--along", "1", "1", "--along", "9", "9", "--size", "500"],
            "size must be",
        ),
        (
            &["--along", "1", "1", "--along", "9", "9", "--pressure", "1"],
            "one --pressure for each",
        ),
        (
            &["--strokes", &strokes.to_string_lossy()],
            "point 1 of stroke 0",
        ),
        (&[], "--along"),
    ];
    for (extra, message) in cases {
        let mut args = vec!["brush", file.as_str()];
        args.extend_from_slice(extra);
        let output = run(&args);
        assert!(!output.status.success(), "{extra:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(message), "{extra:?}: {stderr}");
    }
    assert_eq!(std::fs::read(&file).expect("read"), before);
}
