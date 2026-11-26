//! A layer's kind (raster or vector, converted by `flare_runtime::convert_layer`) and blend mode
//! (`flare_commands::layers`).

use flare_document::LayerBlend;
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, target_layer};
use crate::server::{FlareServer, Outcome, State};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerBlendMode {
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// How the layer is mixed with the layers under it: normal, multiply, screen, overlay,
    /// darken, lighten, color-dodge, color-burn, hard-light, soft-light, difference, exclusion,
    /// hue, saturation, color, luminosity or add.
    pub mode: String,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct LayerKind {
    /// Layer id; defaults to the first layer.
    pub layer: Option<String>,
    /// True: a raster layer, painted in pixels (`raster_brush`, `raster_erase`). False: a vector
    /// layer, which takes shapes.
    pub raster: bool,
    /// Fail unless the project is at this revision (from get_info).
    pub expected_revision: Option<u64>,
}

#[tool_router(router = layer_kind_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "Set how a layer is mixed with the layers under it, as a painting program's layer blend modes do (Multiply darkens, Screen lightens, Add makes light...), on the stage and in every picture and movie exported; one undo step, none when it already is. A SWF, an HTML5 page and an Animate file draw it as Normal and say so. A folder, mask, guide or camera layer takes no blend mode (needs write access)."
    )]
    fn set_layer_blend(&self, Parameters(args): Parameters<LayerBlendMode>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let blend = LayerBlend::parse(&args.mode).ok_or_else(|| {
                let known: Vec<&str> = LayerBlend::ALL.iter().map(|b| b.name()).collect();
                format!(
                    "unknown blend mode {:?}; one of {}",
                    args.mode,
                    known.join(", ")
                )
            })?;
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let commands =
                flare_commands::layers::set_blend(state.session.document(), scene, layer, blend)
                    .map_err(|e| e.to_string())?;
            changed(state, commands, json!({ "mode": blend.name() }))
        })
    }

    #[tool(
        description = "Convert a layer to a raster layer (painted in pixels: the Brush paints and the Eraser rubs out pixels on it, as `raster_brush` and `raster_erase` do) or to a vector layer (shapes, what a new layer is), keeping what it shows: each keyframe's shapes and instances are drawn into one painting, or its paintings are traced into filled shapes. One undo step, none when it already is that kind. A locked layer and a tweened keyframe are refused (needs write access)."
    )]
    fn set_layer_kind(&self, Parameters(args): Parameters<LayerKind>) -> Outcome {
        self.write(args.expected_revision, |state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, args.layer.as_deref())?;
            let commands = flare_runtime::convert_layer(
                state.session.document(),
                scene,
                layer,
                args.raster,
                "Painting",
            )
            .map_err(|e| e.to_string())?;
            changed(state, commands, json!({ "raster": args.raster }))
        })
    }
}

/// Applies `commands` (nothing to apply is no change) and reports `out` with whether it changed.
fn changed(
    state: &mut State,
    commands: Vec<flare_commands::Command>,
    mut out: Value,
) -> Result<Value, String> {
    let changed = !commands.is_empty();
    if changed {
        let revision = apply(state, commands)?;
        out["revision"] = revision["revision"].clone();
    }
    out["changed"] = json!(changed);
    Ok(out)
}
