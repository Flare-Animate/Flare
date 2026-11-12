//! Onion skin's settings, in a popup by its buttons in the timeline's toolbar: how many frames
//! before and after it shows, as outlines or pictures, and whether earlier and later frames are
//! tinted, in which colors, as Animate's onion skin settings have them.

use egui::{Color32, Popup, PopupCloseBehavior, Ui};
use flare_core::Rgba8;
use flare_ui::chrome;

use crate::workspace::Workspace;
use crate::workspace::types::{MAX_ONION_FRAMES, ONION_AFTER_COLOR, ONION_BEFORE_COLOR};

fn to_egui(color: Rgba8) -> Color32 {
    Color32::from_rgb(color.r, color.g, color.b)
}

fn from_egui(color: Color32) -> Rgba8 {
    let [r, g, b, _] = color.to_array();
    Rgba8::opaque(r, g, b)
}

impl Workspace {
    /// The button that opens the settings, and the popup itself while it is open.
    pub(super) fn onion_settings_button(&mut self, ui: &mut Ui, size: f32) {
        let hint = tr!(self, "timeline-onion-settings");
        let response = chrome::icon_button(ui, "chevron-down", size, false, true, &hint);
        Popup::from_toggle_button_response(&response)
            .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
            .show(|ui| {
                ui.set_width(220.0);
                ui.strong(tr!(self, "timeline-onion"));
                ui.add_space(4.0);
                self.onion_settings(ui);
            });
    }

    /// The settings: both ranges, outlines, the tint and its two colors.
    pub(in crate::workspace) fn onion_settings(&mut self, ui: &mut Ui) {
        let mut onion = self.onion;
        let (before, after) = (
            tr!(self, "timeline-onion-before"),
            tr!(self, "timeline-onion-after"),
        );
        chrome::slider(
            ui,
            &before,
            &mut onion.before,
            0..=MAX_ONION_FRAMES,
            "",
            false,
        );
        chrome::slider(
            ui,
            &after,
            &mut onion.after,
            0..=MAX_ONION_FRAMES,
            "",
            false,
        );
        ui.checkbox(&mut onion.outlines, tr!(self, "timeline-onion-outlines"));
        ui.checkbox(&mut onion.tinted, tr!(self, "timeline-onion-tinted"));
        let (mut early, mut late) = (to_egui(onion.before_color), to_egui(onion.after_color));
        ui.add_enabled_ui(onion.tinted, |ui| {
            for (label, color) in [
                (tr!(self, "timeline-onion-before-color"), &mut early),
                (tr!(self, "timeline-onion-after-color"), &mut late),
            ] {
                ui.horizontal(|ui| {
                    ui.color_edit_button_srgba(color);
                    ui.label(label);
                });
            }
            if ui
                .button(tr!(self, "timeline-onion-reset-colors"))
                .clicked()
            {
                early = to_egui(ONION_BEFORE_COLOR);
                late = to_egui(ONION_AFTER_COLOR);
            }
        });
        self.set_onion_range(onion.before, onion.after);
        self.set_onion_outlines(onion.outlines);
        self.set_onion_tinted(onion.tinted);
        self.set_onion_colors(from_egui(early), from_egui(late));
    }
}
