//! The Polygon tool: click the corners of an outline, and a double click, Enter or a click on
//! the first corner closes it into one shape in the fill and stroke in hand
//! (`flare_tools::polygon`). The corners are clicked as the Polygon Lasso's are and kept in the
//! same list, `lasso`, so changing tools or pressing Escape drops a half-made outline.

use egui::{Pos2, Stroke};
use flare_tools::lasso::polygon::{self, Click};

use crate::workspace::*;

impl Workspace {
    /// The clicks of the two tools that take corners, Polygon and Polygon Lasso, and the outline
    /// so far drawn over the stage.
    pub(in crate::workspace) fn corner_tools(
        &mut self,
        ui: &egui::Ui,
        response: &egui::Response,
        scale: f32,
        to_doc: impl Fn(Pos2) -> Pos2,
        to_screen: impl Fn(Pos2) -> Pos2,
        painter: &egui::Painter,
    ) {
        let lasso = self.tool == Tool::PolygonLasso;
        if !lasso && self.tool != Tool::Polygon {
            return;
        }
        let additive = ui.input(|i| i.modifiers.shift);
        if response.double_clicked() {
            self.polygon_lasso_finish(additive);
            self.polygon_finish();
        } else if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            if lasso {
                self.polygon_lasso_click(to_doc(p), scale, additive);
            } else {
                self.polygon_click(to_doc(p), scale);
            }
        }
        if lasso {
            self.draw_polygon_lasso(painter, to_screen);
        } else {
            self.draw_polygon_outline(painter, to_screen);
        }
    }

    /// A click with the Polygon tool: a corner, or the end of the outline when it lands on the
    /// first corner of one that already encloses an area.
    pub fn polygon_click(&mut self, at: Pos2, scale: f32) {
        if self.lasso.is_empty() {
            // Starting to draw lets go of the selection, as the other drawing tools do.
            self.selected = None;
            self.also_selected.clear();
        }
        if polygon::click(&mut self.lasso, at, scale) == Click::Close {
            self.commit_polygon();
        }
    }

    /// Enter or a double click: closes the outline, if it has three corners or more.
    pub fn polygon_finish(&mut self) {
        if self.tool != Tool::Polygon || self.lasso.is_empty() {
            return;
        }
        if polygon::can_close(&mut self.lasso) {
            self.commit_polygon();
        }
    }

    /// The corners so far as one shape in the paint in hand, drawn into the active layer as one
    /// undo step.
    fn commit_polygon(&mut self) {
        let corners: Vec<(f64, f64)> = std::mem::take(&mut self.lasso)
            .into_iter()
            .map(|p| (f64::from(p.x), f64::from(p.y)))
            .collect();
        if self.document().scenes().is_empty() {
            let message = tr!(self, "tools-no-scene");
            return self.report(message);
        }
        let shape = match flare_tools::polygon::polygon_shape(&corners, self.paint_fill(), None) {
            Ok(shape) => shape,
            Err(error) => return self.report(error.to_string()),
        };
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for &(x, y) in &corners {
            (x0, y0, x1, y1) = (x0.min(x), y0.min(y), x1.max(x), y1.max(y));
        }
        let Some(shape) = self.painted(shape, (x0, y0, x1 - x0, y1 - y0)) else {
            return;
        };
        let name = tr!(self, "tools-layer-polygon");
        self.draw_shape(shape, &name);
    }

    /// The corners so far, the sides between them and one on to the pointer, in the fill's
    /// color, with the outline closed back to the first corner faintly.
    fn draw_polygon_outline(&self, painter: &egui::Painter, to_screen: impl Fn(Pos2) -> Pos2) {
        if self.lasso.is_empty() {
            return;
        }
        let accent = flare_ui::theme::accent();
        let mut points: Vec<Pos2> = self.lasso.iter().map(|p| to_screen(*p)).collect();
        if let Some(pointer) = self.pointer_stage {
            points.push(to_screen(pointer));
        }
        if points.len() > 2 {
            painter.add(egui::Shape::convex_polygon(
                points.clone(),
                self.paint.fill.gamma_multiply(0.5),
                Stroke::NONE,
            ));
        }
        if points.len() > 1 {
            painter.add(egui::Shape::line(points.clone(), Stroke::new(1.5, accent)));
            let closing = [points[points.len() - 1], points[0]];
            painter.add(egui::Shape::dashed_line(
                &closing,
                Stroke::new(1.0, accent.gamma_multiply(0.6)),
                4.0,
                3.0,
            ));
        }
        for corner in &self.lasso {
            painter.circle_filled(to_screen(*corner), 2.5, accent);
        }
    }
}
