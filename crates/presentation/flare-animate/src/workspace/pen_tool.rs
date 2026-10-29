//! The pen tool.

use egui::Pos2;
use flare_core::{Rgba8, ShapeId};
use flare_document::{Shape, Stroke as ShapeStroke};
use flare_tools::pen::Press;

use super::*;

impl Workspace {
    // ---- pen tool --------------------------------------------------------------------

    pub fn pen_is_active(&self) -> bool {
        self.pen.is_active()
    }

    /// A press with the pen: adds a point (which a following drag turns into a handle), or
    /// closes the path when it lands on the first point.
    pub fn pen_press(&mut self, at: Pos2) {
        if self.tool != Tool::Pen {
            return;
        }
        if self.pen.press(at) == Press::Close {
            self.commit_pen(true);
        }
    }

    pub fn pen_drag(&mut self, to: Pos2) {
        if self.tool == Tool::Pen {
            self.pen.drag(to);
        }
    }

    pub fn pen_release(&mut self) {
        self.pen.release();
    }

    /// Enter or double-click: ends an open path.
    pub fn pen_finish(&mut self) {
        if self.tool != Tool::Pen || !self.pen.is_active() {
            return;
        }
        self.pen.trim_duplicates();
        self.commit_pen(false);
    }

    /// Backspace: removes the last point.
    pub fn pen_remove_last(&mut self) {
        self.pen.remove_last();
    }

    /// Escape: discards the path without touching the document.
    pub fn pen_cancel(&mut self) {
        self.pen.clear();
    }

    /// Turns the pen path into a shape on a new layer, as one undo step. A closed path is
    /// filled (and outlined when Outline is on); an open path has no area, so it is drawn with
    /// the stroke settings, like a line.
    pub(super) fn commit_pen(&mut self, closed: bool) {
        let Some(path) = self.pen.segments(closed) else {
            self.report(if closed {
                "A closed path needs at least three points"
            } else {
                "A path needs at least two points"
            });
            return;
        };
        let stroke = {
            let c = self.stroke_color.to_array();
            ShapeStroke::new(
                f64::from(self.stroke_width),
                Rgba8::new(c[0], c[1], c[2], c[3]),
            )
            .map(|stroke| stroke.with_cap(self.stroke_cap).with_join(self.stroke_join))
        };
        let fill = {
            let c = self.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let fill = if closed { fill } else { Rgba8::TRANSPARENT };
        let shape = match Shape::new(ShapeId::new(), path, fill) {
            Ok(shape) => shape,
            Err(error) => return self.report(error.to_string()),
        };
        let shape = if !closed || self.stroke_enabled {
            match stroke {
                Ok(stroke) => shape.with_stroke(stroke),
                Err(error) => return self.report(error.to_string()),
            }
        } else {
            shape
        };
        self.insert_shape_layer(shape, "Path");
        if self.status.is_none() {
            self.pen.clear();
        }
    }
}
