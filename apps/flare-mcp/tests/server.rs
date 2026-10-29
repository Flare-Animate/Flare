#![allow(clippy::expect_used)]

mod common;

use common::fixture;
use serde_json::json;

#[tokio::test]
async fn the_server_announces_tools_and_every_tool_has_an_object_schema() {
    let f = fixture(true, true).await;
    let info = f.client.peer_info().expect("initialized");
    assert!(info.capabilities.tools.is_some());
    assert!(
        info.instructions
            .as_deref()
            .is_some_and(|i| i.contains("expected_revision"))
    );
    let tools = f.client.list_all_tools().await.expect("tools");
    let names: Vec<&str> = tools.iter().map(|t| t.name.as_ref()).collect();
    for expected in [
        "get_info",
        "evaluate_frame",
        "export_png",
        "draw_rect",
        "add_layer",
        "add_symbol",
        "place_instance",
        "frame_operation",
        "layer_operation",
        "set_stage",
        "undo",
        "redo",
        "save",
    ] {
        assert!(
            names.contains(&expected),
            "{expected} is missing: {names:?}"
        );
    }
    for tool in &tools {
        assert_eq!(
            tool.input_schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "{}",
            tool.name
        );
        assert!(
            tool.description.is_some(),
            "{} has no description",
            tool.name
        );
    }
}

#[tokio::test]
async fn a_read_only_server_refuses_every_change_and_leaves_the_project_untouched() {
    let f = fixture(false, false).await;
    let before = f.revision().await;
    for (tool, args) in [
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5 }),
        ),
        ("add_layer", json!({ "name": "x" })),
        ("set_stage", json!({ "width": 10 })),
        ("undo", json!({})),
        ("save", json!({})),
    ] {
        let (is_error, message) = f.tool(tool, args).await;
        assert!(
            is_error && message.as_str().expect("msg").contains("read-only"),
            "{tool}: {message}"
        );
    }
    assert_eq!(f.revision().await, before);
    assert!(
        !f.ok("get_info", json!({})).await["dirty"]
            .as_bool()
            .expect("dirty")
    );
}

#[tokio::test]
async fn edits_bump_the_revision_undo_and_persist_only_on_save() {
    let f = fixture(true, false).await;
    let start = f.revision().await;
    let drawn = f
        .ok(
            "draw_rect",
            json!({ "x": 2, "y": 2, "width": 10, "height": 10, "color": "FF0000" }),
        )
        .await;
    assert!(drawn["revision"].as_u64().expect("rev") > start);
    let info = f.ok("get_info", json!({})).await;
    assert_eq!(info["scenes"][0]["layers"][0]["spans"], 1);
    assert!(info["dirty"].as_bool().expect("dirty"));
    assert!(info["can_undo"].as_bool().expect("undo"));

    let on_disk = flare_storage::load(&f.path).expect("load");
    assert_eq!(
        on_disk.scenes()[0].timeline().layers()[0].spans().len(),
        0,
        "not saved yet"
    );

    assert!(
        f.ok("undo", json!({})).await["changed"]
            .as_bool()
            .expect("changed")
    );
    assert_eq!(
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][0]["spans"],
        0
    );
    assert!(
        f.ok("redo", json!({})).await["changed"]
            .as_bool()
            .expect("changed")
    );
    assert!(
        !f.ok("redo", json!({})).await["changed"]
            .as_bool()
            .expect("changed"),
        "nothing left to redo"
    );

    f.ok("save", json!({})).await;
    let saved = flare_storage::load(&f.path).expect("load");
    assert_eq!(saved.scenes()[0].timeline().layers()[0].spans().len(), 1);
    assert!(
        !f.ok("get_info", json!({})).await["dirty"]
            .as_bool()
            .expect("dirty")
    );
}

