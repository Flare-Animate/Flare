//! The Help menu's windows: About, and the list of keyboard shortcuts.

use super::Workspace;
use crate::actions::MenuAction;

impl Workspace {
    /// The About and Keyboard Shortcuts windows, while they are open.
    pub(super) fn help_windows(&mut self, ctx: &egui::Context) {
        if self.windows.about {
            let mut open = true;
            egui::Window::new("About Flare")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Flare");
                    ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                    ui.weak("An animation tool in the spirit of Adobe Animate.");
                    ui.weak("Early alpha: keep backups of your work.");
                    ui.hyperlink_to(
                        "github.com/Flare-Animate/Flare",
                        "https://github.com/Flare-Animate/Flare",
                    );
                });
            self.windows.about = open;
        }
        if self.windows.shortcuts {
            let mut open = true;
            let lines = self.shortcut_lines();
            egui::Window::new("Keyboard Shortcuts")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            egui::Grid::new("shortcuts").num_columns(2).show(ui, |ui| {
                                for (name, keys) in &lines {
                                    ui.label(name);
                                    ui.weak(keys);
                                    ui.end_row();
                                }
                            });
                        });
                });
            self.windows.shortcuts = open;
        }
    }

    /// Every command that has a shortcut or a tool key: (name, keys), in menu order.
    pub fn shortcut_lines(&self) -> Vec<(String, String)> {
        MenuAction::ALL
            .into_iter()
            .filter_map(|action| {
                let keys = action
                    .shortcut_text()
                    .or_else(|| action.tool_key().map(str::to_owned))?;
                Some((self.label(action), keys))
            })
            .collect()
    }
}
