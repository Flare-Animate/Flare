//! The Hand tool: a drag on the stage pans the view. It stays in hand when the drag is let go,
//! unlike the Rotation tool's quick turn: getting round a large stage takes pan after pan, and a
//! tool that gave itself back after each one would have to be picked again every time.

use egui::{PointerButton, Response};

use crate::workspace::*;

impl Workspace {
    /// Getting round the stage lets go of the selection, as starting to draw does, so its box
    /// is not left over the drawing while the view moves.
    pub(in crate::workspace) fn view_tool_picked(&mut self, tool: Tool) {
        if tool.only_views() {
            self.selected = None;
            self.also_selected.clear();
            self.selected_anchor = None;
        }
    }

    /// The Hand's drag on the stage: the view follows the pointer.
    pub(in crate::workspace) fn hand_stage(&mut self, response: &Response) {
        if self.tool == Tool::Hand && response.dragged_by(PointerButton::Primary) {
            self.stage_view.pan_by(response.drag_delta());
        }
    }
}
