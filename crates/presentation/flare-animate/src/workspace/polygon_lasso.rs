//! The polygon lasso: click corners, close the loop, and what it encloses is selected. Its
//! corners are kept in `lasso`, the list the freehand lasso fills while dragging.

use egui::{Pos2, Stroke};

use super::*;

/// How far from the first corner, in screen pixels, a click closes the loop.
const CLOSE_RADIUS_PX: f32 = 8.0;

/// The most corners one loop takes; clicks past it are ignored.
const MAX_CORNERS: usize = 4096;

impl Workspace {
    /// A click with the polygon lasso: a corner, or the end of the loop when it lands on the
    /// first corner of one that already encloses an area.
    pub(super) fn polygon_lasso_click(&mut self, at: Pos2, scale: f32, additive: bool) {
        let radius = CLOSE_RADIUS_PX / scale.max(f32::EPSILON);
        if self.lasso.len() >= 3
            && self
                .lasso
                .first()
                .is_some_and(|f| (*f - at).length() <= radius)
        {
            self.finish_lasso(additive);
            return;
        }
        // The second click of a double click lands where the first did.
        let repeated = self
            .lasso
            .last()
            .is_some_and(|last| (*last - at).length() <= radius);
        if !repeated && self.lasso.len() < MAX_CORNERS {
            self.lasso.push(at);
        }
    }

    /// Enter or a double click: closes the loop. Fewer than three corners enclose nothing, so
    /// they are dropped and the selection stays as it was.
    pub(super) fn polygon_lasso_finish(&mut self, additive: bool) {
        if self.tool != Tool::PolygonLasso || self.lasso.is_empty() {
            return;
        }
        if self.lasso.len() < 3 {
            self.lasso.clear();
            return;
        }
        self.finish_lasso(additive);
    }

    /// The corners so far, with a line on to the pointer and back to the first corner.
    pub(super) fn draw_polygon_lasso(
        &self,
        painter: &egui::Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
    ) {
        if self.lasso.is_empty() {
            return;
        }
        let accent = flare_ui::theme::accent();
        let mut points: Vec<Pos2> = self.lasso.iter().map(|p| to_screen(*p)).collect();
        if let Some(pointer) = self.pointer_stage {
            points.push(to_screen(pointer));
        }
        if points.len() > 1 {
            points.push(points[0]);
            painter.add(egui::Shape::line(points, Stroke::new(1.5, accent)));
        }
        for corner in &self.lasso {
            painter.circle_filled(to_screen(*corner), 2.5, accent);
        }
    }
}
