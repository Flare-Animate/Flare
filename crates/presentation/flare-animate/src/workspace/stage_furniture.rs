//! The stage's own furniture: its settings popup, grid, rulers and the timeline's zoom.

use egui::{Align2, Color32, FontId, Pos2, Rect, Stroke, Vec2};
use flare_core::Rgba8;
use flare_document::MAX_STAGE_SIDE;
use flare_editor::timeline_view;

use super::*;

/// The grid's spacing to start with, across and down, in stage units.
pub(super) const DEFAULT_GRID: f32 = 20.0;
/// The closest and widest spacing the grid takes, in stage units.
pub(super) const MIN_GRID: f32 = 1.0;
pub(super) const MAX_GRID: f32 = 500.0;

/// One spacing brought into range; one that is not a number is the default.
pub(super) fn grid_step(step: f32) -> f32 {
    if step.is_finite() {
        step.clamp(MIN_GRID, MAX_GRID)
    } else {
        DEFAULT_GRID
    }
}

/// The grid's lines on a stage of `background` when no color is chosen: grey, as Animate's are,
/// dark on a light stage and light on a dark one, so they show on any stage (white lines were
/// lost on the white stage most documents have).
pub(super) fn automatic_grid_ink(background: Rgba8) -> Color32 {
    let [r, g, b, _] = background.to_array();
    let light = 0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b) > 128.0;
    if light {
        Color32::from_black_alpha(96)
    } else {
        Color32::from_white_alpha(96)
    }
}

impl Workspace {
    /// The grid's spacing across and down, in stage units.
    pub fn grid_spacing(&self) -> Vec2 {
        self.display.grid_size
    }

    /// Sets the grid's spacing across and down, each kept from 1 to 500 units.
    pub fn set_grid_spacing(&mut self, across: f32, down: f32) {
        self.display.grid_size = Vec2::new(grid_step(across), grid_step(down));
    }

    /// The grid's chosen color, or `None` when it follows the stage's background.
    pub fn grid_color(&self) -> Option<Color32> {
        self.display.grid_color
    }

    pub fn set_grid_color(&mut self, color: Option<Color32>) {
        self.display.grid_color = color;
    }

    /// The color the grid's lines are drawn in.
    pub(super) fn grid_ink(&self) -> Color32 {
        self.display
            .grid_color
            .unwrap_or_else(|| automatic_grid_ink(self.document().stage().background()))
    }

