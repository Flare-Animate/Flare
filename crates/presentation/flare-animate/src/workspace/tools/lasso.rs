//! The polygon lasso: click corners, close the loop, and what it encloses is selected. Its
//! corners are kept in `lasso`, the list the freehand lasso fills while dragging.

use egui::{Pos2, Stroke};
use flare_tools::lasso::polygon::{self, Click};

use crate::workspace::*;

impl Workspace {
    /// A click with the polygon lasso: a corner, or the end of the loop when it lands on the
    /// first corner of one that already encloses an area.
    pub(in crate::workspace) fn polygon_lasso_click(
        &mut self,
        at: Pos2,
        scale: f32,
        additive: bool,
    ) {
        if polygon::click(&mut self.lasso, at, scale) == Click::Close {
            self.finish_lasso(additive);
        }
    }

    /// Enter or a double click: closes the loop, if it encloses anything.
    pub(in crate::workspace) fn polygon_lasso_finish(&mut self, additive: bool) {
        if self.tool != Tool::PolygonLasso || self.lasso.is_empty() {
            return;
        }
        if polygon::can_close(&mut self.lasso) {
            self.finish_lasso(additive);
        }
    }

    /// The corners so far, with a line on to the pointer and back to the first corner.
    pub(in crate::workspace) fn draw_polygon_lasso(
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
