//! Bringing a Flash movie (or another file Flare imports) from the export directory into the
//! open project, as File > Import > Import to Stage and Import to Library do in the window.

use flare_editor::import_into::{Destination, import_into};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::color::safe_name;
use crate::ops::{apply, first_scene};
use crate::params::ImportIntoProject;
use crate::server::{FlareServer, Outcome};

#[tool_router(router = import_into_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Bring a Flash movie (.swf, .spl, .swt, .swc) or document (.fla, or another file import_project reads) from the configured export directory into the open project, as Animate's File > Import > Import to Stage: its library comes with it (symbols renamed where names are taken, pictures shared where the project has them), and its main timeline becomes a Graphic symbol named after the file on a new top layer of the first scene from `frame`. With `library: true`, into the library only (Import to Library). Replies with the symbol, its name, the layer and notes on what was left out. One undo step. Requires write permission."
    )]
    fn import_into_project(&self, Parameters(args): Parameters<ImportIntoProject>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let dir = super::export::export_dir(state)?
                .canonicalize()
                .map_err(|error| format!("cannot resolve the export directory: {error}"))?;
            let path = dir
                .join(safe_name(&args.source)?)
                .canonicalize()
                .map_err(|error| {
                    format!(
                        "cannot resolve {} in the export directory: {error}",
                        args.source
                    )
                })?;
            if !path.starts_with(&dir) {
                return Err("the file must stay inside the export directory".to_owned());
            }
            let imported = flare_import::import_path(&path)
                .map_err(|error| format!("cannot import {}: {error}", args.source))?;
            let destination = if args.library.unwrap_or(false) {
                Destination::Library
            } else {
                Destination::Stage {
                    scene: first_scene(state)?,
                    frame: args.frame.unwrap_or(0),
                }
            };
            let name = path.file_stem().map_or_else(
                || "Imported".to_owned(),
                |stem| stem.to_string_lossy().into_owned(),
            );
            let done = import_into(
                state.session.document(),
                &imported.document,
                &name,
                destination,
            )
            .map_err(|refusal| format!("cannot import {}: {refusal}", args.source))?;
            let mut result = apply(state, done.commands)?;
            let named = state
                .session
                .document()
                .symbols()
                .iter()
                .find(|s| s.id() == done.symbol)
                .map(|s| s.name().to_owned());
            let notes: Vec<String> = imported
                .diagnostics
                .iter()
                .map(|d| d.message.clone())
                .chain(done.notes.iter().map(ToString::to_string))
                .collect();
            result["symbol"] = json!(done.symbol.as_uuid());
            result["name"] = json!(named);
            result["layer"] = json!(done.layer.map(|l| l.as_uuid().to_string()));
            result["notes"] = json!(notes);
            Ok(result)
        })
    }
}
