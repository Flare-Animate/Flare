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

#[test]
fn layer_commands_manage_visibility_lock_order_and_deletion() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    let out = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["new", &file, "--width", "10", "--height", "10"]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "0",
        "0",
        "10",
        "10",
        "--color",
        "FF0000",
    ]);
    let second = json(&["add-layer", &file, "Second"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    let layers = |file: &str| json(&["info", file])["scenes"][0]["layers"].clone();

    json(&["layer", "hide", &file]);
    json(&["export", &file, &out]);
    assert_eq!(
        pixel(&read_png(&out), 5, 5),
        [255, 255, 255, 255],
        "hidden layers do not render"
    );
    json(&["layer", "show", &file]);
    json(&["export", &file, &out]);
    assert_eq!(pixel(&read_png(&out), 5, 5), [255, 0, 0, 255]);

    json(&[
        "layer", "rename", &file, "--layer", &second, "--name", "Backdrop",
    ]);
    json(&["layer", "lock", &file, "--layer", &second]);
    let info = layers(&file);
    assert_eq!(info[1]["name"], "Backdrop");
    assert_eq!(info[1]["locked"], true);
    assert_eq!(info[0]["locked"], false);

    json(&["layer", "up", &file, "--layer", &second]);
    assert_eq!(layers(&file)[0]["name"], "Backdrop");
    assert!(
        !run(&["layer", "up", &file, "--layer", &second])
            .status
            .success(),
        "already at the top"
    );

    let before = std::fs::read(&file).expect("read");
    assert!(
        !run(&["layer", "rename", &file]).status.success(),
        "needs --name"
    );
    assert_eq!(std::fs::read(&file).expect("read"), before);

    json(&["layer", "delete", &file, "--layer", &second]);
    assert_eq!(layers(&file).as_array().expect("array").len(), 1);
    assert!(
        !run(&["layer", "delete", &file]).status.success(),
        "the last layer stays"
    );
}

#[test]
fn import_turns_an_xfl_folder_into_a_project_and_lists_what_was_skipped() {
    let dir = tempfile::tempdir().expect("dir");
    let source = dir.path().join("src");
    std::fs::create_dir_all(source.join("LIBRARY")).expect("mkdir");
    std::fs::write(
        source.join("DOMDocument.xml"),
        r##"<DOMDocument frameRate="24" width="20" height="10" backgroundColor="#FFFFFF"><timelines><DOMTimeline name="Main"><layers>
        <DOMLayer name="Box"><frames><DOMFrame index="0" duration="3"><elements><DOMShape>
          <fills><FillStyle index="1"><SolidColor color="#FF0000"/></FillStyle></fills>
          <edges><Edge fillStyle1="1" edges="!0 0|100 0|100 100|0 100|0 0"/></edges>
        </DOMShape><DOMStaticText/></elements></DOMFrame></frames></DOMLayer>
        <DOMLayer name="Cut" layerType="mask"><frames/></DOMLayer></layers></DOMTimeline></timelines></DOMDocument>"##,
    )
    .expect("write");
    let out = dir.path().join("out.flare").to_string_lossy().into_owned();
    let source_arg = source.to_string_lossy().into_owned();

    let report = json(&["import", &source_arg, &out]);
    assert_eq!(
        (report["scenes"].as_u64(), report["symbols"].as_u64()),
        (Some(1), Some(0))
    );
    assert_eq!(
        report["unsupported"], 2,
        "the text object and the mask layer"
    );
    let messages: Vec<&str> = report["diagnostics"]
        .as_array()
        .expect("list")
        .iter()
        .filter_map(|d| d["message"].as_str())
        .collect();
    assert!(
        messages.iter().any(|m| m.contains("mask layers")),
        "{messages:?}"
    );

    // The result is a normal project: it has the imported size and renders the artwork.
    let info = json(&["info", &out]);
    assert_eq!(
        (
            info["stage"]["width"].as_u64(),
            info["stage"]["height"].as_u64()
        ),
        (Some(20), Some(10))
    );
    let png = dir.path().join("f.png").to_string_lossy().into_owned();
    json(&["export", &out, &png]);
    assert_eq!(pixel(&read_png(&png), 2, 2), [255, 0, 0, 255]);

    assert!(
        !run(&["import", &source_arg, &out]).status.success(),
        "never overwrites"
    );
    let missing = dir.path().join("nothing").to_string_lossy().into_owned();
    let other = dir.path().join("o2.flare").to_string_lossy().into_owned();
    assert!(!run(&["import", &missing, &other]).status.success());
    assert!(
        !std::path::Path::new(&other).exists(),
        "a failed import leaves no file behind"
    );
}

#[test]
fn export_xfl_round_trips_through_the_cli_as_a_folder_and_as_a_fla() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&[
        "new",
        &file,
        "--width",
        "40",
        "--height",
        "20",
        "--background",
        "102030",
    ]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "2",
        "2",
        "10",
        "10",
        "--color",
        "FF0000",
        "--duration",
        "4",
    ]);
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
        "00FF00",
    ])["symbol"]
        .as_str()
        .expect("id")
        .to_owned();
    let layer = json(&["add-layer", &file, "Movers"])["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    json(&[
        "place",
        &file,
        &symbol,
        "--layer",
        &layer,
        "--duration",
        "4",
        "--x",
        "20",
        "--y",
        "5",
        "--tween",
    ]);
    json(&[
        "place", &file, &symbol, "--layer", &layer, "--start", "4", "--x", "30", "--y", "5",
    ]);

    let folder = dir.path().join("proj_xfl").to_string_lossy().into_owned();
    let report = json(&["export-xfl", &file, &folder]);
    assert_eq!(report["format"], "xfl");
    assert!(
        std::path::Path::new(&folder)
            .join("DOMDocument.xml")
            .is_file()
    );
    let fla = dir.path().join("proj.fla").to_string_lossy().into_owned();
    assert_eq!(json(&["export-xfl", &file, &fla])["format"], "fla");

    for (source, name) in [(&folder, "from_folder"), (&fla, "from_fla")] {
        let reimported = dir
            .path()
            .join(format!("{name}.flare"))
            .to_string_lossy()
            .into_owned();
        json(&["import", source, &reimported]);
        let info = json(&["info", &reimported]);
        assert_eq!(
            (
                info["stage"]["width"].as_u64(),
                info["stage"]["height"].as_u64()
            ),
            (Some(40), Some(20))
        );
        assert_eq!(info["symbols"].as_array().expect("symbols").len(), 1);
        // The same artwork renders the same way, including the tween halfway through.
        for frame in ["0", "2"] {
            let (a, b) = (
                dir.path()
                    .join(format!("{name}_a{frame}.png"))
                    .to_string_lossy()
                    .into_owned(),
                dir.path()
                    .join(format!("{name}_b{frame}.png"))
                    .to_string_lossy()
                    .into_owned(),
            );
            json(&["export", &file, &a, "--frame", frame]);
            json(&["export", &reimported, &b, "--frame", frame]);
            assert_eq!(
                read_png(&a).0,
                read_png(&b).0,
                "{name} frame {frame} differs"
            );
        }
    }
    assert!(
        !run(&["export-xfl", &file, &folder]).status.success(),
        "never overwrites"
    );
    assert!(!run(&["export-xfl", &file, &fla]).status.success());
}

