//! The Brush's stroke options beyond its nib: the stabilizer that steadies the hand, the tapers
//! at the two ends of a stroke, and its texture. They belong to the brush in hand, so choosing a
//! profile or a custom brush sets them, and editing them edits the custom brush chosen. The
//! Pencil's pressure options are here too, as the Pencil under a pen draws with the brush library.

use egui::Ui;
use flare_ui::chrome;

use crate::workspace::Workspace;

/// The longest taper the options offer, in stage units; the library takes longer ones.
const MAX_TAPER_OPTION: f64 = 500.0;

impl Workspace {
    pub(super) fn brush_stroke_options(&mut self, ui: &mut Ui) {
        let mut level = self.brush_shelf.stabilizer();
        let (label, tip) = (
            tr!(self, "opt-stabilization"),
            tr!(self, "opt-stabilization-tip"),
        );
        chrome::slider(
            ui,
            &label,
            &mut level,
            0..=flare_input::MAX_STABILIZER_LEVEL,
            "",
            false,
        )
        .on_hover_text(tip);
        self.brush_shelf.set_stabilizer(level);
        let taper = self.brush_shelf.taper();
        let (mut start, mut end) = (taper.start(), taper.end());
        let label = tr!(self, "opt-taper-start");
        chrome::slider(ui, &label, &mut start, 0.0..=MAX_TAPER_OPTION, " px", false);
        let label = tr!(self, "opt-taper-end");
        chrome::slider(ui, &label, &mut end, 0.0..=MAX_TAPER_OPTION, " px", false);
        self.brush_shelf.set_taper(start, end);
        let texture = self.brush_shelf.texture();
        let (mut bristles, mut grain) = (texture.bristles(), texture.grain());
        let (label, tip) = (tr!(self, "opt-bristles"), tr!(self, "opt-bristles-tip"));
        chrome::slider(
            ui,
            &label,
            &mut bristles,
            0..=flare_tools::brush::MAX_BRISTLES,
            "",
            false,
        )
        .on_hover_text(tip);
        let (label, tip) = (tr!(self, "opt-grain"), tr!(self, "opt-grain-tip"));
        chrome::slider(ui, &label, &mut grain, 0.0..=1.0, "", false).on_hover_text(tip);
        self.brush_shelf.set_texture(bristles, grain);
    }
    }
}
