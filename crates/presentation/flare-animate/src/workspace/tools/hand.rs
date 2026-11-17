//! The Hand tool: a drag on the stage pans the view. When the drag is let go, the tool picked
//! before the Hand is given back, as the Rotation tool gives its own back, so a pan is a moment
//! between strokes and not a change of tool. The Rotation tool is never what is given back: a
//! Hand picked while turning the view goes back to the tool held before the turn.

use egui::{PointerButton, Response, Ui};

use crate::workspace::*;

/// What the Hand tool remembers between passes.
#[derive(Default)]
pub(in crate::workspace) struct HandSpring {
    /// The tool to give back once a pan is done.
    before: Option<Tool>,
    /// A pan has just finished: the next pass puts `before` back in hand. It waits for that pass
    /// so the tool given back never sees the end of the pan as a drag of its own.
    going_back: bool,
}

/// Whether a tool can be given back after a pan: one that is itself only a way of looking does
/// not count.
fn gives_back(tool: Tool) -> bool {
    !matches!(tool, Tool::Hand | Tool::RotateView)
}

impl Workspace {
    /// Called by [`Self::set_tool`] with the tool that was in hand, before the Rotation tool
    /// forgets its own: picking the Hand remembers the tool to give back.
    pub(in crate::workspace) fn hand_tool_changed(&mut self, before: Tool, tool: Tool) {
        self.springs.hand.going_back = false;
        self.springs.hand.before = if tool != Tool::Hand {
            None
        } else if before == Tool::RotateView {
            self.rotate_view_before()
        } else {
            Some(before)
        }
        .filter(|t| gives_back(*t));
    }

    /// The tool the Hand gives back when a pan is let go, if any.
    pub fn hand_gives_back(&self) -> Option<Tool> {
        self.springs.hand.before
    }

    /// The Hand's drag on the stage: the view follows the pointer, and when it is let go the
    /// tool held before is back in hand on the next pass.
    pub(in crate::workspace) fn hand_stage(&mut self, ui: &Ui, response: &Response) {
        if self.tool != Tool::Hand {
            return;
        }
        if self.springs.hand.going_back {
            if let Some(before) = self.springs.hand.before.take() {
                self.set_tool(before);
            }
            return;
        }
        if response.dragged_by(PointerButton::Primary) {
            self.stage_view.pan_by(response.drag_delta());
        }
        if response.drag_stopped_by(PointerButton::Primary) && self.springs.hand.before.is_some() {
            self.springs.hand.going_back = true;
            ui.ctx().request_repaint();
        }
    }
}
