//! The command palette: every command and tool, one search away (Ctrl+K).
//!
//! The behaviour is AniCraft's (storytold/anicraft, MIT OR Apache-2.0), re-implemented fresh
//! for Flare's `MenuAction` registry: a search box over every action the workspace can
//! perform, matched by the letters of the query appearing in the label in order, run with a
//! click or Enter, moved through with the arrow keys, closed with Escape.

use flare_menubar::MenuAction;

use super::*;

/// The palette's state while it is shut: what was last typed, and which row was chosen.
#[derive(Default)]
pub(super) struct Palette {
    query: String,
    chosen: usize,
}

/// One row of the palette: the action, its words, its shortcut and whether it can run now.
struct Row {
    action: MenuAction,
    label: String,
    shortcut: Option<String>,
    enabled: bool,
}

/// Whether the letters of `query` appear in `label` in order, ignoring case. An empty query
/// matches everything, which is how the palette opens on the whole list.
fn matches(query: &str, label: &str) -> bool {
    let wanted: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();
    if wanted.is_empty() {
        return true;
    }
    let mut next = 0;
    for c in label.chars().flat_map(char::to_lowercase) {
        if next < wanted.len() && c == wanted[next] {
            next += 1;
        }
    }
    next == wanted.len()
}

impl Workspace {
    /// Opens the palette, or closes it when it is open.
    pub fn toggle_palette(&mut self) {
        self.windows.palette = !self.windows.palette;
    }

    /// The palette's window, shown every frame it is open.
    pub(super) fn palette_window(&mut self, ctx: &egui::Context) {
        if !self.windows.palette {
            return;
        }
        // Every action, in the registry's order, whether or not it is in a menu: the palette
        // is also a way to pick a tool by name.
        let rows: Vec<Row> = MenuAction::ALL
            .into_iter()
            .map(|action| Row {
                label: self.label(action),
                shortcut: self.shortcut_label(action),
                enabled: self.is_enabled(action),
                action,
            })
            .filter(|row| matches(&self.palette.query, &row.label))
            .collect();
        let query_before = std::mem::take(&mut self.palette.query);
        // The choice stays within the list the query has left.
        let mut state = (
            query_before.clone(),
            self.palette.chosen.min(rows.len().saturating_sub(1)),
        );
        let (placeholder, none, close_text) = (
            self.tr("palette-search"),
            self.tr("palette-empty"),
            self.tr("close"),
        );
        let mut chosen_action = None;
        let mut close = false;
        let response = egui::Modal::new(egui::Id::unique("command-palette")).show(ctx, |ui| {
            let (enter, up, down, escape) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::Enter),
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                    i.key_pressed(egui::Key::Escape),
                )
            });
            if escape {
                close = true;
                ui.close();
                return;
            }
            let field = ui.add(
                egui::TextEdit::singleline(&mut state.0)
                    .hint_text(placeholder)
                    .desired_width(340.0),
            );
            field.request_focus();
            // Typing again starts the choice over at the top of the new list.
            if state.0 != query_before {
                state.1 = 0;
            }
            if down {
                state.1 = (state.1 + 1).min(rows.len().saturating_sub(1));
            }
            if up {
                state.1 = state.1.saturating_sub(1);
            }
            if rows.is_empty() {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(none).color(flare_ui::theme::text_disabled()));
            }
            egui::ScrollArea::vertical()
                .max_height(360.0)
                .show(ui, |ui| {
                    for (index, row) in rows.iter().enumerate() {
                        let mut text = egui::RichText::new(&row.label);
                        if index == state.1 {
                            text = text.strong();
                        }
                        let mut button = egui::Button::new(text);
                        if let Some(shortcut) = &row.shortcut {
                            button = button.shortcut_text(shortcut.as_str());
                        }
                        if ui.add_enabled(row.enabled, button).clicked() {
                            state.1 = index;
                            chosen_action = Some(row.action);
                            // The modal is shut after the frame: `chosen_action` closes it, so
                            // `ui.close` is not needed from inside the scroll area's ui.
                        }
                    }
                });
            if enter
                && let Some(row) = rows.get(state.1)
                && row.enabled
            {
                chosen_action = Some(row.action);
                ui.close();
            }
            ui.add_space(4.0);
            if ui.button(close_text).clicked() {
                close = true;
                ui.close();
            }
        });
        self.palette.query = state.0;
        self.palette.chosen = state.1;
        if let Some(action) = chosen_action {
            self.windows.palette = false;
            self.perform(action);
        } else if close || response.should_close() {
            self.windows.palette = false;
        }
    }
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
