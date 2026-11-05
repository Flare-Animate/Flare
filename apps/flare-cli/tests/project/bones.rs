use super::*;

/// A 200 by 100 project with two instances of a 10 px red square, at (50, 50) and (100, 50),
/// each on a layer of its own for 10 frames. Returns the file and the two instances.
fn project(dir: &Path) -> (String, String, String) {
    let file = path(dir);
    json(&["new", &file, "--width", "200", "--height", "100"]);
    let symbol = json(&[
        "add-symbol",
        &file,
        "Part",
        "--rect",
        "-5",
        "-5",
        "10",
        "10",
        "--color",
        "FF0000",
    ])["symbol"]
        .as_str()
        .expect("symbol")
        .to_owned();
    let second = json(&["add-layer", &file, "Second"])["layer"]
        .as_str()
        .expect("layer")
        .to_owned();
    let place = |x: &str, layer: Option<&str>| {
        let mut args = vec![
            "place",
            &file,
            &symbol,
            "--x",
            x,
            "--y",
            "50",
            "--duration",
            "10",
        ];
        if let Some(layer) = layer {
            args.extend(["--layer", layer]);
        }
        json(&args)["instance"]
            .as_str()
            .expect("instance")
            .to_owned()
    };
    let shoulder = place("50", None);
    let hand = place("100", Some(&second));
    (file, shoulder, hand)
}

fn picture(file: &str, dir: &Path, frame: u32) -> (Vec<u8>, u32, u32) {
    let out = dir
        .join(format!("b{frame}.png"))
        .to_string_lossy()
        .into_owned();
    json(&["export", file, &out, "--frame", &frame.to_string()]);
    read_png(&out)
}

#[test]
fn a_bone_links_two_instances_and_a_pose_moves_what_is_exported() {
    let dir = tempfile::tempdir().expect("dir");
    let (file, shoulder, hand) = project(dir.path());
    let report = json(&["bone", "add", &file, "--from", &shoulder, "--to", &hand]);
    let joints = &report["armatures"][0]["joints"];
    assert_eq!(joints.as_array().map(Vec::len), Some(2), "{report}");
    assert_eq!(joints[1]["parent"], 0);
    assert_eq!(joints[1]["x"], 100.0);
    let info = json(&["info", &file]);
    assert_eq!(info["scenes"][0]["layers"][0]["armature"], true, "{info}");

    // Turn the shoulder a quarter turn at frame 6: the hand goes to (50, 100), below the stage,
    // and the pose before tweens into it.
    let posed = json(&[
        "bone", "pose", &file, "--joint", "0", "--frame", "6", "--turn", "90",
    ]);
    let hand_at = &posed["armatures"][0]["joints"][1];
    assert!(
        (hand_at["x"].as_f64().expect("x") - 50.0).abs() < 1e-9,
        "{posed}"
    );
    assert!(
        (hand_at["y"].as_f64().expect("y") - 100.0).abs() < 1e-9,
        "{posed}"
    );
    assert_eq!(
        pixel(&picture(&file, dir.path(), 0), 100, 50),
        [255, 0, 0, 255]
    );
    let moved = picture(&file, dir.path(), 6);
    assert_eq!(
        pixel(&moved, 100, 50)[..3],
        [255, 255, 255],
        "the hand left"
    );
    assert_eq!(
        pixel(&moved, 50, 50),
        [255, 0, 0, 255],
        "the shoulder stayed"
    );

    // Pull the hand back up to (90, 50) with inverse kinematics.
    json(&[
        "bone", "pose", &file, "--joint", "1", "--frame", "6", "--to", "90", "50",
    ]);
    let listed = json(&["bones", &file, "--frame", "6"]);
    let hand_at = &listed["armatures"][0]["joints"][1];
    assert!(
        (hand_at["x"].as_f64().expect("x") - 100.0).abs() < 1e-6,
        "{listed}"
    );
    assert!(
        (hand_at["y"].as_f64().expect("y") - 50.0).abs() < 1e-6,
        "out of reach, it points"
    );

    // Release: plain instances again, where the pose put them.
    json(&["bone", "release", &file]);
    let info = json(&["info", &file]);
    assert_eq!(info["scenes"][0]["layers"][0]["armature"], false, "{info}");
}

#[test]
fn bones_say_what_went_wrong() {
    let dir = tempfile::tempdir().expect("dir");
    let (file, shoulder, _) = project(dir.path());
    let failed = run(&["bone", "add", &file, "--from", &shoulder, "--to", &shoulder]);
    assert!(!failed.status.success());
    let message = String::from_utf8_lossy(&failed.stderr);
    assert!(message.contains("bones"), "{message}");
    assert!(message.contains("two different instances"), "{message}");
    let failed = run(&["bone", "pose", &file, "--joint", "0", "--turn", "10"]);
    assert!(!failed.status.success());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("has no armature"));
}
