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
    /// Write an MP4 video (`.mp4`); it needs ffmpeg, which the shell says whether it found.
    pub mp4: bool,
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
            mp4: false,
        }
    }
}

impl PublishSettings {
    /// Whether anything is switched on to publish.
    pub fn any(&self) -> bool {
        self.swf || self.html5 || self.gif || self.mp4
    }
}

impl Workspace {
    /// Tells the workspace whether an `ffmpeg` was found, which MP4 needs.
    pub fn set_video_encoder_available(&mut self, found: bool) {
        self.video_encoder = found;
    }

    /// Whether a format is ticked that can be written on this computer.
    pub(super) fn can_publish_something(&self) -> bool {
        let s = self.publish;
        s.swf || s.html5 || s.gif || (s.mp4 && self.video_encoder)
    }

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
        // Video needs an ffmpeg on this computer: without one the box is dimmed and says why.
        let mut ticked = self.publish.mp4 && self.video_encoder;
        let video = ui.add_enabled(
            self.video_encoder,
            egui::Checkbox::new(&mut ticked, tr.text("pubpop-format-mp4")),
        );
        if self.video_encoder {
            self.publish.mp4 = ticked;
        } else {
            video.on_disabled_hover_text(tr.text("pubpop-mp4-needs-ffmpeg"));
        }
        if !self.can_publish_something() {
            ui.colored_label(flare_ui::theme::warning(), tr.text("pubpop-nothing-chosen"));
        }
    }
}
