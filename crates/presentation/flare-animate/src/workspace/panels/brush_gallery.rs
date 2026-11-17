//! The Brush list as a gallery: every brush, built-in and custom, is shown drawing the same pen
//! stroke, so a brush is chosen by the mark it makes rather than by its name. The fine line
//! brushes come first, then the textured media, then the brushes a person has kept. The brush in
//! hand is drawn under the list too, and redraws as its nib, tapers and texture are edited.
//!
//! Each swatch is the brush library's own outline of the sample stroke, drawn with the same
//! anti-aliased mesh as the live stroke preview, so it is exactly what the brush paints.

use egui::epaint::Mesh;
use egui::{Align2, Color32, FontId, Pos2, Rect, Sense, Ui, Vec2};
use flare_tools::brush::{BrushStroke, BuiltinBrushProfile};
use flare_ui::chrome;

use super::tool_options_labels::builtin_profile_key;
use crate::workspace::Workspace;
use crate::workspace::tools::stroke_preview::{PreviewSides, PreviewStroke, mesh};

/// The size of one gallery entry, in points.
const ENTRY: Vec2 = Vec2::new(200.0, 46.0);
/// The size of the swatch of the brush in hand, in points.
const CURRENT: Vec2 = Vec2::new(200.0, 34.0);
/// Space round a swatch's stroke inside its box, in points.
const PAD: f32 = 8.0;

/// A brush chosen in the gallery.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::workspace) enum BrushPick {
    Builtin(BuiltinBrushProfile),
    Custom(usize),
}

