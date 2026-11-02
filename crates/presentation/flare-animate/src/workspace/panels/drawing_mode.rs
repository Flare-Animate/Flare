//! The Merge Drawing and Object Drawing switch, shown as a dimmed stand-in until Merge Drawing
//! is built.

use crate::workspace::{Tool, Workspace};
use egui::Ui;

impl Workspace {
    /// Animate's Object Drawing switch (J) on the tools that draw. Until shapes can merge every
    /// shape is a drawing object of its own, so the switch is on and cannot be changed.
    pub(super) fn drawing_mode_mock(&self, ui: &mut Ui) {
        if !matches!(
            self.tool,
            Tool::Rectangle
                | Tool::Ellipse
                | Tool::Line
                | Tool::PolyStar
                | Tool::Pencil
                | Tool::Brush
                | Tool::Pen
        ) {
            return;
        }
        let mut object_drawing = true;
        ui.add_enabled(
            false,
            egui::Checkbox::new(&mut object_drawing, tr!(self, "opt-object-drawing")),
        )
        .on_disabled_hover_text(tr!(self, "opt-object-drawing-hint"));
    }
}
