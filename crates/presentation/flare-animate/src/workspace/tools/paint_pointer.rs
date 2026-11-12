//! The Paint Bucket, Ink Bottle and Eyedropper on the stage: they act the moment the button goes
//! down. Their pointer is in `stage_pointer`.

use egui::{Pos2, Response, Ui};

use crate::workspace::*;

impl Workspace {
    /// Acts where the primary button went down on the stage.
    pub(in crate::workspace) fn paint_tool_press(
        &mut self,
        ui: &Ui,
        response: &Response,
        to_doc: &impl Fn(Pos2) -> Pos2,
    ) {
        let (pressed, shift) = ui.input(|i| {
            let pressed = i.pointer.primary_pressed();
            (
                pressed.then(|| i.pointer.press_origin()).flatten(),
                i.modifiers.shift,
            )
        });
        if response.hovered()
            && let Some(p) = pressed
        {
            self.click_with_tool(to_doc(p), shift);
        }
    }
}
