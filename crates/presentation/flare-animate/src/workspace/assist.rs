//! The Ruler and Mirror tools: measuring, the axes that symmetric drawing mirrors across, and
//! copies mirrored across them.

use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use flare_tools::ops;
use flare_tools::rulers::Measurement;
use glamx::DVec2;

use super::*;

/// The color of the symmetry axes on the stage.
const AXIS_INK: Color32 = Color32::from_rgb(255, 120, 200);

impl Workspace {
    /// Where the symmetry axes cross, in stage units: the middle of the stage until set.
    pub(super) fn symmetry_center(&self) -> DVec2 {
        let at = self.paint.symmetry_center.unwrap_or_else(|| {
            let extent = self.stage_extent();
            Pos2::new(extent.x / 2.0, extent.y / 2.0)
        });
        DVec2::new(f64::from(at.x), f64::from(at.y))
    }

    /// What the Ruler tool last measured.
    pub fn measurement(&self) -> Option<Measurement> {
        let (from, to) = self.display.measure?;
        Measurement::between((from.x, from.y), (to.x, to.y))
    }

    /// The Ruler tool is dragged from `from` to `to`; the Mirror tool puts the axes at `to`.
    pub(super) fn assist_drag(&mut self, from: Pos2, to: Pos2) {
        match self.tool {
            Tool::Ruler => self.display.measure = Some((from, to)),
            Tool::Mirror if to.x.is_finite() && to.y.is_finite() => {
                self.paint.symmetry_center = Some(to);
            }
            _ => {}
        }
    }

    /// Copies the selection mirrored across the axis, onto new layers, as one undo step, and
    /// selects the copies. `across_vertical` mirrors left to right, across a vertical axis.
    pub fn mirror_copy_selection(&mut self, across_vertical: bool) {
        let Some(scene) = self.scene_id() else { return };
        let reflection = flare_tools::symmetry::Reflection {
            flip_x: across_vertical,
            flip_y: !across_vertical,
        };
        let matrix = reflection.matrix(self.symmetry_center());
        let mut commands = Vec::new();
        let mut copies = Vec::new();
        for span in self.selection_all() {
            if let Some((batch, copy)) =
                ops::duplicate_through(self.document(), scene, span, matrix, "mirror")
            {
                commands.extend(batch);
                copies.push(copy);
            }
        }
        if commands.is_empty() {
            self.report("Select something to mirror");
            return;
        }
        self.run_batch(commands, "mirror copy");
        if self.status.is_none() {
            self.selected = copies.first().copied();
            self.also_selected = copies.iter().skip(1).copied().collect();
            self.current_layer = self.selected.map(|s| s.layer);
        }
    }

    /// The stroke being drawn as it will be committed: straightened if that is asked for, with
    /// its mirror images. Each is a list of stage points.
    pub(super) fn freehand_preview(&self) -> Vec<Vec<Pos2>> {
        let mut samples = self.freehand.clone();
        if self.paint.straight_edge || self.paint.straight_held {
            samples = flare_tools::rulers::straighten(&samples);
        }
        let about = self.symmetry_center();
        let mut strokes = vec![samples.clone()];
        strokes.extend(
            self.paint
                .symmetry
                .reflections()
                .iter()
                .map(|r| r.samples(&samples, about)),
        );
        strokes
            .iter()
            .map(|s| {
                s.iter()
                    .map(|p| Pos2::new(p.x as f32, p.y as f32))
                    .collect()
            })
            .collect()
    }

    /// Draws the measurement and the symmetry axes over the stage.
    pub(super) fn assist_overlay(&self, painter: &Painter, panel: Rect, image: Rect, scale: f32) {
        let to_screen = |p: Pos2| image.min + p.to_vec2() * scale;
        let drawing = matches!(self.tool, Tool::Pencil | Tool::Brush | Tool::Mirror);
        let (vertical, horizontal) = self.paint.symmetry.axes();
        if drawing && (vertical || horizontal) {
            let center = self.symmetry_center();
            let at = to_screen(Pos2::new(center.x as f32, center.y as f32));
            let stroke = Stroke::new(1.0, AXIS_INK);
            if vertical {
                painter.vline(at.x, panel.y_range(), stroke);
            }
            if horizontal {
                painter.hline(panel.x_range(), at.y, stroke);
            }
        }
        if self.tool != Tool::Ruler {
            return;
        }
        let (Some((from, to)), Some(measured)) = (self.display.measure, self.measurement()) else {
            return;
        };
        let (a, b) = (to_screen(from), to_screen(to));
        let ink = Stroke::new(1.5, flare_ui::theme::ACCENT);
        painter.line_segment([a, b], ink);
        for end in [a, b] {
            painter.circle_filled(end, 3.0, flare_ui::theme::ACCENT);
        }
        let label = measured.label();
        let font = FontId::proportional(12.0);
        let galley = painter.layout_no_wrap(label, font, Color32::WHITE);
        let at = b + Vec2::new(10.0, -10.0 - galley.size().y);
        let pad = Vec2::splat(4.0);
        painter.rect_filled(
            Rect::from_min_size(at - pad, galley.size() + pad * 2.0),
            3.0,
            Color32::from_black_alpha(190),
        );
        painter.galley(at, galley, Color32::WHITE);
    }
}
