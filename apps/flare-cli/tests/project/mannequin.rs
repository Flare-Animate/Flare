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

#[test]
fn a_pose_is_copied_from_marks_on_a_picture() {
    let marks = json(&["mannequin", "marks", "--width", "600", "--height", "800"]);
    let at = |name: &str| {
        let p = &marks["marks"][name];
        [p[0].as_f64().expect("x"), p[1].as_f64().expect("y")]
    };
    assert_eq!(marks["marks"].as_object().map(|m| m.len()), Some(14));
    // Raise the left arm straight out to the side: the elbow and hand level with the shoulder.
    let shoulder = at("left_shoulder");
    let reach = at("left_shoulder")[1] - at("left_wrist")[1];
    let mut moved = marks.clone();
    let half = reach.abs() * 0.53;
    moved["marks"]["left_elbow"] = serde_json::json!([shoulder[0] + half, shoulder[1]]);
    moved["marks"]["left_wrist"] = serde_json::json!([shoulder[0] + 2.0 * half, shoulder[1]]);
    let dir = tempfile::tempdir().expect("dir");
    let file = dir.path().join("marks.json");
    std::fs::write(&file, moved.to_string()).expect("write");
    let figure = json(&[
        "mannequin",
        "figure",
        "--marks",
        &file.to_string_lossy(),
        "--facing",
        "front",
    ]);
    let side = figure["pose"]["joints"]["left_shoulder"][2]
        .as_f64()
        .expect("the shoulder turned");
    assert!((side - 90.0).abs() < 3.0, "{figure}");
    assert_eq!(figure["view"]["yaw"], 0.0);

    std::fs::write(&file, r#"{"marks": {"head": [1, 2]}}"#).expect("write");
    let few = run(&["mannequin", "figure", "--marks", &file.to_string_lossy()]);
    assert!(!few.status.success());
    assert!(String::from_utf8_lossy(&few.stderr).contains("both hips and the neck"));
    let side = run(&["mannequin", "figure", "--facing", "sideways"]);
    assert!(String::from_utf8_lossy(&side.stderr).contains("no side called"));
}

#[test]
fn the_pose_of_the_person_in_a_picture_is_found_and_copied() {
    let dir = tempfile::tempdir().expect("dir");
    let wave = dir.path().join("wave.png").to_string_lossy().into_owned();
    // A picture of the mannequin itself waving, from the front.
    json(&[
        "mannequin",
        "picture",
        &wave,
        "--pose",
        "wave",
        "--yaw",
        "0",
        "--pitch",
        "0",
        "--fov",
        "0",
        "--width",
        "480",
        "--height",
        "640",
    ]);
    let figure = json(&["mannequin", "figure", "--picture", &wave]);
    let elbow = figure["pose"]["joints"]["right_elbow"][0]
        .as_f64()
        .expect("the right elbow bent");
    assert!(elbow < -60.0, "the waving arm is bent: {figure}");
    assert_eq!(figure["view"]["yaw"], 0.0, "seen as the picture is");

    let marks = json(&["mannequin", "marks", "--picture", &wave]);
    let raised = marks["marks"]["right_wrist"][1].as_f64().expect("the hand");
    let shoulder = marks["marks"]["right_shoulder"][1]
        .as_f64()
        .expect("the shoulder");
    assert!(
        raised < shoulder,
        "the hand is found above the shoulder: {marks}"
    );

    let blank = dir.path().join("blank.png");
    let rgba = vec![128u8; 40 * 30 * 4];
    std::fs::write(&blank, flare_runtime::rgba_png(40, 30, &rgba).expect("png")).expect("write");
    let none = run(&["mannequin", "figure", "--picture", &blank.to_string_lossy()]);
    assert!(!none.status.success());
    assert!(String::from_utf8_lossy(&none.stderr).contains("no person was found"));
    let both = run(&["mannequin", "figure", "--picture", &wave, "--marks", &wave]);
    assert!(String::from_utf8_lossy(&both.stderr).contains("cannot be used with"));
}
