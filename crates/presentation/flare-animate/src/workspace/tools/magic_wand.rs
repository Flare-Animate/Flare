//! What the workspace does for the Magic Wand: a click hands the picking to the pixels worker,
//! what comes back is drawn striped over the bitmap, and Delete clears it as one undo step. The
//! work itself is `flare_tools::magic_wand`; the worker keeps a large bitmap from holding up
//! the window.

use std::sync::Arc;

use egui::{Color32, Pos2, Ui};
use flare_runtime::{PixelJob, WandPicked};
use flare_tools::magic_wand::{Combine, MAX_THRESHOLD, Smoothing, WandSettings};
use flare_ui::chrome;

use crate::workspace::*;
use flare_document::Content;

/// The wand's settings and what it has picked.
#[derive(Default)]
pub(in crate::workspace) struct WandTool {
    pub(in crate::workspace) settings: WandSettings,
    /// What is picked, with the document it was picked in.
    pub(in crate::workspace) picked: Option<WandPicked>,
    /// The pick or clear handed to the worker and not back yet. Clicks wait for it, so a quick
    /// second Shift-click builds on the first.
    pub(in crate::workspace) pending: Option<u64>,
    /// The striped picture of `picked`, made once per pick.
    texture: Option<egui::TextureHandle>,
}

#[cfg(test)]
impl WandTool {
    pub(in crate::workspace) fn texture_made(&self) -> bool {
        self.texture.is_some()
    }
}

impl Workspace {
    /// A click of the wand at `point`: Shift adds to what is picked, Alt takes away.
    pub(in crate::workspace) fn wand_click(&mut self, point: Pos2, shift: bool, alt: bool) {
        if self.wand.pending.is_some() {
            return;
        }
        let Some(scene) = self.scene_id() else {
            return;
        };
        let combine = if alt {
            Combine::Subtract
        } else if shift {
            Combine::Add
        } else {
            Combine::Replace
        };
        let id = self.background.next_id();
        self.wand.pending = Some(id);
        self.background.submit_pixels(PixelJob::WandPick {
            id,
            document: self.active().snapshot(),
            scene,
            frame: self.frame,
            point,
            settings: self.wand.settings,
            combine,
            current: self.wand.picked.as_ref().map(|p| p.selection.clone()),
        });
        self.settle_inline();
    }

    pub(in crate::workspace) fn wand_has_pixels(&self) -> bool {
        self.wand.picked.is_some()
    }

    /// Clears the picked pixels; `false` when nothing is picked.
    pub(in crate::workspace) fn wand_clear(&mut self) -> bool {
        let Some(picked) = &self.wand.picked else {
            return false;
        };
        let Some(scene) = self.scene_id() else {
            return false;
        };
        if self.wand.pending.is_some() {
            return true;
        }
        let id = self.background.next_id();
        self.wand.pending = Some(id);
        self.background.submit_pixels(PixelJob::WandClear {
            id,
            document: self.active().snapshot(),
            scene,
            selection: picked.selection.clone(),
            smoothing: self.wand.settings.smoothing,
        });
        self.settle_inline();
        true
    }

    /// Picks every pixel of the bitmap that was not picked, and none that was.
    pub(in crate::workspace) fn wand_invert(&mut self) {
        if self.wand.pending.is_some() {
            return;
        }
        if let Some(picked) = &mut self.wand.picked {
            picked.selection.mask.invert();
            picked.preview.invert();
            self.wand.texture = None;
            if picked.selection.mask.is_empty() {
                self.wand_drop();
            }
        }
    }

    pub(in crate::workspace) fn wand_drop(&mut self) {
        self.wand.picked = None;
        self.wand.texture = None;
    }

    pub(in crate::workspace) fn wand_picked_arrived(
        &mut self,
        id: u64,
        result: Result<Option<WandPicked>, String>,
    ) {
        if self.wand.pending.take_if(|p| *p == id).is_none() {
            return;
        }
        match result {
            Ok(picked) => {
                self.wand.picked = picked;
                self.wand.texture = None;
            }
            Err(message) => self.report(tr!(self, "msg-magic-wand", &[("reason", &message)])),
        }
    }

    pub(in crate::workspace) fn wand_cleared_arrived(
        &mut self,
        id: u64,
        document: Arc<flare_document::Document>,
        result: Result<Vec<flare_commands::Command>, String>,
    ) {
        if self.wand.pending.take_if(|p| *p == id).is_none() {
            return;
        }
        // The commands were made for that document; an edit since would make them wrong.
        if !Arc::ptr_eq(&document, &self.active().snapshot()) {
            self.report(tr!(self, "msg-magic-wand-changed"));
            return;
        }
        match result {
            Ok(commands) => {
                self.run_batch(commands, "magic wand");
                self.wand_drop();
            }
            Err(message) => self.report(tr!(self, "msg-magic-wand", &[("reason", &message)])),
        }
    }

