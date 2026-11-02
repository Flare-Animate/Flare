//! The New Document window: a preset and the width, height, frame rate and background color a
//! project is made with, as Animate's New Document dialog has them. The New File button opens it
//! on the first preset (the sizes Flare has always started with) and the Custom Size card on a size
//! that is none of them.

use egui::Context;
use flare_core::Rgba8;
use flare_document::MAX_STAGE_SIDE;

use super::home::PRESETS;
use super::*;

/// The most frames per second a project can be made at (the document settings allow the same).
const MAX_FPS: u32 = 240;

/// What the New Document window is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NewDocument {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) fps: u32,
    pub(super) background: Rgba8,
}

impl NewDocument {
    /// The numbers of preset `index` of `PRESETS`, on a white stage.
    pub(super) fn preset(index: usize) -> Self {
        let (_, width, height, fps) = PRESETS.get(index).copied().unwrap_or(PRESETS[0]);
        Self {
            width,
            height,
            fps,
            background: Rgba8::WHITE,
        }
    }

    /// The size the Custom Size card starts on: none of the presets', so the window says Custom.
    pub(super) fn custom() -> Self {
        Self {
            width: 1024,
            height: 768,
            fps: 24,
            background: Rgba8::WHITE,
        }
    }

    /// The preset these numbers are exactly those of, if there is one.
    pub(super) fn matching_preset(&self) -> Option<usize> {
        PRESETS
            .iter()
            .position(|&(_, w, h, fps)| (w, h, fps) == (self.width, self.height, self.fps))
    }
}

impl Workspace {
    /// Opens the New Document window on the first preset, as the New File button does.
    pub fn show_new_document(&mut self) {
        self.home.new_document = Some(NewDocument::preset(0));
    }

    /// Opens the window on a custom size, as the Custom Size card does; on the size last
    /// chosen if it is already open.
    pub fn show_custom_size(&mut self) {
        self.home
            .new_document
            .get_or_insert_with(NewDocument::custom);
    }

    pub fn new_document_shown(&self) -> bool {
        self.home.new_document.is_some()
    }

    pub(super) fn new_document_window(&mut self, ctx: &Context) {
        // It belongs to the Home page: a tab chosen over it takes it along.
        if !self.windows.home {
            self.home.new_document = None;
        }
        let Some(mut doc) = self.home.new_document else {
            return;
        };
        let (mut create, mut cancel) = (false, false);
        let words = [
            "start-new-document",
            "start-preset",
            "start-custom-size",
            "start-width",
            "start-height",
            "start-frame-rate",
            "start-background",
            "start-create",
            "start-cancel",
        ]
        .map(|key| self.tr(key));
        let [
            title,
            preset,
            custom,
            width,
            height,
            rate,
            background,
            create_label,
            cancel_label,
        ] = words;
        let preset_names: Vec<String> = PRESETS
            .iter()
            .map(|(name, ..)| self.preset_title(name))
            .collect();
        let chosen = doc.matching_preset();
        let mut open = true;
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("new-document")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label(preset);
                        let shown = chosen.map_or(custom.clone(), |i| preset_names[i].clone());
                        egui::ComboBox::from_id_salt("new-document-preset")
                            .selected_text(shown)
                            .show_ui(ui, |ui| {
                                for (index, name) in preset_names.iter().enumerate() {
                                    if ui.selectable_label(chosen == Some(index), name).clicked() {
                                        let background = doc.background;
                                        doc = NewDocument::preset(index);
                                        doc.background = background;
                                    }
                                }
                                // The numbers are kept as they are; this is what it says when
                                // they are none of the presets'.
                                ui.add_enabled(
                                    chosen.is_none(),
                                    egui::Button::selectable(chosen.is_none(), &custom),
                                );
                            });
                        ui.end_row();
                        ui.label(width);
                        ui.add(
                            egui::DragValue::new(&mut doc.width)
                                .range(1..=MAX_STAGE_SIDE)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label(height);
                        ui.add(
                            egui::DragValue::new(&mut doc.height)
                                .range(1..=MAX_STAGE_SIDE)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label(rate);
                        ui.add(
                            egui::DragValue::new(&mut doc.fps)
                                .range(1..=MAX_FPS)
                                .suffix(" fps"),
                        );
                        ui.end_row();
                        ui.label(background);
                        let [r, g, b, a] = doc.background.to_array();
                        let mut color = egui::Color32::from_rgba_unmultiplied(r, g, b, a);
                        if ui.color_edit_button_srgba(&mut color).changed() {
                            let [r, g, b, a] = color.to_array();
                            doc.background = Rgba8::new(r, g, b, a);
                        }
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    create = ui.button(create_label).clicked();
                    cancel = ui.button(cancel_label).clicked();
                });
            });
        self.home.new_document = Some(doc);
        if create {
            self.home.new_document = None;
            self.new_project_with(doc.width, doc.height, doc.fps, doc.background);
        } else if cancel || !open {
            self.home.new_document = None;
        }
    }
}
