//! The Publish popup: which formats to write, and the button that writes them.

use super::*;

impl Workspace {
    /// The popup, while it is open: a dialog in the middle of the window like Preferences.
    pub(super) fn publish_window(&mut self, ctx: &egui::Context) {
        if !self.windows.publish {
            return;
        }
        let mut request = None;
        // The words are fetched before the popup is drawn, which borrows the workspace mutably.
        let words: std::collections::HashMap<&str, String> = [
            "pubpop-title",
            "close",
            "pubpop-formats",
            "pubpop-button",
            "pubpop-button-hint",
        ]
        .into_iter()
        .map(|key| (key, self.tr(key)))
        .collect();
        let tr = |key: &str| words.get(key).cloned().unwrap_or_default();
        let response = flare_ui::dialog::modal(ctx, self.publish_id).show(ctx, |ui| {
            ui.set_width(340.0);
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.heading(tr("pubpop-title"));
                },
                |ui| {
                    if flare_ui::chrome::icon_button(ui, "close", 14.0, false, true, &tr("close"))
                        .clicked()
                    {
                        ui.close();
                    }
                },
            );
            ui.add_space(4.0);
            flare_ui::chrome::section(ui, "file-publish", &tr("pubpop-formats"));
            self.publish_settings_ui(ui);
            ui.add_space(10.0);
            let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y + 6.0);
            ui.allocate_ui_with_layout(
                row,
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    // Publish is where Close used to be, the corner a dialog's main button is
                    // in; Close is beside it.
                    let ready = self.publish.any() && !self.document().scenes().is_empty();
                    if ui
                        .add_enabled(ready, egui::Button::new(tr("pubpop-button")))
                        .on_hover_text(tr("pubpop-button-hint"))
                        .clicked()
                    {
                        request = Some(DialogRequest::Publish);
                    }
                    if ui.button(tr("close")).clicked() {
                        ui.close();
                    }
                },
            );
        });
        // Choosing where to save leaves the popup.
        if request.is_some() {
            self.windows.publish = false;
            self.dialog = request;
        } else {
            self.windows.publish = !response.should_close();
        }
    }
}
