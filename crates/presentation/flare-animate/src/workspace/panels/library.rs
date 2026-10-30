//! The library panel.

use crate::workspace::{SymbolId, Workspace};
use egui::Ui;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

/// Height of the preview well above the list: room for a picture and the symbol's name.
const PREVIEW_WELL: f32 = 110.0;

impl Workspace {
    // ---- library -----------------------------------------------------------------------

    /// A small picture of the symbol's first frame, drawn once per change to the project.
    pub(crate) fn symbol_preview(
        &mut self,
        ctx: &egui::Context,
        symbol: SymbolId,
    ) -> Option<egui::TextureHandle> {
        const BOX: f32 = 96.0;
        let revision = self.session.revision();
        if let Some((id, at, handle)) = &self.library_preview
            && *id == symbol
            && *at == revision
        {
            return Some(handle.clone());
        }
        let document = self.session.document();
        let bounds = flare_tools::ops::symbol_bounds(document, symbol)?;
        let (w, h) = (bounds.width().max(1.0), bounds.height().max(1.0));
        let scale = (BOX / w).min(BOX / h);
        let (pw, ph) = (
            (w * scale).ceil().max(1.0) as u32,
            (h * scale).ceil().max(1.0) as u32,
        );
        let (scratch, scene) = flare_editor::symbol_workspace(document, symbol)?;
        let frame = flare_eval::evaluate_frame(&scratch, scene, 0).ok()?;
        let viewport = flare_render::Viewport {
            x: f64::from(bounds.min.x),
            y: f64::from(bounds.min.y),
            width: f64::from(w),
            height: f64::from(h),
        };
        let image = flare_render::render_viewport(
            &frame,
            &viewport,
            pw,
            ph,
            flare_core::Rgba8::TRANSPARENT,
        )
        .ok()?;
        let picture =
            egui::ColorImage::from_rgba_unmultiplied([pw as usize, ph as usize], image.rgba());
        let handle = ctx.load_texture("library-preview", picture, egui::TextureOptions::LINEAR);
        self.library_preview = Some((symbol, revision, handle.clone()));
        Some(handle)
    }

    pub(crate) fn library_panel(&mut self, ui: &mut Ui) {
        let entries: Vec<(SymbolId, String)> = self
            .document()
            .symbols()
            .iter()
            .map(|s| (s.id(), s.name().to_owned()))
            .collect();
        let ctx = ui.ctx().clone();
        // A preview well above the list, as in other animation tools.
        let well = egui::Frame::new()
            .fill(theme::FIELD)
            .stroke(egui::Stroke::new(1.0, theme::BORDER))
            .corner_radius(4.0);
        well.show(ui, |ui| {
            ui.set_width(ui.available_width());
            let preview = self
                .selected_symbol
                .and_then(|id| self.symbol_preview(&ctx, id));
            // The same height with or without a picture, with what there is centered in it.
            chrome::centered_block(ui, PREVIEW_WELL, |ui| {
                match &preview {
                    Some(texture) => {
                        ui.add(egui::Image::new(texture).max_size(egui::vec2(80.0, 80.0)));
                    }
                    None => {
                        ui.add(icons::image("symbol-graphic", 28.0, theme::TEXT_DISABLED));
                    }
                }
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
