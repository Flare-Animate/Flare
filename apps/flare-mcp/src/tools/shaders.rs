//! Read and update the project's portable shader graph library.

use flare_commands::Command;
use flare_document::ShaderAsset;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::json;

use crate::ops::apply;
use crate::params_shader::{GetShaderGraph, SetShaderGraph};
use crate::server::{FlareServer, Outcome};

#[tool_router(router = shader_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(description = "Return a named shader graph in Flare's version 1 portable JSON format.")]
    fn get_shader_graph(&self, Parameters(a): Parameters<GetShaderGraph>) -> Outcome {
        self.read(|state| {
            let asset = state
                .session
                .document()
                .shader_assets()
                .iter()
                .find(|asset| asset.name() == a.name)
                .ok_or_else(|| format!("shader graph {:?} is not in this project", a.name))?;
            let bytes = flare_import::export_shader_graph(asset.graph())
                .map_err(|error| error.to_string())?;
            let graph_json = String::from_utf8(bytes).map_err(|error| error.to_string())?;
            Ok(json!({"name": asset.name(), "graph_json": graph_json}))
        })
    }

    #[tool(description = "List the named shader graph assets saved in the open project.")]
    fn list_shader_graphs(&self) -> Outcome {
        self.read(|state| {
            Ok(json!({
                "shader_graphs": state.session.document().shader_assets().iter().map(|asset| {
                    json!({"name": asset.name(), "nodes": asset.graph().nodes().len()})
                }).collect::<Vec<_>>()
            }))
        })
    }

    #[tool(
        description = "Add or replace a named portable Flare shader graph asset using version 1 JSON. Graph nodes and limits are validated before the project changes. One undo step."
    )]
    fn set_shader_graph(&self, Parameters(a): Parameters<SetShaderGraph>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let graph = flare_import::import_shader_graph(a.graph_json.as_bytes())
                .map_err(|error| error.to_string())?;
            let asset = ShaderAsset::new(&a.name, graph).map_err(|error| error.to_string())?;
            let mut assets = state.session.document().shader_assets().to_vec();
            assets.retain(|existing| existing.name() != asset.name());
            assets.push(asset);
            apply(state, vec![Command::SetShaderAssets(assets)])
        })
    }
}
