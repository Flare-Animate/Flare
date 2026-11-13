//! The layer menu's commands for several layers at once, as Animate's has them: Show All, Lock
//! Others, Hide Others, Duplicate Layer (`flare_editor::layer_ops`) and Convert Layers to Symbol
//! (`flare_editor::layers_to_symbol`).

use flare_editor::layer_ops::{self, LayerOpRefusal};

use crate::workspace::*;

/// The opacity Shift on a layer's eye and the Transparent check give a layer, in percent.
pub const DEFAULT_TRANSPARENCY: u8 = 50;

impl Workspace {
    /// Shows every hidden layer, as one undo step.
    pub fn show_all_layers(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let commands = layer_ops::show_all(self.document(), scene);
        self.run_layer_ops(Ok(commands), "show all layers");
    }

    /// Locks every layer but `layer` and the folders it is in, and unlocks those.
    pub fn lock_other_layers(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let built = layer_ops::lock_others(self.document(), scene, layer);
        self.run_layer_ops(built, "lock the other layers");
    }

    /// Hides every layer but `layer` and the folders it is in, and shows those.
    pub fn hide_other_layers(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let built = layer_ops::hide_others(self.document(), scene, layer);
        self.run_layer_ops(built, "hide the other layers");
        self.validate_selection();
    }

    /// Puts a copy of `layer` (a folder with everything in it) just above it and makes the copy
    /// the active layer.
    pub fn duplicate_layer(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        match layer_ops::duplicate(self.document(), scene, layer) {
            Ok((commands, copy)) => {
                self.run_batch(commands, "duplicate the layer");
                if self.layer(copy).is_some() {
                    self.select_layer(copy);
                }
            }
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// Moves `layer` (a folder with everything in it) into a new Graphic symbol named after it,
    /// and puts an instance of it in its place, which becomes the active layer.
    pub fn convert_layer_to_symbol(&mut self, layer: LayerId) {
        use flare_editor::layers_to_symbol::{convert_layers_to_symbol, free_name};
        let (Some(scene), Some(found)) = (self.scene_id(), self.layer(layer)) else {
            return;
        };
        let name = free_name(self.document(), found.name());
        let built = convert_layers_to_symbol(
            self.document(),
            scene,
            &[layer],
            &name,
            flare_document::SymbolKind::Graphic,
        );
        match built {
            Ok(done) => {
                self.run_batch(done.commands, "convert the layer to a symbol");
                if self.layer(done.layer).is_some() {
                    self.select_layer(done.layer);
                }
                self.validate_selection();
            }
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// How opaque the stage draws `layer` while it is edited, in percent (held at 100): one undo
    /// step, none when it is so already. Exports always draw the layer whole.
    pub fn set_layer_view_opacity(&mut self, layer: LayerId, percent: u8) {
        let (Some(scene), Some(found)) = (self.scene_id(), self.layer(layer)) else {
            return;
        };
        let percent = percent.min(100);
        if found.view_opacity() == percent {
            return;
        }
        let change = flare_commands::LayerChange::ViewOpacity(percent);
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change,
            }],
            "change the layer's transparency",
        );
    }

    /// Makes `layer` transparent at half opacity, or opaque again.
    pub fn toggle_layer_transparent(&mut self, layer: LayerId) {
        let opaque = self.layer(layer).is_some_and(|l| l.view_opacity() == 100);
        let percent = if opaque { DEFAULT_TRANSPARENCY } else { 100 };
        self.set_layer_view_opacity(layer, percent);
    }

    /// The layer menu's Transparent toggle and Opacity slider. The slider applies when it is let
    /// go, so a drag is one undo step.
    pub(in crate::workspace) fn transparency_menu(&mut self, ui: &mut egui::Ui, layer: LayerId) {
        let Some(current) = self.layer(layer).map(Layer::view_opacity) else {
            return;
        };
        let mut transparent = current < 100;
        if ui
            .checkbox(&mut transparent, tr!(self, "msg-layer-transparent"))
            .clicked()
        {
            self.toggle_layer_transparent(layer);
        }
        let id = egui::Id::unique("layer opacity being dragged").with(layer);
        let mut percent: u8 = ui.data(|d| d.get_temp(id)).unwrap_or(current);
        ui.horizontal(|ui| {
            ui.label(tr!(self, "msg-layer-opacity"));
            let slider = ui.add(egui::Slider::new(&mut percent, 0..=100).suffix("%"));
            if slider.changed() {
                ui.data_mut(|d| d.insert_temp(id, percent));
            }
            if slider.drag_stopped() || (slider.changed() && !slider.dragged()) {
                ui.data_mut(|d| d.remove::<u8>(id));
                self.set_layer_view_opacity(layer, percent);
            }
        });
    }

    /// Runs what a layer operation built; nothing to change is not an undo step.
    fn run_layer_ops(&mut self, built: Result<Vec<Command>, LayerOpRefusal>, what: &str) {
        match built {
            Ok(commands) if commands.is_empty() => {}
            Ok(commands) => self.run_batch(commands, what),
            Err(LayerOpRefusal::NoSuchLayer) => {}
            Err(refusal) => self.report(refusal.to_string()),
        }
    }
}
