//! `flare fx`: a layer's effect graph edited from the command line, saved, and drawn by export.

use super::*;

fn project(dir: &Path) -> String {
    let file = path(dir);
    json(&[
        "new",
        &file,
        "--width",
        "100",
        "--height",
        "60",
        "--background",
        "FFFFFF",
    ]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "10",
        "10",
        "20",
        "20",
        "--color",
        "FF0000",
    ]);
    file
}

fn exported(dir: &Path, file: &str, frame: u32) -> (Vec<u8>, u32, u32) {
    let out = dir
        .join(format!("f{frame}.png"))
        .to_string_lossy()
        .into_owned();
    json(&["export", file, &out, "--frame", &frame.to_string()]);
    read_png(&out)
}

#[test]
fn a_drop_shadow_is_added_set_saved_and_exported() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    let before = exported(dir.path(), &file, 0);
    assert_eq!(pixel(&before, 35, 20), [255, 255, 255, 255]);

    let added = json(&[
        "fx",
        "add",
        &file,
        "drop-shadow",
        "distance=10",
        "angle=0",
        "blur=0",
        "opacity=100",
    ]);
    assert_eq!(added["node"], 3);
    assert_eq!(added["changed"], true);
    assert_eq!(added["effects"]["order"], serde_json::json!([1, 3, 2]));

    let after = exported(dir.path(), &file, 0);
    assert_eq!(pixel(&after, 20, 20), [255, 0, 0, 255], "the square on top");
    assert_eq!(
        pixel(&after, 35, 20),
        [0, 0, 0, 255],
        "its shadow ten to the right"
    );

    let set = json(&["fx", "set", &file, "--node", "3", "color=0000FF"]);
    assert_eq!(set["effects"]["nodes"][2]["settings"]["color"], "#0000FFFF");
    assert_eq!(
        pixel(&exported(dir.path(), &file, 0), 35, 20),
        [0, 0, 255, 255]
    );

    let shown = json(&["fx", "show", &file]);
    assert_eq!(shown["changed"], false);
    assert_eq!(shown["effects"], set["effects"], "saved in the project");
}

#[test]
fn nodes_are_wired_removed_and_cleared() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    json(&["fx", "add", &file, "glow"]);
    let over = json(&["fx", "add", &file, "over"]);
    assert_eq!(
        over["effects"]["nodes"][3]["inputs"][1]["from"], 3,
        "the chain goes under"
    );
    let wired = json(&[
        "fx", "connect", &file, "--from", "1", "--to", "4", "--input", "over",
    ]);
    assert_eq!(wired["effects"]["nodes"][3]["inputs"][0]["from"], 1);
    let cut = json(&["fx", "disconnect", &file, "--to", "4", "--input", "over"]);
    assert_eq!(
        cut["effects"]["nodes"][3]["inputs"][0]["from"],
        serde_json::Value::Null
    );
    let moved = json(&["fx", "move", &file, "--node", "3", "--at", "-40", "25.5"]);
    assert_eq!(
        moved["effects"]["nodes"][2]["at"],
        serde_json::json!([-40.0, 25.5])
    );
    let removed = json(&["fx", "remove", &file, "--node", "3"]);
    assert_eq!(removed["effects"]["order"], serde_json::json!([1, 4, 2]));
    let cleared = json(&["fx", "clear", &file]);
    assert_eq!(cleared["effects"], serde_json::Value::Null);
    assert_eq!(
        json(&["fx", "clear", &file])["changed"],
        false,
        "nothing left to clear"
    );
}

#[test]
fn snow_moves_from_frame_to_frame_in_an_export() {
    let dir = tempfile::tempdir().expect("dir");
    let file = path(dir.path());
    json(&[
        "new",
        &file,
        "--width",
        "100",
        "--height",
        "60",
        "--background",
        "FFFFFF",
    ]);
    json(&[
        "draw-rect",
        &file,
        "--rect",
        "10",
        "10",
        "20",
        "20",
        "--color",
        "FF0000",
        "--duration",
        "4",
    ]);
    json(&["fx", "add", &file, "snow", "flakes=300", "color=0000FF"]);
    let a = exported(dir.path(), &file, 1);
    assert_eq!(
        a,
        exported(dir.path(), &file, 1),
        "a frame always shows the same snow"
    );
    assert_ne!(
        a,
        exported(dir.path(), &file, 2),
        "and the next frame's has fallen"
    );
    let blue = |img: &(Vec<u8>, u32, u32)| {
        img.0
            .as_chunks::<4>()
            .0
            .iter()
            .filter(|p| i32::from(p[2]) > i32::from(p[0]) + 60)
            .count()
    };
    assert!(blue(&a) > 20, "blue flakes over the stage");
}

#[test]
fn mistakes_are_refused_with_the_reason_and_change_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let file = project(dir.path());
    json(&["fx", "add", &file, "blur"]);
    let before = json(&["fx", "show", &file]);
    for (args, said) in [
        (vec!["fx", "add", &file, "sparkle"], "unknown effect"),
        (
            vec!["fx", "add", &file, "blur", "radius=900"],
            "radius is 0 to 200",
        ),
        (
            vec!["fx", "set", &file, "--node", "3", "glitter=1"],
            "no setting",
        ),
        (
            vec!["fx", "connect", &file, "--from", "2", "--to", "3"],
            "Output",
        ),
        (
            vec!["fx", "connect", &file, "--from", "3", "--to", "3"],
            "loop",
        ),
        (
            vec!["fx", "remove", &file, "--node", "1"],
            "cannot be added or removed",
        ),
        (
            vec!["fx", "disconnect", &file, "--to", "3", "--input", "top"],
            "inputs are in",
        ),
    ] {
        let output = run(&args);
        assert!(!output.status.success(), "{args:?} was accepted");
        let err = String::from_utf8_lossy(&output.stderr);
        assert!(err.contains(said), "{args:?}: {err}");
    }
    assert_eq!(json(&["fx", "show", &file]), before);
    let kinds = json(&["fx", "kinds"]);
    assert_eq!(kinds.as_array().map(Vec::len), Some(8));
}
