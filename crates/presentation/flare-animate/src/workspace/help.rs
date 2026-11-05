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
            let version = tr!(
                self,
                "menu-help-version",
                &[("version", env!("CARGO_PKG_VERSION"))],
            );
            let tagline = tr!(self, "menu-help-tagline");
            let alpha = tr!(self, "menu-help-alpha");
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
            let title = self.label(MenuAction::KeyboardShortcuts);
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    self.shortcut_editor_ui(ui);
                });
            self.windows.shortcuts = open;
        }
    }

    /// Every command that has a shortcut or a tool key: (name, keys), in menu order.
    pub fn shortcut_lines(&self) -> Vec<(String, String)> {
        MenuAction::ALL
            .into_iter()
            .filter_map(|action| {
                let keys = self.shortcut_label(action)?;
                Some((self.label(action), keys))
            })
            .collect()
    }
}
