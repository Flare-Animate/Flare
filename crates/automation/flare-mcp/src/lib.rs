//! A Model Context Protocol server over stdio for one open project.
//!
//! Agents get the same edit commands and validation as the UI: every change goes through
//! an `EditorSession`, so a failed batch changes nothing, edits are undoable, and nothing
//! reaches disk until `save`. Safety defaults: the server is read-only unless writing is
//! allowed, exports only go to a configured directory under a sanitized file name, and a
//! mutating call can carry `expected_revision` so an agent cannot overwrite changes it has
//! not seen. Messages are newline-delimited JSON-RPC 2.0; batches are not supported.
#![forbid(unsafe_code)]

mod tools;

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use flare_edit::EditorSession;
use flare_storage::StorageError;
use serde_json::{Value, json};

/// Largest accepted request line, so a client cannot make the server buffer without bound.
pub const MAX_MESSAGE_BYTES: usize = 1024 * 1024;

const PROTOCOL_VERSION: &str = "2025-06-18";

/// What the connected agent may do.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    /// Allow tools that change the document or save it.
    pub write: bool,
    /// Directory exports may be written into. Exports are disabled when `None`.
    pub export_dir: Option<PathBuf>,
}

pub struct Server {
    pub(crate) session: EditorSession,
    pub(crate) path: PathBuf,
    pub(crate) permissions: Permissions,
}

impl Server {
    pub fn open(path: PathBuf, permissions: Permissions) -> Result<Self, StorageError> {
        let document = flare_storage::load(&path)?;
        Ok(Self {
            session: EditorSession::new(document),
            path,
            permissions,
        })
    }

    fn reply(id: &Value, result: Value) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
    }

    fn error(id: &Value, code: i64, message: &str) -> String {
        json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
            .to_string()
    }

    /// Handles one message. Returns the response line, or `None` for notifications.
    pub fn handle_line(&mut self, line: &str) -> Option<String> {
        if line.len() > MAX_MESSAGE_BYTES {
            return Some(Self::error(&Value::Null, -32600, "message too large"));
        }
        let message: Value = match serde_json::from_str(line) {
            Ok(value) => value,
            Err(_) => return Some(Self::error(&Value::Null, -32700, "parse error")),
        };
        let Some(object) = message.as_object() else {
            return Some(Self::error(
                &Value::Null,
                -32600,
                "expected a JSON object (batches are not supported)",
            ));
        };
        let id = object.get("id").cloned();
        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return id.map(|id| Self::error(&id, -32600, "jsonrpc must be \"2.0\""));
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            // A response from the client (it carries a result or an error) to something we
            // never sent is ignored: answering it could set off an endless exchange of errors.
            if object.contains_key("result") || object.contains_key("error") {
                return None;
            }
            return id.map(|id| Self::error(&id, -32600, "missing method"));
        };
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        // Requests carry an id and need an answer; notifications never get one.
        let id = id?;
        Some(match method {
            "initialize" => Self::reply(&id, self.initialize(&params)),
            "ping" => Self::reply(&id, json!({})),
            "tools/list" => Self::reply(
                &id,
                json!({ "tools": tools::definitions(&self.permissions) }),
            ),
            "tools/call" => self.tools_call(&id, &params),
            _ => Self::error(&id, -32601, "method not found"),
        })
    }

    fn initialize(&self, params: &Value) -> Value {
        // Echo the client's protocol version when we know it; otherwise offer ours.
        let requested = params.get("protocolVersion").and_then(Value::as_str);
        let version = match requested {
            Some(v) if ["2025-06-18", "2025-03-26", "2024-11-05"].contains(&v) => v,
            _ => PROTOCOL_VERSION,
        };
        json!({
            "protocolVersion": version,
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "flare", "version": env!("CARGO_PKG_VERSION") },
            "instructions": "Edit the open Flare project. Read tools are always available; \\
                changes need write access and are only saved by the save tool. Pass \\
                expected_revision (from get_info) to avoid overwriting newer changes."
        })
    }

    fn tools_call(&mut self, id: &Value, params: &Value) -> String {
        let Some(name) = params.get("name").and_then(Value::as_str) else {
            return Self::error(id, -32602, "tools/call needs a tool name");
        };
        let empty = serde_json::Map::new();
        let arguments = match params.get("arguments") {
            None | Some(Value::Null) => &empty,
            Some(Value::Object(map)) => map,
            Some(_) => return Self::error(id, -32602, "arguments must be an object"),
        };
        if !tools::exists(name) {
            return Self::error(id, -32602, &format!("unknown tool {name}"));
        }
        // Tool failures are results with isError set, so the agent can read and react.
        let outcome = tools::call(self, name, arguments);
        let (text, is_error) = match outcome {
            Ok(value) => (value.to_string(), false),
            Err(message) => (message, true),
        };
        Self::reply(
            id,
            json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
        )
    }

    /// Reads requests from `input` and writes responses to `output` until input ends.
    /// Lines longer than [`MAX_MESSAGE_BYTES`] are rejected without being buffered.
    pub fn serve(&mut self, mut input: impl BufRead, mut output: impl Write) -> io::Result<()> {
        while let Some(read) = read_bounded_line(&mut input, MAX_MESSAGE_BYTES)? {
            let response = match read {
                Line::TooLong => Some(Self::error(&Value::Null, -32600, "message too large")),
                Line::Text(line) if line.trim().is_empty() => None,
                Line::Text(line) => self.handle_line(&line),
            };
            if let Some(response) = response {
                writeln!(output, "{response}")?;
                output.flush()?;
            }
        }
        Ok(())
    }
}

enum Line {
    Text(String),
    TooLong,
}

/// Reads one line of at most `max` bytes. A longer line is skipped to its end without being
/// stored. Returns `None` at end of input.
fn read_bounded_line(input: &mut impl BufRead, max: usize) -> io::Result<Option<Line>> {
    let mut buffer = Vec::new();
    let mut too_long = false;
    let mut saw_any = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            break;
        }
        saw_any = true;
        let newline = available.iter().position(|b| *b == b'\n');
        let take = newline.map_or(available.len(), |i| i + 1);
        if !too_long {
            if buffer.len() + take > max + 1 {
                too_long = true;
                buffer.clear();
            } else {
                buffer.extend_from_slice(&available[..take]);
            }
        }
        input.consume(take);
        if newline.is_some() {
            break;
        }
    }
    if !saw_any {
        return Ok(None);
    }
    if too_long {
        return Ok(Some(Line::TooLong));
    }
    while matches!(buffer.last(), Some(b'\n' | b'\r')) {
        buffer.pop();
    }
    Ok(Some(Line::Text(
        String::from_utf8_lossy(&buffer).into_owned(),
    )))
}
