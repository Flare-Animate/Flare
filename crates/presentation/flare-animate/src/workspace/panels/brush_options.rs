//! The Brush's stroke options beyond its nib: the stabilizer that steadies the hand and the
//! tapers at the two ends of a stroke. They belong to the brush in hand, so choosing a profile
//! or a custom brush sets them, and editing them edits the custom brush chosen.

use egui::Ui;
use flare_ui::chrome;

use crate::workspace::Workspace;

/// The longest taper the options offer, in stage units; the library takes longer ones.
const MAX_TAPER_OPTION: f64 = 500.0;

impl Workspace {
    pub(super) fn brush_stroke_options(&mut self, ui: &mut Ui) {
        let mut level = self.brush_shelf.stabilizer();
        let tip = tr!(self, "opt-stabilization-tip");
        chrome::row(ui, &tr!(self, "opt-stabilization"), |ui| {
            ui.add(egui::DragValue::new(&mut level).range(0..=flare_input::MAX_STABILIZER_LEVEL))
                .on_hover_text(&tip);
        });
        self.brush_shelf.set_stabilizer(level);
        let taper = self.brush_shelf.taper();
        let (mut start, mut end) = (taper.start(), taper.end());
        chrome::row(ui, &tr!(self, "opt-taper-start"), |ui| {
            ui.add(
                egui::DragValue::new(&mut start)
                    .range(0.0..=MAX_TAPER_OPTION)
                    .suffix(" px"),
            );
        });
        chrome::row(ui, &tr!(self, "opt-taper-end"), |ui| {
            ui.add(
                egui::DragValue::new(&mut end)
                    .range(0.0..=MAX_TAPER_OPTION)
                    .suffix(" px"),
            );
        });
        self.brush_shelf.set_taper(start, end);
    }
}
