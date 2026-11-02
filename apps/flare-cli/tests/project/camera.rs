use super::*;

/// A 200 by 100 project with a red 20 px square centered at (150, 50), 12 frames long.
fn project(dir: &Path) -> String {
    let file = path(dir);
    json(&["new", &file, "--width", "200", "--height", "100"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "140",
        "40",
        "20",
        "20",
        "--color",
        "FF0000",
        "--duration",
        "12",
    ]);
    file
}

fn picture(file: &str, dir: &Path, frame: u32) -> (Vec<u8>, u32, u32) {
    let out = dir
        .join(format!("f{frame}.png"))
        .to_string_lossy()
        .into_owned();
    json(&["export", file, &out, "--frame", &frame.to_string()]);
    read_png(&out)
}

#[test]
fn camera_makes_the_layer_and_the_exports_show_what_it_sees() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    let before = picture(&file, dir.path(), 0);
    assert_eq!(pixel(&before, 150, 50), [255, 0, 0, 255]);
    assert_eq!(pixel(&before, 100, 50)[..3], [255, 255, 255]);

    let report = json(&["camera", &file, "--x", "150", "--y", "50", "--zoom", "2"]);
    assert_eq!(report["zoom"], 2.0);
    assert_eq!(report["rotation"], 0.0);
    let after = picture(&file, dir.path(), 0);
    assert_eq!(
        pixel(&after, 100, 50),
        [255, 0, 0, 255],
        "the square is in the middle now"
    );
    assert_eq!(
        pixel(&after, 150, 50)[..3],
        [255, 255, 255],
        "and its old place is empty"
    );
    // It held to the end of the movie.
    assert_eq!(
        pixel(&picture(&file, dir.path(), 11), 100, 50),
        [255, 0, 0, 255]
    );
    let info = json(&["info", &file]);
    assert!(
        info.to_string().contains("Camera"),
        "the camera layer is listed: {info}"
    );
}

#[test]
fn a_motion_between_two_keyframes_moves_the_picture_and_rotation_is_in_degrees() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    json(&[
        "camera", &file, "--frame", "0", "--x", "100", "--y", "50", "--zoom", "1", "--motion",
        "motion",
    ]);
    json(&[
        "camera", &file, "--frame", "8", "--x", "150", "--y", "50", "--zoom", "2",
    ]);
    let (a, b, c) = (
        picture(&file, dir.path(), 0),
        picture(&file, dir.path(), 4),
        picture(&file, dir.path(), 8),
    );
    assert_ne!(a.0, b.0);
    assert_ne!(b.0, c.0);
    assert_eq!(pixel(&c, 100, 50), [255, 0, 0, 255]);
    // A quarter turn clockwise about the stage's middle puts the square above it.
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    let report = json(&[
        "camera",
        &file,
        "--x",
        "100",
        "--y",
        "50",
        "--rotation",
        "90",
    ]);
    assert!((report["rotation"].as_f64().expect("number") - 90.0).abs() < 1e-9);
    assert_eq!(
        pixel(&picture(&file, dir.path(), 0), 100, 3),
        [255, 0, 0, 255]
    );
}

#[test]
fn what_is_not_given_stays_and_a_bad_camera_changes_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    json(&["camera", &file, "--x", "30", "--y", "20", "--zoom", "3"]);
    // Only the zoom changes at frame 5; the point stays.
    let report = json(&["camera", &file, "--frame", "5", "--zoom", "4"]);
    assert_eq!(
        (report["x"].as_f64(), report["y"].as_f64()),
        (Some(30.0), Some(20.0))
    );
    assert_eq!(report["zoom"], 4.0);

    let saved = std::fs::read(&file).expect("read");
    for bad in [["--zoom", "0"], ["--zoom", "500"], ["--frame", "70000"]] {
        let out = run(&["camera", &file, bad[0], bad[1]]);
        assert!(!out.status.success(), "{bad:?}");
        assert!(!String::from_utf8_lossy(&out.stderr).is_empty());
    }
    assert_eq!(
        std::fs::read(&file).expect("read"),
        saved,
        "a refused camera writes nothing"
    );
    let missing = dir.path().join("none.flare").to_string_lossy().into_owned();
    assert!(!run(&["camera", &missing, "--zoom", "2"]).status.success());
}

#[test]
fn drawing_after_the_camera_goes_to_the_art_layer_not_the_camera() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    json(&["camera", &file, "--zoom", "1"]);
    // Frame 12 is past the art's 12 frames, so this only works if it lands on the art layer: a
    // camera layer refuses a shape.
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "100",
        "50",
        "10",
        "10",
        "--color",
        "00FF00",
        "--start",
        "12",
        "--duration",
        "2",
    ]);
    let green = picture(&file, dir.path(), 12);
    assert_eq!(pixel(&green, 105, 55), [0, 255, 0, 255]);
}
