//! The Merge Drawing and Object Drawing switch on the tools that draw shapes, as in Animate.

use crate::workspace::{Tool, Workspace};
use egui::Ui;
use flare_document::Shape;
use flare_menubar::MenuAction;

impl Tool {
    /// Whether the tool draws shapes, which Merge Drawing or Object Drawing decides about.
    pub(crate) fn has_drawing_mode(self) -> bool {
        matches!(
            self,
            Tool::Rectangle
                | Tool::Ellipse
                | Tool::Line
                | Tool::PolyStar
                | Tool::Polygon
                | Tool::Pencil
                | Tool::Brush
                | Tool::Pen
        )
    }
}

impl Workspace {
    /// Animate's Object Drawing switch (J) on the tools that draw. Off is Merge Drawing.
    pub(super) fn drawing_mode_switch(&mut self, ui: &mut Ui) {
        if !self.tool.has_drawing_mode() {
            return;
        }
        let mut object_drawing = self.paint.object_drawing;
        let switch = ui
            .checkbox(&mut object_drawing, tr!(self, "opt-object-drawing"))
            .on_hover_text(tr!(self, "opt-object-drawing-hint"));
        if switch.changed() {
            self.perform(MenuAction::ToggleObjectDrawing);
        }
    }

    /// `shapes`, drawn by one of the tools with the switch, as the switch says: merged drawing
    /// unless Object Drawing is on. Text is always a drawing object, as in Animate. (Only the
    /// drawing tools and the Text tool place shapes this way; the Paint Bucket has its own road.)
    pub(in crate::workspace) fn in_drawing_mode(&self, shapes: Vec<Shape>) -> Vec<Shape> {
        let merge = !self.paint.object_drawing;
        shapes
            .into_iter()
            .map(|shape| {
                let merged = merge && shape.text().is_none();
                shape.merged(merged)
            })
            .collect()
    }
}
