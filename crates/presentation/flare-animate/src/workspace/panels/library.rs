//! The library panel.

//! The panels of the workspace: tools, properties, library, color, align, transform, history,
//! the timeline's toolbar and the status bar.
//!
//! Controls that act on the document are real. Controls for features that do not exist yet are
//! drawn the same way but dimmed, with a hover note, using the mock helpers in `chrome`; each
//! is a place a later change fills in.

use crate::workspace::{SymbolId, Workspace};
use egui::Ui;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

impl Workspace {
    // ---- library -----------------------------------------------------------------------

    pub(crate) fn library_panel(&mut self, ui: &mut Ui) {
        let entries: Vec<(SymbolId, String)> = self
            .document()
            .symbols()
            .iter()
            .map(|s| (s.id(), s.name().to_owned()))
            .collect();
        // A preview well above the list, as in other animation tools.
        let well = egui::Frame::new()
            .fill(theme::FIELD)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(4.0);
        well.show(ui, |ui| {
            ui.set_min_height(64.0);
            ui.set_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.add_space(14.0);
                ui.add(icons::image("symbol-graphic", 28.0, theme::TEXT_DISABLED));
                ui.weak(
                    match self.selected_symbol.and_then(|id| {
                        entries
                            .iter()
                            .find(|(s, _)| *s == id)
                            .map(|(_, n)| n.clone())
                    }) {
                        Some(name) => name,
                        None => "No symbol selected".to_owned(),
                    },
                );
            });
        });
        ui.add_space(4.0);
        ui.add(
            egui::TextEdit::singleline(&mut self.library_search)
                .hint_text("Search library")
                .desired_width(f32::INFINITY),
        );
        let needle = self.library_search.trim().to_lowercase();
        let entries: Vec<(SymbolId, String)> = entries
            .into_iter()
            .filter(|(_, name)| needle.is_empty() || name.to_lowercase().contains(&needle))
            .collect();
        ui.add_space(4.0);
        let list_height = (ui.available_height() - 34.0).max(40.0);
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink(false)
            .show(ui, |ui| {
                if entries.is_empty() {
                    chrome::empty_state(ui, "panel-library", "No symbols yet");
                    ui.vertical_centered(|ui| {
                        ui.weak("Select artwork and press F8 to make one.");
                    });
                }
                for (id, name) in entries {
                    let chosen = self.selected_symbol == Some(id);
                    if let Some((target, text)) = &mut self.renaming
                        && *target == id
                    {
                        let edit =
                            ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                        edit.request_focus();
                        let done = edit.lost_focus();
                        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if done || cancel {
                            let text = std::mem::take(text);
                            self.renaming = None;
                            if !cancel {
                                self.rename_symbol(id, &text);
                            }
                        }
                        continue;
                    }
                    let response = ui
                        .horizontal(|ui| {
                            ui.add(icons::image("symbol-graphic", 16.0, theme::TEXT_DIM));
                            ui.selectable_label(chosen, name.clone())
                        })
                        .inner;
                    response.context_menu(|ui| {
                        if ui.button("Rename").clicked() {
                            self.renaming = Some((id, name.clone()));
                            ui.close();
                        }
                        if ui.button("Delete").clicked() {
                            self.delete_symbol(id);
                            ui.close();
                        }
                    });
                    if response.clicked() {
                        self.selected_symbol = Some(id);
                    }
                    if response.double_clicked() {
                        self.selected_symbol = Some(id);
                        self.place_selected_symbol();
                    }
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "plus",
                BAR_ICON,
                false,
                self.selected_symbol.is_some(),
                "Place on stage",
            )
            .clicked()
            {
                self.place_selected_symbol();
            }
            let editable = !self.in_symbol();
            if chrome::icon_button(
                ui,
                "symbol-movie-clip",
                BAR_ICON,
                false,
                editable,
                "New symbol",
            )
            .clicked()
            {
                self.new_symbol();
            }
            chrome::mock_icon_button(ui, "folder", BAR_ICON, "New folder");
            let selected = self.selected_symbol.filter(|_| editable);
            if chrome::icon_button(
                ui,
                "settings",
                BAR_ICON,
                false,
                selected.is_some(),
                "Rename symbol",
            )
            .clicked()
                && let Some(id) = selected
                && let Some(symbol) = self.document().symbol(id)
            {
                self.renaming = Some((id, symbol.name().to_owned()));
            }
            if chrome::icon_button(
                ui,
                "delete",
                BAR_ICON,
                false,
                selected.is_some(),
                "Delete symbol",
            )
            .clicked()
                && let Some(id) = selected
            {
                self.delete_symbol(id);
            }
        });
    }
}
