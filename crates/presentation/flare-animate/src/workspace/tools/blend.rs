//! Blur, Smudge, and the Brush and Eraser on a raster layer in the workspace: a drag is handed to
//! the pixels worker a
//! few samples a frame, and each edit that comes back is applied at once, all of them joined into one
//! undo step, so the stroke shows on the stage as it is drawn. The work itself is
//! `flare_tools::blend`; the worker (`flare_runtime::BlendJob`) keeps building the new picture
//! off the window's thread, however large the bitmap.

use std::sync::Arc;

use egui::{Pos2, Ui};
use flare_core::Rgba8;
use flare_input::InputSample;
use flare_runtime::{BlendJob, PixelJob};
use flare_tools::blend::{BlendEdit, BlendKind, BlendSettings, MAX_SIZE, MAX_STRETCH, RasterPaint};
use flare_ui::chrome;

use crate::workspace::*;

/// The pixel tools' settings and the stroke on the worker.
pub(in crate::workspace) struct BlendTools {
    pub(in crate::workspace) blur: BlendSettings,
    pub(in crate::workspace) smudge: BlendSettings,
    /// The Brush on a raster layer.
    pub(in crate::workspace) raster: BlendSettings,
    /// The Eraser on a raster layer.
    pub(in crate::workspace) erase: BlendSettings,
    /// The pictures of the Brush's presets on a raster layer, made once each.
    pub(in crate::workspace) swatches: super::raster_swatch::Swatches,
    live: Option<LiveStroke>,
    /// A drag let go while the stroke before it was still finishing: started when that ends.
    queued: Option<(BlendKind, Vec<InputSample>)>,
}

