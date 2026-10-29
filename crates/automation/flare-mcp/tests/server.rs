#![allow(clippy::expect_used)]

use std::io::Cursor;
use std::path::PathBuf;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Document, Layer, Scene, Stage, Timeline};
use flare_mcp::{MAX_MESSAGE_BYTES, Permissions, Server};
use serde_json::{Value, json};

struct Fixture {
    server: Server,
    path: PathBuf,
    _dir: tempfile::TempDir,
    next_id: u64,
}

fn fixture(write: bool, exports: bool) -> Fixture {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let stage = Stage::new(40, 20, Rgba8::WHITE).expect("stage");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
        .expect("doc")
        .with_stage(stage);
    flare_storage::save(&document, &path).expect("save");
    let export_dir = exports.then(|| dir.path().join("exports"));
    let server = Server::open(path.clone(), Permissions { write, export_dir }).expect("open");
    Fixture {
        server,
        path,
        _dir: dir,
        next_id: 1,
    }
}

impl Fixture {
    fn request(&mut self, method: &str, params: Value) -> Value {
        self.next_id += 1;
        let line =
            json!({ "jsonrpc": "2.0", "id": self.next_id, "method": method, "params": params });
        let reply = self
            .server
            .handle_line(&line.to_string())
            .expect("a response");
        serde_json::from_str(&reply).expect("json reply")
    }

    /// Calls a tool; returns (is_error, parsed text content).
    fn tool(&mut self, name: &str, arguments: Value) -> (bool, Value) {
        let reply = self.request(
            "tools/call",
            json!({ "name": name, "arguments": arguments }),
        );
        let result = &reply["result"];
        let text = result["content"][0]["text"].as_str().expect("text content");
        let value = serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_owned()));
        (result["isError"].as_bool().expect("isError"), value)
    }

    fn ok(&mut self, name: &str, arguments: Value) -> Value {
        let (is_error, value) = self.tool(name, arguments);
        assert!(!is_error, "{name} failed: {value}");
        value
    }

    fn revision(&mut self) -> u64 {
        self.ok("get_info", json!({}))["revision"]
            .as_u64()
            .expect("revision")
    }
}

#[test]
fn initialize_negotiates_a_version_and_notifications_get_no_reply() {
    let mut f = fixture(false, false);
    let reply = f.request(
        "initialize",
        json!({ "protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {} }),
    );
    assert_eq!(reply["result"]["protocolVersion"], "2025-03-26");
    assert_eq!(reply["result"]["serverInfo"]["name"], "flare");
    assert!(reply["result"]["capabilities"]["tools"].is_object());
    let unknown = f.request("initialize", json!({ "protocolVersion": "1999-01-01" }));
    assert_eq!(
        unknown["result"]["protocolVersion"], "2025-06-18",
        "offers its own version instead"
    );

    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }).to_string();
    assert!(f.server.handle_line(&note).is_none());
    assert_eq!(f.request("ping", json!({}))["result"], json!({}));
}

#[test]
fn protocol_errors_use_the_standard_codes() {
    let mut f = fixture(false, false);
    let code = |f: &mut Fixture, line: &str| -> i64 {
        let reply = f.server.handle_line(line).expect("error response");
        serde_json::from_str::<Value>(&reply).expect("json")["error"]["code"]
            .as_i64()
            .expect("code")
    };
    assert_eq!(code(&mut f, "{ not json"), -32700);
    assert_eq!(
        code(
            &mut f,
            "[{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"ping\"}]"
        ),
        -32600,
        "no batches"
    );
    assert_eq!(
        code(&mut f, "{\"jsonrpc\":\"1.0\",\"id\":1,\"method\":\"ping\"}"),
        -32600
    );
    assert_eq!(
        code(&mut f, "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"nope\"}"),
        -32601
    );
    assert_eq!(
        code(
            &mut f,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{}}"
        ),
        -32602
    );
    assert_eq!(
        code(
            &mut f,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"nope\"}}"
        ),
        -32602
    );
    assert_eq!(
        code(
            &mut f,
            "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"tools/call\",\"params\":{\"name\":\"get_info\",\"arguments\":5}}"
        ),
        -32602
    );
    // Messages that are not requests get no reply at all.
    assert!(
        f.server
            .handle_line("{\"jsonrpc\":\"2.0\",\"method\":\"anything\"}")
            .is_none()
    );
}

