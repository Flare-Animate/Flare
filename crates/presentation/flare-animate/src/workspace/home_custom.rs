//! The Custom Size card on Home: a small window to choose a width, height and frame rate that
//! none of the presets has.

use egui::Context;
use flare_document::MAX_STAGE_SIDE;

use super::*;

/// The most frames per second a project can be made at (the document settings allow the same).
const MAX_FPS: u32 = 240;

/// What the Custom Size window is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct CustomSize {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) fps: u32,
}

impl Default for CustomSize {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            fps: 24,
        }
    }
}

impl Workspace {
    /// Opens the Custom Size window, on the size last chosen if there was one.
    pub fn show_custom_size(&mut self) {
        self.home
            .custom_size
            .get_or_insert_with(CustomSize::default);
    }

    pub fn custom_size_shown(&self) -> bool {
        self.home.custom_size.is_some()
    }

    pub(super) fn custom_size_window(&mut self, ctx: &Context) {
        // It belongs to the Home page: a tab chosen over it takes it along.
        if !self.windows.home {
            self.home.custom_size = None;
        }
        let Some(mut size) = self.home.custom_size else {
            return;
        };
        let (mut create, mut cancel) = (false, false);
        let mut open = true;
        egui::Window::new("Custom Size")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
            .show(ctx, |ui| {
                egui::Grid::new("custom-size")
                    .num_columns(2)
                    .show(ui, |ui| {
                        ui.label("Width");
                        ui.add(
                            egui::DragValue::new(&mut size.width)
                                .range(1..=MAX_STAGE_SIDE)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label("Height");
                        ui.add(
                            egui::DragValue::new(&mut size.height)
                                .range(1..=MAX_STAGE_SIDE)
                                .suffix(" px"),
                        );
                        ui.end_row();
                        ui.label("Frame rate");
                        ui.add(
                            egui::DragValue::new(&mut size.fps)
                                .range(1..=MAX_FPS)
                                .suffix(" fps"),
                        );
                        ui.end_row();
                    });
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    create = ui.button("Create").clicked();
                    cancel = ui.button("Cancel").clicked();
                });
            });
        self.home.custom_size = Some(size);
        if create {
            self.home.custom_size = None;
            self.new_project(size.width, size.height, size.fps);
        } else if cancel || !open {
            self.home.custom_size = None;
        }
    }
}
