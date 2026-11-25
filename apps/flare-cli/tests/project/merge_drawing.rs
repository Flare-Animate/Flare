//! `--merge` on the drawing commands: Merge Drawing, as with Object Drawing off on the desktop.
//! What merging does is tested in the library; here the flag, what `info` says and the picture.

use super::*;

fn layer0(file: &str) -> Value {
    json(&["info", file])["scenes"][0]["layers"][0].clone()
}

fn rect(file: &str, x: &str, color: &str, merge: bool) -> Value {
    let mut args = vec![
        "draw-rect",
        file,
        "--rect",
        x,
        "10",
        "40",
        "40",
        "--color",
        color,
    ];
    if merge {
        args.push("--merge");
    }
    json(&args)
}

#[test]
fn merged_rectangles_of_one_colour_join_and_objects_do_not() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "100", "--height", "60"]);
    rect(&file, "10", "FF0000", true);
    let joined = rect(&file, "30", "FF0000", true);
    assert_eq!(joined["elements"], serde_json::json!([0]), "{joined}");
    let layer = layer0(&file);
    assert_eq!(
        (layer["elements"].as_u64(), layer["merged"].as_u64()),
        (Some(1), Some(1)),
        "{layer}"
    );
    rect(&file, "50", "0000FF", false);
    let layer = layer0(&file);
    assert_eq!(
        (layer["elements"].as_u64(), layer["merged"].as_u64()),
        (Some(2), Some(1)),
        "{layer}"
    );
}

#[test]
fn a_merged_rectangle_of_another_colour_paints_over_and_both_stay_merged() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "100", "--height", "60"]);
    rect(&file, "10", "FF0000", true);
    rect(&file, "30", "0000FF", true);
    let out = dir.path().join("frame.png").to_string_lossy().into_owned();
    json(&["export", &file, &out]);
    let image = read_png(&out);
    assert_eq!(
        pixel(&image, 20, 30),
        [255, 0, 0, 255],
        "red left of the blue"
    );
    assert_eq!(pixel(&image, 40, 30), [0, 0, 255, 255], "blue on top");
    assert_eq!(layer0(&file)["merged"], 2);
}

#[test]
fn merged_brush_strokes_and_polygons_say_where_their_pieces_went() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "100", "--height", "60"]);
    let polygon = json(&[
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
        "50",
        "--color",
        "00FF00",
        "--stroke",
        "000000",
        "--merge",
    ]);
    assert_eq!(
        polygon["elements"],
        serde_json::json!([0, 1]),
        "a fill and a line: {polygon}"
    );
    let brush = json(&[
        "brush",
        &file,
        "--along",
        "0",
        "30",
        "--along",
        "100",
        "30",
        "--color",
        "00FF00",
        "--stabilizer",
        "0",
        "--merge",
    ]);
    assert_eq!(brush["drawn"], 1, "{brush}");
    assert_eq!(
        layer0(&file)["merged"],
        layer0(&file)["elements"],
        "all merged"
    );
}