#[test]
fn publish_swf_writes_a_file_the_ruffle_reader_accepts_and_never_overwrites() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&[
        "new",
        &file,
        "--width",
        "64",
        "--height",
        "32",
        "--background",
        "102030",
    ]);
    let symbol = json(&[
        "add-symbol",
        &file,
        "Dot",
        "--rect",
        "0",
        "0",
        "8",
        "8",
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
        "6",
        "--x",
        "0",
        "--y",
        "4",
        "--tween",
    ]);
    json(&[
        "place", &file, &symbol, "--start", "6", "--x", "40", "--y", "4",
    ]);

    let out = dir.path().join("movie.swf").to_string_lossy().into_owned();
    let report = json(&["publish-swf", &file, &out]);
    assert_eq!(report["frames"], 7);
    assert!(
        report["diagnostics"]
            .as_array()
            .expect("list")
            .iter()
            .any(|d| d["message"].as_str().is_some_and(|m| m.contains("baked")))
    );

    let bytes = std::fs::read(&out).expect("read");
    assert_eq!(&bytes[..3], b"CWS");
    let buf = swf::decompress_swf(&bytes[..]).expect("decompress");
    let parsed = swf::parse_swf(&buf).expect("the SWF parses");
    assert_eq!(parsed.header.num_frames(), 7);
    assert_eq!(
        (
            parsed.header.stage_size().x_max.to_pixels(),
            parsed.header.stage_size().y_max.to_pixels()
        ),
        (64.0, 32.0)
    );
    assert_eq!(
        parsed
            .tags
            .iter()
            .filter(|t| matches!(t, swf::Tag::ShowFrame))
            .count(),
        7
    );

    let plain = dir.path().join("plain.swf").to_string_lossy().into_owned();
    json(&["publish-swf", &file, &plain, "--no-compress"]);
    assert_eq!(&std::fs::read(&plain).expect("read")[..3], b"FWS");
    assert!(
        !run(&["publish-swf", &file, &out]).status.success(),
        "never overwrites"
    );
    let missing = dir.path().join("none.flare").to_string_lossy().into_owned();
    let other = dir.path().join("o.swf").to_string_lossy().into_owned();
    assert!(!run(&["publish-swf", &missing, &other]).status.success());
    assert!(
        !std::path::Path::new(&other).exists(),
        "a failed publish leaves no file"
    );
}
