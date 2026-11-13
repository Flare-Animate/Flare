//! The layer menu's commands for several layers at once: Show All, Lock Others, Hide Others,
//! Duplicate Layer (`flare_editor::layer_ops`) and Convert Layers to Symbol
//! (`flare_editor::layers_to_symbol`).

use flare_editor::layer_ops;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, target_layer};
use crate::server::{FlareServer, Outcome, State};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct OneLayer {
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayersToSymbol {
    /// The ids of the layers to convert; a folder brings everything in it.
    pub layers: Vec<String>,
    /// The new symbol's name; a name the library already has is refused.
    pub name: String,
    /// "graphic" (the default), "movie clip" or "button".
    pub kind: Option<String>,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerOpacity {
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// How opaque the stage draws the layer while editing, 0 to 100; 100 is opaque. Exports
    /// always draw the layer whole.
    pub percent: u8,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct WholeScene {
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

/// Applies the commands as one undo step; nothing to change is not a step, and says so.
fn apply_some(state: &mut State, commands: Vec<flare_commands::Command>) -> Result<Value, String> {
    if commands.is_empty() {
        return Ok(json!({ "revision": state.session.revision(), "changed": false }));
    }
    apply(state, commands)
}

#[tool_router(router = layer_op_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(description = "Show every hidden layer of the first scene (needs write access).")]
    fn show_all_layers(&self, Parameters(args): Parameters<WholeScene>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let commands = layer_ops::show_all(state.session.document(), scene);
            apply_some(state, commands)
        })
    }

    #[tool(
        description = "Lock every layer but this one and the folders it is in, and unlock those (needs write access)."
    )]
    fn lock_other_layers(&self, Parameters(args): Parameters<OneLayer>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let commands = layer_ops::lock_others(state.session.document(), scene, layer)
                .map_err(|e| e.to_string())?;
            apply_some(state, commands)
        })
    }

    #[tool(
        description = "Hide every layer but this one and the folders it is in, and show those (needs write access)."
    )]
    fn hide_other_layers(&self, Parameters(args): Parameters<OneLayer>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let commands = layer_ops::hide_others(state.session.document(), scene, layer)
                .map_err(|e| e.to_string())?;
            apply_some(state, commands)
        })
    }

    #[tool(
        description = "Put a copy of a layer (a folder with everything in it) just above it, named \"<name> copy\"; returns the copy's id (needs write access)."
    )]
    fn duplicate_layer(&self, Parameters(args): Parameters<OneLayer>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let (commands, copy) = layer_ops::duplicate(state.session.document(), scene, layer)
                .map_err(|e| e.to_string())?;
            let mut out = apply(state, commands)?;
            out["copy"] = json!(copy.as_uuid());
            Ok(out)
        })
    }

    #[tool(
        description = "Make a layer see-through on the stage while editing (percent 0 to 100; 100 is opaque), to draw over a rough. Exports always draw it whole (needs write access)."
    )]
    fn set_layer_opacity(&self, Parameters(args): Parameters<LayerOpacity>) -> Outcome {
        self.write(args.expected_revision, |state| {
            if args.percent > 100 {
                return Err(format!("percent is 0 to 100, not {}", args.percent));
            }
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let same = state
                .session
                .document()
                .scenes()
                .iter()
                .flat_map(|s| s.timeline().layers())
                .any(|l| l.id() == layer && l.view_opacity() == args.percent);
            let commands = if same {
                Vec::new()
            } else {
                vec![flare_commands::Command::UpdateLayer {
                    scene,
                    layer,
                    change: flare_commands::LayerChange::ViewOpacity(args.percent),
                }]
            };
            apply_some(state, commands)
        })
    }

    #[tool(
        description = "Convert Layers to Symbol: move layers (a folder with everything in it) into a new symbol and put one layer with an instance of it where they were; returns the symbol's and the layer's ids (needs write access)."
    )]
    fn convert_layers_to_symbol(&self, Parameters(args): Parameters<LayersToSymbol>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let kind = match args.kind.as_deref() {
                None => flare_document::SymbolKind::Graphic,
                Some(text) => flare_document::SymbolKind::parse(text)
                    .ok_or_else(|| format!("unknown kind {text:?}"))?,
            };
            let picked = args
                .layers
                .iter()
                .map(|id| target_layer(state, Some(id)))
                .collect::<Result<Vec<_>, _>>()?;
            let done = flare_editor::layers_to_symbol::convert_layers_to_symbol(
                state.session.document(),
                scene,
                &picked,
                &args.name,
                kind,
            )
            .map_err(|e| e.to_string())?;
            let mut out = apply(state, done.commands)?;
            out["symbol"] = json!(done.symbol.as_uuid());
            out["layer"] = json!(done.layer.as_uuid());
            Ok(out)
        })
    }
}
