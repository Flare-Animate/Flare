//! A project on disk, a server for it and an MCP client wired together in memory.
#![allow(clippy::expect_used, dead_code)]

use std::path::PathBuf;

use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId};
use flare_document::{Document, Layer, Scene, Stage, Timeline};
use flare_mcp::{FlareServer, Permissions};
use rmcp::{RoleClient, ServiceExt, model::CallToolRequestParams, service::RunningService};
use serde_json::{Value, json};

pub struct Fixture {
    pub client: RunningService<RoleClient, ()>,
    pub path: PathBuf,
    pub dir: tempfile::TempDir,
}

pub async fn fixture(write: bool, exports: bool) -> Fixture {
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
    let server = FlareServer::open(path.clone(), Permissions { write, export_dir }).expect("open");
    let (server_io, client_io) = tokio::io::duplex(64 * 1024);
    tokio::spawn(async move {
        if let Ok(running) = server.serve(server_io).await {
            let _ = running.waiting().await;
        }
    });
    let client = ().serve(client_io).await.expect("client connects");
    Fixture { client, path, dir }
}

impl Fixture {
    /// Calls a tool; returns (is_error, parsed JSON or the message). A call the protocol itself
    /// rejects (arguments of the wrong shape) counts as an error with the protocol's message.
    pub async fn tool(&self, name: &'static str, arguments: Value) -> (bool, Value) {
        let object = arguments.as_object().cloned().expect("object arguments");
        let request = CallToolRequestParams::new(name).with_arguments(object);
        match self.client.call_tool(request).await {
            Ok(result) => {
                let text = result
                    .content
                    .first()
                    .and_then(|c| c.as_text())
                    .map(|t| t.text.clone())
                    .expect("text content");
                let value = serde_json::from_str(&text).unwrap_or(Value::String(text));
                (result.is_error.unwrap_or(false), value)
            }
            Err(error) => (true, Value::String(error.to_string())),
        }
    }

    pub async fn ok(&self, name: &'static str, arguments: Value) -> Value {
        let (is_error, value) = self.tool(name, arguments).await;
        assert!(!is_error, "{name} failed: {value}");
        value
    }

    pub async fn revision(&self) -> u64 {
        self.ok("get_info", json!({})).await["revision"]
            .as_u64()
            .expect("revision")
    }
}
