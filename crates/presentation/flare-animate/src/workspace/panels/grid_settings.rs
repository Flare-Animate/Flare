//! The grid's settings, in a popup by the grid button in the stage's bar, as Animate's Edit Grid
//! has them: whether the grid shows and snaps, its spacing across and down, and its color, which
//! follows the stage's background until one is chosen.

use egui::{Popup, PopupCloseBehavior, Ui};
use flare_ui::chrome;

use crate::workspace::Workspace;
use crate::workspace::stage_furniture::{DEFAULT_GRID, MAX_GRID, MIN_GRID};

impl Workspace {
    /// The button that opens the settings, and the popup itself while it is open.
    pub(super) fn grid_settings_button(&mut self, ui: &mut Ui, size: f32) {
        let hint = tr!(self, "bar-grid-settings");
        let response = chrome::icon_button(ui, "chevron-down", size, false, true, &hint);
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(240.0);
                ui.strong(tr!(self, "bar-grid"));
                ui.add_space(4.0);
                self.grid_settings(ui);
            });
    }

    /// The settings: show, snap, the spacing each way and the color.
    pub(in crate::workspace) fn grid_settings(&mut self, ui: &mut Ui) {
        let (show, snap) = (
            tr!(self, "prefs-show-grid"),
            tr!(self, "prefs-snap-to-grid"),
        );
        ui.checkbox(&mut self.display.show_grid, show);
        ui.checkbox(&mut self.display.snap_to_grid, snap);
        let mut spacing = self.grid_spacing();
        let (across, down) = (tr!(self, "bar-grid-across"), tr!(self, "bar-grid-down"));
        let range = MIN_GRID..=MAX_GRID;
        chrome::slider(ui, &across, &mut spacing.x, range.clone(), " px", true);
        chrome::slider(ui, &down, &mut spacing.y, range, " px", true);
        self.set_grid_spacing(spacing.x, spacing.y);
        // Automatic until a color is chosen; the chosen one starts from what the grid shows now.
        let mut automatic = self.grid_color().is_none();
        let mut color = self.grid_ink();
        ui.checkbox(&mut automatic, tr!(self, "bar-grid-color-auto"));
        let texts = self.color_texts();
        let label = tr!(self, "bar-grid-color");
        ui.add_enabled_ui(!automatic, |ui| {
            ui.horizontal(|ui| {
                flare_ui::color_picker::color_button(ui, &mut color, &texts);
                ui.label(label);
            });
        });
        self.set_grid_color((!automatic).then_some(color));
        if ui.button(tr!(self, "bar-grid-reset")).clicked() {
            self.set_grid_spacing(DEFAULT_GRID, DEFAULT_GRID);
            self.set_grid_color(None);
        }
    }
}
