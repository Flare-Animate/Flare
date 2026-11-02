use super::*;

fn layer_of(info: &Value, index: usize) -> &Value {
    &info["scenes"][0]["layers"][index]
}

#[test]
fn rectangles_drawn_at_one_frame_share_the_layer_and_its_keyframe() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let first = json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "8",
        "8",
        "--duration",
        "10",
    ]);
    let second = json(&[
        "draw-rect",
        &file,
        "--rect",
        "20",
        "0",
        "8",
        "8",
        "--start",
        "4",
    ]);
    assert_eq!(first["layer"], second["layer"], "the same layer");
    assert_eq!(
        (first["element"].as_u64(), second["element"].as_u64()),
        (Some(0), Some(1))
    );
    let info = json(&["info", &file]);
    assert_eq!(layer_of(&info, 0)["spans"], 1);
    assert_eq!(layer_of(&info, 0)["elements"], 2);
    assert_eq!(
        info["scenes"][0]["layers"]
            .as_array()
            .expect("layers")
            .len(),
        1
    );
}

#[test]
fn a_frame_without_a_keyframe_starts_one_and_a_named_layer_is_used() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file]);
    let ground = json(&["add-layer", &file, "Ground"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "16",
        "16",
        "--color",
        "FF0000",
        "--duration",
        "3",
    ]);
    json(&[
        "draw-rect",
        &file,
        "--layer",
        &ground,
        "--rect",
        "0",
        "0",
        "16",
        "16",
        "--color",
        "0000FF",
        "--start",
        "5",
        "--duration",
        "2",
    ]);
    let info = json(&["info", &file]);
    let counts: Vec<u64> = info["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .iter()
        .map(|l| l["elements"].as_u64().expect("count"))
        .collect();
    assert_eq!(counts.iter().sum::<u64>(), 2);
    json(&[
        "export", &file, &out, "--width", "16", "--height", "16", "--frame", "5",
    ]);
    assert_eq!(pixel(&read_png(&out), 8, 8), [0, 0, 255, 255]);
}

#[test]
fn a_layer_that_takes_no_drawing_is_refused_and_the_project_is_not_touched() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    json(&["layer", "lock", &file]);
    let before = std::fs::read(&file).expect("read");
    let refused = run(&["draw-rect", &file, "--rect", "0", "0", "5", "5"]);
    assert!(!refused.status.success());
    let message = format!("{:#?}", String::from_utf8_lossy(&refused.stderr));
    assert!(message.contains("locked"), "{message}");
    assert_eq!(std::fs::read(&file).expect("read"), before);
    json(&["layer", "unlock", &file]);
    json(&["layer", "hide", &file]);
    assert!(
        !run(&["draw-rect", &file, "--rect", "0", "0", "5", "5"])
            .status
            .success()
    );
}
