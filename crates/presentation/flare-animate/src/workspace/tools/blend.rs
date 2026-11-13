//! Blur and Smudge in the workspace: a drag over a bitmap is handed to the pixels worker a few
//! samples a frame, and each edit that comes back is applied at once, all of them joined into one
//! undo step, so the stroke shows on the stage as it is drawn. The work itself is
//! `flare_tools::blend`; the worker (`flare_runtime::BlendJob`) keeps building the new picture
//! off the window's thread, however large the bitmap.

use std::sync::Arc;

use egui::{Pos2, Ui};
use flare_input::InputSample;
use flare_runtime::{BlendJob, PixelJob};
use flare_tools::blend::{BlendEdit, BlendKind, BlendSettings, MAX_SIZE, MAX_STRETCH};
use flare_ui::chrome;

use crate::workspace::*;

/// The two tools' settings and the stroke on the worker.
pub(in crate::workspace) struct BlendTools {
    pub(in crate::workspace) blur: BlendSettings,
    pub(in crate::workspace) smudge: BlendSettings,
    live: Option<LiveStroke>,
    /// A drag let go while the stroke before it was still finishing: started when that ends.
    queued: Option<(BlendKind, Vec<InputSample>)>,
}

impl Default for BlendTools {
    fn default() -> Self {
        Self {
            blur: BlendSettings::for_kind(BlendKind::Blur),
            smudge: BlendSettings::for_kind(BlendKind::Smudge),
            live: None,
            queued: None,
        }
    }
}

/// A stroke handed to the pixels worker.
#[derive(Clone, Copy, Debug)]
struct LiveStroke {
    id: u64,
    kind: BlendKind,
    /// How many samples of the drag have been handed over, and the last one, which the
    /// freehand list may move in place.
    sent: usize,
    reached: Option<InputSample>,
    /// The revision the stroke's last edit left the document at, to join the next to it.
    step: Option<u64>,
    /// The drag has ended; the worker is showing what is left of it.
    released: bool,
}

#[cfg(test)]
impl BlendTools {
    pub(in crate::workspace) fn stroking(&self) -> bool {
        self.live.is_some()
    }
}

impl LiveStroke {
    fn label(&self) -> &'static str {
        match self.kind {
            BlendKind::Blur => "Blur",
            BlendKind::Smudge => "Smudge",
        }
    }
}

impl Workspace {
    /// Which of the two the tool in hand is, if either.
    pub(in crate::workspace) fn blend_kind(&self) -> Option<BlendKind> {
        match self.tool {
            Tool::Blur => Some(BlendKind::Blur),
            Tool::Smudge => Some(BlendKind::Smudge),
            _ => None,
        }
    }

    fn blend_settings(&self, kind: BlendKind) -> BlendSettings {
        match kind {
            BlendKind::Blur => self.paint.blend.blur,
            BlendKind::Smudge => self.paint.blend.smudge,
        }
    }

    /// The width of the tool in hand, in stage units, for its pointer.
    pub(in crate::workspace) fn blend_size(&self) -> Option<f32> {
        self.blend_kind().map(|kind| self.blend_settings(kind).size)
    }

    /// Hands the part of the drag in progress that the worker has not had yet; the first frame of
    /// a drag starts the stroke. A drag that starts while the last stroke is still finishing
    /// waits for it.
    pub(in crate::workspace) fn blend_as_dragged(&mut self) {
        let Some(kind) = self.blend_kind() else {
            return;
        };
        if self.paint.blend.live.is_some_and(|l| l.released) || self.freehand.is_empty() {
            return;
        }
        if self.paint.blend.live.is_none() && !self.blend_begin(kind, self.freehand[0]) {
            return;
        }
        let samples = std::mem::take(&mut self.freehand);
        self.blend_send(&samples, false);
        self.freehand = samples;
    }

    /// Ends the drag through `samples` (already taken off the workspace): the rest of it goes to
    /// the worker, which shows it and ends the stroke.
    pub(in crate::workspace) fn blend_release(&mut self, samples: Vec<InputSample>) {
        let (Some(kind), Some(&first)) = (self.blend_kind(), samples.first()) else {
            return;
        };
        if self.paint.blend.live.is_some_and(|l| l.released) {
            self.paint.blend.queued = Some((kind, samples));
            return;
        }
        if self.paint.blend.live.is_none() && !self.blend_begin(kind, first) {
            return;
        }
        self.blend_send(&samples, true);
    }

    /// A click: one dab where it lands (Blur softens a spot; Smudge has picked up nothing yet).
    pub(in crate::workspace) fn blend_dot(&mut self, point: Pos2) {
        if let Some(at) = InputSample::mouse(f64::from(point.x), f64::from(point.y)) {
            self.blend_release(vec![at]);
        }
    }

    /// Starts a stroke on the worker at `first`, the first sample of the drag; `false` when
    /// there is no scene to stroke in.
    fn blend_begin(&mut self, kind: BlendKind, first: InputSample) -> bool {
        let Some(scene) = self.scene_id() else {
            return false;
        };
        let id = self.background.next_id();
        self.paint.blend.live = Some(LiveStroke {
            id,
            kind,
            sent: 0,
            reached: None,
            step: None,
            released: false,
        });
        self.background
            .submit_pixels(PixelJob::Blend(BlendJob::Begin {
                id,
                document: self.active().snapshot(),
                scene,
                frame: self.frame,
                start: Pos2::new(first.x as f32, first.y as f32),
                kind,
                settings: self.blend_settings(kind),
            }));
        true
    }

