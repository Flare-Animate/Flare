//! The History and Output panels.

use crate::actions::MenuAction;
use crate::workspace::Workspace;
use egui::{RichText, Ui};
use flare_ui::chrome;
use flare_ui::icons;
use flare_ui::theme;

use super::*;

impl Workspace {
    /// Messages from earlier actions, newest at the bottom.
    pub(crate) fn output_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if chrome::icon_button(ui, "delete", BAR_ICON, false, !self.log.is_empty(), "Clear")
                .clicked()
            {
                self.log.clear();
            }
            ui.weak(format!("{} messages", self.log.len()));
        });
        ui.separator();
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                if self.log.is_empty() {
                    chrome::empty_state(ui, "info", "Nothing to report");
                }
                for line in &self.log {
                    ui.horizontal(|ui| {
                        ui.add(icons::image("warning", 13.0, theme::WARNING));
                        ui.label(RichText::new(line).monospace());
                    });
                }
            });
    }

    // ---- history -----------------------------------------------------------------------

    pub(crate) fn history_panel(&mut self, ui: &mut Ui) {
        ui.horizontal(|ui| {
            if chrome::icon_button(
                ui,
                "undo",
                BAR_ICON,
                false,
                self.is_enabled(MenuAction::Undo),
                "Undo",
            )
            .clicked()
            {
                self.perform(MenuAction::Undo);
            }
            if chrome::icon_button(
                ui,
                "redo",
                BAR_ICON,
                false,
                self.is_enabled(MenuAction::Redo),
                "Redo",
            )
            .clicked()
            {
                self.perform(MenuAction::Redo);
            }
            chrome::mock_icon_button(ui, "delete", BAR_ICON, "Clear history");
        });
        chrome::empty_state(ui, "panel-history", "Each edit will be listed here");
        ui.vertical_centered(|ui| {
            ui.weak("Click an entry to return to that point.");
        });
    }
}
