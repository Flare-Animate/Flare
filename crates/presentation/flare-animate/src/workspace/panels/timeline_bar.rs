//! The timeline toolbar.

use crate::actions::MenuAction;
use crate::workspace::{Command, Layer, LayerId, MAX_ONION_FRAMES, Workspace};
use egui::{RichText, Ui};
use flare_ui::chrome;
use flare_ui::theme;

use super::*;

/// Height of the row of controls above the timeline.
const TOOLBAR_ROW: f32 = 24.0;

impl Workspace {
    // ---- timeline toolbar --------------------------------------------------------------

    /// Transport, onion skin, zoom and layer buttons above the timeline.
    pub(crate) fn timeline_toolbar(&mut self, ui: &mut Ui) {
        // The same space on every side, and one row height with everything centred in it, so the
        // buttons, number fields and text box do not sit at different heights.
        let margin = egui::Margin::symmetric(8, 6);
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            let size = egui::vec2(ui.available_width(), TOOLBAR_ROW);
            ui.allocate_ui_with_layout(
                size,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| self.timeline_toolbar_row(ui),
            );
        });
    }

    fn timeline_toolbar_row(&mut self, ui: &mut Ui) {
        let last = self.scene_frames() - 1;
        if chrome::icon_button(ui, "transport-first", BAR_ICON, false, true, "First frame")
            .clicked()
        {
            self.set_frame(0);
        }
        if chrome::icon_button(
            ui,
            "transport-prev",
            BAR_ICON,
            false,
            true,
            "Previous frame",
        )
        .clicked()
        {
            self.set_frame(self.frame.saturating_sub(1));
        }
        let (icon, hint) = if self.playing {
            ("transport-pause", "Pause")
        } else {
            ("transport-play", "Play")
        };
        if chrome::icon_button(ui, icon, BAR_ICON, self.playing, true, hint).clicked() {
            self.playing = !self.playing;
        }
        if chrome::icon_button(ui, "transport-next", BAR_ICON, false, true, "Next frame").clicked()
        {
            self.set_frame(self.frame.saturating_add(1));
        }
        if chrome::icon_button(ui, "transport-last", BAR_ICON, false, true, "Last frame").clicked()
        {
            self.set_frame(last);
        }
        if chrome::icon_button(
            ui,
            "transport-loop",
            BAR_ICON,
            self.loop_playback,
            true,
            "Loop playback",
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleLoop);
        }
        ui.separator();
        let mut frame = self.frame;
        ui.add(
            egui::DragValue::new(&mut frame)
                .range(0..=last)
                .prefix("Frame "),
        );
        if frame != self.frame {
            self.set_frame(frame);
        }
        let rate = self.document().frame_rate();
        let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
        ui.label(RichText::new(format!("{fps:.2} fps")).color(theme::TEXT_DIM));
        ui.separator();
        let mut onion = self.onion.enabled;
        if chrome::icon_button(ui, "onion-skin", BAR_ICON, onion, true, "Onion skin").clicked() {
            onion = !onion;
        }
        self.set_onion_skin(onion);
        ui.add_enabled_ui(onion, |ui| {
            if chrome::icon_button(
                ui,
                "onion-outline",
                BAR_ICON,
                self.onion.outlines,
                true,
                "Show onion skin as outlines",
            )
            .clicked()
            {
                self.set_onion_outlines(!self.onion.outlines);
            }
        });
        ui.add_enabled_ui(onion, |ui| {
            let (mut before, mut after) = (self.onion.before, self.onion.after);
            ui.add(
                egui::DragValue::new(&mut before)
                    .range(0..=MAX_ONION_FRAMES)
                    .prefix("\u{2190} "),
            );
            ui.add(
                egui::DragValue::new(&mut after)
                    .range(0..=MAX_ONION_FRAMES)
                    .suffix(" \u{2192}"),
            );
            self.set_onion_range(before, after);
        });
        ui.separator();
        if chrome::icon_button(ui, "layer-new", BAR_ICON, false, true, "New layer").clicked() {
            self.add_layer();
        }
        if chrome::icon_button(
            ui,
            "layer-folder",
            BAR_ICON,
            false,
            true,
            "New layer folder",
        )
        .clicked()
        {
            self.perform(MenuAction::NewLayerFolder);
        }
        let folders = self.folders_for_active_layer();
        let in_folder = self.active_layer().is_some_and(|l| l.parent().is_some());
        ui.add_enabled_ui(!folders.is_empty() || in_folder, |ui| {
            ui.menu_button("Folder", |ui| {
                for (id, name) in &folders {
                    if ui.button(format!("Move into {name}")).clicked() {
                        self.move_active_layer_to_folder(Some(*id));
                        ui.close();
                    }
                }
                if in_folder && ui.button("Move out of its folder").clicked() {
                    self.move_active_layer_to_folder(None);
                    ui.close();
                }
            });
        });
        if chrome::icon_button(
            ui,
            "layer-motion-guide",
            BAR_ICON,
            self.active_layer().is_some_and(Layer::is_guide),
            self.is_enabled(MenuAction::ToggleLayerGuide),
            "Motion guide for the layer below",
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleLayerGuide);
        }
        if chrome::icon_button(
            ui,
            "layer-mask",
            BAR_ICON,
            self.active_layer().is_some_and(Layer::is_mask),
            self.is_enabled(MenuAction::ToggleLayerMask),
            "Mask the layer below",
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleLayerMask);
        }
        if chrome::icon_button(
            ui,
            "layer-delete",
            BAR_ICON,
            false,
            self.is_enabled(MenuAction::DeleteLayer),
            "Delete layer",
        )
        .clicked()
        {
            self.perform(MenuAction::DeleteLayer);
        }
        ui.separator();
        for (icon, hint, factor) in [
            (
                "zoom-out",
                "Zoom timeline out",
                1.0 / crate::timeline_view::STEP,
            ),
            ("zoom-in", "Zoom timeline in", crate::timeline_view::STEP),
        ] {
            if chrome::icon_button(ui, icon, BAR_ICON, false, true, hint).clicked() {
                self.zoom_timeline(factor);
            }
        }
        if chrome::icon_button(ui, "zoom-fit", BAR_ICON, false, true, "Fit the scene").clicked() {
            self.timeline_zoom = crate::timeline_view::MIN_ZOOM;
        }
        ui.separator();
        let edit = ui.add(
            egui::TextEdit::singleline(&mut self.layer_name_edit)
                .hint_text("Rename layer")
                .desired_width(120.0),
        );
        let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
        if submitted && !self.layer_name_edit.is_empty() {
            let name = std::mem::take(&mut self.layer_name_edit);
            self.rename_active_layer(&name);
        }
    }

    /// A new empty layer above the others, made the active one: one undo step.
    pub fn add_layer(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        let id = LayerId::new();
        let layer = match Layer::new(id, format!("Layer {}", count + 1), vec![]) {
            Ok(layer) => layer,
            Err(error) => return self.report(error.to_string()),
        };
        self.run_batch(vec![Command::AddLayer { scene, layer }], "add a layer");
        if self.status.is_none() {
            self.current_layer = Some(id);
        }
    }
}
