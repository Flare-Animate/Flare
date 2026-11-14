//! The reference mannequin through the MCP server.
#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn the_catalog_lists_poses_joints_and_an_example_figure() {
    let f = fixture(false, false).await;
    let catalog = f.ok("list_mannequin", json!({})).await;
    assert!(catalog["poses"].as_array().is_some_and(|p| p.len() >= 10));
    assert_eq!(catalog["joints"].as_array().map(Vec::len), Some(17));
    assert_eq!(catalog["example"]["version"], 1);
    assert_eq!(catalog["marks"].as_array().map(Vec::len), Some(14));
    assert_eq!(
        catalog["example_marks"]["marks"]
            .as_object()
            .map(|m| m.len()),
        Some(14)
    );
}

#[tokio::test]
async fn a_pose_is_copied_from_marks_and_wrong_marks_are_refused() {
    let f = fixture(true, true).await;
    let catalog = f.ok("list_mannequin", json!({})).await;
    let mut marks = catalog["example_marks"].clone();
    let shoulder = &catalog["example_marks"]["marks"]["left_shoulder"];
    let (x, y) = (
        shoulder[0].as_f64().expect("x"),
        shoulder[1].as_f64().expect("y"),
    );
    // The left arm raised straight out, level with the shoulder.
    marks["marks"]["left_elbow"] = json!([x + 80.0, y]);
    marks["marks"]["left_wrist"] = json!([x + 160.0, y]);
    let written = f
        .ok(
            "picture_mannequin",
            json!({ "marks": marks, "facing": "front", "width": 120, "height": 160, "name": "copied" }),
        )
        .await;
    assert!(
        written["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("copied.png"))
    );
    let placed = f
        .ok(
            "place_mannequin",
            json!({ "marks": marks, "facing": "left" }),
        )
        .await;
    assert!(placed["layer"].as_str().is_some(), "{placed}");
    for (args, says) in [
        (
            json!({ "marks": { "marks": { "head": [1, 2] } } }),
            "both hips and the neck",
        ),
        (
            json!({ "marks": { "marks": { "pelvis": [1, 2] } } }),
            "pelvis",
        ),
        (json!({ "facing": "sideways" }), "sideways"),
    ] {
        let (is_error, message) = f.tool("place_mannequin", args.clone()).await;
        assert!(is_error, "{args}");
        assert!(message.to_string().contains(says), "{args}: {message}");
    }
}

#[tokio::test]
async fn a_posed_figure_is_placed_on_a_locked_layer_and_undone_in_one_step() {
    let f = fixture(true, true).await;
    let before = f.revision().await;
    let placed = f
        .ok(
            "place_mannequin",
            json!({
                "body": "female",
                "preset": "run",
                "joints": { "head": [-10, 20, 0] },
                "reach": { "left_wrist": [0.2, 0.7, 0.15] },
                "yaw": 60, "pitch": 5, "fov": 30,
                "x": 20, "y": 10, "height": 18,
            }),
        )
        .await;
    assert!(
        placed["bitmap"]["width"].as_u64().is_some_and(|w| w > 10),
        "{placed}"
    );
    assert!(f.revision().await > before);
    let info = f.ok("get_info", json!({})).await;
    let layers = info["scenes"][0]["layers"].to_string();
    assert!(layers.contains("Mannequin"), "{info}");
    let png = f.ok("export_png", json!({ "frame": 0, "name": "m" })).await;
    assert!(png["path"].as_str().is_some());

    f.ok("undo", json!({})).await;
    let info = f.ok("get_info", json!({})).await;
    assert!(
        !info["scenes"][0]["layers"]
            .to_string()
            .contains("Mannequin")
    );
}

#[tokio::test]
async fn a_wrong_figure_is_refused_and_a_read_only_server_places_nothing() {
    let f = fixture(true, false).await;
    for (args, says) in [
        (json!({ "preset": "moonwalk" }), "moonwalk"),
        (
            json!({ "joints": { "left_knee": [-30, 0, 0] } }),
            "left_knee",
        ),
        (json!({ "figure": { "version": 9 } }), "version 9"),
        (json!({ "height": -5 }), "height"),
        (json!({ "name": " " }), "name"),
        (json!({ "reach": { "head": [0, 1, 0] } }), "hand or a foot"),
        (
            json!({ "reach": { "left_ankle": [0, 3, 0] } }),
            "cannot reach",
        ),
    ] {
        let (is_error, message) = f.tool("place_mannequin", args.clone()).await;
        assert!(is_error, "{args}");
        assert!(message.to_string().contains(says), "{args}: {message}");
    }
    let read_only = fixture(false, false).await;
    let (is_error, message) = read_only.tool("place_mannequin", json!({})).await;
    assert!(is_error && message.to_string().contains("read-only"));
}

#[tokio::test]
async fn a_picture_of_a_pose_is_written_to_the_export_directory() {
    let f = fixture(false, true).await;
    let written = f
        .ok(
            "picture_mannequin",
            json!({ "preset": "run", "yaw": 40, "width": 120, "height": 160, "name": "run" }),
        )
        .await;
    let path = written["path"].as_str().expect("path");
    assert!(path.ends_with("run.png"));
    let bytes = std::fs::read(path).expect("written");
    assert!(bytes.starts_with(b"\x89PNG"));
    let (is_error, message) = f
        .tool("picture_mannequin", json!({ "width": 9000, "name": "big" }))
        .await;
    assert!(
        is_error && message.to_string().contains("16 to 4000"),
        "{message}"
    );
    let none = fixture(false, false).await;
    let (is_error, _) = none.tool("picture_mannequin", json!({ "name": "x" })).await;
    assert!(is_error, "no export directory, no picture");
}

#[tokio::test]
async fn more_figures_join_a_layer_or_take_a_keyframe_of_their_own_there() {
    let f = fixture(true, false).await;
    let first = f.ok("place_mannequin", json!({})).await;
    let layer = first["layer"].as_str().expect("layer").to_owned();
    let joined = f
        .ok(
            "place_mannequin",
            json!({ "layer": layer, "preset": "run" }),
        )
        .await;
    assert_eq!(
        (joined["keyframe"].as_u64(), joined["element"].as_u64()),
        (Some(0), Some(1))
    );
    let own = f
        .ok(
            "place_mannequin",
            json!({ "layer": layer, "frame": 3, "new_keyframe": true, "preset": "jump" }),
        )
        .await;
    assert_eq!(
        (own["keyframe"].as_u64(), own["element"].as_u64()),
        (Some(3), Some(0))
    );
    let (is_error, message) = f
        .tool("place_mannequin", json!({ "new_keyframe": true }))
        .await;
    assert!(
        is_error && message.to_string().contains("layer"),
        "{message}"
    );
}

#[tokio::test]
async fn the_pose_of_the_person_in_a_picture_is_found_and_copied() {
    let f = fixture(true, true).await;
    // A picture of the mannequin itself waving, written to the export directory.
    f.ok(
        "picture_mannequin",
        json!({ "preset": "wave", "yaw": 0, "pitch": 0, "fov": 0, "width": 480, "height": 640, "name": "wave" }),
    )
    .await;
    let copied = f
        .ok(
            "picture_mannequin",
            json!({ "picture": "wave.png", "name": "copied" }),
        )
        .await;
    assert!(
        copied["path"]
            .as_str()
            .is_some_and(|p| p.ends_with("copied.png"))
    );
    let placed = f
        .ok("place_mannequin", json!({ "picture": "wave.png" }))
        .await;
    assert!(placed["layer"].as_str().is_some(), "{placed}");

    let blank = f.dir.path().join("exports").join("blank.png");
    let rgba = vec![128u8; 40 * 30 * 4];
    std::fs::write(&blank, flare_runtime::rgba_png(40, 30, &rgba).expect("png")).expect("write");
    for (args, says) in [
        (json!({ "picture": "blank.png" }), "no person"),
        (json!({ "picture": "missing.png" }), "missing.png"),
        (json!({ "picture": "../wave.png" }), "letters, digits"),
        (
            json!({ "picture": "wave.png", "marks": { "marks": {} } }),
            "not both",
        ),
    ] {
        let (is_error, message) = f.tool("place_mannequin", args.clone()).await;
        assert!(is_error, "{args}");
        assert!(message.to_string().contains(says), "{args}: {message}");
    }
}