#[test]
fn every_tool_is_listed_with_an_object_schema() {
    let mut f = fixture(false, false);
    let reply = f.request("tools/list", json!({}));
    let tools = reply["result"]["tools"].as_array().expect("tools");
    let names: Vec<&str> = tools
        .iter()
        .map(|t| t["name"].as_str().expect("name"))
        .collect();
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
        assert!(names.contains(&expected), "{expected} missing");
    }
    for tool in tools {
        assert_eq!(tool["inputSchema"]["type"], "object");
        assert!(tool["description"].as_str().is_some_and(|d| !d.is_empty()));
    }
    let draw = tools
        .iter()
        .find(|t| t["name"] == "draw_rect")
        .expect("draw_rect");
    assert!(
        draw["description"]
            .as_str()
            .expect("d")
            .contains("read-only"),
        "the agent is told up front"
    );
}

#[test]
fn a_read_only_server_refuses_every_change_and_leaves_the_project_untouched() {
    let mut f = fixture(false, false);
    let before = f.revision();
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
        let (is_error, message) = f.tool(tool, args);
        assert!(
            is_error && message.as_str().expect("msg").contains("read-only"),
            "{tool}: {message}"
        );
    }
    assert_eq!(f.revision(), before);
    assert!(
        !f.ok("get_info", json!({}))["dirty"]
            .as_bool()
            .expect("dirty")
    );
}

#[test]
fn edits_bump_the_revision_undo_and_persist_only_on_save() {
    let mut f = fixture(true, false);
    let start = f.revision();
    let drawn = f.ok(
        "draw_rect",
        json!({ "x": 2, "y": 2, "width": 10, "height": 10, "color": "FF0000" }),
    );
    assert!(drawn["revision"].as_u64().expect("rev") > start);
    let info = f.ok("get_info", json!({}));
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
        f.ok("undo", json!({}))["changed"]
            .as_bool()
            .expect("changed")
    );
    assert_eq!(
        f.ok("get_info", json!({}))["scenes"][0]["layers"][0]["spans"],
        0
    );
    assert!(
        f.ok("redo", json!({}))["changed"]
            .as_bool()
            .expect("changed")
    );
    assert!(
        !f.ok("redo", json!({}))["changed"]
            .as_bool()
            .expect("changed"),
        "nothing left to redo"
    );

    f.ok("save", json!({}));
    let saved = flare_storage::load(&f.path).expect("load");
    assert_eq!(saved.scenes()[0].timeline().layers()[0].spans().len(), 1);
    assert!(
        !f.ok("get_info", json!({}))["dirty"]
            .as_bool()
            .expect("dirty")
    );
}

#[test]
fn expected_revision_stops_an_agent_from_overwriting_changes_it_has_not_seen() {
    let mut f = fixture(true, false);
    let seen = f.revision();
    f.ok("add_layer", json!({ "name": "first" })); // someone else changes the project
    let (is_error, message) = f.tool(
        "add_layer",
        json!({ "name": "stale", "expected_revision": seen }),
    );
    assert!(
        is_error && message.as_str().expect("m").contains("revision mismatch"),
        "{message}"
    );
    let layers = f.ok("get_info", json!({}))["scenes"][0]["layers"]
        .as_array()
        .expect("layers")
        .len();
    assert_eq!(layers, 2, "the stale edit changed nothing");
    let current = f.revision();
    f.ok(
        "add_layer",
        json!({ "name": "fresh", "expected_revision": current }),
    );
    let (is_error, _) = f.tool(
        "add_layer",
        json!({ "name": "x", "expected_revision": "nope" }),
    );
    assert!(is_error);
}

#[test]
fn invalid_arguments_are_tool_errors_that_change_nothing() {
    let mut f = fixture(true, false);
    let start = f.revision();
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
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "color": "nope" }),
        ),
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "stroke": "000000", "stroke_width": 0 }),
        ),
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "layer": "not-a-uuid" }),
        ),
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "duration": 0 }),
        ),
        (
            "draw_rect",
            json!({ "x": 0, "y": 0, "width": 5, "height": 5, "start": 99999999999u64 }),
        ),
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
        let (is_error, message) = f.tool(tool, args.clone());
        assert!(is_error, "{tool} {args} should have failed, got {message}");
    }
    assert_eq!(f.revision(), start, "no failed call changed the project");
}

