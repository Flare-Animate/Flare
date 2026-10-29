//! Shortcuts, menus and the stage furniture.

use crate::timeline_view;
use crate::{MENU_LAYOUT, MenuAction, MenuEntry};
use egui::{Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Stroke, Vec2};
use flare_core::Rgba8;
use flare_document::MAX_STAGE_SIDE;

use super::*;

impl Workspace {
    // ---- UI --------------------------------------------------------------------------

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.chrome_ready {
            flare_ui::theme::apply(&ctx);
            flare_ui::icons::install(&ctx);
            self.chrome_ready = true;
        }
        let now = ui.input(|i| i.time);
        self.drain_background();
        self.advance(now);
        self.ensure_sounds_decoded();
        self.sync_audio();
        self.autosave(now);
        self.handle_shortcuts(&ctx);
        // Only a changed document can make the selection stale; checking it every frame cost
        // time in proportion to the selection times the layers.
        if self.selection_checked != Some((self.context_serial, self.active().revision())) {
            self.validate_selection();
        }
        self.ensure_stage_texture(&ctx);

        egui::Panel::bottom("status")
            .exact_size(24.0)
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::top("menu")
            .exact_size(if self.titlebar_inset > 0.0 {
                INTEGRATED_TOP_BAR_HEIGHT
            } else {
                TOP_BAR_HEIGHT
            })
            .frame(egui::Frame::new().fill(flare_ui::theme::BG))
            .show(ui, |ui| self.menu_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            // The dock needs `&mut self` to draw panel contents, so take it out while it runs.
            let mut layout = std::mem::replace(
                &mut self.layout,
                egui_tiles::Tree::empty(egui::Id::unique("taken")),
            );
            layout.ui(&mut Dock { workspace: self }, ui);
            self.layout = layout;
        });

        self.preferences_window(&ctx);
        self.file_windows(&ctx);
        self.help_windows(&ctx);
        if self.playing {
            ctx.request_repaint();
        }
    }

    pub(super) fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (redo, undo, save, toggle, left, right, escape, delete, convert) = ctx.input_mut(|i| {
            // Redo must be consumed first: egui lets Cmd+Shift+Z also match plain Cmd+Z.
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::S),
                !i.modifiers.any() && i.key_pressed(Key::Space),
                !i.modifiers.any() && i.key_pressed(Key::ArrowLeft),
                !i.modifiers.any() && i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Escape),
                !i.modifiers.any() && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
                !i.modifiers.any() && i.key_pressed(Key::F8),
            )
        });
        // With a native menu bar the menu owns these shortcuts; handling them here too
        // would apply them twice.
        if !self.native_menu {
            if redo {
                self.redo();
            } else if undo {
                self.undo();
            }
            if save {
                self.save();
            }
            let frames_chord = Modifiers::COMMAND | Modifiers::ALT;
            if ctx.input_mut(|i| i.consume_key(frames_chord, Key::C)) {
                self.copy_frames();
            }
            if ctx.input_mut(|i| i.consume_key(frames_chord, Key::V)) {
                self.paste_frames();
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D))
                && !ctx.egui_wants_keyboard_input()
            {
                self.duplicate_selection();
            }
        }
        let typing = ctx.egui_wants_keyboard_input();
        // Letters pick tools, but only when nothing is being typed and no modifier is held, so
        // they never fight a text field or a shortcut.
        if !typing {
            let picked = TOOL_KEYS
                .iter()
                .find(|(key, _)| key_press(ctx, *key).is_some_and(|m| !m.any()))
                .map(|(_, tool)| *tool);
            if let Some(tool) = picked {
                self.set_tool(tool);
            }
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
            }
            if convert && !self.native_menu {
                self.convert_selection_to_symbol();
            }
            if left {
                self.set_frame(self.frame.saturating_sub(1));
            }
            if right {
                self.set_frame(self.frame.saturating_add(1));
            }
        }
        // Delete only gives way to a text field being edited: a button that kept the focus after a
        // click must not swallow it.
        if delete && !ctx.text_edit_focused() {
            if self.pen.is_active() {
                self.pen_remove_last();
            } else {
                self.delete_selection();
            }
        }
        if escape {
            self.cancel_drag();
            self.pen_cancel();
        }
        if !typing && !self.native_menu {
            // Modifiers come from the key event itself, which is what the platform reported
            // for that press.
            let f5 = key_press(ctx, Key::F5);
            let f6 = key_press(ctx, Key::F6);
            let f7 = key_press(ctx, Key::F7);
            if let Some(m) = f5 {
                self.frame_edit(if m.shift {
                    FrameOp::RemoveFrame
                } else {
                    FrameOp::InsertFrame
                });
            }
            if let Some(m) = f6 {
                self.frame_edit(if m.shift {
                    FrameOp::ClearKeyframe
                } else {
                    FrameOp::InsertKeyframe
                });
            }
            if f7.is_some() {
                self.frame_edit(FrameOp::InsertBlankKeyframe);
            }
        }
    }

    /// One menu item for `action`: greyed out when it cannot run, runs it when clicked.
    pub(super) fn action_item(&mut self, ui: &mut egui::Ui, action: MenuAction) {
        let enabled = self.is_enabled(action);
        let tint = if enabled {
            flare_ui::theme::TEXT
        } else {
            flare_ui::theme::TEXT_DISABLED
        };
        let label = self.label(action);
        let mut button = match action.icon() {
            Some(icon) => {
                egui::Button::image_and_text(flare_ui::icons::image(icon, 14.0, tint), label)
            }
            None => egui::Button::new(label),
        };
        // The shortcut, or for a tool the key that picks it, right-aligned in the item.
        if let Some(text) = action
            .shortcut_text()
            .or_else(|| action.tool_key().map(str::to_owned))
        {
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
    pub(super) fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let bar = ui.max_rect();
        let drag = ui.interact(bar, self.top_bar_id, egui::Sense::click_and_drag());
        if drag.double_clicked() {
            self.window_request = Some(WindowRequest::ToggleMaximize);
        } else if drag.drag_started() {
            self.window_request = Some(WindowRequest::Drag);
        }
        ui.painter().hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            egui::Stroke::new(1.0, flare_ui::theme::BORDER),
        );
        ui.horizontal_centered(|ui| {
            ui.add_space(self.titlebar_inset.max(10.0));
            ui.add(flare_ui::icons::image("home", 16.0, flare_ui::theme::TEXT));
            ui.label(
                egui::RichText::new("Flare")
                    .strong()
                    .color(flare_ui::theme::TEXT),
            );
            ui.add_space(6.0);
            egui::MenuBar::new().ui(ui, |ui| {
                if self.native_menu {
                    return;
                }
                for (title, entries) in MENU_LAYOUT {
                    ui.menu_button(*title, |ui| {
                        for entry in *entries {
                            match entry {
                                MenuEntry::Action(action) => self.action_item(ui, *action),
                                MenuEntry::Separator => {
                                    ui.separator();
                                }
                                MenuEntry::Mock(label) => {
                                    ui.add_enabled(false, egui::Button::new(*label))
                                        .on_disabled_hover_text(flare_ui::chrome::NOT_YET);
                                }
                            }
                        }
                    });
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                let can_redo = self.active().can_redo();
                if flare_ui::chrome::icon_button(ui, "redo", 16.0, false, can_redo, "Redo")
                    .clicked()
                {
                    self.redo();
                }
                let can_undo = self.active().can_undo();
                if flare_ui::chrome::icon_button(ui, "undo", 16.0, false, can_undo, "Undo")
                    .clicked()
                {
                    self.undo();
                }
                let (icon, hint) = if self.is_playing() {
                    ("transport-pause", "Pause")
                } else {
                    ("transport-play", "Play")
                };
                if flare_ui::chrome::icon_button(ui, icon, 16.0, false, true, hint).clicked() {
                    self.perform(MenuAction::PlayPause);
                }
                // The name sits in the middle of what is left, whatever the window width.
                let title = egui::RichText::new(self.title()).color(flare_ui::theme::TEXT_DIM);
                let rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    egui::pos2(bar.center().x, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    title.text(),
                    egui::FontId::proportional(13.0),
                    flare_ui::theme::TEXT_DIM,
                );
            });
        });
    }

    /// Stage size and background. Size edits apply on release or Enter so that scrubbing a
    /// value is a single undo step.
    pub(super) fn stage_settings(&mut self, ui: &mut egui::Ui) {
        let stage = self.document().stage();
        let (mut width, mut height) = self.stage_edit.unwrap_or((stage.width(), stage.height()));
        let wr = ui.add(
            egui::DragValue::new(&mut width)
                .range(1..=MAX_STAGE_SIDE)
                .prefix("W "),
        );
        let hr = ui.add(
            egui::DragValue::new(&mut height)
                .range(1..=MAX_STAGE_SIDE)
                .prefix("H "),
        );
        if wr.changed() || hr.changed() {
            self.stage_edit = Some((width, height));
        }
        let done = wr.drag_stopped() || wr.lost_focus() || hr.drag_stopped() || hr.lost_focus();
        if done
            && let Some((w, h)) = self.stage_edit.take()
            && (w, h) != (stage.width(), stage.height())
        {
            self.set_stage_size(w, h);
        }
        let bg = stage.background().to_array();
        let mut color = Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]);
        if ui.color_edit_button_srgba(&mut color).changed() {
            let c = color.to_srgba_unmultiplied();
            self.set_stage_background(Rgba8::new(c[0], c[1], c[2], c[3]));
        }
    }

    /// Rulers along the top and left edge of the stage view, in stage units.
    /// Grid lines over the stage, skipped when they would be too close to see.
    pub(super) fn stage_grid(
        &self,
        painter: &egui::Painter,
        panel: Rect,
        image: Rect,
        extent: Vec2,
        scale: f32,
    ) {
        let step = self.grid_size.max(1.0);
        if step * scale < 6.0 {
            return;
        }
        let ink = Stroke::new(1.0, Color32::from_white_alpha(22));
        let clip = image.intersect(panel);
        let mut x = 0.0;
        while x <= extent.x {
            let sx = image.min.x + x * scale;
            if sx >= clip.min.x && sx <= clip.max.x {
                painter.vline(sx, clip.y_range(), ink);
            }
            x += step;
        }
        let mut y = 0.0;
        while y <= extent.y {
            let sy = image.min.y + y * scale;
            if sy >= clip.min.y && sy <= clip.max.y {
                painter.hline(clip.x_range(), sy, ink);
            }
            y += step;
        }
    }

    pub(super) fn stage_rulers(
        &self,
        painter: &egui::Painter,
        panel: Rect,
        image: Rect,
        scale: f32,
    ) {
        const THICKNESS: f32 = 16.0;
        let top = Rect::from_min_max(panel.min, Pos2::new(panel.max.x, panel.min.y + THICKNESS));
        let left = Rect::from_min_max(panel.min, Pos2::new(panel.min.x + THICKNESS, panel.max.y));
        let fill = flare_ui::theme::PANEL_RAISED.gamma_multiply(0.92);
        painter.rect_filled(top, 0.0, fill);
        painter.rect_filled(left, 0.0, fill);
        let tick = Stroke::new(1.0, flare_ui::theme::TEXT_DISABLED);
        let font = FontId::monospace(9.0);
        for t in flare_ui::view::ruler_ticks(
            image.min.x,
            scale,
            panel.min.x + THICKNESS,
            panel.max.x,
            56.0,
        ) {
            let height = if t.major { THICKNESS - 4.0 } else { 4.0 };
            painter.vline(t.screen, (top.bottom() - height)..=top.bottom(), tick);
            if t.major {
                painter.text(
                    Pos2::new(t.screen + 2.0, top.top() + 1.0),
                    Align2::LEFT_TOP,
                    format!("{}", t.value.round()),
                    font.clone(),
                    flare_ui::theme::TEXT_DIM,
                );
            }
        }
        for t in flare_ui::view::ruler_ticks(
            image.min.y,
            scale,
            panel.min.y + THICKNESS,
            panel.max.y,
            56.0,
        ) {
            let width = if t.major { THICKNESS - 4.0 } else { 4.0 };
            painter.hline((left.right() - width)..=left.right(), t.screen, tick);
            if t.major {
                painter.text(
                    Pos2::new(left.left() + 1.0, t.screen + 2.0),
                    Align2::LEFT_TOP,
                    format!("{}", t.value.round()),
                    font.clone(),
                    flare_ui::theme::TEXT_DIM,
                );
            }
        }
        painter.rect_filled(
            Rect::from_min_size(panel.min, Vec2::splat(THICKNESS)),
            0.0,
            flare_ui::theme::PANEL_RAISED,
        );
    }

    pub(super) fn zoom_timeline(&mut self, factor: f32) {
        self.timeline_zoom = timeline_view::zoomed(self.timeline_zoom, factor);
    }
}
