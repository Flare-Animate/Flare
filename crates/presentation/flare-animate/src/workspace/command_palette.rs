//! Help > Find a Command (Cmd/Ctrl+Shift+P): a field that finds any menu command by its name in
//! the person's language, its id or its keys (`ShortcutMap::find_commands`, the same matching as
//! the shortcut list), and runs the one chosen. Animate has no such thing; every command it lists
//! is one the menus already have, so it is a quicker way to the same place, never a new one.

use egui::{Align2, Key, Modifiers, Order, Pos2, RichText};
use flare_menubar::{MAX_SEARCH_CHARS, MenuAction};
use flare_ui::theme;

use super::*;

/// How many commands the list shows at once.
pub(super) const MAX_ROWS: usize = 12;
const WIDTH: f32 = 460.0;

#[derive(Default)]
pub(crate) struct Palette {
    pub(crate) open: bool,
    pub(crate) query: String,
    /// The row Enter runs, counted from the top of what is listed.
    pub(crate) chosen: usize,
    /// The field takes the keys when the palette has just opened.
    focus: bool,
}

impl Workspace {
    /// Opens the palette with an empty field.
    pub(super) fn open_command_palette(&mut self) {
        self.palette = Palette {
            open: true,
            focus: true,
            ..Palette::default()
        };
    }

    /// The commands the palette lists for its query, at most [`MAX_ROWS`].
    pub fn palette_commands(&self) -> Vec<MenuAction> {
        let mut found = self
            .shortcuts
            .find_commands(&self.palette.query, |action| self.label(action));
        // The palette never lists itself.
        found.retain(|action| *action != MenuAction::CommandPalette);
        found.truncate(MAX_ROWS);
        found
    }

    /// Runs a listed command if it can run now, closing the palette first so the command's own
    /// window or dialog is what shows.
    pub(super) fn run_from_palette(&mut self, action: MenuAction) {
        if !self.is_enabled(action) {
            return;
        }
        self.palette.open = false;
        self.perform(action);
    }

    pub(super) fn command_palette_window(&mut self, ctx: &egui::Context) {
        if !self.palette.open {
            return;
        }
        let found = self.palette_commands();
        self.palette.chosen = self.palette.chosen.min(found.len().saturating_sub(1));
        let (down, up, enter, escape) = ctx.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::Enter),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if escape {
            self.palette.open = false;
            return;
        }
        if down && self.palette.chosen + 1 < found.len() {
            self.palette.chosen += 1;
        }
        if up {
            self.palette.chosen = self.palette.chosen.saturating_sub(1);
        }
        if enter && let Some(action) = found.get(self.palette.chosen).copied() {
            self.run_from_palette(action);
            return;
        }
        let screen = ctx.content_rect();
        let hint = tr!(self, "palette-hint");
        let none = tr!(self, "palette-none");
        let mut clicked = None;
        let area = egui::Area::new(egui::Id::NULL.with("command-palette"))
            .order(Order::Foreground)
            .pivot(Align2::CENTER_TOP)
            .fixed_pos(Pos2::new(screen.center().x, screen.top() + 72.0))
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(theme::panel_raised())
                    .stroke(egui::Stroke::new(1.0, theme::border()))
                    .corner_radius(6)
                    .inner_margin(8)
                    .shadow(ui.visuals().popup_shadow)
                    .show(ui, |ui| {
                        ui.set_width(WIDTH);
                        let mut query = self.palette.query.clone();
                        let field = ui.add(
                            egui::TextEdit::singleline(&mut query)
                                .hint_text(hint)
                                .char_limit(MAX_SEARCH_CHARS)
                                .desired_width(f32::INFINITY),
                        );
                        if std::mem::take(&mut self.palette.focus) {
                            field.request_focus();
                        }
                        if field.changed() {
                            self.palette.query = query;
                            self.palette.chosen = 0;
                        }
                        ui.add_space(4.0);
                        if found.is_empty() {
                            ui.weak(none);
                        }
                        for (row, action) in found.iter().copied().enumerate() {
                            let enabled = self.is_enabled(action);
                            let mut name = RichText::new(self.label(action));
                            if !enabled {
                                name = name.color(theme::text_disabled());
                            }
                            let keys = self.shortcuts.text(action).unwrap_or_default();
                            let response = ui
                                .horizontal(|ui| {
                                    let selected = row == self.palette.chosen;
                                    let label = ui.add_enabled(
                                        enabled,
                                        egui::Button::selectable(selected, name)
                                            .min_size(egui::vec2(WIDTH - 120.0, 0.0)),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| ui.weak(keys),
                                    );
                                    label
                                })
                                .inner;
                            if response.clicked() {
                                clicked = Some(action);
                            }
                        }
                    });
            });
        if let Some(action) = clicked {
            self.run_from_palette(action);
            return;
        }
        let away = ctx.input(|i| {
            i.pointer.any_pressed()
                && i.pointer
                    .interact_pos()
                    .is_some_and(|p| !area.response.rect.contains(p))
        });
        if away {
            self.palette.open = false;
        }
    }
}
