//! The frame buttons of the timeline toolbar: the frame commands a person uses most, one click
//! away, as Animate has them on its timeline.

use crate::workspace::Workspace;
use egui::Ui;
use flare_menubar::MenuAction;
use flare_ui::chrome;

use super::*;

/// The frame commands on the timeline toolbar, in the order they are drawn. They are the same
/// commands as in the Insert menu and the frames' own menu, so each acts on the active layer at
/// the playhead and is one undo step.
pub(crate) const FRAME_BUTTONS: [MenuAction; 4] = [
    MenuAction::InsertKeyframe,
    MenuAction::InsertBlankKeyframe,
    MenuAction::InsertFrame,
    MenuAction::RemoveFrame,
];

impl Workspace {
    /// What a frame button says when pointed at: what the command does, and its keys if it has
    /// any.
    pub(crate) fn frame_button_hint(&self, action: MenuAction) -> String {
        // The Insert menu calls these just Frame, Keyframe and Blank Keyframe; away from that
        // menu they say what they do, as in the frames' own menu.
        let name = match action {
            MenuAction::InsertFrame => tr!(self, "msg-frame-insert"),
            MenuAction::InsertKeyframe => tr!(self, "msg-frame-insert-keyframe"),
            MenuAction::InsertBlankKeyframe => tr!(self, "msg-frame-insert-blank"),
            _ => self.label(action),
        };
        match self.shortcut_label(action) {
            Some(keys) => format!("{name} ({keys})"),
            None => name,
        }
    }

    /// Draws the frame buttons; each is dimmed where its command cannot run (a locked layer).
    pub(super) fn timeline_frame_buttons(&mut self, ui: &mut Ui) {
        for action in FRAME_BUTTONS {
            let Some(icon) = action.icon() else { continue };
            let enabled = self.is_enabled(action);
            let hint = self.frame_button_hint(action);
            let response = chrome::icon_button(ui, icon, BAR_ICON, false, enabled, &hint);
            // Where the button is, so a test can press it.
            #[cfg(test)]
            ui.ctx().data_mut(|data| {
                data.insert_temp(
                    egui::Id::unique(("timeline-frame-button", icon)),
                    response.rect,
                )
            });
            if response.clicked() {
                self.perform(action);
            }
        }
    }
}
