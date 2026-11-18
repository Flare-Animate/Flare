//! The Effects panel's state and what it does to the active layer: each edit is one undo step
//! through `flare_tools::effects`, the same edits `flare fx` and the MCP server make.
//!
//! A setting being dragged or typed is held, not applied: the stage is drawn from the document
//! the held graph would give (a preview kept out of the history, as a held move is), and the
//! graph is applied once, as one undo step, when nothing in the panel is held any more.

use std::sync::Arc;

use flare_commands::Command;
use flare_core::{LayerId, SceneId};
use flare_document::{Document, Layer};
use flare_fx::{EffectGraph, EffectKind, NodeId, Value};
use flare_tools::effects::{self as model, EffectsRefusal};

use super::drag_preview::{HeldPreview, PreviewKey};
use super::*;

/// The history's name for every effect edit.
pub(crate) const EFFECTS_STEP: &str = "change the layer's effects";

/// A change the panel is holding: the graph it would give the layer, and the document state it
/// was made from, so a held change never lands on a document that has moved on.
#[derive(Clone, Debug)]
pub(crate) struct HeldEffects {
    pub(crate) layer: LayerId,
    pub(crate) graph: EffectGraph,
    revision: u64,
    context: u64,
    serial: u64,
}

#[derive(Default)]
pub(crate) struct EffectsState {
    /// The node whose settings the panel shows.
    pub(crate) selected: Option<NodeId>,
    /// How far the graph view has been dragged from its start.
    pub(crate) pan: egui::Vec2,
    /// A node being dragged in the view, and how far it has gone.
    pub(crate) dragging: Option<(NodeId, egui::Vec2)>,
    /// A wire being drawn from a node's output.
    pub(crate) wiring: Option<NodeId>,
    /// Where the pointer was pressed for the drag going on, and the view's pan then.
    pub(crate) press: Option<(egui::Pos2, egui::Vec2)>,
    /// A setting being dragged or typed, not yet applied.
    pub(crate) held: Option<HeldEffects>,
    /// The pass in which the panel last had a setting held under the pointer or the keyboard.
    pub(crate) held_pass: Option<u64>,
    serial: u64,
}

impl Workspace {
    /// The layer the Effects panel works on: the active layer of the scene. `None` inside a
    /// symbol, where effects do not go.
    pub fn effects_layer(&self) -> Option<&Layer> {
        if !self.contexts.is_empty() {
            return None;
        }
        self.active_layer()
    }

    /// The active layer's graph as the panel shows it: the held one while a setting is held,
    /// else the layer's own, else the plain graph to start from.
    pub(crate) fn shown_effects(&self) -> Option<EffectGraph> {
        let layer = self.effects_layer()?;
        if let Some(held) = self.effects.held.as_ref().filter(|h| h.layer == layer.id()) {
            return Some(held.graph.clone());
        }
        Some(layer.effects().cloned().unwrap_or_default())
    }

    /// Runs the edit `build` makes of the active layer's effects as one undo step; a refusal is
    /// reported. Whether the layer has what was asked for afterwards.
    fn edit_effects(
        &mut self,
        build: impl FnOnce(&Document, SceneId, LayerId) -> Result<Vec<Command>, EffectsRefusal>,
    ) -> bool {
        self.release_layer_effects();
        let (Some(scene), Some(layer)) = (self.scene_id(), self.effects_layer().map(Layer::id))
        else {
            return false;
        };
        match build(self.document(), scene, layer) {
            Ok(commands) if commands.is_empty() => true,
            Ok(commands) => {
                let before = self.active().revision();
                self.run_batch(commands, EFFECTS_STEP);
                self.active().revision() != before
            }
            Err(refusal) => {
                self.report(refusal.to_string());
                false
            }
        }
    }

    /// Adds an effect of `kind` at the end of the active layer's chain and selects it.
    pub fn add_layer_effect(&mut self, kind: EffectKind) -> Option<NodeId> {
        let mut added = None;
        let done = self.edit_effects(|doc, scene, layer| {
            let (commands, id) = model::add(doc, scene, layer, kind, &[])?;
            added = Some(id);
            Ok(commands)
        });
        let added = added.filter(|_| done);
        if added.is_some() {
            self.effects.selected = added;
        }
        added
    }

    /// Removes `node` from the active layer's graph.
    pub fn remove_layer_effect(&mut self, node: NodeId) {
        if self.edit_effects(|doc, scene, layer| model::remove(doc, scene, layer, node))
            && self.effects.selected == Some(node)
        {
            self.effects.selected = None;
        }
    }

