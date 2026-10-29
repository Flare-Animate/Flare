#![allow(clippy::expect_used)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flare-cli"))
        .args(args)
        .output()
        .expect("run cli")
}

fn json(args: &[&str]) -> Value {
    let output = run(args);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("json")
}

fn path(dir: &Path) -> String {
    dir.join("t.flare").to_string_lossy().into_owned()
}

#[test]
fn create_edit_reopen_workflow() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Art"])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "place",
        &file,
        &symbol,
        "--start",
        "0",
        "--duration",
        "12",
        "--x",
        "10",
    ]);
    json(&["place", &file, &symbol, "--start", "12", "--duration", "6"]);

    let info = json(&["info", &file]);
    assert_eq!(info["symbols"].as_array().expect("symbols").len(), 1);
    assert_eq!(info["scenes"][0]["layers"][0]["spans"], 2);

    let first = json(&["eval", &file, "--frame", "3"]);
    assert_eq!(first["placed"].as_array().expect("placed").len(), 1);
    assert_eq!(first["placed"][0]["matrix"][4], 10.0);
    assert_eq!(first["placed"][0]["matrix"][5], 0.0);
    let second = json(&["eval", &file, "--frame", "12"]);
    assert_eq!(second["placed"][0]["matrix"][4], 0.0);
    assert!(
        json(&["eval", &file, "--frame", "18"])["placed"]
            .as_array()
            .expect("placed")
            .is_empty()
    );
}

#[test]
fn rejected_edit_leaves_project_unchanged() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Art"])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&["place", &file, &symbol, "--duration", "10"]);
    let before = std::fs::read(&file).expect("read");

    let overlap = run(&["place", &file, &symbol, "--start", "5", "--duration", "3"]);
    assert!(!overlap.status.success());
    let dangling = run(&["place", &file, "00000000-0000-0000-0000-000000000000"]);
    assert!(!dangling.status.success());
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn new_refuses_to_overwrite() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    assert!(!run(&["new", &file]).status.success());
}

#[test]
fn tween_survives_save_and_animates_on_eval() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&["new", &file]);
    let symbol = json(&["add-symbol", &file, "Art"])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&["place", &file, &symbol, "--duration", "10", "--tween"]);
    json(&[
        "place",
        &file,
        &symbol,
        "--start",
        "10",
        "--duration",
        "5",
        "--x",
        "100",
    ]);
    let mid = json(&["eval", &file, "--frame", "5"]);
    assert_eq!(mid["placed"][0]["matrix"][4], 50.0);
}

fn read_png(path: &str) -> (Vec<u8>, u32, u32) {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open png"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf.truncate(info.buffer_size());
    (buf, info.width, info.height)
}

fn pixel(image: &(Vec<u8>, u32, u32), x: u32, y: u32) -> [u8; 4] {
    let i = ((y * image.1 + x) * 4) as usize;
    [image.0[i], image.0[i + 1], image.0[i + 2], image.0[i + 3]]
}

#[test]
fn art_to_symbol_to_animated_instances_to_png() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("frame.png").to_string_lossy().into_owned();
    json(&["new", &file]);
    // Create art, as a symbol: a 20x20 red square.
    let symbol = json(&[
        "add-symbol",
        &file,
        "Square",
        "--rect",
        "0",
        "0",
        "20",
        "20",
        "--color",
        "FF0000",
    ])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    // Two instances on separate spans of the same layer: one tweens right.
    json(&[
        "place",
        &file,
        &symbol,
        "--duration",
        "10",
        "--x",
        "10",
        "--y",
        "10",
        "--tween",
    ]);
    json(&[
        "place",
        &file,
        &symbol,
        "--start",
        "10",
        "--duration",
        "5",
        "--x",
        "110",
        "--y",
        "10",
    ]);

    // Frame 0: square at x=10..30. Frame 5: tweened to x=60..80.
    json(&[
        "export", &file, &out, "--frame", "0", "--width", "200", "--height", "60",
    ]);
    let first = read_png(&out);
    assert_eq!((first.1, first.2), (200, 60));
    assert_eq!(pixel(&first, 20, 20), [255, 0, 0, 255]);
    assert_eq!(pixel(&first, 70, 20), [255, 255, 255, 255]);

    json(&[
        "export", &file, &out, "--frame", "5", "--width", "200", "--height", "60",
    ]);
    let mid = read_png(&out);
    assert_eq!(pixel(&mid, 20, 20), [255, 255, 255, 255]);
    assert_eq!(pixel(&mid, 70, 20), [255, 0, 0, 255]);

    // Transparent export keeps empty areas fully transparent.
    json(&[
        "export",
        &file,
        &out,
        "--frame",
        "0",
        "--width",
        "200",
        "--height",
        "60",
        "--transparent",
    ]);
    assert_eq!(pixel(&read_png(&out), 150, 50)[3], 0);
}

#[test]
fn scene_level_rectangles_and_bad_input_are_handled() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--color",
        "00FF0080",
    ]);
    json(&[
        "export",
        &file,
        &out,
        "--width",
        "16",
        "--height",
        "16",
        "--transparent",
    ]);
    let image = read_png(&out);
    let p = pixel(&image, 5, 5);
    assert_eq!((p[0], p[2], p[3]), (0, 0, 128));

    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&["draw-rect", &file, "--rect", "0", "0", "-1", "5"])
            .status
            .success()
    );
    assert!(
        !run(&[
            "draw-rect",
            &file,
            "--rect",
            "0",
            "0",
            "5",
            "5",
            "--color",
            "zzz"
        ])
        .status
        .success()
    );
    assert!(
        !run(&["export", &file, &out, "--width", "0"])
            .status
            .success()
    );
    assert!(
        !run(&["export", &file, &out, "--width", "99999"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn layers_allow_overlapping_frames_and_unknown_layers_are_rejected() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file]);
    let ground = json(&["add-layer", &file, "Ground"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    // Same frames on two layers is fine; the first layer paints on top.
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "8",
        "8",
        "--color",
        "FF0000",
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
    ]);
    json(&["export", &file, &out, "--width", "16", "--height", "16"]);
    let image = read_png(&out);
    assert_eq!(pixel(&image, 4, 4), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 12, 12), [0, 0, 255, 255]);

    let missing = "00000000-0000-0000-0000-000000000000";
    assert!(
        !run(&[
            "draw-rect",
            &file,
            "--layer",
            missing,
            "--rect",
            "0",
            "0",
            "1",
            "1"
        ])
        .status
        .success()
    );
}
