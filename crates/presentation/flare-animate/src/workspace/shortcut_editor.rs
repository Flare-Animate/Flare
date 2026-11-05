//! The keyboard shortcut editor: pick a set, change or clear a command's keys. The rules (what a
//! shortcut may be, conflicts) live in `flare_menubar::ShortcutMap`; this only reads keys and
//! shows the result.

use crate::workspace::*;
use egui::{Key, Modifiers};
use flare_menubar::{MenuAction, SHORTCUT_SET_IDS, ShortcutError, ShortcutMap};

/// A key press as the text a shortcut is written in; `None` for a key nothing can bind.
pub(super) fn chord_text(key: Key, mods: Modifiers) -> String {
    let mut out = String::new();
    if mods.command {
        out.push_str("CmdOrCtrl+");
    }
    if mods.alt {
        out.push_str("Alt+");
    }
    if mods.shift {
        out.push_str("Shift+");
    }
    out.push_str(match key {
        Key::ArrowUp => "Up",
        Key::ArrowDown => "Down",
        Key::ArrowLeft => "Left",
        Key::ArrowRight => "Right",
        Key::Comma => ",",
        Key::Period => ".",
        Key::Minus => "-",
        Key::Equals => "=",
        Key::Slash => "/",
        Key::Semicolon => ";",
        Key::OpenBracket => "[",
        Key::CloseBracket => "]",
        other => other.name(),
    });
    out
}

/// What the editor is doing right now.
#[derive(Default)]
pub(super) struct ShortcutEditor {
    /// The command waiting for its new keys.
    pub(super) capturing: Option<MenuAction>,
    /// Why the last change was refused, already in the person's language.
    pub(super) error: Option<String>,
}

impl Workspace {
    /// The shortcuts in force.
    pub fn shortcuts(&self) -> &ShortcutMap {
        &self.shortcuts
    }

    /// A command's shortcut as a person reads it, from the map in force.
    pub(super) fn shortcut_label(&self, action: MenuAction) -> Option<String> {
        self.shortcuts.text(action)
    }

    /// Start from another set; edits made so far are dropped.
    pub fn choose_shortcut_set(&mut self, id: &str) -> Result<(), ShortcutError> {
        self.shortcuts = ShortcutMap::from_set(id)?;
        self.shortcut_editor.error = None;
        Ok(())
    }

    /// Give a command new keys. A refusal leaves every shortcut as it was and is returned in the
    /// person's language.
    pub fn set_shortcut(&mut self, action: MenuAction, keys: &str) -> Result<(), String> {
        let result = self.shortcuts.bind(action, keys);
        self.shortcut_result(result)
    }

    /// Free a command's keys.
    pub fn clear_shortcut(&mut self, action: MenuAction) {
        self.shortcuts.unbind(action);
        self.shortcut_editor.error = None;
    }

    /// Put a command back on its set's keys.
    pub fn reset_shortcut(&mut self, action: MenuAction) -> Result<(), String> {
        let result = self.shortcuts.reset(action);
        self.shortcut_result(result)
    }

    fn shortcut_result(&mut self, result: Result<(), ShortcutError>) -> Result<(), String> {
        let message = result.err().map(|error| match error {
            ShortcutError::Conflict { accelerator, with } => {
                let action = MenuAction::from_id(&with).map_or(with, |a| self.label(a));
                tr!(
                    self,
                    "shortcuts-conflict",
                    &[("keys", accelerator.as_str()), ("action", action.as_str())]
                )
            }
            _ => tr!(self, "shortcuts-bad"),
        });
        self.shortcut_editor.error = message.clone();
        message.map_or(Ok(()), Err)
    }

