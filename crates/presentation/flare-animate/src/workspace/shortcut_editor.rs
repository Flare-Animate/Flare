//! The keyboard shortcut editor: pick a set, find a command, change or clear its keys. Keys that
//! another command holds are taken from it, and the editor says which command lost them. The rules
//! (what a shortcut may be, what a search finds) live in `flare_menubar::ShortcutMap`; this only
//! reads keys and shows the result.

use crate::workspace::*;
use egui::{Key, Modifiers};
use flare_menubar::{
    Accelerator, MAX_KEYS_PER_COMMAND, MAX_SEARCH_CHARS, MenuAction, SHORTCUT_SET_IDS,
    ShortcutError, ShortcutMap,
};

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

/// A change asked for from a row of the list, made once the list is drawn.
enum RowEdit {
    Clear,
    Reset,
    /// Take these keys (in the file's spelling) off the command.
    Remove(String),
}

/// What the editor is doing right now.
#[derive(Default)]
pub(super) struct ShortcutEditor {
    /// The command waiting for its new keys.
    pub(super) capturing: Option<MenuAction>,
    /// The keys pressed for it are one more shortcut, not a replacement for those it has.
    pub(super) adding: bool,
    /// Why the last change was refused, already in the person's language.
    pub(super) error: Option<String>,
    /// Which command the last change took its keys from, already in the person's language.
    pub(super) notice: Option<String>,
    /// What is typed in the search field: the list shows only the commands it finds.
    pub(super) query: String,
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

    /// The commands the shortcut list shows for `query`, found by their names in the person's
    /// language, their ids or their keys (`Cmd+S`, `F5`, `option`).
    pub fn search_shortcuts(&self, query: &str) -> Vec<MenuAction> {
        self.shortcuts.search(query, |action| self.label(action))
    }

    /// Narrows the shortcut list to what `query` finds; empty shows every command.
    pub fn set_shortcut_search(&mut self, query: &str) {
        self.shortcut_editor.query = query.chars().take(MAX_SEARCH_CHARS).collect();
    }

    /// Start from another set; edits made so far are dropped.
    pub fn choose_shortcut_set(&mut self, id: &str) -> Result<(), ShortcutError> {
        self.shortcuts = ShortcutMap::from_set(id)?;
        self.shortcut_editor.error = None;
        self.shortcut_editor.notice = None;
        Ok(())
    }

    /// Give a command new keys in place of all it has. Keys another command holds are taken from
    /// it, and that command is returned. A refusal (keys that are not a shortcut) leaves every
    /// shortcut as it was and is returned in the person's language.
    pub fn set_shortcut(
        &mut self,
        action: MenuAction,
        keys: &str,
    ) -> Result<Option<MenuAction>, String> {
        let result = self.shortcuts.bind_taking(action, keys);
        self.shortcut_result(action, keys, result)
    }

    /// Give a command one more shortcut beside those it has, taking the keys from any command
    /// that holds them, as [`Self::set_shortcut`] does. Refused, changing nothing, for keys that
    /// are not a shortcut or a command that has [`MAX_KEYS_PER_COMMAND`] already.
    pub fn add_shortcut(
        &mut self,
        action: MenuAction,
        keys: &str,
    ) -> Result<Option<MenuAction>, String> {
        let result = self.shortcuts.add_key_taking(action, keys);
        self.shortcut_result(action, keys, result)
    }

    /// Take one of a command's shortcuts off it; its others stay. False when it did not have
    /// those keys.
    pub fn remove_shortcut(&mut self, action: MenuAction, keys: &str) -> bool {
        self.shortcut_editor.error = None;
        self.shortcut_editor.notice = None;
        Accelerator::parse(keys, action)
            .is_ok_and(|accel| self.shortcuts.remove_key(action, &accel))
    }

