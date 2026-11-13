//! Shortcuts, menus and the stage furniture.

use egui::Key;
use flare_menubar::{MENU_LAYOUT, MenuAction, MenuEntry};

use super::*;

impl Workspace {
    // ---- UI --------------------------------------------------------------------------

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        flare_ui::theme::set_mode(self.theme_mode);
        if !self.chrome_ready {
            flare_ui::theme::apply(&ctx);
            flare_ui::icons::install(&ctx);
            // Cmd/Ctrl with + and - zooms the stage (the Zoom In and Zoom Out shortcuts), never
            // the whole window, as egui would by default.
            ctx.options_mut(|o| o.zoom_with_keyboard = false);
            self.chrome_ready = true;
            self.styled_for = Some(self.theme_mode);
        }
        self.ensure_palette(&ctx);
        let now = ui.input(|i| i.time);
        self.open_dropped(&ctx);
        self.drain_background();
        self.advance(now);
        self.ensure_sounds_decoded();
        self.sync_audio();
        self.autosave(now);
        // Keys act on the document, which Home hides.
        if !self.windows.home {
            self.handle_shortcuts(&ctx);
        }
        // Only a changed document can make the selection stale; checking it every frame cost
        // time in proportion to the selection times the layers.
        if self.selection_checked != Some((self.context_serial, self.active().revision())) {
            self.validate_selection();
        }
        self.ensure_stage_texture(&ctx);

