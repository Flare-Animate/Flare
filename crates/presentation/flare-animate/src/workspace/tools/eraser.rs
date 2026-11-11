//! The Eraser in the workspace: dragging rubs out what the eraser passes over, on every layer a
//! drawing tool may change, as one undo step; a click rubs out a dot. With Faucet on, a click
//! deletes the whole shape under it instead, as the Eraser always did before it could rub.

use egui::{Color32, Pos2, Ui};
use flare_input::InputSample;
use flare_tools::eraser::{MAX_ERASER_SIZE, eraser_outline};
use flare_ui::chrome;

use super::stroke_preview::{PreviewSides, PreviewStroke};
use crate::workspace::*;

/// What the area an eraser drag covers is shown in while dragging: see-through, so what will be
/// rubbed out still shows under it.
const ERASER_INK: Color32 = Color32::from_rgba_premultiplied(70, 90, 140, 110);

impl Workspace {
    /// Whether the Eraser in hand rubs out along a drag (rather than deleting whole shapes).
    pub(in crate::workspace) fn eraser_rubs(&self) -> bool {
        self.tool == Tool::Eraser && !self.eraser_faucet
    }

    /// Rubs out what the drag through `samples` (and its mirror images) passed over.
    pub(in crate::workspace) fn finish_erase(&mut self, samples: Vec<InputSample>) {
        let size = f64::from(self.eraser_size);
        let outlines: Vec<_> = self
            .stroke_set(samples)
            .iter()
            .map(|stroke| eraser_outline(stroke, size))
            .filter(|outline| outline.len() >= 3)
            .collect();
        self.erase_outlines(&outlines);
    }

    /// Rubs out a dot the size of the eraser at `point`.
    pub(in crate::workspace) fn erase_dot(&mut self, point: Pos2) {
        let Some(at) = InputSample::mouse(f64::from(point.x), f64::from(point.y)) else {
            return;
        };
        self.finish_erase(vec![at]);
    }

    fn erase_outlines(&mut self, outlines: &[Vec<glamx::DVec2>]) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let commands = flare_tools::eraser::erase(self.document(), scene, self.frame, outlines);
        if !commands.is_empty() {
            let name = self.tr("msg-erase");
            self.run_batch(commands, &name);
        }
    }

    /// The area an eraser drag through `strokes` covers, to show while dragging.
    pub(in crate::workspace) fn eraser_previews(
        &self,
        strokes: &[Vec<InputSample>],
    ) -> Vec<PreviewStroke> {
        let size = f64::from(self.eraser_size);
        strokes
            .iter()
            .filter_map(|stroke| {
                let firm: Vec<InputSample> = stroke
                    .iter()
                    .map(|s| InputSample {
                        pressure: 1.0,
                        ..*s
                    })
                    .collect();
                let round = flare_tools::brush::BrushStroke::new(
                    size,
                    flare_input::PressureCurve::new(1.0, 1.0),
                );
                let sides = round.sides(&firm, 0.0)?;
                Some(PreviewStroke {
                    points: Vec::new(),
                    halves: Vec::new(),
                    color: ERASER_INK,
                    sides: Some(PreviewSides::of(&sides)),
                })
            })
            .collect()
    }

    /// The Eraser's options: its size, and Faucet.
    pub(in crate::workspace) fn eraser_options(&mut self, ui: &mut Ui) {
        chrome::row(ui, &tr!(self, "opt-size"), |ui| {
            ui.add(
                egui::DragValue::new(&mut self.eraser_size)
                    .range(1.0..=MAX_ERASER_SIZE as f32)
                    .suffix(" px"),
            );
        });
        let (faucet, tip) = (tr!(self, "opt-faucet"), tr!(self, "opt-faucet-tip"));
        chrome::row(ui, &faucet, |ui| {
            ui.checkbox(&mut self.eraser_faucet, "").on_hover_text(&tip);
        });
        ui.weak(if self.eraser_faucet {
            tr!(self, "tools-help-eraser-faucet")
        } else {
            tr!(self, "tools-help-eraser")
        });
    }
}