    /// Puts the keys of Animate's shortcut file (its bytes, as picked) on Animate's set and makes
    /// that the map in force. What was skipped is listed in Output, each with its reason; a file
    /// that cannot be read changes nothing and says why.
    pub fn import_animate_shortcuts(&mut self, bytes: &[u8]) {
        let text = String::from_utf8_lossy(bytes);
        match ShortcutMap::from_animate_xml(&text) {
            Ok(import) => {
                for skipped in &import.skipped {
                    let line =
                        format!("{} ({}): {}", skipped.command, skipped.keys, skipped.reason);
                    self.report(tr!(self, "shortcuts-import-skipped", &[("line", &line)]));
                }
                let (applied, skipped) = (
                    import.applied.len().to_string(),
                    import.skipped.len().to_string(),
                );
                self.shortcuts = import.map;
                let summary = tr!(
                    self,
                    "shortcuts-imported",
                    &[("applied", &applied), ("skipped", &skipped)]
                );
                self.shortcut_editor.error = None;
                self.shortcut_editor.notice = Some(summary.clone());
                self.report(summary);
            }
            Err(error) => {
                let reason = error.to_string();
                let message = tr!(self, "shortcuts-import-refused", &[("reason", &reason)]);
                self.shortcut_editor.error = Some(message.clone());
                self.report(message);
            }
        }
    }

    /// Free a command's keys.
    pub fn clear_shortcut(&mut self, action: MenuAction) {
        self.shortcuts.unbind(action);
        self.shortcut_editor.error = None;
        self.shortcut_editor.notice = None;
    }

    /// Put a command back on its set's keys, taking them from any command that holds them now.
    pub fn reset_shortcut(&mut self, action: MenuAction) -> Result<Option<MenuAction>, String> {
        let result = self.shortcuts.reset_taking(action);
        let keys = self.shortcuts.get(action).map(ToString::to_string);
        self.shortcut_result(action, keys.as_deref().unwrap_or_default(), result)
    }

    /// Says what a change did: which command it took `keys` from, or why it was refused.
    fn shortcut_result(
        &mut self,
        action: MenuAction,
        keys: &str,
        result: Result<Option<MenuAction>, ShortcutError>,
    ) -> Result<Option<MenuAction>, String> {
        let taken = result.as_ref().ok().copied().flatten();
        self.shortcut_editor.notice = taken.map(|previous| {
            let keys =
                Accelerator::parse(keys, action).map_or_else(|_| keys.to_owned(), |a| a.display());
            // A command that had other keys keeps them; say which case it is.
            let left = self.shortcuts.keys(previous).is_empty();
            let previous = self.label(previous);
            let args = [("keys", keys.as_str()), ("previous", previous.as_str())];
            if left {
                tr!(self, "shortcuts-taken", &args)
            } else {
                tr!(self, "shortcuts-taken-kept", &args)
            }
        });
        let result = result.map(|_| ());
        let message = result.err().map(|error| match error {
            ShortcutError::Conflict { accelerator, with } => {
                let action = MenuAction::from_id(&with).map_or(with, |a| self.label(a));
                tr!(
                    self,
                    "shortcuts-conflict",
                    &[("keys", accelerator.as_str()), ("action", action.as_str())]
                )
            }
            ShortcutError::TooManyKeys(most) => {
                tr!(
                    self,
                    "shortcuts-too-many",
                    &[("most", most.to_string().as_str())]
                )
            }
            _ => tr!(self, "shortcuts-bad"),
        });
        self.shortcut_editor.error = message.clone();
        message.map_or(Ok(taken), Err)
    }