impl Default for BlendTools {
    fn default() -> Self {
        Self {
            blur: BlendSettings::for_kind(BlendKind::Blur),
            smudge: BlendSettings::for_kind(BlendKind::Smudge),
            raster: BlendSettings::for_kind(BlendKind::RasterBrush),
            erase: BlendSettings::for_kind(BlendKind::Erase),
            swatches: Default::default(),
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

impl Workspace {
    /// Which pixel stroke the tool in hand makes, if any: Blur and Smudge anywhere, and the Brush
    /// and the rubbing Eraser on a raster layer.
    pub(in crate::workspace) fn blend_kind(&self) -> Option<BlendKind> {
        match self.tool {
            Tool::Blur => Some(BlendKind::Blur),
            Tool::Smudge => Some(BlendKind::Smudge),
            Tool::Brush if self.on_raster_layer() => Some(BlendKind::RasterBrush),
            Tool::Eraser if self.on_raster_layer() && !self.eraser_faucet => Some(BlendKind::Erase),
            _ => None,
        }
    }

    fn blend_settings(&self, kind: BlendKind) -> BlendSettings {
        match kind {
            BlendKind::Blur => self.paint.blend.blur,
            BlendKind::Smudge => self.paint.blend.smudge,
            BlendKind::RasterBrush => self.paint.blend.raster,
            BlendKind::Erase => self.paint.blend.erase,
        }
    }

    /// What a stroke of `kind` going wrong is reported as.
    fn blend_message(&self, kind: BlendKind, reason: &str) -> String {
        match kind {
            BlendKind::RasterBrush => tr!(self, "msg-raster-brush", &[("reason", reason)]),
            BlendKind::Erase => tr!(self, "msg-raster-erase", &[("reason", reason)]),
            BlendKind::Blur | BlendKind::Smudge => tr!(self, "msg-blend", &[("reason", reason)]),
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

    /// A click: one dab where it lands (Blur softens a spot, the Raster Brush paints a dot; Smudge
    /// has picked up nothing yet).
    pub(in crate::workspace) fn blend_dot(&mut self, point: Pos2) {
        if let Some(at) = InputSample::mouse(f64::from(point.x), f64::from(point.y)) {
            self.blend_release(vec![at]);
        }
    }

    /// Starts a stroke on the worker at `first`, the first sample of the drag; `false` when
    /// there is no scene to stroke in, or a layer the Raster Brush cannot paint on (which is said
    /// as drawing a shape there would be).
    fn blend_begin(&mut self, kind: BlendKind, first: InputSample) -> bool {
        let Some(scene) = self.scene_id() else {
            return false;
        };
        let start = Pos2::new(first.x as f32, first.y as f32);
        let settings = self.blend_settings(kind);
        let document = self.active().snapshot();
        let id = self.background.next_id();
        let job = if kind == BlendKind::Erase {
            let target = self.drawing_target();
            if let Err(refusal) =
                flare_tools::blend::check_paint(&document, scene, target, self.frame)
            {
                let message = self.drawing_refusal(&refusal);
                self.report(message);
                return false;
            }
            let Some(layer) = self.active_layer_id() else {
                return false;
            };
            BlendJob::BeginErase {
                id,
                document,
                scene,
                frame: self.frame,
                start,
                settings,
                layer,
            }
        } else if kind == BlendKind::RasterBrush {
            let target = self.drawing_target();
            if let Err(refusal) =
                flare_tools::blend::check_paint(&document, scene, target, self.frame)
            {
                let message = self.drawing_refusal(&refusal);
                self.report(message);
                return false;
            }
            let c = self.paint.fill.to_array();
            BlendJob::BeginPaint {
                id,
                document,
                scene,
                frame: self.frame,
                start,
                settings,
                paint: RasterPaint {
                    color: Rgba8::new(c[0], c[1], c[2], c[3]),
                    layer: target,
                    new_layer: tr!(self, "name-new-layer", &[("n", "1")]),
                    bitmap_name: tr!(self, "name-painting"),
                },
            }
        } else {
            BlendJob::Begin {
                id,
                document,
                scene,
                frame: self.frame,
                start,
                kind,
                settings,
            }
        };
        self.paint.blend.live = Some(LiveStroke {
            id,
            kind,
            sent: 0,
            reached: None,
            step: None,
            released: false,
        });
        self.background.submit_pixels(PixelJob::Blend(job));
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
            .apply_joining(edit.commands, live.kind.label(), live.step)
        {
            Ok(revision) => live.step = revision.or(live.step),
            Err(error) => {
                let message = self.blend_message(live.kind, &error.to_string());
                return self.blend_stop(id, message);
            }
        }
        self.status = None;
        if live.kind == BlendKind::RasterBrush {
            // A painting may have made the layer it went on; that layer is the one now chosen.
            self.current_layer = Some(edit.at.layer);
        }
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
        let Some(live) = self.paint.blend.live.filter(|l| l.id == id) else {
            return;
        };
        self.paint.blend.live = None;
        if let Some(reason) = error {
            let message = self.blend_message(live.kind, &reason);
            self.report(message);
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
        if kind == BlendKind::RasterBrush {
            self.raster_presets(ui);
        }
        let strength = if kind == BlendKind::RasterBrush {
            tr!(self, "opt-opacity")
        } else {
            tr!(self, "opt-strength")
        };
        let labels = [
            tr!(self, "opt-size"),
            strength,
            tr!(self, "opt-hardness"),
            tr!(self, "opt-pressure-size"),
            tr!(self, "opt-pressure-size-tip"),
            tr!(self, "opt-color-stretch"),
            tr!(self, "opt-color-stretch-tip"),
            tr!(self, "opt-color-mix"),
            tr!(self, "opt-color-mix-tip"),
        ];
        let help = self.tr(&format!("tools-help-{}", kind.name()));
        let settings = match kind {
            BlendKind::Blur => &mut self.paint.blend.blur,
            BlendKind::Smudge => &mut self.paint.blend.smudge,
            BlendKind::RasterBrush => &mut self.paint.blend.raster,
            BlendKind::Erase => &mut self.paint.blend.erase,
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
        if kind == BlendKind::RasterBrush {
            percent(ui, &labels[7], &mut settings.mix).on_hover_text(&labels[8]);
        }
        if kind.carries_color() {
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
fn percent(ui: &mut Ui, label: &str, value: &mut f32) -> egui::Response {
    let mut shown = (*value * 100.0).round().clamp(0.0, 100.0) as u8;
    let response = chrome::slider(ui, label, &mut shown, 0..=100, " %", false);
    if response.changed() {
        *value = f32::from(shown) / 100.0;
    }
    response
}
