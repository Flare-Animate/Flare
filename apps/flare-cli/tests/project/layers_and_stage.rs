use super::*;

#[test]
fn stage_size_and_background_drive_export_defaults() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&[
        "new",
        &file,
        "--width",
        "32",
        "--height",
        "16",
        "--background",
        "102030",
    ]);
    let info = json(&["info", &file]);
    assert_eq!(
        (
            info["stage"]["width"].as_u64(),
            info["stage"]["height"].as_u64()
        ),
        (Some(32), Some(16))
    );

    // No size flags: the PNG matches the stage and uses its background color.
    json(&["export", &file, &out]);
    let image = read_png(&out);
    assert_eq!((image.1, image.2), (32, 16));
    assert_eq!(pixel(&image, 5, 5), [0x10, 0x20, 0x30, 255]);

    // The stage can be changed later, and only the given fields change.
    json(&["set-stage", &file, "--width", "8"]);
    json(&["export", &file, &out]);
    let image = read_png(&out);
    assert_eq!((image.1, image.2), (8, 16));
    assert_eq!(pixel(&image, 1, 1), [0x10, 0x20, 0x30, 255]);
}

#[test]
fn invalid_stages_are_rejected_without_touching_the_file() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    assert!(!run(&["new", &file, "--width", "0"]).status.success());
    assert!(!std::path::Path::new(&file).exists());
    json(&["new", &file]);
    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&["set-stage", &file, "--height", "99999"])
            .status
            .success()
    );
    assert!(
        !run(&["set-stage", &file, "--background", "nope"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn outlined_rectangles_render_with_their_stroke() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "30", "--height", "30"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "10",
        "10",
        "10",
        "10",
        "--color",
        "FF0000",
        "--stroke",
        "0000FF",
        "--stroke-width",
        "4",
    ]);
    json(&["export", &file, &out]);
    let image = read_png(&out);
    assert_eq!(pixel(&image, 15, 15), [255, 0, 0, 255], "fill inside");
    assert_eq!(
        pixel(&image, 10, 15),
        [0, 0, 255, 255],
        "stroke on the edge"
    );
    assert_eq!(
        pixel(&image, 8, 15),
        [0, 0, 255, 255],
        "stroke reaches 2px outside"
    );
    assert_eq!(
        pixel(&image, 5, 15),
        [255, 255, 255, 255],
        "background beyond"
    );

    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&[
            "draw-rect",
            &file,
            "--rect",
            "0",
            "0",
            "5",
            "5",
            "--stroke",
            "000000",
            "--stroke-width",
            "0"
        ])
        .status
        .success()
    );
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn frame_operations_edit_the_timeline_and_reject_what_does_not_apply() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let spans = |file: &str| json(&["info", file])["scenes"][0]["layers"][0]["spans"].as_u64();
    json(&["new", &file, "--width", "20", "--height", "20"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "5",
        "5",
        "--duration",
        "10",
    ]);

    json(&["frame", "insert-keyframe", &file, "--at", "4"]);
    assert_eq!(spans(&file), Some(2));
    json(&["frame", "insert-blank-keyframe", &file, "--at", "7"]);
    assert_eq!(spans(&file), Some(3));

    // Frame 7 onward is now empty, so a render there shows only the background.
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["export", &file, &out, "--frame", "8"]);
    assert_eq!(pixel(&read_png(&out), 2, 2), [255, 255, 255, 255]);
    json(&["export", &file, &out, "--frame", "5"]);
    assert_eq!(pixel(&read_png(&out), 2, 2), [0, 0, 0, 255]);

    json(&["frame", "clear-keyframe", &file, "--at", "7"]);
    assert_eq!(
        spans(&file),
        Some(2),
        "the blank keyframe merges into the previous span"
    );
    json(&["frame", "remove-frame", &file, "--at", "0"]);

    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&["frame", "insert-keyframe", &file, "--at", "0"])
            .status
            .success()
    );
    assert!(
        !run(&["frame", "remove-frame", &file, "--at", "500"])
            .status
            .success()
    );
    assert_eq!(std::fs::read(&file).expect("read"), before);
}

#[test]
fn export_frames_writes_a_numbered_animation_sequence() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let frames = dir.path().join("out").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "40", "--height", "10"]);
    let symbol = json(&[
        "add-symbol",
        &file,
        "Dot",
        "--rect",
        "0",
        "0",
        "4",
        "4",
        "--color",
        "FF0000",
    ])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "place",
        &file,
        &symbol,
        "--duration",
        "4",
        "--x",
        "0",
        "--y",
        "2",
        "--tween",
    ]);
    json(&[
        "place",
        &file,
        &symbol,
        "--start",
        "4",
        "--duration",
        "1",
        "--x",
        "32",
        "--y",
        "2",
    ]);

    let summary = json(&["export-frames", &file, &frames]);
    assert_eq!(
        (summary["frames"].as_u64(), summary["last"].as_u64()),
        (Some(5), Some(4))
    );
    for (frame, x) in [(0, 1), (2, 17), (4, 33)] {
        let image = read_png(&format!("{frames}/frame_{frame:05}.png"));
        assert_eq!(pixel(&image, x, 3), [255, 0, 0, 255], "frame {frame}");
        assert_eq!(
            pixel(&image, (x + 20) % 40, 3),
            [255, 255, 255, 255],
            "frame {frame} elsewhere"
        );
    }

    let range = json(&[
        "export-frames",
        &file,
        &frames,
        "--start",
        "1",
        "--end",
        "2",
    ]);
    assert_eq!(range["frames"], 2);
    assert!(
        !run(&[
            "export-frames",
            &file,
            &frames,
            "--start",
            "5",
            "--end",
            "2"
        ])
        .status
        .success()
    );
    assert!(
        !run(&["export-frames", &file, &frames, "--end", "999999"])
            .status
            .success()
    );
}