#[test]
fn symbols_instances_tweens_and_evaluation_work_through_tools() {
    let mut f = fixture(true, false);
    let symbol = f.ok(
        "add_symbol",
        json!({ "name": "Dot", "x": 0, "y": 0, "width": 4, "height": 4, "color": "00FF00" }),
    );
    let id = symbol["symbol"].as_str().expect("id").to_owned();
    f.ok(
        "place_instance",
        json!({ "symbol": id, "duration": 4, "x": 0, "y": 5, "tween": true }),
    );
    f.ok(
        "place_instance",
        json!({ "symbol": id, "start": 4, "x": 30, "y": 5 }),
    );
    let at_two = f.ok("evaluate_frame", json!({ "frame": 2 }));
    assert_eq!(at_two["placed"].as_array().expect("placed").len(), 1);
    assert_eq!(
        at_two["placed"][0]["matrix"][4], 15.0,
        "halfway through the tween"
    );
    assert_eq!(at_two["shapes"].as_array().expect("shapes").len(), 1);
    assert_eq!(
        f.ok("evaluate_frame", json!({ "frame": 50 }))["placed"],
        json!([])
    );
}

#[test]
fn frame_layer_and_stage_operations_edit_the_timeline() {
    let mut f = fixture(true, false);
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 5, "height": 5, "duration": 10 }),
    );
    f.ok(
        "frame_operation",
        json!({ "operation": "insert_keyframe", "at": 4 }),
    );
    assert_eq!(
        f.ok("get_info", json!({}))["scenes"][0]["layers"][0]["spans"],
        2
    );
    f.ok(
        "frame_operation",
        json!({ "operation": "clear_keyframe", "at": 4 }),
    );
    assert_eq!(
        f.ok("get_info", json!({}))["scenes"][0]["layers"][0]["spans"],
        1
    );

    let layer = f.ok("add_layer", json!({ "name": "Back" }))["layer"]
        .as_str()
        .expect("id")
        .to_owned();
    f.ok(
        "layer_operation",
        json!({ "operation": "rename", "layer": layer, "name": "Backdrop" }),
    );
    f.ok(
        "layer_operation",
        json!({ "operation": "hide", "layer": layer }),
    );
    f.ok(
        "layer_operation",
        json!({ "operation": "lock", "layer": layer }),
    );
    let layers = f.ok("get_info", json!({}))["scenes"][0]["layers"].clone();
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
    );
    assert_eq!(
        f.ok("get_info", json!({}))["scenes"][0]["layers"][0]["name"],
        "Backdrop"
    );
    f.ok(
        "layer_operation",
        json!({ "operation": "delete", "layer": layer }),
    );

    f.ok("set_stage", json!({ "width": 64, "background": "102030" }));
    let stage = f.ok("get_info", json!({}))["stage"].clone();
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

#[test]
fn exports_are_disabled_without_a_directory_and_confined_to_it_with_one() {
    let mut off = fixture(false, false);
    let (is_error, message) = off.tool("export_png", json!({ "frame": 0, "name": "a" }));
    assert!(is_error && message.as_str().expect("m").contains("disabled"));

    // Reading is enough to export when a directory was configured.
    let mut f = fixture(true, true);
    f.ok(
        "draw_rect",
        json!({ "x": 0, "y": 0, "width": 10, "height": 10, "color": "FF0000" }),
    );
    let out = f.ok("export_png", json!({ "frame": 0, "name": "shot" }));
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
        let (is_error, _) = f.tool("export_png", json!({ "frame": 0, "name": name }));
        assert!(is_error, "{name:?} must be rejected");
    }
    let listing: Vec<_> = std::fs::read_dir(PathBuf::from(&path).parent().expect("dir"))
        .expect("ls")
        .collect();
    assert_eq!(
        listing.len(),
        1,
        "rejected names wrote nothing, and no temp files remain"
    );
}

#[test]
fn serve_handles_a_stream_skips_blank_lines_and_rejects_oversized_messages() {
    let mut f = fixture(false, false);
    let ping = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" }).to_string();
    let huge = "x".repeat(MAX_MESSAGE_BYTES + 10);
    let input = format!("{ping}\n\n{huge}\n{ping}\n");
    let mut output = Vec::new();
    f.server
        .serve(Cursor::new(input), &mut output)
        .expect("serve");
    let lines: Vec<Value> = String::from_utf8(output)
        .expect("utf8")
        .lines()
        .map(|l| serde_json::from_str(l).expect("json"))
        .collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0]["result"], json!({}));
    assert_eq!(
        lines[1]["error"]["code"], -32600,
        "the oversized line is rejected"
    );
    assert_eq!(
        lines[2]["result"],
        json!({}),
        "and the stream carries on afterwards"
    );
}

#[test]
fn a_final_line_without_a_newline_is_still_handled() {
    let mut f = fixture(false, false);
    let ping = json!({ "jsonrpc": "2.0", "id": 7, "method": "ping" }).to_string();
    let mut output = Vec::new();
    f.server
        .serve(Cursor::new(ping), &mut output)
        .expect("serve");
    assert!(
        String::from_utf8(output)
            .expect("utf8")
            .contains("\"id\":7")
    );
}
