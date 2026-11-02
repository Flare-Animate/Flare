//! The menu a right click of the stage opens: what can be done with what is under the pointer, in
//! the order of Animate's own.

use crate::workspace::*;
use flare_menubar::MenuAction;

impl Workspace {
    /// A right click picks what is under the pointer, unless it is already part of the selection
    /// (so the menu acts on all of that), and clicking empty stage clears the selection.
    pub(super) fn pick_for_stage_menu(&mut self, point: egui::Pos2) {
        let hit = self.hit(point);
        match hit {
            Some(span) if self.selection_all().contains(&span) => {}
            _ => self.select_at(point, false),
        }
    }

    pub(super) fn stage_menu(&mut self, ui: &mut egui::Ui) {
        let groups: [&[MenuAction]; 5] = [
            &[
                MenuAction::Cut,
                MenuAction::Copy,
                MenuAction::Paste,
                MenuAction::PasteInPlace,
                MenuAction::Duplicate,
                MenuAction::Delete,
            ],
            &[
                MenuAction::Group,
                MenuAction::Ungroup,
                MenuAction::BreakApart,
                MenuAction::ConvertToSymbol,
            ],
            &[MenuAction::SelectAll, MenuAction::DeselectAll],
            &[MenuAction::Undo, MenuAction::Redo],
            &[MenuAction::ZoomIn, MenuAction::ZoomOut, MenuAction::ZoomFit],
        ];
        for (n, group) in groups.into_iter().enumerate() {
            if n > 0 {
                ui.separator();
            }
            for &action in group {
                let button = egui::Button::new(self.label(action));
                if ui.add_enabled(self.is_enabled(action), button).clicked() {
                    self.perform(action);
                    ui.close();
                }
            }
        }
    }
}
