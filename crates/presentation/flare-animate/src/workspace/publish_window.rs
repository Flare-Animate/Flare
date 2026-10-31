//! The Publish popup: which formats to write, the button that writes them, and where to share
//! what was made.

use super::*;

impl Workspace {
    /// The popup, while it is open: a dialog in the middle of the window like Preferences.
    pub(super) fn publish_window(&mut self, ctx: &egui::Context) {
        if !self.windows.publish {
            return;
        }
        let mut request = None;
        let response = egui::Modal::new(self.publish_id).show(ctx, |ui| {
            ui.set_width(340.0);
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.heading("Publish");
                },
                |ui| {
                    if flare_ui::chrome::icon_button(ui, "close", 14.0, false, true, "Close")
                        .clicked()
                    {
                        ui.close();
                    }
                },
            );
            ui.add_space(4.0);
            flare_ui::chrome::section(ui, "file-publish", "Formats");
            self.publish_settings_ui(ui);
            ui.add_space(6.0);
            let ready = self.publish.any() && !self.document().scenes().is_empty();
            if ui
                .add_enabled(ready, egui::Button::new("Publish"))
                .on_hover_text("Choose where the files go, and write each format ticked")
                .clicked()
            {
                request = Some(DialogRequest::Publish);
            }
            flare_ui::chrome::section(ui, "link", "Share");
            ui.weak("Publish first, then add the file on the page that opens.");
            ui.horizontal(|ui| {
                for target in ShareTarget::ALL {
                    if ui.button(target.label()).clicked() {
                        request = Some(DialogRequest::Share(target));
                    }
                }
            });
            ui.add_space(10.0);
            let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y + 6.0);
            ui.allocate_ui_with_layout(
                row,
                egui::Layout::right_to_left(egui::Align::Center),
                |ui| {
                    if ui.button("Close").clicked() {
                        ui.close();
                    }
                },
            );
        });
        // Choosing where to save leaves the popup; sharing keeps it, so a person can share the
        // same file in several places.
        if request == Some(DialogRequest::Publish) {
            self.windows.publish = false;
        } else {
            self.windows.publish = !response.should_close();
        }
        if request.is_some() {
            self.dialog = request;
        }
    }
}
