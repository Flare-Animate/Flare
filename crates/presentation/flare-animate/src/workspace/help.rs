//! The Help menu's windows: About, and the list of keyboard shortcuts.

use super::Workspace;
use flare_menubar::MenuAction;

/// Where the project lives and where its people talk.
pub const GITHUB_URL: &str = "https://github.com/Flare-Animate/Flare";
pub const DISCORD_URL: &str = "https://discord.com/invite/JpeScW8Awa";

impl Workspace {
    /// The About and Keyboard Shortcuts windows, while they are open.
    pub(super) fn help_windows(&mut self, ctx: &egui::Context) {
        if self.windows.about {
            let mut open = true;
            let title = self.label(MenuAction::About);
            let version = self.tr_with(
                "menu-help-version",
                &[("version", env!("CARGO_PKG_VERSION"))],
            );
            let tagline = self.tr("menu-help-tagline");
            let alpha = self.tr("menu-help-alpha");
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.heading("Flare");
                    ui.label(version);
                    ui.weak(tagline);
                    ui.weak(alpha);
                    ui.hyperlink_to("GitHub", GITHUB_URL);
                    ui.hyperlink_to("Discord", DISCORD_URL);
                });
            self.windows.about = open;
        }
        if self.windows.shortcuts {
            let mut open = true;
            let lines = self.shortcut_lines();
            let title = self.label(MenuAction::KeyboardShortcuts);
            egui::Window::new(title)
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
