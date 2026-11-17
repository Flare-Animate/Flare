//! The Effects panel: the active layer's effects as a node graph (OpenToonz and Tahoma2D call it
//! the FX schematic), an Add menu, and the chosen node's settings. A layer's picture runs from
//! its Layer node through the effects wired to its Output, on the stage and in every export.

use egui::{Color32, Ui};
use flare_core::Rgba8;
use flare_fx::{EffectGraph, EffectKind, Node, ParamKind, Unit, Value};
use flare_ui::{chrome, theme};

use crate::workspace::Workspace;

/// How tall the graph view is.
const GRAPH_HEIGHT: f32 = 210.0;

fn to_egui(c: Rgba8) -> Color32 {
    Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

fn from_egui(c: Color32) -> Rgba8 {
    let [r, g, b, a] = c.to_srgba_unmultiplied();
    Rgba8::new(r, g, b, a)
}

fn suffix(unit: Unit) -> &'static str {
    match unit {
        Unit::Length | Unit::Speed => " px",
        Unit::Angle => "\u{b0}",
        Unit::Percent => "%",
        Unit::Count => "",
    }
}

impl Workspace {
    pub(crate) fn effects_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| self.effects_contents(ui));
    }

    fn effects_contents(&mut self, ui: &mut Ui) {
        if !self.contexts.is_empty() {
            chrome::empty_state(ui, "panel-effects", &tr!(self, "fx-in-symbol"));
            return;
        }
        let Some(layer) = self.active_layer() else {
            chrome::empty_state(ui, "panel-effects", &tr!(self, "fx-no-layer"));
            return;
        };
        let name = layer.name().to_owned();
        let drawn =
            !(layer.is_mask() || layer.is_guide() || layer.is_camera() || layer.is_folder());
        let has = layer.effects().is_some();
        ui.label(egui::RichText::new(tr!(self, "fx-layer", &[("name", &name)])).strong());
        if !drawn {
            chrome::empty_state(ui, "panel-effects", &tr!(self, "fx-not-drawn"));
            return;
        }
        let Some(graph) = self.shown_effects() else {
            return;
        };
        self.effects_toolbar(ui, &graph, has);
        ui.add_space(4.0);
        self.effects_graph(ui, &graph, GRAPH_HEIGHT);
        ui.add_space(2.0);
        let hint = if has {
            tr!(self, "fx-hint")
        } else {
            tr!(self, "fx-none")
        };
        ui.label(egui::RichText::new(hint).small().color(theme::text_dim()));
        ui.add_space(6.0);
        let chosen = self.effects.selected.and_then(|id| graph.node(id)).cloned();
        match chosen {
            Some(node) => self.effect_settings(ui, &node),
            None => {
                ui.label(egui::RichText::new(tr!(self, "fx-pick")).color(theme::text_dim()));
            }
        }
    }

    /// Add, Remove and Clear All.
    fn effects_toolbar(&mut self, ui: &mut Ui, graph: &EffectGraph, has: bool) {
        let names: Vec<(EffectKind, String)> = EffectKind::ADDABLE
            .iter()
            .map(|k| (*k, self.tr(&format!("fx-kind-{}", k.name()))))
            .collect();
        let removable = self
            .effects
            .selected
            .filter(|id| graph.node(*id).is_some_and(|n| !n.kind().is_fixed()));
        let (add, remove, clear) = (
            tr!(self, "fx-add"),
            tr!(self, "fx-remove"),
            tr!(self, "fx-clear"),
        );
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(add, |ui| {
                for (kind, name) in &names {
                    if ui.button(name).clicked() {
                        self.add_layer_effect(*kind);
                        ui.close();
                    }
                }
            });
            if ui
                .add_enabled(removable.is_some(), egui::Button::new(remove))
                .clicked()
                && let Some(id) = removable
            {
                self.remove_layer_effect(id);
            }
            if ui.add_enabled(has, egui::Button::new(clear)).clicked() {
                self.clear_layer_effects();
            }
        });
    }

    /// The chosen node's settings. A slider or field is held while it is dragged or typed in, and
    /// shown on the stage; it is applied as one undo step when let go.
    fn effect_settings(&mut self, ui: &mut Ui, node: &Node) {
        let title = self.tr(&format!("fx-kind-{}", node.kind().name()));
        chrome::section(ui, "panel-effects", &title);
        let specs = node.kind().params();
        if specs.is_empty() {
            ui.label(egui::RichText::new(tr!(self, "fx-no-settings")).color(theme::text_dim()));
            return;
        }
        let texts = self.color_texts();
        let mut holding = false;
        for (spec, value) in specs.iter().zip(node.values()) {
            let label = self.tr(&format!("fx-param-{}", spec.name));
            match (spec.kind, *value) {
                (ParamKind::Number { min, max, unit, .. }, Value::Number(mut n)) => {
                    let response = if unit == Unit::Count {
                        let mut whole = n.round() as i32;
                        let r = chrome::slider(
                            ui,
                            &label,
                            &mut whole,
                            min as i32..=max as i32,
                            suffix(unit),
                            false,
                        );
                        n = whole as f32;
                        r
                    } else {
                        let log = unit.scales() && min >= 0.0;
                        chrome::slider(ui, &label, &mut n, min..=max, suffix(unit), log)
                    };
                    holding |= response.dragged() || response.has_focus();
                    if response.changed() {
                        self.hold_layer_effect_value(node.id(), spec.name, Value::Number(n));
                    }
                }
                (ParamKind::Color { .. }, Value::Color(c)) => {
                    let mut color = to_egui(c);
                    chrome::row(ui, &label, |ui| {
                        if flare_ui::color_picker::color_button(ui, &mut color, &texts).changed() {
                            self.hold_layer_effect_value(
                                node.id(),
                                spec.name,
                                Value::Color(from_egui(color)),
                            );
                        }
                    });
                }
                _ => {}
            }
        }
        // A color is picked by dragging inside its popup, or typed: held while either goes on.
        let busy = ui.input(|i| i.pointer.any_down()) || ui.memory(|m| m.focused().is_some());
        if holding || (self.effects.held.is_some() && busy) {
            self.effects.held_pass = Some(ui.ctx().cumulative_pass_nr());
        }
    }
}
