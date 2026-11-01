//! The stage's own furniture: its settings popup, grid, rulers and the timeline's zoom.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use flare_core::Rgba8;
use flare_document::MAX_STAGE_SIDE;
use flare_editor::timeline_view;

use super::*;

impl Workspace {
    /// Stage size and background. Size edits apply on release or Enter so that scrubbing a
    /// value is a single undo step.
    pub(super) fn stage_settings(&mut self, ui: &mut egui::Ui) {
        let stage = self.document().stage();
        let (mut width, mut height) = self.stage_edit.unwrap_or((stage.width(), stage.height()));
        let wr = ui.add(
            egui::DragValue::new(&mut width)
                .range(1..=MAX_STAGE_SIDE)
                .prefix("W "),
        );
        let hr = ui.add(
            egui::DragValue::new(&mut height)
                .range(1..=MAX_STAGE_SIDE)
                .prefix("H "),
        );
        if wr.changed() || hr.changed() {
            self.stage_edit = Some((width, height));
        }
        let done = wr.drag_stopped() || wr.lost_focus() || hr.drag_stopped() || hr.lost_focus();
        if done
            && let Some((w, h)) = self.stage_edit.take()
            && (w, h) != (stage.width(), stage.height())
        {
            self.set_stage_size(w, h);
        }
        let bg = stage.background().to_array();
        let mut color = Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]);
        if ui.color_edit_button_srgba(&mut color).changed() {
            let c = color.to_srgba_unmultiplied();
            self.set_stage_background(Rgba8::new(c[0], c[1], c[2], c[3]));
        }
    }

    /// Rulers along the top and left edge of the stage view, in stage units.
    /// Grid lines over the stage, skipped when they would be too close to see.
    pub(super) fn stage_grid(
        &self,
        painter: &egui::Painter,
        panel: Rect,
        image: Rect,
        extent: Vec2,
        scale: f32,
    ) {
        let step = self.display.grid_size.max(1.0);
        if step * scale < 6.0 {
            return;
        }
        let ink = Stroke::new(1.0, Color32::from_white_alpha(22));
        let clip = image.intersect(panel);
        let mut x = 0.0;
        while x <= extent.x {
            let sx = image.min.x + x * scale;
            if sx >= clip.min.x && sx <= clip.max.x {
                painter.vline(sx, clip.y_range(), ink);
            }
            x += step;
        }
        let mut y = 0.0;
        while y <= extent.y {
            let sy = image.min.y + y * scale;
            if sy >= clip.min.y && sy <= clip.max.y {
                painter.hline(clip.x_range(), sy, ink);
            }
            y += step;
        }
    }

    pub(super) fn stage_rulers(
        &self,
        painter: &egui::Painter,
        panel: Rect,
        image: Rect,
        scale: f32,
    ) {
        const THICKNESS: f32 = 16.0;
        let top = Rect::from_min_max(panel.min, Pos2::new(panel.max.x, panel.min.y + THICKNESS));
        let left = Rect::from_min_max(panel.min, Pos2::new(panel.min.x + THICKNESS, panel.max.y));
        let fill = flare_ui::theme::panel_raised().gamma_multiply(0.92);
        painter.rect_filled(top, 0.0, fill);
        painter.rect_filled(left, 0.0, fill);
        let tick = Stroke::new(1.0, flare_ui::theme::text_disabled());
        let font = FontId::monospace(9.0);
        for t in flare_ui::view::ruler_ticks(
            image.min.x,
            scale,
            panel.min.x + THICKNESS,
            panel.max.x,
            56.0,
        ) {
            let height = if t.major { THICKNESS - 4.0 } else { 4.0 };
            painter.vline(t.screen, (top.bottom() - height)..=top.bottom(), tick);
            if t.major {
                painter.text(
                    Pos2::new(t.screen + 2.0, top.top() + 1.0),
                    Align2::LEFT_TOP,
                    format!("{}", t.value.round()),
                    font.clone(),
                    flare_ui::theme::text_dim(),
                );
            }
        }
        for t in flare_ui::view::ruler_ticks(
            image.min.y,
            scale,
            panel.min.y + THICKNESS,
            panel.max.y,
            56.0,
        ) {
            let width = if t.major { THICKNESS - 4.0 } else { 4.0 };
            painter.hline((left.right() - width)..=left.right(), t.screen, tick);
            if t.major {
                painter.text(
                    Pos2::new(left.left() + 1.0, t.screen + 2.0),
                    Align2::LEFT_TOP,
                    format!("{}", t.value.round()),
                    font.clone(),
                    flare_ui::theme::text_dim(),
                );
            }
        }
        painter.rect_filled(
            Rect::from_min_size(panel.min, Vec2::splat(THICKNESS)),
            0.0,
            flare_ui::theme::panel_raised(),
        );
    }

    pub(super) fn zoom_timeline(&mut self, factor: f32) {
        self.timeline_zoom = timeline_view::zoomed(self.timeline_zoom, factor);
    }
}
