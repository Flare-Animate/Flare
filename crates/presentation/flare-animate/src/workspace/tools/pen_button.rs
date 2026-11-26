//! A pen's side button on the stage. Pressed once it puts the Eraser in hand, pressed again it
//! gives back the tool held before, so a pen rubs out and draws again without a trip to the
//! toolbar. Tablet drivers send that button as a right click; a right click from a mouse still
//! opens the stage's menu.

use crate::workspace::*;

impl Workspace {
    /// Called by [`Self::set_tool`]: a tool picked any other way than by the button ends the
    /// flip, so the button does not give back a tool from before it.
    pub(in crate::workspace) fn pen_button_tool_changed(&mut self, tool: Tool) {
        if tool != Tool::Eraser {
            self.springs.pen_button = None;
        }
    }

    /// The pen's side button: the Eraser, or with the Eraser in hand the tool before it. An
    /// Eraser picked from the toolbar has nothing before it, and gives the Brush.
    pub fn pen_button(&mut self) {
        if self.tool == Tool::Eraser {
            let back = self.springs.pen_button.take().unwrap_or(Tool::Brush);
            self.set_tool(back);
        } else {
            let before = self.tool;
            self.set_tool(Tool::Eraser);
            self.springs.pen_button = Some(before);
        }
    }

    /// The tool the pen's button gives back, while the Eraser it gave is in hand.
    pub fn pen_button_gives_back(&self) -> Option<Tool> {
        self.springs.pen_button
    }

    /// Whether the pointer on the stage is a pen: the tablet reader, or the browser, says one
    /// is the device in use.
    pub(in crate::workspace) fn pen_in_use(&self) -> bool {
        self.pen_feed.reading().is_some()
    }
}
