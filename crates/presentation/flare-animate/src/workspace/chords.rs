//! Command chords (Ctrl, Alt and the F-keys) from the shortcut map in force, so rebinding a
//! command changes the key that runs it and not only the label beside it. The bare tool letters
//! are in `shortcut_editor.rs`.

use crate::workspace::*;
use egui::{Key, Modifiers};
use flare_menubar::{Accelerator, MenuAction};

/// The second key for Redo, beside the one the shortcut map gives it.
const REDO_ALTERNATE: &str = "CmdOrCtrl+Shift+Z";

/// Whether the accelerator is a chord at all: it carries Ctrl or Cmd, or Alt, or it is a
/// function key. A bare letter or digit is not one: that is the tool keys' form, and a command
/// on one would take the key from every text field (meta.rs, `shortcut`'s rule).
fn is_chord(accel: &Accelerator) -> bool {
    accel.has_command() || accel.has_alt() || is_f_key(accel.key())
}

/// Whether a key name is a function key, from its name in the accelerator (`F5` and the like).
fn is_f_key(name: &str) -> bool {
    match name.strip_prefix('f').or_else(|| name.strip_prefix('F')) {
        Some(digits) => !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// The commands whose chords must not run here, because they arrive as events: Copy, Cut and
/// Paste are the text fields' own while one is being edited, so a bound chord would act twice.
/// Their frames forms (CopyFrames and the like) are separate commands and stay.
fn arrives_as_event(action: MenuAction) -> bool {
    matches!(
        action,
        MenuAction::Copy | MenuAction::Cut | MenuAction::Paste
    )
}

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
    /// menu bar the menu owns each command's first chord, the one it shows; the others a command
    /// has are still run here, since a menu item can register only one.
    pub(super) fn press_command_chords(&mut self, ctx: &egui::Context) {
        if self.shortcut_editor.capturing.is_some() {
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
        let native = self.native_menu;
        let mut chords: Vec<(MenuAction, Modifiers, Key)> = MenuAction::ALL
            .into_iter()
            .flat_map(|action| {
                let keys = self.shortcuts.keys(action);
                // The native menu registers the first, except for a tool's (flare-shell's
                // `bound_accelerator`); skip it then, or it would run twice.
                let menu_owns = native && !action.is_tool();
                keys.iter()
                    .skip(usize::from(menu_owns))
                    .map(move |accel| (action, accel))
            })
            .filter(|(action, accel)| is_chord(accel) && !arrives_as_event(*action))
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

    /// Cmd/Ctrl+Shift+Z also redoes, the second key most programs give Redo, while Redo is on its
    /// own key and nothing else has been bound to this one. No menu registers it, so it is read
    /// here even with a native menu bar. It runs before the chords so Cmd/Ctrl+Z never takes it.
    pub(super) fn press_redo_alternate(&mut self, ctx: &egui::Context) {
        if self.shortcut_editor.capturing.is_some() {
            return;
        }
        let default_redo = self
            .shortcuts
            .get(MenuAction::Redo)
            .map(ToString::to_string)
            == MenuAction::Redo.shortcut().map(str::to_owned);
        let taken = self
            .shortcuts
            .entries()
            .into_iter()
            .any(|(_, accel)| accel.to_string() == REDO_ALTERNATE);
        if !default_redo || taken {
            return;
        }
        let shifted = Modifiers::COMMAND | Modifiers::SHIFT;
        if ctx.input_mut(|i| i.consume_key(shifted, Key::Z)) {
            self.perform(MenuAction::Redo);
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
