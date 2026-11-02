//! The pen tool.

use egui::Pos2;
use flare_core::{Rgba8, ShapeId};
use flare_tools::pen::{PenStyle, Press, path_shape};

use crate::workspace::*;

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
    pub(in crate::workspace) fn commit_pen(&mut self, closed: bool) {
        let Some(path) = self.pen.segments(closed) else {
            let message = self.tr(if closed {
                "tools-pen-closed-points"
            } else {
                "tools-pen-open-points"
            });
            self.report(message);
            return;
        };
        let color = |c: egui::Color32| {
            let c = c.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let style = PenStyle {
            fill: color(self.paint.fill),
            outline: self.paint.stroke_enabled,
            stroke_width: f64::from(self.paint.stroke_width),
            stroke_color: color(self.paint.stroke_color),
            cap: self.paint.stroke_cap,
            join: self.paint.stroke_join,
        };
        let shape = match path_shape(ShapeId::new(), path, closed, &style) {
            Ok(shape) => shape,
            Err(error) => return self.report(error.to_string()),
        };
        let name = tr!(self, "tools-layer-path");
        self.draw_shape(shape, &name);
        if self.status.is_none() {
            self.pen.clear();
        }
    }
}
