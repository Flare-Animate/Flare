//! A compact layer stack for selecting and managing the active scene's layers.

use crate::workspace::{LayerId, Workspace};
use egui::Ui;
use flare_ui::chrome;

use super::BAR_ICON;

impl Workspace {
    pub(crate) fn layers_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "layer-new",
                BAR_ICON,
                false,
                !self.in_symbol(),
                &self.tr("msg-layer-new"),
            )
            .clicked()
            {
                self.add_layer();
            }
            if chrome::icon_button(
                ui,
                "layer-folder",
                BAR_ICON,
                false,
                !self.in_symbol(),
                &self.tr("msg-layer-new-folder"),
            )
            .clicked()
            {
                self.add_layer_folder();
            }
            ui.separator();
            let active = self.active_layer_id();
            for (icon, label, direction) in [
                ("chevron-up", "msg-layer-move-up", -1),
                ("chevron-down", "msg-layer-move-down", 1),
            ] {
                if chrome::icon_button(ui, icon, BAR_ICON, false, active.is_some(), &self.tr(label))
                    .clicked()
                {
                    self.move_active_layer(direction);
                }
            }
            if chrome::icon_button(
                ui,
                "layer-delete",
                BAR_ICON,
                false,
                active.is_some() && !self.in_symbol(),
                &self.tr("msg-layer-delete"),
            )
            .clicked()
            {
                self.delete_active_layer();
            }
        });
        ui.separator();

        let rows: Vec<(LayerId, String, bool, bool, bool)> = self
            .document()
            .scenes()
            .first()
            .map(|scene| {
                scene
                    .timeline()
                    .layers()
                    .iter()
                    .map(|layer| {
                        (
                            layer.id(),
                            layer.name().to_owned(),
                            layer.is_visible(),
                            layer.is_locked(),
                            layer.is_folder(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        if rows.is_empty() {
            ui.weak(self.tr("layers-empty"));
            return;
        }

        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .show(ui, |ui| {
                for (id, name, visible, locked, folder) in rows {
                    if let Some((renaming, text)) = &mut self.layer_renaming
                        && *renaming == id
                    {
                        let edit =
                            ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                        edit.request_focus();
                        let done = edit.lost_focus()
                            || ui.input(|input| input.key_pressed(egui::Key::Enter));
                        let cancel = ui.input(|input| input.key_pressed(egui::Key::Escape));
                        if done || cancel {
                            let text = std::mem::take(text);
                            self.layer_renaming = None;
                            if !cancel {
                                self.select_layer(id);
                                self.rename_active_layer(&text);
                            }
                        }
                        continue;
                    }

                    let active = self.active_layer_id() == Some(id);
                    ui.horizontal(|ui| {
                        let mut shown = visible;
                        if ui
                            .checkbox(&mut shown, "")
                            .on_hover_text(self.tr(if visible {
                                "msg-layer-hide"
                            } else {
                                "msg-layer-show"
                            }))
                            .changed()
                        {
                            self.toggle_layer_visible(id);
                        }
                        let mut is_locked = locked;
                        if ui
                            .checkbox(&mut is_locked, "")
                            .on_hover_text(self.tr(if locked {
                                "msg-layer-unlock"
                            } else {
                                "msg-layer-lock"
                            }))
                            .changed()
                        {
                            self.toggle_layer_locked(id);
                        }
                        let icon = if folder { "layer-folder" } else { "document" };
                        ui.add(flare_ui::icons::image(
                            icon,
                            BAR_ICON,
                            flare_ui::theme::text_dim(),
                        ));
                        let response = ui.selectable_label(active, name);
                        response.context_menu(|ui| self.layer_menu(ui, id));
                        if response.clicked() {
                            self.select_layer(id);
                        }
                        if response.double_clicked() {
                            self.begin_layer_rename(id);
                        }
                    });
                }
            });
    }
}
