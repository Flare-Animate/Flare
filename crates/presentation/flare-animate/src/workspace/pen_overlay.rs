//! The pen path being built, over the stage: the curve so far, its points, and the dragged
//! handles.

use egui::{Color32, Pos2, Rect, Stroke, StrokeKind, Vec2};

use super::*;

impl Workspace {
    pub(super) fn draw_pen_path(&self, painter: &egui::Painter, to_screen: &dyn Fn(Pos2) -> Pos2) {
        if self.pen.is_active() {
            let ink = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
            let handle = Stroke::new(1.0, Color32::from_rgb(120, 120, 120));
            let points = self.pen.points();
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let c1 = a.out.unwrap_or(a.anchor);
                let c2 = b.out.map_or(b.anchor, |o| b.anchor + (b.anchor - o));
                let steps = 20;
                let sample = |t: f32| {
                    let u = 1.0 - t;
                    to_screen(Pos2::new(
                        u * u * u * a.anchor.x
                            + 3.0 * u * u * t * c1.x
                            + 3.0 * u * t * t * c2.x
                            + t * t * t * b.anchor.x,
                        u * u * u * a.anchor.y
                            + 3.0 * u * u * t * c1.y
                            + 3.0 * u * t * t * c2.y
                            + t * t * t * b.anchor.y,
                    ))
                };
                let line: Vec<Pos2> = (0..=steps)
                    .map(|i| sample(i as f32 / steps as f32))
                    .collect();
                painter.add(egui::Shape::line(line, ink));
            }
            for point in points {
                if let Some(out) = point.out {
                    let mirror = point.anchor + (point.anchor - out);
                    painter.line_segment([to_screen(mirror), to_screen(out)], handle);
                    painter.circle_filled(to_screen(out), 3.0, Color32::from_rgb(120, 120, 120));
                }
                let at = to_screen(point.anchor);
                painter.rect_filled(
                    Rect::from_center_size(at, Vec2::splat(7.0)),
                    0.0,
                    Color32::WHITE,
                );
                painter.rect_stroke(
                    Rect::from_center_size(at, Vec2::splat(7.0)),
                    0.0,
                    ink,
                    StrokeKind::Outside,
                );
            }
        }
    }
}
