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
    /// Write an animated GIF (`.gif`).
    pub gif: bool,
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
            gif: false,
        }
    }
}

impl PublishSettings {
    /// Whether anything is switched on to publish.
    pub fn any(&self) -> bool {
        self.swf || self.html5 || self.gif
    }
}

impl Workspace {
    pub fn publish_settings(&self) -> PublishSettings {
        self.publish
    }

    /// The Publish Settings choices, as the window and the Document tab show them.
    pub(super) fn publish_settings_ui(&mut self, ui: &mut egui::Ui) {
        let tr = &self.translator;
        ui.checkbox(&mut self.publish.swf, tr.text("pubpop-format-swf"));
        ui.add_enabled_ui(self.publish.swf, |ui| {
            ui.indent("swf_options", |ui| {
                ui.checkbox(&mut self.publish.compress, tr.text("pubpop-compress"));
            });
        });
        ui.checkbox(&mut self.publish.html5, tr.text("pubpop-format-html5"));
        ui.add_enabled_ui(self.publish.html5, |ui| {
            ui.indent("html5_options", |ui| {
                ui.checkbox(&mut self.publish.loop_timeline, tr.text("pubpop-loop"));
                ui.checkbox(&mut self.publish.center_stage, tr.text("pubpop-center"));
                ui.checkbox(&mut self.publish.responsive, tr.text("pubpop-responsive"));
            });
        });
        ui.checkbox(&mut self.publish.gif, tr.text("pubpop-format-gif"));
        // Video needs an encoder Flare does not have yet: shown, dimmed, where it will go.
        ui.add_enabled(
            false,
            egui::Checkbox::new(&mut false, tr.text("pubpop-format-mp4")),
        )
        .on_disabled_hover_text(flare_ui::chrome::NOT_YET);
        if !self.publish.any() {
            ui.colored_label(flare_ui::theme::warning(), tr.text("pubpop-nothing-chosen"));
        }
    }
}