        if self.document_controls_shown() {
            egui::Panel::bottom("status")
                .exact_size(24.0)
                .frame(
                    egui::Frame::new()
                        .fill(flare_ui::theme::chrome())
                        .inner_margin(egui::Margin::symmetric(8, 2)),
                )
                .show(ui, |ui| self.status_bar(ui));
        }
        egui::Panel::top("menu")
            .exact_size(if self.titlebar_inset > 0.0 || self.window_controls {
                INTEGRATED_TOP_BAR_HEIGHT
            } else {
                TOP_BAR_HEIGHT
            })
            .frame(egui::Frame::new().fill(flare_ui::theme::chrome()))
            .show(ui, |ui| self.menu_bar(ui));
        if self.document_controls_shown() {
            egui::Panel::right("panel_rail")
                .exact_size(rail::RAIL_WIDTH)
                .frame(egui::Frame::new().fill(flare_ui::theme::chrome()))
                .show(ui, |ui| self.panel_rail(ui));
        }
        let gutter = flare_ui::theme::GUTTER as i8;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(flare_ui::theme::bg())
                    .inner_margin(egui::Margin::same(gutter)),
            )
            .show(ui, |ui| {
                if self.windows.home {
                    self.home_page(ui);
                    return;
                }
                // The dock needs `&mut self` to draw panel contents, so take it out while it runs.
                let mut tree = std::mem::replace(
                    &mut self.panels.layout.tree,
                    egui_tiles::Tree::empty(egui::Id::unique("taken")),
                );
                tree.ui(&mut Dock { workspace: self }, ui);
                self.panels.layout.tree = tree;
                self.apply_panel_change();
            });

        if self.document_controls_shown() {
            self.panel_flyout(&ctx);
        }
        self.preferences_window(&ctx);
        self.publish_window(&ctx);
        self.increment_instances_window(&ctx);
        self.ensure_language_font(&ctx);
        self.file_windows(&ctx);
        self.close_prompt_window(&ctx);
        self.help_windows(&ctx);
        self.command_palette_window(&ctx);
        self.new_document_window(&ctx);
        self.splash_window(&ctx);
        if self.playing {
            ctx.request_repaint();
        }
    }

    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (toggle, left, right, escape, delete) = ctx.input(|i| {
            (
                !i.modifiers.any() && i.key_pressed(Key::Space),
                !i.modifiers.any() && i.key_pressed(Key::ArrowLeft),
                !i.modifiers.any() && i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Escape),
                !i.modifiers.any() && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
            )
        });
        // Ctrl, Alt and F-key commands come from the shortcut map, so a rebinding moves them. With
        // a native menu bar the menu owns them and nothing runs here (applying twice).
        self.press_command_chords(ctx);
        self.watch_rotation_tap(ctx);
        self.press_zoom_plus(ctx);
        let typing = ctx.egui_wants_keyboard_input();
        // Letters pick tools, but only when nothing is being typed and no modifier is held, so
        // they never fight a text field or a shortcut.
        if !typing {
            self.press_tool_keys(ctx);
        }
        // Cmd/Ctrl+C and V arrive as Copy and Paste events. They are not menu accelerators, so
        // text fields keep them; when no field wants the keyboard they act on the selection.
        let (copy, cut, paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        if !typing {
            if copy {
                self.copy_selection();
            }
            if cut {
                self.cut_selection();
            }
            if paste {
                self.paste();
            }
            if toggle {
                self.playing = !self.playing;
            }
            if enter_pressed(ctx) {
                self.pen_finish();
                let additive = ctx.input(|i| i.modifiers.shift);
                self.polygon_lasso_finish(additive);
            }
            // Animate steps frames with the comma and the period, and the arrow keys nudge the
            // selection by screen pixels instead (issue #213): one press moves one screen
            // pixel, Shift makes it ten, whatever the zoom. With nothing selected the left
            // and right arrows step frames as they did before the nudge, so transport
            // always has a way in.
            let (comma, period) = ctx.input(|i| {
                (
                    !i.modifiers.any() && i.key_pressed(Key::Comma),
                    !i.modifiers.any() && i.key_pressed(Key::Period),
                )
            });
            let (nudge_left, nudge_right, nudge_up, nudge_down, coarse) = ctx.input(|i| {
                (
                    i.key_pressed(Key::ArrowLeft),
                    i.key_pressed(Key::ArrowRight),
                    i.key_pressed(Key::ArrowUp),
                    i.key_pressed(Key::ArrowDown),
                    i.modifiers.shift_only(),
                )
            });
            if !(nudge_left || nudge_right || nudge_up || nudge_down)
                || self.selection_all().is_empty()
            {
                if left || comma {
                    self.set_frame(self.frame.saturating_sub(1));
                }
                if right || period {
                    self.set_frame(self.frame.saturating_add(1));
                }
            } else {
                self.nudge_selection(
                    nudge_right as i32 - nudge_left as i32,
                    nudge_down as i32 - nudge_up as i32,
                    coarse,
                );
            }
        }
        // Delete only gives way to a text field being edited: a button that kept the focus after a
        // click must not swallow it.
        if delete && !ctx.text_edit_focused() {
            if self.pen.is_active() {
                self.pen_remove_last();
            } else if self.wand_clear() {
                // The picked pixels went; the bitmap stays.
            } else if self.tool == Tool::Subselection && self.delete_selected_anchor() {
                // The chosen point went; the shape stays.
            } else {
                self.delete_selection();
            }
        }
        if escape {
            self.cancel_drag();
            self.pen_cancel();
            self.wand_drop();
        }
    }

    /// One menu item for `action`: greyed out when it cannot run, runs it when clicked.
    pub(super) fn action_item(&mut self, ui: &mut egui::Ui, action: MenuAction) {
        let enabled = self.is_enabled(action);
        let tint = if enabled {
            flare_ui::theme::text()
        } else {
            flare_ui::theme::text_disabled()
        };
        let label = self.label(action);
        let mut button = match action.icon() {
            Some(icon) => {
                egui::Button::image_and_text(flare_ui::icons::image(icon, 14.0, tint), label)
            }
            None => egui::Button::new(label),
        };
        // The shortcut, or for a tool the key that picks it, right-aligned in the item.
        if let Some(text) = self.shortcut_label(action) {
            button = button.shortcut_text(text);
        }
        let clicked = ui.add_enabled(enabled, button).clicked();
        if clicked {
            self.perform(action);
            ui.close();
        }
    }

    /// The top bar: room for the window buttons, the app mark, the in-window menus (when there
    /// is no native menu bar), the document name, and quick actions. Empty space drags the
    /// window.
    /// The entries of one menu, with a menu inside it opened as its own.
    fn menu_entries(&mut self, ui: &mut egui::Ui, entries: &[MenuEntry]) {
        for entry in entries {
            match entry {
                MenuEntry::Action(action) => self.action_item(ui, *action),
                MenuEntry::Separator => {
                    ui.separator();
                }
                MenuEntry::Mock(label) => {
                    ui.add_enabled(false, egui::Button::new(self.mock_label(label)))
                        .on_disabled_hover_text(flare_ui::chrome::NOT_YET);
                }
                MenuEntry::Submenu(title, inner) => {
                    ui.menu_button(self.submenu_title(title), |ui| self.menu_entries(ui, inner));
                }
            }
        }
    }

    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let bar = ui.max_rect();
        let drag = ui.interact(bar, self.top_bar_id, egui::Sense::click_and_drag());
        if drag.double_clicked() {
            self.window_request = Some(WindowRequest::ToggleMaximize);
        } else if drag.drag_started() {
            self.window_request = Some(WindowRequest::Drag);
        }
        self.window_buttons(ui, bar);
        ui.painter().hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            egui::Stroke::new(1.0, flare_ui::theme::border()),
        );
        let publish_tip = tr!(self, "bar-publish");
        ui.horizontal_centered(|ui| {
            ui.add_space(self.titlebar_inset.max(10.0));
            let home = ui
                .horizontal(|ui| {
                    ui.add(flare_ui::icons::image(
                        "home",
                        16.0,
                        flare_ui::theme::text(),
                    ));
                    // With the system's menu bar the bar keeps its name beside the house; with
                    // the in-window menus the menus take that place, as in Animate.
                    if self.native_menu {
                        ui.label(
                            egui::RichText::new("Flare")
                                .strong()
                                .color(flare_ui::theme::text()),
                        );
                    }
                })
                .response
                .interact(egui::Sense::click())
                .on_hover_text(tr!(self, "bar-home"));
            // The house goes to Home and stays there; it is not a switch. Pressing it on Home used
            // to leave for the untouched document underneath, which looked like a new file. A tab
            // is how a document is gone back to.
            if home.clicked() {
                self.windows.home = true;
            }
            ui.add_space(6.0);
            egui::MenuBar::new().ui(ui, |ui| {
                if self.native_menu {
                    return;
                }
                for (title, entries) in MENU_LAYOUT {
                    ui.menu_button(self.menu_title(title), |ui| self.menu_entries(ui, entries));
                }
            });
            let controls_width = self.window_controls_width();
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(controls_width);
                if self.native_menu {
                    self.classic_bar_right(ui, bar, controls_width);
                    return;
                }
                // Play, Publish and the workspace switcher belong to a document; Home has none,
                // and no interface to switch.
                if !self.document_controls_shown() {
                    return;
                }
                // Animate's workspace switcher is in this corner too.
                self.workspace_switcher(ui);
                ui.add_space(6.0);
                // Animate's Publish button is next to Play too.
                let can_publish = self.is_enabled(MenuAction::Publish);
                if flare_ui::chrome::icon_button(
                    ui,
                    "publish",
                    16.0,
                    false,
                    can_publish,
                    &publish_tip,
                )
                .clicked()
                {
                    self.perform(MenuAction::Publish);
                }
                let (icon, hint) = if self.is_playing() {
                    ("transport-pause", tr!(self, "bar-pause"))
                } else {
                    ("transport-play", tr!(self, "bar-play"))
                };
                let play = flare_ui::chrome::icon_button(ui, icon, 16.0, false, true, &hint);
                if play.clicked() {
                    self.perform(MenuAction::PlayPause);
                }
                play.context_menu(|ui| self.transport_menu(ui));
            });
        });
    }

    /// The menu on a right click of the Play button: where Test Movie is, one click from Play.
    pub(super) fn transport_menu(&mut self, ui: &mut egui::Ui) {
        let enabled = self.is_enabled(MenuAction::TestMovie);
        if ui
            .add_enabled(enabled, egui::Button::new(tr!(self, "bar-test-movie")))
            .on_hover_text(tr!(self, "bar-test-movie-tip"))
            .clicked()
        {
            self.perform(MenuAction::TestMovie);
            ui.close();
        }
    }
}