    /// Lets go of what is picked when the bitmap it was picked in is not on the stage as it was
    /// (an undo, a delete, another frame's keyframe, a symbol entered or left).
    pub(in crate::workspace) fn wand_check(&mut self) {
        let Some(picked) = &self.wand.picked else {
            return;
        };
        let still = self.scene_id().is_some_and(|scene| {
            let at = picked.selection.at;
            let on_frame = flare_tools::ops::find_span(self.document(), scene, at)
                .is_some_and(|s| s.start() <= self.frame && u64::from(self.frame) < s.end());
            on_frame
                && matches!(
                    flare_tools::ops::find_element(self.document(), scene, at),
                    Some(Content::Bitmap(b)) if b.bitmap() == picked.selection.bitmap
                )
        });
        if !still {
            self.wand_drop();
        }
    }

    /// The picked pixels, striped in the accent color over the bitmap where it is drawn.
    pub(in crate::workspace) fn draw_wand(
        &mut self,
        painter: &egui::Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
    ) {
        self.wand_check();
        let (Some(picked), Some(scene)) = (&self.wand.picked, self.scene_id()) else {
            return;
        };
        let Some(Content::Bitmap(placed)) =
            flare_tools::ops::find_element(self.document(), scene, picked.selection.at)
        else {
            return;
        };
        let (w, h) = (
            f64::from(picked.selection.mask.width()),
            f64::from(picked.selection.mask.height()),
        );
        let matrix = placed.transform().as_affine();
        let corner = |x: f64, y: f64| {
            let p = matrix.transform_point2(glamx::DVec2::new(x, y));
            to_screen(Pos2::new(p.x as f32, p.y as f32))
        };
        let points = [
            corner(0.0, 0.0),
            corner(w, 0.0),
            corner(w, h),
            corner(0.0, h),
        ];
        let image = self
            .wand
            .texture
            .is_none()
            .then(|| striped(&picked.preview));
        if let Some(image) = image {
            let options = egui::TextureOptions::NEAREST;
            let handle = painter
                .ctx()
                .load_texture("magic-wand-picked", image, options);
            self.wand.texture = Some(handle);
        }
        let Some(texture) = self.wand.texture.as_ref().map(|t| t.id()) else {
            return;
        };
        let mut mesh = egui::Mesh::with_texture(texture);
        let uv = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        for (pos, (u, v)) in points.into_iter().zip(uv) {
            mesh.vertices.push(egui::epaint::Vertex {
                pos,
                uv: Pos2::new(u, v),
                color: Color32::WHITE,
            });
        }
        mesh.indices.extend([0, 1, 2, 0, 2, 3]);
        painter.add(egui::Shape::mesh(mesh));
    }

    pub(in crate::workspace) fn wand_options(&mut self, ui: &mut Ui) {
        let label = tr!(self, "opt-threshold");
        chrome::slider(
            ui,
            &label,
            &mut self.wand.settings.threshold,
            0..=MAX_THRESHOLD,
            "",
            false,
        );
        let (contiguous, tip) = (tr!(self, "opt-contiguous"), tr!(self, "opt-contiguous-tip"));
        chrome::row(ui, &contiguous, |ui| {
            ui.checkbox(&mut self.wand.settings.contiguous, "")
                .on_hover_text(&tip);
        });
        let names: Vec<String> = Smoothing::ALL
            .iter()
            .map(|s| self.tr(&format!("opt-smoothing-{}", s.name())))
            .collect();
        let current = Smoothing::ALL
            .iter()
            .position(|s| *s == self.wand.settings.smoothing)
            .unwrap_or(2);
        chrome::row(ui, &tr!(self, "opt-smoothing"), |ui| {
            egui::ComboBox::from_id_salt("magic_wand_smoothing")
                .selected_text(&names[current])
                .show_ui(ui, |ui| {
                    for (smoothing, name) in Smoothing::ALL.into_iter().zip(&names) {
                        ui.selectable_value(&mut self.wand.settings.smoothing, smoothing, name);
                    }
                });
        });
        let count = self
            .wand
            .picked
            .as_ref()
            .map_or(0, |p| p.selection.mask.count());
        if count > 0 {
            chrome::row(ui, &tr!(self, "opt-picked"), |ui| {
                ui.label(self.tr_count("opt-picked-pixels", count as u64));
            });
        }
        let ready = count > 0 && self.wand.pending.is_none();
        let (invert, clear) = (tr!(self, "opt-wand-invert"), tr!(self, "opt-wand-clear"));
        ui.horizontal(|ui| {
            if ui.add_enabled(ready, egui::Button::new(invert)).clicked() {
                self.wand_invert();
            }
            if ui.add_enabled(ready, egui::Button::new(clear)).clicked() {
                self.wand_clear();
            }
        });
        ui.weak(tr!(self, "tools-help-magic-wand"));
    }
}

/// The picked pixels as an image: diagonal stripes of the accent color, clear elsewhere.
fn striped(mask: &flare_tools::magic_wand::Mask) -> egui::ColorImage {
    let accent = flare_ui::theme::accent();
    let (strong, faint) = (
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 170),
        Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 70),
    );
    let w = mask.width() as usize;
    let pixels = mask
        .pixels()
        .iter()
        .enumerate()
        .map(|(i, picked)| match (*picked, (i % w + i / w) % 6 < 3) {
            (false, _) => Color32::TRANSPARENT,
            (true, true) => strong,
            (true, false) => faint,
        })
        .collect();
    egui::ColorImage::new([w, mask.height() as usize], pixels)
}
