//! `flare draw-polygon`: the Polygon tool on the command line.

use super::*;

#[test]
fn the_corners_given_become_one_filled_shape_that_renders() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "100", "--height", "100"]);
    let drawn = json(&[
        "draw-polygon",
        &file,
        "--point",
        "10",
        "10",
        "--point",
        "90",
        "10",
        "--point",
        "50",
        "90",
        "--color",
        "FF0000",
        "--stroke",
        "0000FF",
        "--stroke-style",
        "dashed",
    ]);
    assert_eq!(drawn["corners"], 3, "{drawn}");
    let info = json(&["info", &file]);
    assert_eq!(info["scenes"][0]["layers"][0]["elements"], 1, "{info}");
    let out = dir.path().join("frame.png").to_string_lossy().into_owned();
    json(&["export", &file, &out]);
    let image = read_png(&out);
    assert_eq!(
        pixel(&image, 50, 35),
        [255, 0, 0, 255],
        "inside the triangle"
    );
    assert_eq!(pixel(&image, 10, 80), [255, 255, 255, 255], "outside it");
}

#[test]
fn too_few_corners_or_corners_on_a_line_are_refused_and_the_project_is_untouched() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let before = std::fs::read(&file).expect("read");
    for args in [
        vec!["--point", "0", "0", "--point", "5", "5"],
        vec![
            "--point", "0", "0", "--point", "5", "5", "--point", "10", "10",
        ],
    ] {
        let mut all = vec!["draw-polygon", file.as_str()];
        all.extend(args);
        let result = run(&all);
        assert!(!result.status.success());
        let said = String::from_utf8_lossy(&result.stderr);
        assert!(
            said.contains("corners") || said.contains("enclose"),
            "{said}"
        );
    }
    assert_eq!(std::fs::read(&file).expect("read"), before);
}
