//! The Eraser in the workspace: dragging rubs out what the eraser passes over as it goes, on every
//! layer a drawing tool may change, as one undo step; a click rubs out a dot. With Faucet on, a
//! click deletes the whole shape under it instead, as the Eraser always did before it could rub.
//! A straightened drag (Straight, or Shift held as it starts) cannot be known until it ends, so it
//! shows the area it will rub and rubs it out on release.

use egui::{Color32, Pos2, Ui};
use flare_input::InputSample;
use flare_tools::eraser::{MAX_ERASER_SIZE, eraser_outline};
use flare_ui::chrome;

use super::stroke_preview::{PreviewSides, PreviewStroke};
use crate::workspace::selection::step_label;
use crate::workspace::*;

/// What the area an eraser drag covers is shown in while dragging: see-through, so what will be
/// rubbed out still shows under it.
const ERASER_INK: Color32 = Color32::from_rgba_premultiplied(70, 90, 140, 110);

/// How far a drag that rubs out as it goes has got.
#[derive(Clone, Copy, Debug, Default)]
pub(in crate::workspace) struct LiveErase {
    /// How many samples of the drag have been rubbed out along.
    done: usize,
    /// The sample the rubbing has reached; the next stretch starts here, so the stretches meet.
    reached: Option<InputSample>,
    /// The revision the drag's own last change left the document at, to add the next to it.
    step: Option<u64>,
}

impl Workspace {
    /// Whether the Eraser in hand rubs out along a drag (rather than deleting whole shapes).
    pub(in crate::workspace) fn eraser_rubs(&self) -> bool {
        self.tool == Tool::Eraser && !self.eraser_faucet
    }

    /// Rubs out the stretch of the drag in progress since the last frame, and its mirror images,
    /// as part of the drag's one undo step. Each frame cuts only that short stretch, so the cost
    /// stays the same however long the drag gets. A drag that starts straightened waits for
    /// release instead.
    pub(in crate::workspace) fn erase_as_dragged(&mut self) {
        if !self.eraser_rubs() {
            return;
        }
        if self.freehand.len() == 1 && self.live_erase.is_none() {
            if self.paint.straight_edge || self.paint.straight_held {
                return;
            }
            self.live_erase = Some(LiveErase::default());
        }
        let samples = std::mem::take(&mut self.freehand);
        self.erase_new(&samples);
        self.freehand = samples;
    }

    /// Rubs out the part of the drag through `samples` that has not been rubbed out yet.
    fn erase_new(&mut self, samples: &[InputSample]) {
        let Some(mut live) = self.live_erase else {
            return;
        };
        let Some(&last) = samples.last() else {
            return;
        };
        if live.reached == Some(last) {
            return;
        }
        let from = live.done.min(samples.len() - 1);
        let stretch: Vec<InputSample> = live
            .reached
            .into_iter()
            .chain(samples[from..].iter().copied())
            .collect();
        live.done = samples.len();
        live.reached = Some(last);
        live.step = self.erase_joining(stretch, live.step);
        self.live_erase = Some(live);
    }

    /// Ends a drag that rubs out as it goes, rubbing out what is left of it; a drag that waited
    /// for release rubs out all of `samples` now. The stroke has already been taken off the
    /// workspace by then, so what is left is found in `samples`.
    pub(in crate::workspace) fn finish_erase(&mut self, samples: Vec<InputSample>) {
        if self.live_erase.is_some() {
            self.erase_new(&samples);
            self.live_erase = None;
            return;
        }
        self.erase_joining(samples, None);
    }

    /// Takes back what a drag that was rubbing out as it went has rubbed out, when the drag is
    /// cancelled, as a cancelled Brush stroke leaves nothing.
    pub(in crate::workspace) fn cancel_erase(&mut self) {
        let Some(live) = self.live_erase.take() else {
            return;
        };
        if live.step == Some(self.active().revision()) {
            self.active_mut().undo();
        }
    }

    /// Rubs out what `samples` (and its mirror images) passed over, joining the undo step the
    /// revision `joining` names when it is still the newest. Gives the revision to join next.
    fn erase_joining(&mut self, samples: Vec<InputSample>, joining: Option<u64>) -> Option<u64> {
        let size = f64::from(self.eraser_size);
        let outlines: Vec<_> = self
            .stroke_set(samples)
            .iter()
            .map(|stroke| eraser_outline(stroke, size))
            .filter(|outline| outline.len() >= 3)
            .collect();
        let scene = self.scene_id()?;
        let commands = flare_tools::eraser::erase(self.document(), scene, self.frame, &outlines);
        if commands.is_empty() {
            return joining;
        }
        let label = step_label(&self.tr("msg-erase"));
        match self.active_mut().apply_joining(commands, &label, joining) {
            Ok(Some(revision)) => {
                self.status = None;
                Some(revision)
            }
            Ok(None) => joining,
            Err(error) => {
                self.report(format!("Cannot {}: {error}", self.tr("msg-erase")));
                joining
            }
        }
    }

    /// Rubs out a dot the size of the eraser at `point`.
    pub(in crate::workspace) fn erase_dot(&mut self, point: Pos2) {
        let Some(at) = InputSample::mouse(f64::from(point.x), f64::from(point.y)) else {
            return;
        };
        self.live_erase = None;
        self.erase_joining(vec![at], None);
    }

    /// The area an eraser drag through `strokes` covers, to show while dragging. A drag that
    /// rubs out as it goes shows nothing: what it has rubbed out is already gone.
    pub(in crate::workspace) fn eraser_previews(
        &self,
        strokes: &[Vec<InputSample>],
    ) -> Vec<PreviewStroke> {
        if self.live_erase.is_some() {
            return Vec::new();
        }
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
