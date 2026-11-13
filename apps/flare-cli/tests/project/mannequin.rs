use super::*;

#[test]
fn presets_and_a_figure_are_printed() {
    let catalog = json(&["mannequin", "presets"]);
    assert!(catalog["poses"].as_array().is_some_and(|p| p.len() >= 10));
    assert_eq!(catalog["joints"].as_array().map(Vec::len), Some(17));

    let figure = json(&[
        "mannequin",
        "figure",
        "--body",
        "female",
        "--pose",
        "sit",
        "--yaw",
        "-30",
        "--joint",
        "head=-10,5,0",
    ]);
    assert_eq!(figure["body"], "female");
    assert_eq!(figure["view"]["yaw"], -30.0);
    assert_eq!(
        figure["pose"]["joints"]["head"],
        serde_json::json!([-10.0, 5.0, 0.0])
    );

    // What it prints is a figure file the next command reads back.
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("figure.json");
    std::fs::write(&file, figure.to_string()).expect("write");
    let again = json(&["mannequin", "figure", "--figure", &file.to_string_lossy()]);
    assert_eq!(again, figure);
}

#[test]
fn the_figure_is_placed_on_its_own_layer_and_drawn_where_asked() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file, "--width", "400", "--height", "400"]);
    let placed = json(&[
        "mannequin",
        "place",
        &file,
        "--pose",
        "t_pose",
        "--yaw",
        "0",
        "--pitch",
        "0",
        "--fov",
        "0",
        "--x",
        "200",
        "--y",
        "200",
        "--height",
        "300",
    ]);
    assert!(
        placed["bitmap"]["height"].as_u64().is_some_and(|h| h > 300),
        "{placed}"
    );
    let info = json(&["info", &file]);
    let layer = &info["scenes"][0]["layers"][0];
    assert_eq!(layer["name"], "Mannequin", "{info}");
    assert_eq!(layer["locked"], false, "{info}");

    let png = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["export", &file, &png]);
    let image = read_png(&png);
    // The middle of the chest is figure, and outside the reach of the arms is stage.
    assert_ne!(pixel(&image, 200, 120), [255, 255, 255, 255]);
    assert_eq!(pixel(&image, 200, 390), [255, 255, 255, 255]);
}

#[test]
fn a_wrong_name_or_angle_is_refused_and_nothing_is_written() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let before = std::fs::read(&file).expect("read");
    for (args, says) in [
        (vec!["--pose", "moonwalk"], "moonwalk"),
        (vec!["--joint", "left_knee=-30,0,0"], "left_knee"),
        (vec!["--body", "robot"], "robot"),
        (vec!["--pitch", "100"], "tilts"),
        (vec!["--height", "0"], "height"),
        (vec!["--reach", "left_elbow=0,0.5,0"], "hand or a foot"),
        (vec!["--reach", "left_wrist=0,3,0"], "cannot reach"),
        (vec!["--reach", "left_wrist"], "end=x,y,z"),
    ] {
        let mut all = vec!["mannequin", "place", file.as_str()];
        all.extend(args);
        let out = run(&all);
        assert!(!out.status.success(), "{all:?}");
        let error = String::from_utf8_lossy(&out.stderr);
        assert!(error.contains(says), "{all:?}: {error}");
    }
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn a_picture_of_the_figure_is_written_as_a_png() {
    let dir = tempfile::tempdir().expect("dir");
    let out = dir.path().join("pose.png").to_string_lossy().into_owned();
    let written = json(&[
        "mannequin",
        "picture",
        &out,
        "--body",
        "female",
        "--pose",
        "walk",
        "--reach",
        "right_wrist=-0.15,0.75,0.2",
        "--width",
        "200",
        "--height",
        "300",
    ]);
    assert_eq!(written["width"], 200);
    let image = read_png(&out);
    // The middle of the body is figure; the corner is see-through.
    assert_eq!(pixel(&image, 100, 150)[3], 255);
    assert_eq!(pixel(&image, 2, 2)[3], 0);

    let too_big = run(&["mannequin", "picture", &out, "--width", "9000"]);
    assert!(!too_big.status.success());
    assert!(String::from_utf8_lossy(&too_big.stderr).contains("16 to 4000"));
}

#[test]
fn more_figures_go_on_one_layer_in_the_same_keyframe_or_a_keyframe_of_their_own() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let first = json(&["mannequin", "place", &file]);
    let layer = first["layer"].as_str().expect("layer").to_owned();
    let layers = |file: &str| {
        json(&["info", file])["scenes"][0]["layers"]
            .as_array()
            .map(Vec::len)
    };
    let before = layers(&file);

    let joined = json(&[
        "mannequin",
        "place",
        &file,
        "--layer",
        &layer,
        "--pose",
        "run",
    ]);
    assert_eq!(
        (joined["keyframe"].as_u64(), joined["element"].as_u64()),
        (Some(0), Some(1))
    );
    let own = json(&[
        "mannequin",
        "place",
        &file,
        "--layer",
        &layer,
        "--frame",
        "10",
        "--new-keyframe",
        "--pose",
        "jump",
    ]);
    assert_eq!(
        (own["keyframe"].as_u64(), own["element"].as_u64()),
        (Some(10), Some(0))
    );
    assert_eq!(layers(&file), before, "all on the one layer");

    let bad = run(&["mannequin", "place", &file, "--new-keyframe"]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("--layer"));
    let missing = run(&[
        "mannequin",
        "place",
        &file,
        "--layer",
        "00000000-0000-0000-0000-000000000000",
    ]);
    assert!(String::from_utf8_lossy(&missing.stderr).contains("no layer"));
}