    /// Runs a tool whose letter was pressed, from the map in force. Only when nothing is typed.
    pub(super) fn press_tool_keys(&mut self, ctx: &egui::Context) {
        if self.shortcut_editor.capturing.is_some() {
            return;
        }
        // A tool's key is a letter, alone or with Shift; every key a tool has picks it.
        let hit = self
            .shortcuts
            .entries()
            .into_iter()
            .find_map(|(action, accel)| {
                if !action.is_tool() || accel.has_command() || accel.has_alt() {
                    return None;
                }
                let key = Key::from_name(accel.key())?;
                key_press(ctx, key)
                    .is_some_and(|m| {
                        !m.command
                            && !m.ctrl
                            && !m.mac_cmd
                            && !m.alt
                            && m.shift == accel.has_shift()
                    })
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
        let (set_title, change, press, clear, reset, hint, search) = (
            tr!(self, "shortcuts-set"),
            tr!(self, "shortcuts-change"),
            tr!(self, "shortcuts-press"),
            tr!(self, "shortcuts-clear"),
            tr!(self, "shortcuts-reset"),
            tr!(self, "shortcuts-hint"),
            tr!(self, "shortcuts-search"),
        );
        let tap_key = super::tools::rotate_view::tap_key(ui.ctx());
        let tap = tr!(self, "shortcuts-rotate-tap", &[("key", tap_key)]);
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
        if ui.button(tr!(self, "shortcuts-import-animate")).clicked() {
            self.dialog = Some(DialogRequest::ImportAnimateShortcuts);
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
                let adding = std::mem::take(&mut self.shortcut_editor.adding);
                if key != Key::Escape {
                    let keys = chord_text(key, mods);
                    let _ = if adding {
                        self.add_shortcut(action, &keys)
                    } else {
                        self.set_shortcut(action, &keys)
                    };
                }
            }
        }
        let mut query = self.shortcut_editor.query.clone();
        let field = ui.add(
            egui::TextEdit::singleline(&mut query)
                .hint_text(search)
                .char_limit(MAX_SEARCH_CHARS)
                .desired_width(f32::INFINITY),
        );
        if field.changed() {
            self.set_shortcut_search(&query);
        }
        if let Some(error) = &self.shortcut_editor.error {
            ui.colored_label(ui.visuals().warn_fg_color, error);
        }
        if let Some(notice) = &self.shortcut_editor.notice {
            ui.weak(notice);
        }
        let rows = self.search_shortcuts(&self.shortcut_editor.query);
        if rows.is_empty() {
            let query = self.shortcut_editor.query.as_str();
            ui.weak(tr!(self, "shortcuts-none", &[("query", query)]));
        }
        let (add, remove) = (tr!(self, "shortcuts-add"), tr!(self, "shortcuts-remove"));
        let mut todo: Option<(MenuAction, RowEdit)> = None;
        egui::ScrollArea::vertical()
            .max_height(360.0)
            .show(ui, |ui| {
                egui::Grid::new("shortcuts").num_columns(4).show(ui, |ui| {
                    for action in rows {
                        ui.label(self.label(action));
                        // Each key on its own, so one of several can be taken off.
                        ui.horizontal_wrapped(|ui| {
                            for accel in self.shortcuts.keys(action) {
                                let chip = ui
                                    .small_button(format!("{} \u{d7}", accel.display()))
                                    .on_hover_text(&remove);
                                if chip.clicked() {
                                    todo = Some((action, RowEdit::Remove(accel.to_string())));
                                }
                            }
                            if self.shortcut_editor.capturing == Some(action) {
                                ui.weak(&press);
                            }
                        });
                        ui.horizontal(|ui| {
                            let full = self.shortcuts.keys(action).len() >= MAX_KEYS_PER_COMMAND;
                            for (label, adding, enabled) in
                                [(&change, false, true), (&add, true, !full)]
                            {
                                if ui
                                    .add_enabled(enabled, egui::Button::new(label).small())
                                    .clicked()
                                {
                                    self.shortcut_editor.capturing = Some(action);
                                    self.shortcut_editor.adding = adding;
                                    self.shortcut_editor.error = None;
                                    self.shortcut_editor.notice = None;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            if ui.small_button(&clear).clicked() {
                                todo = Some((action, RowEdit::Clear));
                            }
                            if ui.small_button(&reset).clicked() {
                                todo = Some((action, RowEdit::Reset));
                            }
                        });
                        ui.end_row();
                    }
                });
            });
        match todo {
            Some((action, RowEdit::Clear)) => self.clear_shortcut(action),
            Some((action, RowEdit::Reset)) => {
                let _ = self.reset_shortcut(action);
            }
            Some((action, RowEdit::Remove(keys))) => {
                self.remove_shortcut(action, &keys);
            }
            None => {}
        }
        ui.weak(hint);
        ui.weak(tap);
    }
}
