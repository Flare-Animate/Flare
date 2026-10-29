//! The server: the open project, what the agent may do to it, and the rmcp handler.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};

use flare_history::EditorSession;
use flare_storage::StorageError;
use rmcp::{
    ServerHandler,
    handler::server::router::tool::ToolRouter,
    model::{ServerCapabilities, ServerConfig},
    tool_handler,
};
use serde_json::Value;

/// What the connected agent may do.
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    /// Allow tools that change the document or save it.
    pub write: bool,
    /// Directory exports may be written into. Exports are disabled when `None`.
    pub export_dir: Option<PathBuf>,
}

pub(crate) struct State {
    pub session: EditorSession,
    pub path: PathBuf,
    pub permissions: Permissions,
}

/// The open project behind a handle that rmcp can clone into each request.
#[derive(Clone)]
pub struct FlareServer {
    pub(crate) state: Arc<Mutex<State>>,
    pub(crate) tool_router: ToolRouter<Self>,
}

/// A tool's answer: JSON text on success, a message the agent can read on failure (rmcp marks
/// the failure as an error result).
pub(crate) type Outcome = Result<String, String>;

impl FlareServer {
    pub fn open(path: PathBuf, permissions: Permissions) -> Result<Self, StorageError> {
        let document = flare_storage::load(&path)?;
        Ok(Self {
            state: Arc::new(Mutex::new(State {
                session: EditorSession::new(document),
                path,
                permissions,
            })),
            tool_router: Self::router(),
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while the project was locked cannot leave it half-edited (edits are applied
        // whole or not at all), so the data is still sound.
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Runs a tool that only looks at the project.
    pub(crate) fn read(&self, f: impl FnOnce(&State) -> Result<Value, String>) -> Outcome {
        f(&self.lock()).map(|value| value.to_string())
    }

    /// Runs a tool that changes the project, after the permission and revision checks.
    pub(crate) fn write(
        &self,
        expected_revision: Option<u64>,
        f: impl FnOnce(&mut State) -> Result<Value, String>,
    ) -> Outcome {
        let mut state = self.lock();
        if !state.permissions.write {
            return Err(
                "this server is read-only; restart it with write access to change the project"
                    .to_owned(),
            );
        }
        if let Some(expected) = expected_revision {
            let current = state.session.revision();
            if expected != current {
                return Err(format!(
                    "revision mismatch: expected {expected} but the project is at {current}; call get_info and re-read before editing"
                ));
            }
        }
        f(&mut state).map(|value| value.to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for FlareServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build()).with_instructions(
            "Edit the open Flare project. Read tools are always available; changes need write \
             access and are only saved by the save tool. Pass expected_revision (from get_info) \
             to avoid overwriting newer changes.",
        )
    }
}