    /// Stage size and background. Size edits apply on release or Enter so that scrubbing a
    /// value is a single undo step.
    pub(super) fn stage_settings(&mut self, ui: &mut egui::Ui) {
        let stage = self.document().stage();
        let (mut width, mut height) = self.stage_edit.unwrap_or((stage.width(), stage.height()));
        let (w_prefix, h_prefix) = (
            format!("{} ", tr!(self, "doc-width")),
            format!("{} ", tr!(self, "doc-height")),
        );
        let wr = ui.add(
            egui::DragValue::new(&mut width)
                .range(1..=MAX_STAGE_SIDE)
                .prefix(w_prefix),
        );
        let hr = ui.add(
            egui::DragValue::new(&mut height)
                .range(1..=MAX_STAGE_SIDE)
                .prefix(h_prefix),
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
        let texts = self.color_texts();
        if flare_ui::color_picker::color_button(ui, &mut color, &texts).changed() {
            let c = color.to_srgba_unmultiplied();
            self.set_stage_background(Rgba8::new(c[0], c[1], c[2], c[3]));
        }
    }

    /// Motion guides are never drawn into the animation, but their paths show on the stage so
    /// the path a layer follows can be seen and edited.
    pub(super) fn stage_motion_guides(
        &self,
        painter: &egui::Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
    ) {
        if let Some(scene) = self.document().scenes().first() {
            let ink = Stroke::new(1.5, Color32::from_rgb(230, 80, 200));
            for layer in scene
                .timeline()
                .layers()
                .iter()
                .filter(|l| l.is_guide() && l.is_visible())
            {
                if let Some(span) = layer.span_at(self.frame)
                    && let flare_document::Content::Shape(shape) = span.content()
                {
                    for line in flare_eval::flatten(shape.path()) {
                        let points: Vec<Pos2> = line
                            .iter()
                            .map(|p| to_screen(Pos2::new(p.x as f32, p.y as f32)))
                            .collect();
                        painter.add(egui::Shape::line(points, ink));
                    }
                }
            }
        }
    }

    /// Grid lines over the stage, skipped when they would be too close to see.
    pub(super) fn stage_grid(
        &self,
        painter: &egui::Painter,
        panel: Rect,
        image: Rect,
        extent: Vec2,
        scale: f32,
    ) {
        // Lines too close together to see are not drawn: those across and those down apart.
        let step = self.display.grid_size.max(Vec2::splat(MIN_GRID));
        let ink = Stroke::new(1.0, self.grid_ink());
        let clip = image.intersect(panel);
        let mut x = 0.0;
        while step.x * scale >= 6.0 && x <= extent.x {
            let sx = image.min.x + x * scale;
            if sx >= clip.min.x && sx <= clip.max.x {
                painter.vline(sx, clip.y_range(), ink);
            }
            x += step.x;
        }
        let mut y = 0.0;
        while step.y * scale >= 6.0 && y <= extent.y {
            let sy = image.min.y + y * scale;
            if sy >= clip.min.y && sy <= clip.max.y {
                painter.hline(clip.x_range(), sy, ink);
            }
            y += step.y;
        }
    }

    /// Rulers along the top and left edge of the stage view, in stage units.
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
        let corner = Rect::from_min_size(panel.min, Vec2::splat(THICKNESS));
        // Turned, the stage's axes no longer run along the rulers, so numbers would mislead.
        if self.stage_view.is_turned() {
            painter.rect_filled(corner, 0.0, flare_ui::theme::panel_raised());
            return;
        }
        let tick = Stroke::new(1.0, flare_ui::theme::text_disabled());
        let font = FontId::monospace(9.0);
        // Each ruler's numbers stay inside its own strip, never on the stage or past the panel.
        let top_painter = painter.with_clip_rect(top.intersect(painter.clip_rect()));
        let left_painter = painter.with_clip_rect(left.intersect(painter.clip_rect()));
        for t in flare_ui::view::ruler_ticks(
            image.min.x,
            scale,
            panel.min.x + THICKNESS,
            panel.max.x,
            56.0,
        ) {
            let height = if t.major { THICKNESS - 4.0 } else { 4.0 };
            top_painter.vline(t.screen, (top.bottom() - height)..=top.bottom(), tick);
            if t.major {
                top_painter.text(
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
            left_painter.hline((left.right() - width)..=left.right(), t.screen, tick);
            if t.major {
                // Written down the strip, a quarter turn clockwise: "1000" is wider than the ruler.
                let ink = flare_ui::theme::text_dim();
                let galley =
                    left_painter.layout_no_wrap(format!("{}", t.value.round()), font.clone(), ink);
                left_painter.add(
                    egui::epaint::TextShape::new(
                        Pos2::new(left.right() - 1.0, t.screen + 2.0),
                        galley,
                        ink,
                    )
                    .with_angle(std::f32::consts::FRAC_PI_2),
                );
            }
        }
        painter.rect_filled(corner, 0.0, flare_ui::theme::panel_raised());
    }

    /// How wide one frame is drawn in the timeline now, in points.
    pub fn timeline_frame_width(&self) -> f32 {
        timeline_view::frame_width(self.timeline_zoom)
    }

    pub(super) fn zoom_timeline(&mut self, factor: f32) {
        self.timeline_zoom = timeline_view::zoomed(self.timeline_zoom, factor);
    }
}
