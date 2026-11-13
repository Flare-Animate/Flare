//! Command chords (Ctrl, Alt and the F-keys) from the shortcut map in force, so rebinding a
//! command changes the key that runs it and not only the label beside it. The bare tool letters
//! are in `shortcut_editor.rs`.

use crate::workspace::*;
use egui::{Key, Modifiers};
use flare_menubar::MenuAction;

/// The commands a chord runs in the window. The rest of the menu is reached from the menu
/// itself; Copy, Cut and Paste arrive as events, which text fields keep.
const CHORD_ACTIONS: [MenuAction; 16] = [
    MenuAction::Undo,
    MenuAction::Redo,
    MenuAction::Save,
    MenuAction::CopyFrames,
    MenuAction::PasteFrames,
    MenuAction::Preferences,
    MenuAction::Duplicate,
    MenuAction::ConvertToSymbol,
    MenuAction::InsertFrame,
    MenuAction::RemoveFrame,
    MenuAction::InsertKeyframe,
    MenuAction::ClearKeyframe,
    MenuAction::InsertBlankKeyframe,
    MenuAction::ZoomIn,
    MenuAction::ZoomOut,
    MenuAction::CommandPalette,
];

/// Whether a text field being edited keeps the chord for itself. Undo, Redo, Save and the like
/// still act, as they always did; commands on the selection or the frames wait.
fn waits_for_typing(action: MenuAction) -> bool {
    !matches!(
        action,
        MenuAction::Undo
            | MenuAction::Redo
            | MenuAction::Save
            | MenuAction::Preferences
            | MenuAction::CommandPalette
    )
}

impl Workspace {
    /// Runs the command whose chord was pressed this frame, from the map in force. With a native
    /// menu bar the menu owns the chords, so nothing runs here.
    pub(super) fn press_command_chords(&mut self, ctx: &egui::Context) {
        if self.native_menu || self.shortcut_editor.capturing.is_some() {
            return;
        }
        // Most frames have no key press: leave before the map is read.
        let any_key = ctx.input(|i| {
            i.events
                .iter()
                .any(|e| matches!(e, egui::Event::Key { pressed: true, .. }))
        });
        if !any_key {
            return;
        }
        let typing = ctx.egui_wants_keyboard_input();
        let mut chords: Vec<(MenuAction, Modifiers, Key)> = self
            .shortcuts
            .entries()
            .into_iter()
            .filter(|(action, _)| CHORD_ACTIONS.contains(action))
            .filter_map(|(action, accel)| {
                let key = Key::from_name(accel.key())?;
                let mods = Modifiers {
                    command: accel.has_command(),
                    alt: accel.has_alt(),
                    shift: accel.has_shift(),
                    ..Modifiers::NONE
                };
                Some((action, mods, key))
            })
            .collect();
        // egui lets Cmd+Shift+Z also match Cmd+Z, so the chord with more modifiers goes first.
        chords.sort_by_key(|(_, m, _)| {
            std::cmp::Reverse(usize::from(m.command) + usize::from(m.alt) + usize::from(m.shift))
        });
        for (action, mods, key) in chords {
            if typing && waits_for_typing(action) {
                continue;
            }
            if ctx.input_mut(|i| i.consume_key(mods, key)) {
                self.perform(action);
            }
        }
    }

    /// Cmd and the plus key (Shift and `=` on most layouts) zooms the stage in as Cmd+`=` does.
    /// No menu registers it, so it is read here even with a native menu bar, unless Zoom In has
    /// been bound to something else on purpose.
    pub(super) fn press_zoom_plus(&mut self, ctx: &egui::Context) {
        let default_in = self
            .shortcuts
            .get(MenuAction::ZoomIn)
            .map(ToString::to_string)
            == MenuAction::ZoomIn.shortcut().map(str::to_owned);
        if !default_in || ctx.egui_wants_keyboard_input() {
            return;
        }
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::Plus)) {
            self.perform(MenuAction::ZoomIn);
        }
    }
}