    /// Hands over what `samples` has that the worker has not had; `last` ends the stroke.
    fn blend_send(&mut self, samples: &[InputSample], last: bool) {
        let Some(mut live) = self.paint.blend.live else {
            return;
        };
        let Some(&end) = samples.last() else {
            return;
        };
        let stretch: Vec<InputSample> = if live.sent < samples.len() {
            samples[live.sent..].to_vec()
        } else if live.reached != Some(end) {
            vec![end]
        } else {
            Vec::new()
        };
        if stretch.is_empty() && !last {
            return;
        }
        live.sent = samples.len();
        live.reached = Some(end);
        live.released |= last;
        self.paint.blend.live = Some(live);
        self.background
            .submit_pixels(PixelJob::Blend(BlendJob::Step {
                id: live.id,
                document: self.active().snapshot(),
                samples: stretch,
                last,
            }));
        self.settle_inline();
    }

    /// Takes back what a drag in progress has done, when it is cancelled. A drag already let go
    /// is left to finish.
    pub(in crate::workspace) fn cancel_blend(&mut self) {
        let Some(live) = self.paint.blend.live.filter(|l| !l.released) else {
            return;
        };
        self.paint.blend.live = None;
        self.background
            .submit_pixels(PixelJob::Blend(BlendJob::Drop { id: live.id }));
        if live.step.is_some() && live.step == Some(self.active().revision()) {
            self.active_mut().undo();
        }
    }

    /// An edit of the stroke came back: shows it, joined to the stroke's undo step, and tells the
    /// worker it is in so the stroke goes on.
    pub(in crate::workspace) fn blend_edited(
        &mut self,
        id: u64,
        document: Arc<flare_document::Document>,
        edit: BlendEdit,
    ) {
        let Some(mut live) = self.paint.blend.live.filter(|l| l.id == id) else {
            return;
        };
        // The edit was made for that document; an edit since would make it wrong.
        if !Arc::ptr_eq(&document, &self.active().snapshot()) {
            return self.blend_stop(id, tr!(self, "msg-blend-changed"));
        }
        match self
            .active_mut()
            .apply_joining(edit.commands, live.label(), live.step)
        {
            Ok(revision) => live.step = revision.or(live.step),
            Err(error) => {
                let message = tr!(self, "msg-blend", &[("reason", &error.to_string())]);
                return self.blend_stop(id, message);
            }
        }
        self.status = None;
        self.paint.blend.live = Some(live);
        self.background
            .submit_pixels(PixelJob::Blend(BlendJob::Step {
                id,
                document: self.active().snapshot(),
                samples: Vec::new(),
                last: live.released,
            }));
        self.settle_inline();
    }

    /// The stroke is over; `error` says why, when it stopped early.
    pub(in crate::workspace) fn blend_ended(&mut self, id: u64, error: Option<String>) {
        if self.paint.blend.live.is_none_or(|l| l.id != id) {
            return;
        }
        self.paint.blend.live = None;
        if let Some(reason) = error {
            self.report(tr!(self, "msg-blend", &[("reason", &reason)]));
        }
        if let Some((kind, samples)) = self.paint.blend.queued.take()
            && let Some(&first) = samples.first()
            && self.blend_begin(kind, first)
        {
            self.blend_send(&samples, true);
        }
    }

    fn blend_stop(&mut self, id: u64, message: String) {
        self.paint.blend.live = None;
        self.background
            .submit_pixels(PixelJob::Blend(BlendJob::Drop { id }));
        self.report(message);
    }

    pub(in crate::workspace) fn blend_options(&mut self, ui: &mut Ui) {
        let Some(kind) = self.blend_kind() else {
            return;
        };
        let labels = [
            tr!(self, "opt-size"),
            tr!(self, "opt-strength"),
            tr!(self, "opt-hardness"),
            tr!(self, "opt-pressure-size"),
            tr!(self, "opt-pressure-size-tip"),
            tr!(self, "opt-color-stretch"),
            tr!(self, "opt-color-stretch-tip"),
        ];
        let help = self.tr(&format!("tools-help-{}", kind.name()));
        let settings = match kind {
            BlendKind::Blur => &mut self.paint.blend.blur,
            BlendKind::Smudge => &mut self.paint.blend.smudge,
        };
        chrome::slider(
            ui,
            &labels[0],
            &mut settings.size,
            1.0..=MAX_SIZE,
            " px",
            true,
        );
        percent(ui, &labels[1], &mut settings.strength);
        percent(ui, &labels[2], &mut settings.hardness);
        chrome::row(ui, &labels[3], |ui| {
            ui.checkbox(&mut settings.pressure_size, "")
                .on_hover_text(&labels[4]);
        });
        if kind == BlendKind::Smudge {
            chrome::slider(
                ui,
                &labels[5],
                &mut settings.stretch,
                0..=MAX_STRETCH,
                "",
                false,
            )
            .on_hover_text(&labels[6]);
        }
        ui.weak(help);
    }
}

/// A slider for an amount kept from 0 to 1 and shown as a percentage.
fn percent(ui: &mut Ui, label: &str, value: &mut f32) {
    let mut shown = (*value * 100.0).round().clamp(0.0, 100.0) as u8;
    if chrome::slider(ui, label, &mut shown, 0..=100, " %", false).changed() {
        *value = f32::from(shown) / 100.0;
    }
}
