//! A layer's effect graph, through the same edits as `flare fx` and the Effects panel. The
//! behaviour is in `flare_tools::effects`; this maps arguments and results.

use std::collections::BTreeMap;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::Document;
use flare_tools::effects::{self as model, EffectsRefusal};
use rmcp::{handler::server::wrapper::Parameters, tool, tool_router};
use serde_json::{Value, json};

use crate::ops::{apply, first_scene, target_layer};
use crate::params_effects::{
    AddEffect, ClearEffects, ConnectEffects, DisconnectEffect, EffectNode, EffectsOf, Input,
    MoveEffect, SetEffect,
};
use crate::server::{FlareServer, Outcome, State};

fn refused(error: EffectsRefusal) -> String {
    format!("effects: {error}")
}

fn fields(settings: &BTreeMap<String, String>) -> Vec<(String, String)> {
    settings
        .iter()
        .map(|(name, value)| (name.clone(), value.clone()))
        .collect()
}

/// The layer's graph as JSON, `null` when it has none.
fn shown(document: &Document, scene: SceneId, layer: LayerId) -> Value {
    let effects = document
        .scenes()
        .iter()
        .find(|s| s.id() == scene)
        .and_then(|s| s.timeline().layers().iter().find(|l| l.id() == layer))
        .and_then(flare_document::Layer::effects);
    model::describe(effects)
}

/// Builds the batch for the layer, applies it as one undo step (none when it changes nothing),
/// and reports the revision and the graph.
fn edit(
    state: &mut State,
    layer: Option<&str>,
    build: impl FnOnce(&Document, SceneId, LayerId) -> Result<Vec<Command>, EffectsRefusal>,
) -> Result<Value, String> {
    let scene = first_scene(state)?;
    let layer = target_layer(state, layer)?;
    let commands = build(state.session.document(), scene, layer).map_err(refused)?;
    let changed = !commands.is_empty();
    let mut out = if changed {
        apply(state, commands)?
    } else {
        json!({ "revision": state.session.revision() })
    };
    out["changed"] = json!(changed);
    out["layer"] = json!(layer.as_uuid());
    out["effects"] = shown(state.session.document(), scene, layer);
    Ok(out)
}

fn port(
    document: &Document,
    scene: SceneId,
    layer: LayerId,
    to: flare_fx::NodeId,
    input: Option<&Input>,
) -> Result<usize, EffectsRefusal> {
    let text = input.map_or_else(|| "0".to_owned(), Input::text);
    model::port(document, scene, layer, to, &text)
}

#[tool_router(router = effect_tools, vis = "pub(super)")]
impl FlareServer {
    #[tool(
        description = "The kinds of layer effect node that can be added (drop-shadow, glow, blur, adjust, tint, opacity, snow, over), each with its inputs and settings: ranges, defaults and units."
    )]
    fn list_effect_kinds(&self) -> Outcome {
        self.read(|_| Ok(model::kinds()))
    }

    #[tool(
        description = "A layer's effect graph: its nodes (the Layer node is 1, the Output 2), where each input comes from, their settings, their place in the graph view, and the order they run in. Null when the layer has none."
    )]
    fn get_effects(&self, Parameters(a): Parameters<EffectsOf>) -> Outcome {
        self.read(|state| {
            let scene = first_scene(state)?;
            let layer = target_layer(state, a.layer.as_deref())?;
            Ok(json!({
                "layer": layer.as_uuid(),
                "effects": shown(state.session.document(), scene, layer),
            }))
        })
    }

    #[tool(
        description = "Add an effect to a layer at the end of its chain, so it shows at once, with settings by name. Returns the new node's number and the graph. One undo step (needs write access)."
    )]
    fn add_effect(&self, Parameters(a): Parameters<AddEffect>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let kind = model::kind(&a.kind).map_err(refused)?;
            let fields = fields(&a.settings);
            let mut added = None;
            let mut out = edit(state, a.layer.as_deref(), |doc, scene, layer| {
                let (commands, id) = model::add(doc, scene, layer, kind, &fields)?;
                added = Some(id.get());
                Ok(commands)
            })?;
            out["node"] = json!(added);
            Ok(out)
        })
    }

    #[tool(
        description = "Set settings of an effect node by name, all or none (a color is RRGGBB or RRGGBBAA). One undo step (needs write access)."
    )]
    fn set_effect_settings(&self, Parameters(a): Parameters<SetEffect>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let node = model::node(a.node).map_err(refused)?;
            let fields = fields(&a.settings);
            edit(state, a.layer.as_deref(), |doc, scene, layer| {
                model::set_fields(doc, scene, layer, node, &fields)
            })
        })
    }

    #[tool(
        description = "Connect the output of one effect node to an input of another (by number or name). A connection that would make a loop is refused. One undo step (needs write access)."
    )]
    fn connect_effects(&self, Parameters(a): Parameters<ConnectEffects>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let from = model::node(a.from).map_err(refused)?;
            let to = model::node(a.to).map_err(refused)?;
            edit(state, a.layer.as_deref(), |doc, scene, layer| {
                let port = port(doc, scene, layer, to, a.input.as_ref())?;
                model::connect(doc, scene, layer, from, to, port)
            })
        })
    }

    #[tool(
        description = "Leave an input of an effect node unconnected (it takes nothing). Disconnecting the Output draws nothing on the layer. One undo step (needs write access)."
    )]
    fn disconnect_effect(&self, Parameters(a): Parameters<DisconnectEffect>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let to = model::node(a.to).map_err(refused)?;
            edit(state, a.layer.as_deref(), |doc, scene, layer| {
                let port = port(doc, scene, layer, to, a.input.as_ref())?;
                model::disconnect(doc, scene, layer, to, port)
            })
        })
    }

    #[tool(
        description = "Remove an effect node; what it fed takes what it was given, so a chain stays joined. The Layer and Output nodes stay. One undo step (needs write access)."
    )]
    fn remove_effect(&self, Parameters(a): Parameters<EffectNode>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let node = model::node(a.node).map_err(refused)?;
            edit(state, a.layer.as_deref(), |doc, scene, layer| {
                model::remove(doc, scene, layer, node)
            })
        })
    }

    #[tool(
        description = "Move an effect node in the graph view; the picture does not change. One undo step (needs write access)."
    )]
    fn move_effect_node(&self, Parameters(a): Parameters<MoveEffect>) -> Outcome {
        self.write(a.expected_revision, |state| {
            let node = model::node(a.node).map_err(refused)?;
            edit(state, a.layer.as_deref(), |doc, scene, layer| {
                model::move_node(doc, scene, layer, node, [a.x, a.y])
            })
        })
    }

    #[tool(description = "Take all of a layer's effects away. One undo step (needs write access).")]
    fn clear_effects(&self, Parameters(a): Parameters<ClearEffects>) -> Outcome {
        self.write(a.expected_revision, |state| {
            edit(state, a.layer.as_deref(), model::clear)
        })
    }
}