#[tokio::test]
async fn expected_revision_stops_an_agent_from_overwriting_changes_it_has_not_seen() {
    let f = fixture(true, false).await;
    let seen = f.revision().await;
    f.ok("add_layer", json!({ "name": "first" })).await; // someone else changes the project
    let (is_error, message) = f
        .tool(
            "add_layer",
            json!({ "name": "stale", "expected_revision": seen }),
        )
        .await;
    assert!(
        is_error && message.as_str().expect("m").contains("revision mismatch"),
        "{message}"
    );
    let layers = f.ok("get_info", json!({})).await["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .len();
    assert_eq!(layers, 2, "the stale edit changed nothing");
    let current = f.revision().await;
    f.ok(
        "add_layer",
        json!({ "name": "fresh", "expected_revision": current }),
    )
    .await;
    let (is_error, _) = f
        .tool(
            "add_layer",
            json!({ "name": "x", "expected_revision": "nope" }),
        )
        .await;
    assert!(is_error);
}

#[tokio::test]
async fn invalid_arguments_are_errors_that_change_nothing() {
    let f = fixture(true, false).await;
    let start = f.revision().await;
    let rect = |extra: serde_json::Value| {
        let mut base = json!({ "x": 0, "y": 0, "width": 5, "height": 5 });
        for (k, v) in extra.as_object().expect("object") {
            base[k] = v.clone();
        }
        base
    };
    let bad = [
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": -5, "height": 5 }),
        ),
        ("draw_rect", json!({ "x": 0, "y": 0, "width": 5 })),
        (
            "draw_rect",
            json!({ "x": "zero", "y": 0, "width": 5, "height": 5 }),
        ),
        ("draw_rect", rect(json!({ "color": "nope" }))),
        (
            "draw_rect",
            rect(json!({ "stroke": "000000", "stroke_width": 0 })),
        ),
        ("draw_rect", rect(json!({ "layer": "not-a-uuid" }))),
        ("draw_rect", rect(json!({ "duration": 0 }))),
        ("draw_rect", rect(json!({ "start": 99_999_999_999_u64 }))),
        (
            "place_instance",
            json!({ "symbol": "00000000-0000-0000-0000-000000000000" }),
        ),
        ("set_stage", json!({ "width": 0 })),
        ("set_stage", json!({ "height": 99999 })),
        (
            "frame_operation",
            json!({ "operation": "insert_keyframe", "at": 0 }),
        ),
        (
            "frame_operation",
            json!({ "operation": "explode", "at": 0 }),
        ),
        ("layer_operation", json!({ "operation": "delete" })),
        ("layer_operation", json!({ "operation": "rename" })),
        ("layer_operation", json!({ "operation": "move_up" })),
        ("evaluate_frame", json!({})),
    ];
    for (tool, args) in bad {
        let (is_error, message) = f.tool(tool, args.clone()).await;
        assert!(is_error, "{tool} {args} should have failed, got {message}");
    }
    assert_eq!(
        f.revision().await,
        start,
        "no failed call changed the project"
    );
}

#[tokio::test]
async fn symbols_instances_tweens_and_evaluation_work_through_tools() {
    let f = fixture(true, false).await;
    let symbol = f
        .ok(
            "add_symbol",
            json!({ "name": "Dot", "x": 0, "y": 0, "width": 4, "height": 4, "color": "00FF00" }),
        )
        .await;
    let id = symbol["symbol"].as_str().expect("id").to_owned();
    f.ok(
        "place_instance",
        json!({ "symbol": id, "duration": 4, "x": 0, "y": 5, "tween": true }),
    )
    .await;
    f.ok(
        "place_instance",
        json!({ "symbol": id, "start": 4, "x": 30, "y": 5 }),
    )
    .await;
    let at_two = f.ok("evaluate_frame", json!({ "frame": 2 })).await;
    assert_eq!(at_two["placed"].as_array().expect("placed").len(), 1);
    assert_eq!(
        at_two["placed"][0]["matrix"][4], 15.0,
        "halfway through the tween"
    );
    assert_eq!(at_two["shapes"].as_array().expect("shapes").len(), 1);
    assert_eq!(
        f.ok("evaluate_frame", json!({ "frame": 50 })).await["placed"],
        json!([])
    );
}

