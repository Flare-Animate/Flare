//! A layer's kind and blend mode in the workspace, and the Properties panel's Layer tab that sets
//! them with the layer's transparency. A layer is converted between vector and raster on the
//! pixels worker (`flare_runtime::convert_layer`, which every surface calls), and the edit that
//! comes back is applied as one undo step. The rules are `flare_commands::layers`.

use std::sync::Arc;

use egui::Ui;
use flare_commands::Command;
use flare_core::LayerId;
use flare_document::{Layer, LayerBlend};
use flare_runtime::PixelJob;
use flare_ui::chrome;

use super::*;

impl Workspace {
    /// Whether the layer being drawn on is a raster layer, where the Brush and the Eraser work
    /// in pixels.
    pub(in crate::workspace) fn on_raster_layer(&self) -> bool {
        self.active_layer().is_some_and(Layer::is_raster)
    }

    /// Sets how `layer` is mixed with the layers under it: one undo step, none when it already
    /// is.
    pub fn set_layer_blend(&mut self, layer: LayerId, blend: LayerBlend) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        match flare_commands::layers::set_blend(self.document(), scene, layer, blend) {
            Ok(commands) if commands.is_empty() => {}
            Ok(commands) => self.run_batch(commands, "change the layer's blend mode"),
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// Makes `layer` a raster layer or a vector layer, converting what it holds so it looks the
    /// same: its shapes drawn into paintings, or its paintings traced into shapes. The work is
    /// handed to the pixels worker; nothing happens when it is that kind already.
    pub fn convert_layer(&mut self, layer: LayerId, raster: bool) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        if self.layer_converting.is_some()
            || self.layer(layer).is_none_or(|l| l.is_raster() == raster)
        {
            return;
        }
        let id = self.background.next_id();
        self.layer_converting = Some(id);
        self.background.submit_pixels(PixelJob::ConvertLayer {
            id,
            document: self.active().snapshot(),
            scene,
            layer,
            raster,
            name: tr!(self, "name-painting"),
        });
        self.settle_inline();
    }

    /// Whether a layer is being converted.
    pub(in crate::workspace) fn converting_layer(&self) -> bool {
        self.layer_converting.is_some()
    }

    /// The conversion came back: applied if the document is still the one it was made for.
    pub(in crate::workspace) fn layer_conversion_arrived(
        &mut self,
        id: u64,
        document: Arc<flare_document::Document>,
        result: Result<Vec<Command>, String>,
    ) {
        if self
            .layer_converting
            .take_if(|pending| *pending == id)
            .is_none()
        {
            return;
        }
        if !Arc::ptr_eq(&document, &self.active().snapshot()) {
            self.report(tr!(self, "msg-layer-convert-changed"));
            return;
        }
        match result {
            Ok(commands) if commands.is_empty() => {}
            Ok(commands) => {
                self.run_batch(commands, "convert the layer");
                self.validate_selection();
            }
            Err(reason) => self.report(tr!(self, "msg-layer-convert", &[("reason", &reason)])),
        }
    }

    /// The Properties panel's Layer tab: the active layer's kind (converting it), blend mode and
    /// transparency. A layer that draws nothing (a folder, mask, guide or camera) has no kind or
    /// blend mode, so those are dimmed for it.
    pub(crate) fn layer_tab(&mut self, ui: &mut Ui) {
        let Some(found) = self.active_layer() else {
            ui.weak(tr!(self, "prop-no-layer"));
            return;
        };
        let (layer, name) = (found.id(), found.name().to_owned());
        let draws =
            !(found.is_folder() || found.is_mask() || found.is_guide() || found.is_camera());
        let (raster, current) = (found.is_raster(), found.blend());
        chrome::section(ui, "panel-layers", &name);

        let kinds = [
            tr!(self, "prop-layer-vector"),
            tr!(self, "prop-layer-raster"),
        ];
        let kinds: Vec<&str> = kinds.iter().map(String::as_str).collect();
        let busy = self.converting_layer();
        let mut chosen = None;
        chrome::row(ui, &tr!(self, "prop-layer-kind"), |ui| {
            let enabled = [draws && !busy, draws && !busy];
            chosen = chrome::segmented_some(ui, &kinds, usize::from(raster), &enabled);
        });
        if let Some(index) = chosen {
            self.convert_layer(layer, index == 1);
        }
        if busy {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(tr!(self, "prop-layer-converting"));
            });
        }
        ui.weak(tr!(self, "prop-layer-kind-hint"));

        let names: Vec<(LayerBlend, String)> = LayerBlend::ALL
            .into_iter()
            .map(|b| (b, self.tr(&blend_key(b))))
            .collect();
        let shown = self.tr(&blend_key(current));
        let mut picked = None;
        chrome::row(ui, &tr!(self, "prop-layer-blend"), |ui| {
            ui.add_enabled_ui(draws, |ui| {
                egui::ComboBox::from_id_salt("blend mode of the layer")
                    .selected_text(shown)
                    .show_ui(ui, |ui| {
                        for (blend, label) in &names {
                            if ui.selectable_label(*blend == current, label).clicked() {
                                picked = Some(*blend);
                            }
                        }
                    });
            });
        });
        if let Some(blend) = picked {
            self.set_layer_blend(layer, blend);
        }
        if !draws {
            ui.weak(tr!(self, "prop-layer-draws-nothing"));
        }

        self.transparency_menu(ui, layer);
        ui.weak(tr!(self, "prop-layer-opacity-hint"));
    }
}

/// The message key of a blend mode's name.
pub(in crate::workspace) fn blend_key(blend: LayerBlend) -> String {
    format!("blend-{}", blend.name())
}
