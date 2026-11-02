//! The library panel.

use crate::workspace::{SymbolId, Workspace};
use egui::Ui;
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

/// Height of the preview well above the list: room for a picture and the symbol's name.
const PREVIEW_WELL: f32 = 110.0;

/// The icon that tells the kinds of symbol apart in the list.
pub(crate) fn kind_icon(kind: flare_document::SymbolKind) -> &'static str {
    match kind {
        flare_document::SymbolKind::Graphic => "symbol-graphic",
        flare_document::SymbolKind::MovieClip => "symbol-movie-clip",
        flare_document::SymbolKind::Button => "symbol-button",
    }
}

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
        if let Some((id, at, handle)) = &self.library.preview
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
        self.library.preview = Some((symbol, revision, handle.clone()));
        Some(handle)
    }

    pub(crate) fn library_panel(&mut self, ui: &mut Ui) {
        let entries: Vec<(SymbolId, String, flare_document::SymbolKind)> = self
            .document()
            .symbols()
            .iter()
            .map(|s| (s.id(), s.name().to_owned(), s.kind()))
            .collect();
        let ctx = ui.ctx().clone();
        // A preview well above the list, as in other animation tools.
        let well = egui::Frame::new()
            .fill(theme::field())
            .stroke(egui::Stroke::new(1.0, theme::border()))
            .corner_radius(4.0);
        well.show(ui, |ui| {
            ui.set_width(ui.available_width());
            let preview = self
                .library
                .selected
                .and_then(|id| self.symbol_preview(&ctx, id));
            // The same height with or without a picture, with what there is centered in it.
            chrome::centered_block(ui, PREVIEW_WELL, |ui| {
                match &preview {
                    Some(texture) => {
                        ui.add(egui::Image::new(texture).max_size(egui::vec2(80.0, 80.0)));
                    }
                    None => {
                        ui.add(icons::image("symbol-graphic", 28.0, theme::text_disabled()));
                    }
                }
                ui.weak(
                    match self.library.selected.and_then(|id| {
                        entries
                            .iter()
                            .find(|(s, _, _)| *s == id)
                            .map(|(_, n, _)| n.clone())
                    }) {
                        Some(name) => name,
                        None => tr!(self, "lib-none-selected"),
                    },
                );
            });
        });
        ui.add_space(4.0);
        let new_symbols = tr!(self, "lib-new-symbols");
        let kind_names = flare_document::SymbolKind::ALL.map(|kind| (kind, self.kind_label(kind)));
        let kind_name = |kind: flare_document::SymbolKind| {
            kind_names
                .iter()
                .find(|(k, _)| *k == kind)
                .map_or_else(String::new, |(_, name)| name.clone())
        };
        chrome::row(ui, &new_symbols, |ui| {
            egui::ComboBox::from_id_salt("new_symbol_kind")
                .selected_text(kind_name(self.library.new_kind))
                .show_ui(ui, |ui| {
                    for kind in flare_document::SymbolKind::ALL {
                        ui.selectable_value(&mut self.library.new_kind, kind, kind_name(kind));
                    }
                });
        });
        let search_hint = tr!(self, "lib-search");
        ui.add(
            egui::TextEdit::singleline(&mut self.library.search)
                .hint_text(search_hint)
                .desired_width(f32::INFINITY),
        );
        let needle = self.library.search.trim().to_lowercase();
        let entries: Vec<(SymbolId, String, flare_document::SymbolKind)> = entries
            .into_iter()
            .filter(|(_, name, _)| needle.is_empty() || name.to_lowercase().contains(&needle))
            .collect();
        ui.add_space(4.0);
        let (empty, empty_hint) = (tr!(self, "lib-empty"), tr!(self, "lib-empty-hint"));
        let (rename, type_label, delete) = (
            tr!(self, "lib-rename"),
            tr!(self, "lib-type"),
            tr!(self, "lib-delete"),
        );
        let list_height = (ui.available_height() - 34.0).max(40.0);
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .auto_shrink(false)
            .show(ui, |ui| {
                if entries.is_empty() {
                    chrome::empty_state(ui, "panel-library", &empty);
                    ui.vertical_centered(|ui| {
                        ui.weak(&empty_hint);
                    });
                }
                for (id, name, kind) in entries {
                    let chosen = self.library.selected == Some(id);
                    if let Some((target, text)) = &mut self.library.renaming
                        && *target == id
                    {
                        let edit =
                            ui.add(egui::TextEdit::singleline(text).desired_width(f32::INFINITY));
                        edit.request_focus();
                        let done = edit.lost_focus();
                        let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
                        if done || cancel {
                            let text = std::mem::take(text);
                            self.library.renaming = None;
                            if !cancel {
                                self.rename_symbol(id, &text);
                            }
                        }
                        continue;
                    }
                    let response = ui
                        .horizontal(|ui| {
                            ui.add(icons::image(kind_icon(kind), 16.0, theme::text_dim()))
                                .on_hover_text(kind_name(kind));
                            ui.selectable_label(chosen, name.clone())
                        })
                        .inner;
                    response.context_menu(|ui| {
                        if ui.button(&rename).clicked() {
                            self.library.renaming = Some((id, name.clone()));
                            ui.close();
                        }
                        ui.menu_button(&type_label, |ui| {
                            for option in flare_document::SymbolKind::ALL {
                                if ui
                                    .selectable_label(kind == option, kind_name(option))
                                    .clicked()
                                {
                                    self.set_symbol_kind(id, option);
                                    ui.close();
                                }
                            }
                        });
                        if ui.button(&delete).clicked() {
                            self.delete_symbol(id);
                            ui.close();
                        }
                    });
                    if response.clicked() {
                        self.library.selected = Some(id);
                    }
                    if response.double_clicked() {
                        self.library.selected = Some(id);
                        self.place_selected_symbol();
                    }
                }
            });
        ui.separator();
        let place = tr!(self, "lib-place");
        let new_symbol = tr!(
            self,
            "lib-new-symbol",
            &[("kind", kind_name(self.library.new_kind).as_str())],
        );
        let (new_folder, rename_symbol, delete_symbol) = (
            tr!(self, "lib-new-folder"),
            tr!(self, "lib-rename-symbol"),
            tr!(self, "lib-delete-symbol"),
        );
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "plus",
                BAR_ICON,
                false,
                self.library.selected.is_some(),
                &place,
            )
            .clicked()
            {
                self.place_selected_symbol();
            }
            let editable = !self.in_symbol();
            if chrome::icon_button(
                ui,
                kind_icon(self.library.new_kind),
                BAR_ICON,
                false,
                editable,
                &new_symbol,
            )
            .clicked()
            {
                self.new_symbol();
            }
            chrome::mock_icon_button(ui, "folder", BAR_ICON, &new_folder);
            let selected = self.library.selected.filter(|_| editable);
            if chrome::icon_button(
                ui,
                "settings",
                BAR_ICON,
                false,
                selected.is_some(),
                &rename_symbol,
            )
            .clicked()
                && let Some(id) = selected
                && let Some(symbol) = self.document().symbol(id)
            {
                self.library.renaming = Some((id, symbol.name().to_owned()));
            }
            if chrome::icon_button(
                ui,
                "delete",
                BAR_ICON,
                false,
                selected.is_some(),
                &delete_symbol,
            )
            .clicked()
                && let Some(id) = selected
            {
                self.delete_symbol(id);
            }
        });
    }
}