impl Workspace {
    /// The Brush row: the gallery, and New and Delete for custom brushes.
    pub(super) fn brush_picker(&mut self, ui: &mut Ui) {
        let (new, delete) = (tr!(self, "opt-new"), tr!(self, "opt-delete"));
        let (new_tip, delete_tip) = (
            tr!(self, "opt-new-brush-tip"),
            tr!(self, "opt-delete-brush-tip"),
        );
        let chosen = self.brush_shelf.selected();
        chrome::row(ui, &tr!(self, "opt-brush"), |ui| {
            let mut pick = None;
            // As wide as the gallery's entries where the panel has room, and no wider than it.
            egui::ComboBox::from_id_salt("brush-list")
                .selected_text(self.brush_in_hand_name())
                .width(ui.available_width().min(ENTRY.x))
                .truncate()
                .height(420.0)
                .show_ui(ui, |ui| pick = self.brush_gallery(ui));
            if let Some(pick) = pick {
                self.pick_brush(pick);
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui.button(&new).on_hover_text(&new_tip).clicked() {
                self.new_custom_brush();
            }
            if ui
                .add_enabled(chosen.is_some(), egui::Button::new(&delete))
                .on_hover_text(&delete_tip)
                .clicked()
            {
                self.brush_shelf.delete_selected();
            }
        });
        // The brush in hand, as it draws now, narrower in a narrow panel.
        let size = Vec2::new(ui.available_width().clamp(1.0, CURRENT.x), CURRENT.y);
        let stroke = self.brush_shelf.stroke(swatch_width(size));
        let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
        let color = ui.visuals().text_color();
        let pixel = 1.0 / ui.ctx().pixels_per_point();
        ui.painter()
            .add(egui::Shape::mesh(swatch_mesh(&stroke, rect, color, pixel)));
    }

    /// The name of the brush in hand: a custom brush's own, a built-in's, or Custom for an edited
    /// one.
    fn brush_in_hand_name(&self) -> String {
        match self.brush_shelf.selected() {
            Some(i) => self.brush_shelf.presets()[i].name.clone(),
            None => self.brush_shelf.builtin().map_or_else(
                || tr!(self, "opt-profile-custom"),
                |profile| self.tr(builtin_profile_key(profile)),
            ),
        }
    }

    /// Every brush, each drawing its swatch, in three groups. Returns the one clicked.
    fn brush_gallery(&self, ui: &mut Ui) -> Option<BrushPick> {
        let chosen = self.brush_shelf.selected();
        let held = self.brush_shelf.builtin().filter(|_| chosen.is_none());
        let mut pick = None;
        for (heading, textured) in [
            ("opt-brush-group-lines", false),
            ("opt-brush-group-textured", true),
        ] {
            ui.weak(self.tr(heading));
            for profile in BuiltinBrushProfile::ALL
                .into_iter()
                .filter(|p| p.is_textured() == textured)
            {
                let stroke = profile.preset().stroke(swatch_width(ENTRY));
                let name = self.tr(builtin_profile_key(profile));
                if entry(ui, &stroke, &name, held == Some(profile)) {
                    pick = Some(BrushPick::Builtin(profile));
                }
            }
        }
        if !self.brush_shelf.presets().is_empty() {
            ui.weak(tr!(self, "opt-brush-group-custom"));
        }
        for (i, brush) in self.brush_shelf.presets().iter().enumerate() {
            let stroke = brush.stroke(swatch_width(ENTRY));
            if entry(ui, &stroke, &brush.name, chosen == Some(i)) {
                pick = Some(BrushPick::Custom(i));
            }
        }
        pick
    }

    /// Takes `pick` in hand, with the pressure controls reading from it.
    pub(in crate::workspace) fn pick_brush(&mut self, pick: BrushPick) {
        match pick {
            BrushPick::Builtin(profile) => self.brush_shelf.choose_builtin(profile),
            BrushPick::Custom(i) => self.brush_shelf.choose(Some(i)),
        }
        self.brush_light = self.brush_shelf.pressure_min();
        self.brush_pressure_gamma = self.brush_shelf.pressure_gamma();
    }
}

/// One gallery entry: the brush's name over its swatch, highlighted when it is the brush in hand.
/// Whether it was clicked.
fn entry(ui: &mut Ui, stroke: &BrushStroke, name: &str, selected: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(ENTRY, Sense::click());
    let visuals = ui.style().interact_selectable(&response, selected);
    if selected || response.hovered() {
        ui.painter()
            .rect_filled(rect, visuals.corner_radius, visuals.weak_bg_fill);
    }
    let text = visuals.text_color();
    ui.painter().text(
        rect.left_top() + Vec2::new(PAD, 3.0),
        Align2::LEFT_TOP,
        name,
        FontId::proportional(11.0),
        text,
    );
    let swatch = Rect::from_min_max(rect.left_top() + Vec2::new(0.0, 14.0), rect.right_bottom());
    let pixel = 1.0 / ui.ctx().pixels_per_point();
    ui.painter()
        .add(egui::Shape::mesh(swatch_mesh(stroke, swatch, text, pixel)));
    response.on_hover_text(name).clicked()
}

/// How wide a swatch's stroke is at full pressure in a box of `size`: about half the box's
/// height, so the thick and thin of every brush show and none runs off its box.
fn swatch_width(size: Vec2) -> f64 {
    f64::from((size.y - 14.0).max(8.0) * 0.5)
}

/// The sample stroke `stroke` draws, laid across `rect` (inside its padding) in `color`.
pub(in crate::workspace) fn swatch_mesh(
    stroke: &BrushStroke,
    rect: Rect,
    color: Color32,
    pixel: f32,
) -> Mesh {
    let inner = rect.shrink(PAD);
    if !(inner.width() > 0.0 && inner.height() > 0.0) {
        return Mesh::default();
    }
    let pieces: Vec<PreviewStroke> =
        flare_tools::brush::swatch(stroke, f64::from(inner.width()), f64::from(inner.height()))
            .iter()
            .map(|piece| PreviewStroke {
                points: Vec::new(),
                halves: Vec::new(),
                color,
                sides: Some(PreviewSides::of(piece)),
            })
            .collect();
    let origin = inner.left_top();
    mesh(&pieces, |p: Pos2| origin + p.to_vec2(), pixel)
}

#[cfg(test)]
#[path = "brush_gallery_tests.rs"]
mod tests;