#[tokio::test]
async fn frame_layer_and_stage_operations_edit_the_timeline() {
    let f = fixture(true, false).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 5, "height": 5, "duration": 10 }),
    )
    .await;
    f.ok(
        "frame_operation",
        json!({ "operation": "insert_keyframe", "at": 4 }),
    )
    .await;
    assert_eq!(
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][0]["spans"],
        2
    );
    f.ok(
        "frame_operation",
        json!({ "operation": "clear_keyframe", "at": 4 }),
    )
    .await;
    assert_eq!(
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][0]["spans"],
        1
    );

    let layer = f.ok("add_layer", json!({ "name": "Back" })).await["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    for op in [
        json!({ "operation": "rename", "layer": layer, "name": "Backdrop" }),
        json!({ "operation": "hide", "layer": layer }),
        json!({ "operation": "lock", "layer": layer }),
    ] {
        f.ok("layer_operation", op).await;
    }
    let layers = f.ok("get_info", json!({})).await["scenes"][0]["layers"].clone();
    assert_eq!(
        (
            layers[1]["name"].as_str(),
            layers[1]["visible"].as_bool(),
            layers[1]["locked"].as_bool()
        ),
        (Some("Backdrop"), Some(false), Some(true))
    );
    f.ok(
        "layer_operation",
        json!({ "operation": "move_up", "layer": layer }),
    )
    .await;
    assert_eq!(
        f.ok("get_info", json!({})).await["scenes"][0]["layers"][0]["name"],
        "Backdrop"
    );
    f.ok(
        "layer_operation",
        json!({ "operation": "delete", "layer": layer }),
    )
    .await;

    f.ok("set_stage", json!({ "width": 64, "background": "102030" }))
        .await;
    let stage = f.ok("get_info", json!({})).await["stage"].clone();
    assert_eq!(
        (stage["width"].as_u64(), stage["height"].as_u64()),
        (Some(64), Some(20))
    );
    assert_eq!(stage["background"], json!([16, 32, 48, 255]));
}

fn read_png(path: &str) -> (Vec<u8>, u32) {
    let file = std::io::BufReader::new(std::fs::File::open(path).expect("open"));
    let mut reader = png::Decoder::new(file).read_info().expect("info");
    let mut buf = vec![0; reader.output_buffer_size().expect("size")];
    let info = reader.next_frame(&mut buf).expect("frame");
    buf.truncate(info.buffer_size());
    (buf, info.width)
}

#[tokio::test]
async fn exports_are_disabled_without_a_directory_and_confined_to_it_with_one() {
    let off = fixture(false, false).await;
    let (is_error, message) = off
        .tool("export_png", json!({ "frame": 0, "name": "a" }))
        .await;
    assert!(is_error && message.as_str().expect("m").contains("disabled"));

    // Reading is enough to export when a directory was configured.
    let f = fixture(true, true).await;
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 10, "height": 10, "color": "FF0000" }),
    )
    .await;
    let out = f
        .ok("export_png", json!({ "frame": 0, "name": "shot" }))
        .await;
    let path = out["path"].as_str().expect("path").to_owned();
    assert!(path.ends_with("exports/shot.png"));
    let (pixels, width) = read_png(&path);
    assert_eq!(width, 40, "defaults to the stage size");
    assert_eq!(
        &pixels[(5 * 40 + 5) * 4..(5 * 40 + 5) * 4 + 4],
        &[255, 0, 0, 255]
    );

    for name in [
        "../escape",
        "a/b",
        "/abs",
        ".hidden",
        "-flag",
        "",
        "with space",
        "x\0y",
    ] {
        let (is_error, _) = f
            .tool("export_png", json!({ "frame": 0, "name": name }))
            .await;
        assert!(is_error, "{name:?} must be rejected");
    }
    let listing = std::fs::read_dir(std::path::Path::new(&path).parent().expect("dir"))
        .expect("ls")
        .count();
    assert_eq!(
        listing, 1,
        "rejected names wrote nothing, and no temp files remain"
    );
}
