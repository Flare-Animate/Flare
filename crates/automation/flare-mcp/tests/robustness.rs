//! A misbehaving or hostile agent. The server must answer every request with well-formed
//! JSON-RPC, never panic, and never let a bad call change the project.
#![allow(clippy::expect_used)]

use flare_core::{DocumentId, FrameRate, LayerId, SceneId};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_mcp::{Permissions, Server};
use serde_json::{Value, json};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() as usize) % n.max(1)
    }
}

fn server() -> (Server, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
    let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
    let rate = FrameRate::new(24, 1).expect("rate");
    let document = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
    flare_storage::save(&document, &path).expect("save");
    let permissions = Permissions {
        write: true,
        export_dir: Some(dir.path().join("out")),
    };
    (Server::open(path, permissions).expect("open"), dir)
}

fn revision(server: &mut Server) -> u64 {
    let reply = server
        .handle_line(
            r#"{"jsonrpc":"2.0","id":0,"method":"tools/call","params":{"name":"get_info"}}"#,
        )
        .expect("reply");
    let value: Value = serde_json::from_str(&reply).expect("json");
    let text = value["result"]["content"][0]["text"]
        .as_str()
        .expect("text");
    serde_json::from_str::<Value>(text).expect("info")["revision"]
        .as_u64()
        .expect("revision")
}

/// Every reply must be a JSON-RPC 2.0 response with exactly one of result and error.
fn assert_well_formed(reply: &str) {
    let value: Value = serde_json::from_str(reply).expect("replies are always valid JSON");
    assert_eq!(value["jsonrpc"], "2.0");
    assert!(value.get("id").is_some());
    assert!(
        value.get("result").is_some() ^ value.get("error").is_some(),
        "{reply}"
    );
}

#[test]
fn random_garbage_lines_get_a_wellformed_error_or_no_reply() {
    let (mut server, _dir) = server();
    let mut rng = Rng(0xA6E17);
    let mut replies = 0;
    for _ in 0..5_000 {
        let len = rng.below(200);
        let bytes: Vec<u8> = (0..len).map(|_| (rng.next() % 256) as u8).collect();
        let line = String::from_utf8_lossy(&bytes).into_owned();
        if let Some(reply) = server.handle_line(&line) {
            assert_well_formed(&reply);
            replies += 1;
        }
    }
    assert!(
        replies > 4_000,
        "garbage is answered, not ignored ({replies})"
    );
}

/// A random JSON value, deliberately the wrong type for most parameters.
fn random_value(rng: &mut Rng, depth: u32) -> Value {
    match rng.below(if depth > 2 { 6 } else { 8 }) {
        0 => Value::Null,
        1 => json!(rng.next().is_multiple_of(3)),
        2 => json!(rng.next() as i64 - 1_000_000),
        3 => json!(rng.next() as f64 / 7.0),
        4 => json!(
            [
                "",
                "x",
                "00000000-0000-0000-0000-000000000000",
                "../../etc/passwd",
                "FF00FF",
                "\u{0}"
            ][rng.below(6)]
        ),
        5 => json!(u64::MAX),
        6 => Value::Array(
            (0..rng.below(3))
                .map(|_| random_value(rng, depth + 1))
                .collect(),
        ),
        _ => {
            let mut map = serde_json::Map::new();
            for _ in 0..rng.below(3) {
                map.insert(format!("k{}", rng.below(5)), random_value(rng, depth + 1));
            }
            Value::Object(map)
        }
    }
}

#[test]
fn type_confused_tool_arguments_never_panic_and_failed_calls_change_nothing() {
    let (mut server, dir) = server();
    let tools = [
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
        "not_a_tool",
    ];
    let keys = [
        "x",
        "y",
        "width",
        "height",
        "color",
        "stroke",
        "stroke_width",
        "start",
        "duration",
        "layer",
        "name",
        "symbol",
        "frame",
        "at",
        "operation",
        "tween",
        "expected_revision",
        "background",
        "index",
    ];
    let mut rng = Rng(0xFEED);
    for round in 0..4_000 {
        let tool = tools[rng.below(tools.len())];
        let mut args = serde_json::Map::new();
        for _ in 0..rng.below(6) {
            args.insert(
                keys[rng.below(keys.len())].to_owned(),
                random_value(&mut rng, 0),
            );
        }
        let arguments = if rng.below(10) == 0 {
            random_value(&mut rng, 0)
        } else {
            Value::Object(args)
        };
        let line = json!({ "jsonrpc": "2.0", "id": round, "method": "tools/call",
                           "params": { "name": tool, "arguments": arguments } });

        let before = revision(&mut server);
        let reply = server
            .handle_line(&line.to_string())
            .expect("requests are always answered");
        assert_well_formed(&reply);
        let value: Value = serde_json::from_str(&reply).expect("json");
        let failed = value.get("error").is_some() || value["result"]["isError"] == json!(true);
        if failed {
            assert_eq!(
                revision(&mut server),
                before,
                "a failed {tool} call changed the project: {line}"
            );
        }
    }
    // Whatever was random, no export escaped its directory.
    let outside: Vec<_> = std::fs::read_dir(dir.path())
        .expect("ls")
        .filter_map(Result::ok)
        .filter(|e| e.file_name() != "p.flare" && e.file_name() != "out")
        .collect();
    assert!(
        outside.is_empty(),
        "stray files outside the export directory: {outside:?}"
    );
}

#[test]
fn request_ids_of_any_json_type_are_echoed_back_unchanged() {
    let (mut server, _dir) = server();
    for id in [
        json!(1),
        json!("abc"),
        json!(1.5),
        json!(-7),
        json!({"a": 1}),
        json!([1, 2]),
        json!(true),
    ] {
        let line = json!({ "jsonrpc": "2.0", "id": id, "method": "ping" }).to_string();
        let reply: Value =
            serde_json::from_str(&server.handle_line(&line).expect("reply")).expect("json");
        assert_eq!(reply["id"], id);
    }
    // A null id is a malformed request in JSON-RPC 2.0 usage here; it must still not panic.
    let null_id = json!({ "jsonrpc": "2.0", "id": null, "method": "ping" }).to_string();
    let _ = server.handle_line(&null_id);
}

#[test]
fn deeply_nested_and_huge_json_is_handled_without_blowing_the_stack() {
    let (mut server, _dir) = server();
    let nested = format!("{}1{}", "[".repeat(5_000), "]".repeat(5_000));
    let line = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{{"name":"get_info","arguments":{nested}}}}}"#
    );
    let reply = server.handle_line(&line).expect("reply");
    assert_well_formed(&reply);
    let wide = format!(
        r#"{{"jsonrpc":"2.0","id":1,"method":"ping","params":{{{}}}}}"#,
        (0..20_000)
            .map(|i| format!("\"k{i}\":{i}"))
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_well_formed(&server.handle_line(&wide).expect("reply"));
}
