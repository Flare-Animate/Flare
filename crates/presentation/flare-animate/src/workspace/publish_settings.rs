//! What File > Publish writes and how: the formats and, for HTML5 Canvas, the options Animate's
//! Publish Settings call Loop timeline, Center stage and Make responsive.

use super::*;

/// The publish choices, kept between sessions with the other preferences.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PublishSettings {
    /// Write a Flash movie (`.swf`).
    pub swf: bool,
    /// Compress the movie.
    pub compress: bool,
    /// Write an HTML5 Canvas page (`.html`).
    pub html5: bool,
    pub loop_timeline: bool,
    pub center_stage: bool,
    pub responsive: bool,
}

impl Default for PublishSettings {
    fn default() -> Self {
        Self {
            swf: true,
            compress: true,
            html5: true,
            loop_timeline: true,
            center_stage: true,
            responsive: false,
        }
    }
}

impl PublishSettings {
    /// Whether anything is switched on to publish.
    pub fn any(&self) -> bool {
        self.swf || self.html5
    }
}

impl Workspace {
    pub fn publish_settings(&self) -> PublishSettings {
        self.publish
    }

    /// The Publish Settings choices, as the window and the Document tab show them.
    pub(super) fn publish_settings_ui(&mut self, ui: &mut egui::Ui) {
        ui.checkbox(&mut self.publish.swf, "Flash (.swf)");
        ui.add_enabled_ui(self.publish.swf, |ui| {
            ui.indent("swf_options", |ui| {
                ui.checkbox(&mut self.publish.compress, "Compress movie");
            });
        });
        ui.checkbox(&mut self.publish.html5, "HTML5 Canvas (.html)");
        ui.add_enabled_ui(self.publish.html5, |ui| {
            ui.indent("html5_options", |ui| {
                ui.checkbox(&mut self.publish.loop_timeline, "Loop timeline");
                ui.checkbox(&mut self.publish.center_stage, "Center stage");
                ui.checkbox(&mut self.publish.responsive, "Make responsive");
            });
        });
        if !self.publish.any() {
            ui.colored_label(
                flare_ui::theme::WARNING,
                "Choose a format, or Publish has nothing to write.",
            );
        }
    }
}
