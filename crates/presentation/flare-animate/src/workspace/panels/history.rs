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
            let any = self.active().can_undo() || self.active().can_redo();
            if chrome::icon_button(ui, "delete", BAR_ICON, false, any, "Clear history").clicked() {
                self.clear_history();
            }
        });
        ui.separator();
        let (entries, current) = self.history_entries();
        let mut jump = None;
        egui::ScrollArea::vertical()
            .auto_shrink(false)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for (index, label) in entries.iter().enumerate() {
                    // Steps after the current one are undone: shown dim, still reachable.
                    let text = if index > current {
                        RichText::new(label).color(theme::TEXT_DISABLED)
                    } else {
                        RichText::new(label)
                    };
                    if ui.selectable_label(index == current, text).clicked() {
                        jump = Some(index);
                    }
                }
            });
        if let Some(index) = jump {
            self.jump_in_history(index);
        }
    }
}
