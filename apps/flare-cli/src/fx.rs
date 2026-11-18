//! `flare fx`: a layer's effect graph from the command line. The behaviour is in
//! `flare_tools::effects`; this parses arguments, applies the batch as one undo step, saves the
//! project and prints what changed.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use clap::{Args, Subcommand};
use flare_commands::Command;
use flare_fx::parse_fields;
use flare_tools::effects as model;
use serde_json::{Value, json};
use uuid::Uuid;

use crate::project::{commit_session, first_scene_layer, open_session};

/// The project and the layer an effect edit is on.
#[derive(Args)]
pub struct Target {
    pub path: PathBuf,
    /// The layer, by ID; the first layer of the first scene when not given.
    #[arg(long)]
    pub layer: Option<Uuid>,
}

#[derive(Subcommand)]
pub enum FxCommand {
    /// List the kinds of effect node, with their inputs and settings (ranges and defaults).
    Kinds,
    /// Print a layer's effect graph: its nodes, inputs, settings and the order they run in
    /// (`null` when the layer has none).
    Show(Target),
    /// Add an effect at the end of the layer's chain, so it shows at once: drop-shadow, glow,
    /// blur, adjust, tint, opacity, snow or over, then `name=value` settings. Prints its number.
    Add {
        #[command(flatten)]
        target: Target,
        kind: String,
        #[arg(num_args = 0..)]
        settings: Vec<String>,
    },
    /// Set settings of a node as `name=value` (a color is RRGGBB or RRGGBBAA).
    Set {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        node: u32,
        #[arg(num_args = 1..)]
        settings: Vec<String>,
    },
    /// Connect the output of node `--from` to an input of node `--to` (by number or name, the
    /// first when not given). The Layer node is 1 and the Output 2.
    Connect {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        from: u32,
        #[arg(long)]
        to: u32,
        #[arg(long, default_value = "0")]
        input: String,
    },
    /// Leave an input of node `--to` unconnected.
    Disconnect {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        to: u32,
        #[arg(long, default_value = "0")]
        input: String,
    },
    /// Remove a node; what it fed takes what it was given instead.
    Remove {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        node: u32,
    },
    /// Move a node in the graph view (the picture does not change).
    Move {
        #[command(flatten)]
        target: Target,
        #[arg(long)]
        node: u32,
        #[arg(long, num_args = 2, value_names = ["X", "Y"], allow_negative_numbers = true)]
        at: Vec<f32>,
    },
    /// Take all of the layer's effects away.
    Clear(Target),
}

/// Applies the batch `build` makes for the layer as one undo step, saves, and prints the graph.
fn edit(
    target: &Target,
    build: impl FnOnce(
        &flare_document::Document,
        flare_core::SceneId,
        flare_core::LayerId,
    ) -> Result<Vec<Command>>,
) -> Result<Value> {
    let path: &Path = &target.path;
    let mut session = open_session(path)?;
    let (scene, layer) = first_scene_layer(&session, target.layer)?;
    let commands = build(session.document(), scene, layer)?;
    let changed = !commands.is_empty();
    if changed {
        session.apply(commands)?;
        commit_session(&session, path)?;
    }
    let graph = model::graph_of(session.document(), scene, layer).map_err(refused)?;
    let has = session
        .document()
        .scenes()
        .iter()
        .flat_map(|s| s.timeline().layers())
        .find(|l| l.id() == layer)
        .and_then(flare_document::Layer::effects)
        .is_some();
    Ok(json!({
        "layer": layer.as_uuid(),
        "changed": changed,
        "effects": model::describe(has.then_some(&graph)),
    }))
}

fn refused(error: model::EffectsRefusal) -> anyhow::Error {
    anyhow!("effects: {error}")
}

fn fields(settings: &[String]) -> Result<Vec<(String, String)>> {
    parse_fields(settings).map_err(|e| anyhow!("effects: {e}"))
}

pub fn run(op: FxCommand) -> Result<Value> {
    match op {
        FxCommand::Kinds => Ok(model::kinds()),
        FxCommand::Show(target) => edit(&target, |_, _, _| Ok(Vec::new())),
        FxCommand::Add {
            target,
            kind,
            settings,
        } => {
            let kind = model::kind(&kind).map_err(refused)?;
            let fields = fields(&settings)?;
            let mut added = None;
            let mut out = edit(&target, |doc, scene, layer| {
                let (commands, id) =
                    model::add(doc, scene, layer, kind, &fields).map_err(refused)?;
                added = Some(id.get());
                Ok(commands)
            })?;
            out["node"] = json!(added);
            Ok(out)
        }
        FxCommand::Set {
            target,
            node,
            settings,
        } => {
            let (node, fields) = (model::node(node).map_err(refused)?, fields(&settings)?);
            edit(&target, |doc, scene, layer| {
                model::set_fields(doc, scene, layer, node, &fields).map_err(refused)
            })
        }
        FxCommand::Connect {
            target,
            from,
            to,
            input,
        } => {
            let (from, to) = (
                model::node(from).map_err(refused)?,
                model::node(to).map_err(refused)?,
            );
            edit(&target, |doc, scene, layer| {
                let port = model::port(doc, scene, layer, to, &input).map_err(refused)?;
                model::connect(doc, scene, layer, from, to, port).map_err(refused)
            })
        }
        FxCommand::Disconnect { target, to, input } => {
            let to = model::node(to).map_err(refused)?;
            edit(&target, |doc, scene, layer| {
                let port = model::port(doc, scene, layer, to, &input).map_err(refused)?;
                model::disconnect(doc, scene, layer, to, port).map_err(refused)
            })
        }
        FxCommand::Remove { target, node } => {
            let node = model::node(node).map_err(refused)?;
            edit(&target, |doc, scene, layer| {
                model::remove(doc, scene, layer, node).map_err(refused)
            })
        }
        FxCommand::Move { target, node, at } => {
            let node = model::node(node).map_err(refused)?;
            let [x, y] = at[..] else {
                return Err(anyhow!("--at takes X and Y"));
            };
            edit(&target, |doc, scene, layer| {
                model::move_node(doc, scene, layer, node, [x, y]).map_err(refused)
            })
        }
        FxCommand::Clear(target) => edit(&target, |doc, scene, layer| {
            model::clear(doc, scene, layer).map_err(refused)
        }),
    }
}