    /// Runs a tool whose letter was pressed, from the map in force. Only when nothing is typed.
    pub(super) fn press_tool_keys(&mut self, ctx: &egui::Context) {
        if self.shortcut_editor.capturing.is_some() {
            return;
        }
        let hit = self
            .shortcuts
            .entries()
            .into_iter()
            .find_map(|(action, accel)| {
                if action.tool_key().is_none() || !accel.is_bare() {
                    return None;
                }
                let key = Key::from_name(accel.key())?;
                key_press(ctx, key)
                    .is_some_and(|m| !m.any())
                    .then_some(action)
            });
        if let Some(action) = hit {
            self.perform(action);
            return;
        }
        // Tools the menu does not name (the anchor tools) keep their fixed keys, unless the
        // person gave that key to something else.
        let picked = TOOL_KEYS
            .iter()
            .filter(|(_, tool)| {
                matches!(
                    tool,
                    Tool::AddAnchor | Tool::DeleteAnchor | Tool::ConvertAnchor
                )
            })
            .find(|(key, _)| {
                let taken = self
                    .shortcuts
                    .entries()
                    .iter()
                    .any(|(_, a)| a.is_bare() && Key::from_name(a.key()) == Some(*key));
                !taken && key_press(ctx, *key).is_some_and(|m| !m.any())
            })
            .map(|(_, tool)| *tool);
        if let Some(tool) = picked {
            self.set_tool(tool);
        }
    }

    /// The body of the Keyboard Shortcuts window.
    pub(super) fn shortcut_editor_ui(&mut self, ui: &mut egui::Ui) {
        let (set_title, change, press, clear, reset, hint) = (
            tr!(self, "shortcuts-set"),
            tr!(self, "shortcuts-change"),
            tr!(self, "shortcuts-press"),
            tr!(self, "shortcuts-clear"),
            tr!(self, "shortcuts-reset"),
            tr!(self, "shortcuts-hint"),
        );
        let set_name = |ws: &Self, id: &str| match id {
            "opentoonz" => tr!(ws, "shortcuts-set-opentoonz"),
            "animate" => tr!(ws, "shortcuts-set-animate"),
            "tahoma" => tr!(ws, "shortcuts-set-tahoma"),
            _ => tr!(ws, "shortcuts-set-flare"),
        };
        let mut chosen = None;
        ui.horizontal(|ui| {
            ui.label(set_title);
            egui::ComboBox::from_id_salt("shortcut-set")
                .selected_text(set_name(self, self.shortcuts.set()))
                .show_ui(ui, |ui| {
                    for id in SHORTCUT_SET_IDS {
                        if ui
                            .selectable_label(self.shortcuts.set() == id, set_name(self, id))
                            .clicked()
                        {
                            chosen = Some(id);
                        }
                    }
                });
        });
        if let Some(id) = chosen {
            // The ids come from the registry's own list, so this cannot fail.
            let _ = self.choose_shortcut_set(id);
        }
        // While a command waits for keys, the next key press (not Escape) is its new shortcut.
        if let Some(action) = self.shortcut_editor.capturing {
            let pressed = ui.input(|i| {
                i.events.iter().find_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, mods)) = pressed {
                self.shortcut_editor.capturing = None;
                if key != Key::Escape {
                    let _ = self.set_shortcut(action, &chord_text(key, mods));
                }
            }
        }
        if let Some(error) = &self.shortcut_editor.error {
            ui.colored_label(ui.visuals().warn_fg_color, error);
        }
        let mut todo: Option<(MenuAction, bool)> = None;
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                egui::Grid::new("shortcuts").num_columns(4).show(ui, |ui| {
                    for action in MenuAction::ALL {
                        let keys = self.shortcuts.text(action);
                        let (own, tool) =
                            (action.shortcut().is_some(), action.tool_key().is_some());
                        if keys.is_none() && !own && !tool {
                            continue;
                        }
                        ui.label(self.label(action));
                        if self.shortcut_editor.capturing == Some(action) {
                            ui.weak(&press);
                        } else {
                            ui.weak(keys.unwrap_or_default());
                        }
                        if ui.small_button(&change).clicked() {
                            self.shortcut_editor.capturing = Some(action);
                            self.shortcut_editor.error = None;
                        }
                        ui.horizontal(|ui| {
                            if ui.small_button(&clear).clicked() {
                                todo = Some((action, false));
                            }
                            if ui.small_button(&reset).clicked() {
                                todo = Some((action, true));
                            }
                        });
                        ui.end_row();
                    }
                });
            });
        match todo {
            Some((action, false)) => self.clear_shortcut(action),
            Some((action, true)) => {
                let _ = self.reset_shortcut(action);
            }
            None => {}
        }
        ui.weak(hint);
    }
}
