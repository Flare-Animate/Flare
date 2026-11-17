//! What File > Publish writes and how: the formats and, for HTML5 Canvas, the options Animate's
//! Publish Settings call Loop timeline, Center stage and Make responsive. The transparent
//! background is here too, and Export Image and Export Movie follow it as Publish does.

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
    /// Write an MP4 video (`.mp4`).
    pub mp4: bool,
    /// Write a QuickTime movie (`.mov`).
    pub mov: bool,
    /// Leave the stage colour out of the GIF, the videos and the exported pictures, so what is
    /// not drawn is see-through.
    pub transparent: bool,
}

/// Whether this build writes MP4 and QuickTime movies. The browser build does not yet: its video
/// encoder is native only (docs/SURFACES.md).
pub(super) const VIDEO_PUBLISHES_HERE: bool = !cfg!(target_arch = "wasm32");

/// Which defaults the format choices in a preferences file were saved under: 0 is before MP4
/// became the format a new install publishes, when a Flash movie and an HTML5 page were.
pub(super) const PUBLISH_DEFAULTS: u32 = 1;

impl Default for PublishSettings {
    /// An MP4 video alone: what most people publish an animation as. The Flash movie and the
    /// HTML5 page are a tick away. Where no video can be written (the browser), an animated GIF.
    fn default() -> Self {
        Self {
            swf: false,
            compress: true,
            html5: false,
            loop_timeline: true,
            center_stage: true,
            responsive: false,
            gif: !VIDEO_PUBLISHES_HERE,
            mp4: VIDEO_PUBLISHES_HERE,
            mov: false,
            transparent: false,
        }
    }
}

impl PublishSettings {
    /// The formats a file saved before [`PUBLISH_DEFAULTS`] 1 holds when the person never changed
    /// them: those old defaults are taken as not chosen, and the new ones are used instead.
    pub(super) fn update_old_defaults(self, saved_under: u32) -> Self {
        let old_defaults = self.swf && self.html5 && !self.gif && !self.mp4 && !self.mov;
        if saved_under >= PUBLISH_DEFAULTS || !old_defaults {
            return self;
        }
        let new = Self::default();
        Self {
            swf: new.swf,
            html5: new.html5,
            gif: new.gif,
            mp4: new.mp4,
            ..self
        }
    }

    /// Whether anything is switched on to publish.
    pub fn any(&self) -> bool {
        self.swf || self.html5 || self.gif || self.mp4 || self.mov
    }

    /// What the exports show where nothing is drawn.
    pub fn backdrop(&self) -> flare_render::Backdrop {
        flare_render::Backdrop::transparent_if(self.transparent)
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
        for (ticked, key) in [
            (&mut self.publish.mp4, "pubpop-format-mp4"),
            (&mut self.publish.mov, "pubpop-format-mov"),
        ] {
            ui.add_enabled(
                VIDEO_PUBLISHES_HERE,
                egui::Checkbox::new(ticked, tr.text(key)),
            )
            .on_disabled_hover_text(tr.text("pubpop-video-browser"));
        }
        ui.separator();
        ui.checkbox(&mut self.publish.transparent, tr.text("pubpop-transparent"))
            .on_hover_text(tr.text("pubpop-transparent-hint"));
        if !self.publish.any() {
            ui.colored_label(flare_ui::theme::warning(), tr.text("pubpop-nothing-chosen"));
        }
        if self.publish.transparent && self.publish.mp4 {
            ui.colored_label(
                flare_ui::theme::warning(),
                tr.text("pubpop-transparent-mp4"),
            );
        }
    }
}