    /// Connects the output of `from` to input `port` of `to`.
    pub fn connect_layer_effects(&mut self, from: NodeId, to: NodeId, port: usize) -> bool {
        self.edit_effects(|doc, scene, layer| model::connect(doc, scene, layer, from, to, port))
    }

    /// Leaves input `port` of `to` unconnected.
    pub fn disconnect_layer_effect(&mut self, to: NodeId, port: usize) -> bool {
        self.edit_effects(|doc, scene, layer| model::disconnect(doc, scene, layer, to, port))
    }

    /// Moves `node` in the graph view.
    pub fn move_layer_effect(&mut self, node: NodeId, at: [f32; 2]) -> bool {
        self.edit_effects(|doc, scene, layer| model::move_node(doc, scene, layer, node, at))
    }

    /// Takes all of the active layer's effects away.
    pub fn clear_layer_effects(&mut self) {
        if self.edit_effects(model::clear) {
            self.effects.selected = None;
        }
    }

    /// Sets the setting `name` of `node` at once, as one undo step.
    pub fn set_layer_effect_value(&mut self, node: NodeId, name: &str, value: Value) -> bool {
        self.edit_effects(|doc, scene, layer| {
            let graph = model::graph_of(doc, scene, layer)?.set_value(node, name, value)?;
            model::set(doc, scene, layer, Some(graph))
        })
    }

    /// Holds the setting `name` of `node` at `value` while it is dragged or typed: the stage
    /// shows it, and it is applied when nothing is held any more.
    pub fn hold_layer_effect_value(&mut self, node: NodeId, name: &str, value: Value) {
        let Some(layer) = self.effects_layer().map(Layer::id) else {
            return;
        };
        let Some(base) = self.shown_effects() else {
            return;
        };
        match base.set_value(node, name, value) {
            Ok(graph) => {
                self.effects.serial = self.effects.serial.wrapping_add(1);
                self.effects.held = Some(HeldEffects {
                    layer,
                    graph,
                    revision: self.active().revision(),
                    context: self.context_serial,
                    serial: self.effects.serial,
                });
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Applies the held setting, if any, as one undo step. A held change made from a document
    /// that has since changed (an undo while it was held) is dropped.
    pub fn release_layer_effects(&mut self) {
        let Some(held) = self.effects.held.take() else {
            return;
        };
        if held.revision != self.active().revision() || held.context != self.context_serial {
            return;
        }
        let Some(scene) = self.scene_id() else { return };
        match model::set(self.document(), scene, held.layer, Some(held.graph)) {
            Ok(commands) if commands.is_empty() => {}
            Ok(commands) => self.run_batch(commands, EFFECTS_STEP),
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// At the start of a pass: applies a held setting the panel stopped holding in the last pass
    /// (the slider was let go, the field lost focus, or the panel closed).
    pub(super) fn settle_effects(&mut self, pass: u64) {
        let held_last_pass = self
            .effects
            .held_pass
            .is_some_and(|p| p.saturating_add(1) >= pass);
        if self.effects.held.is_some() && !held_last_pass {
            self.release_layer_effects();
        }
    }

    /// The document a held setting would give, for the stage, with its stamp; `None` when
    /// nothing is held or it would draw what the layer draws already.
    pub(super) fn effects_preview(&mut self) -> Option<(u64, Arc<Document>)> {
        let held = self.effects.held.clone()?;
        let scene = self.scene_id()?;
        let current = self.effects_layer().filter(|l| l.id() == held.layer)?;
        if current
            .effects()
            .is_some_and(|g| g.same_picture(&held.graph))
        {
            return None;
        }
        let key = PreviewKey::Effects {
            context: self.context_serial,
            revision: self.active().revision(),
            serial: held.serial,
        };
        if let Some(kept) = self
            .drag_preview
            .held
            .as_ref()
            .and_then(|p| p.matching(&key))
        {
            return Some(kept);
        }
        let commands = model::set(self.document(), scene, held.layer, Some(held.graph)).ok()?;
        let document = Arc::new(flare_commands::apply_all(self.document(), commands).ok()??);
        let stamp = self.drag_preview.next_stamp();
        self.drag_preview.held = Some(HeldPreview {
            key,
            stamp,
            document: Arc::clone(&document),
        });
        Some((stamp, document))
    }
}
