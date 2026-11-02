//! Tools that undo, redo and save.

use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::color::safe_name;
use crate::params::{ExportName, Guarded};
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

    #[tool(
        description = "Save the project (needs write access) to the file it was opened from, in that file's format: a .fla or .xfl is written as an Animate document (an unchanged one as the original, an edited one keeping every file Flare did not touch), a .flare as a project. The old file is replaced only by a whole new one. Reports what was rebuilt and what the format could not hold."
    )]
    fn save(&self, Parameters(args): Parameters<Guarded>) -> Outcome {
        self.write(args.expected_revision, |state| {
            if !state.owned && !flare_runtime::saves_in_place(state.session.document(), &state.path) {
                return Err(format!(
                    "{} cannot be written back as it is (a movie, a pre-CS5 .fla or a file too big to keep); use save_as to make a project from it",
                    state.path.display()
                ));
            }
            let saved = flare_runtime::save(state.session.document(), &state.path)
                .map_err(|e| e.to_string())?;
            state.session.mark_saved();
            Ok(saved_json(&saved))
        })
    }

    #[tool(
        description = "Save the project under a new name in the export directory (needs write access), as Save As does: the ending picks the format (.fla, .xfl as a folder with the .xfl inside, or .flare; a name with no ending gets .fla). Never replaces a file. The project is then that file, so save writes to it."
    )]
    fn save_as(&self, Parameters(args): Parameters<ExportName>) -> Outcome {
        self.write(None, |state| {
            let dir = state
                .permissions
                .export_dir
                .clone()
                .ok_or("save_as is disabled: no export directory was configured")?;
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let (target, _) = flare_runtime::resolve_save_path(&dir.join(safe_name(&args.name)?));
            if target.exists() || flare_runtime::xfl_folder(&target).exists() {
                return Err(format!("{} already exists", args.name));
            }
            let saved = flare_runtime::save(state.session.document(), &target)
                .map_err(|e| e.to_string())?;
            state.session.mark_saved();
            state.path = saved.path.clone();
            state.owned = true;
            Ok(saved_json(&saved))
        })
    }
}

/// What a save reports: where, in what format and, for an Animate format, what was written.
fn saved_json(saved: &flare_runtime::Saved) -> serde_json::Value {
    let mut answer = json!({
        "saved": saved.path.to_string_lossy(),
        "format": saved.format.extension(),
    });
    if let (Some(summary), Some(map)) = (&saved.summary, answer.as_object_mut()) {
        map.insert("files".into(), json!(summary.files));
        map.insert(
            "lossless".into(),
            json!({ "exact": summary.exact, "reason": summary.reason }),
        );
        map.insert("rebuilt".into(), json!(summary.rebuilt));
        map.insert("warnings".into(), json!(summary.warnings));
        map.insert("notes".into(), json!(summary.notes));
    }
    answer
}
