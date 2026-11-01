//! The Ruler and Mirror tools: measuring, the axes that symmetric drawing mirrors across, and
//! copies mirrored across them.

use egui::{Color32, FontId, Painter, Pos2, Rect, Stroke, Vec2};
use flare_tools::ops;
use flare_tools::ruler::Measurement;
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
        let reflection = flare_tools::mirror::Reflection {
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

    /// The outline of the brush at the pointer, in stage units: the nib at the size it will paint
    /// at, so the cursor shows what a click would leave. `None` unless the Brush is in hand over
    /// the stage.
    pub(super) fn brush_cursor(&self) -> Option<Vec<Pos2>> {
        if self.tool != Tool::Brush {
            return None;
        }
        let at = self.pointer_stage?;
        let half = f64::from(self.brush_size) / 2.0;
        Some(
            self.brush_shelf
                .nib()
                .outline_points(half, 48)
                .into_iter()
                .map(|p| Pos2::new(at.x + p.x as f32, at.y + p.y as f32))
                .collect(),
        )
    }

    /// Draws the stroke that was just committed, until the stage picture includes it.
    pub(super) fn draw_settling(
        &self,
        painter: &Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
        scale: f32,
    ) {
        if let Some(settling) = &self.paint.settling {
            for shape in &settling.shapes {
                match shape {
                    stroke_preview::SettleShape::Rect {
                        rect,
                        radius,
                        fill,
                        outline,
                    } => {
                        let screen = Rect::from_two_pos(to_screen(rect.min), to_screen(rect.max));
                        painter.rect_filled(screen, *radius * scale, *fill);
                        if let Some((width, color)) = outline {
                            painter.rect_stroke(
                                screen,
                                *radius * scale,
                                Stroke::new(*width * scale, *color),
                                egui::StrokeKind::Middle,
                            );
                        }
                    }
                    stroke_preview::SettleShape::Ellipse {
                        center,
                        radius,
                        fill,
                        outline,
                    } => {
                        let stroke = outline.map_or(Stroke::NONE, |(width, color)| {
                            Stroke::new(width * scale, color)
                        });
                        painter.add(egui::epaint::EllipseShape {
                            center: to_screen(*center),
                            radius: *radius * scale,
                            angle: 0.0,
                            fill: *fill,
                            stroke,
                        });
                    }
                }
            }
            painter.add(egui::Shape::mesh(stroke_preview::mesh(
                &settling.strokes,
                to_screen,
                scale,
            )));
        }
    }

    /// Draws the measurement and the symmetry axes over the stage.
    pub(super) fn assist_overlay(
        &mut self,
        painter: &Painter,
        panel: Rect,
        image: Rect,
        scale: f32,
    ) {
        let to_screen = |p: Pos2| image.min + p.to_vec2() * scale;
        self.retire_settled();
        self.draw_settling(painter, to_screen, scale);
        if let Some(outline) = self.brush_cursor() {
            let points: Vec<Pos2> = outline.into_iter().map(to_screen).collect();
            // A dark line under a light one reads on any color, and neither covers the artwork.
            painter.add(egui::Shape::closed_line(
                points.clone(),
                Stroke::new(2.0, Color32::from_black_alpha(160)),
            ));
            painter.add(egui::Shape::closed_line(
                points,
                Stroke::new(1.0, Color32::WHITE),
            ));
        }
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
        let ink = Stroke::new(1.5, flare_ui::theme::accent());
        painter.line_segment([a, b], ink);
        for end in [a, b] {
            painter.circle_filled(end, 3.0, flare_ui::theme::accent());
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

    /// The rubber band of a rectangle, oval or line being dragged out, until the pointer is let go
    /// (what it becomes is held by [`Self::draw_settling`]).
    pub(super) fn draw_drag_preview(
        &self,
        painter: &Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
        scale: f32,
    ) {
        if let Some(drag) = self.drag.filter(|d| d.mode == DragMode::Draw) {
            let (from, to) = (to_screen(drag.start), to_screen(drag.current));
            let preview = Rect::from_two_pos(from, to);
            let ink = Stroke::new(1.0, Color32::BLACK);
            match self.tool {
                Tool::Line => {
                    painter.line_segment(
                        [from, to],
                        Stroke::new(self.paint.stroke_width * scale, self.paint.stroke_color),
                    );
                }
                Tool::Ellipse => {
                    let radius = preview.size() / 2.0;
                    painter.add(egui::epaint::EllipseShape::filled(
                        preview.center(),
                        radius,
                        self.paint.fill.gamma_multiply(0.5),
                    ));
                    painter.rect_stroke(preview, 0.0, ink, egui::StrokeKind::Outside);
                }
                _ => {
                    painter.rect_filled(preview, 0.0, self.paint.fill.gamma_multiply(0.5));
                    painter.rect_stroke(preview, 0.0, ink, egui::StrokeKind::Outside);
                }
            }
        }
    }
}
