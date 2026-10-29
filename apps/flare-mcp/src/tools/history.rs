//! Tools that undo, redo and save.

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::params::Guarded;
use crate::server::{FlareServer, Outcome};

#[tool_router(router = history_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(description = "Undo the last change (needs write access).")]
    fn undo(&self, Parameters(args): Parameters<Guarded>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let changed = state.session.undo();
            Ok(json!({ "changed": changed, "revision": state.session.revision() }))
        })
    }

    #[tool(description = "Redo the last undone change (needs write access).")]
    fn redo(&self, Parameters(args): Parameters<Guarded>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let changed = state.session.redo();
            Ok(json!({ "changed": changed, "revision": state.session.revision() }))
        })
    }

    #[tool(description = "Save the project file (needs write access).")]
    fn save(&self, Parameters(args): Parameters<Guarded>) -> Outcome {
        self.write(args.expected_revision, |state| {
            flare_storage::save(state.session.document(), &state.path)
                .map_err(|e| e.to_string())?;
            state.session.mark_saved();
            Ok(json!({ "saved": state.path.to_string_lossy() }))
        })
    }
}
