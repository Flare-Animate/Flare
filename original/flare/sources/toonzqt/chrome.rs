//! Workspace chrome and command discovery; document edits still use Studio::dispatch.
use super::*;

impl Studio {
    pub(super) fn options_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.prefs.theme.palette();
        egui::Panel::top("studio.options")
            .exact_size(42.0)
            .frame(
                egui::Frame::NONE
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(12, 5))
                    .stroke(Stroke::new(1.0, p.line)),
            )
            .show(ui, |ui| {
                egui::ScrollArea::horizontal()
                    .id_salt("studio.options.scroll")
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(vec2(20.0, 20.0), Sense::hover());
                            theme::paint_icon(ui, r, self.tool.icon(), p.text);
                            ui.label(
                                RichText::new(self.i18n.text(self.tool.label()))
                                    .font(theme::medium(12.0)),
                            );
                            widgets::separator(ui);
                            ui.label(RichText::new(self.i18n.text("field-fill")).color(p.muted));
                            ui.color_edit_button_srgba(&mut self.fill);
                            ui.label(
                                RichText::new(format!(
                                    "#{:02X}{:02X}{:02X}",
                                    self.fill.r(),
                                    self.fill.g(),
                                    self.fill.b()
                                ))
                                .monospace()
                                .size(11.0)
                                .color(p.muted),
                            );
                            widgets::separator(ui);
                            for (cmd, icon) in
                                [(Command::Undo, Icon::Undo), (Command::Redo, Icon::Redo)]
                            {
                                let enabled = self.enabled(cmd);
                                ui.add_enabled_ui(enabled, |ui| {
                                    if theme::icon_button(
                                        ui,
                                        icon,
                                        false,
                                        &self.i18n.text(cmd.label()),
                                    )
                                    .clicked()
                                    {
                                        self.dispatch(cmd, ui.ctx());
                                    }
                                });
                            }
                            widgets::separator(ui);
                            if ui
                                .add_enabled(
                                    self.enabled(Command::Symbol),
                                    egui::Button::new(self.i18n.text("cmd-symbol")),
                                )
                                .clicked()
                            {
                                self.dispatch(Command::Symbol, ui.ctx());
                            }
                            ui.add_space(12.0);
                            if theme::icon_button(
                                ui,
                                Icon::Search,
                                false,
                                &self.i18n.text("action-search"),
                            )
                            .clicked()
                            {
                                self.open_commands();
                            }
                            ui.label(
                                RichText::new("Ctrl K")
                                    .monospace()
                                    .size(11.0)
                                    .color(p.muted),
                            );
                        });
                    });
            });
    }

    pub(super) fn status_bar(&mut self, ui: &mut egui::Ui) {
        let p = self.prefs.theme.palette();
        egui::Panel::bottom("studio.status")
            .exact_size(26.0)
            .frame(
                egui::Frame::NONE
                    .fill(p.tab)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let message = if self.status.is_empty() {
                        self.i18n.text(if self.dirty() {
                            "label-edited"
                        } else {
                            "status-ready"
                        })
                    } else {
                        self.status.clone()
                    };
                    ui.add(
                        egui::Label::new(RichText::new(message).size(11.0).color(if self.error {
                            p.playhead
                        } else {
                            p.muted
                        }))
                        .truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(
                                self.i18n
                                    .count("layer-count", self.editor.timeline().layers.len()),
                            )
                            .size(11.0)
                            .color(p.muted),
                        );
                        widgets::separator(ui);
                        ui.label(
                            RichText::new(self.i18n.format(
                                "stage-dimensions",
                                &[
                                    ("width", &self.i18n.number(self.editor.doc.width, 0)),
                                    ("height", &self.i18n.number(self.editor.doc.height, 0)),
                                ],
                            ))
                            .size(11.0)
                            .color(p.muted),
                        );
                    });
                });
            });
    }

    pub(super) fn zoom_control(&mut self, ui: &mut egui::Ui) {
        let current = self.i18n.format(
            "zoom-value",
            &[("value", &self.i18n.number(self.zoom * 100.0, 0))],
        );
        let response = ui
            .scope(|ui| {
                // Compact inset readout and narrow arrow well, with egui retaining
                // popup, accessibility and keyboard ownership.
                ui.spacing_mut().interact_size.y = 18.0;
                ui.spacing_mut().button_padding = vec2(3.0, 1.0);
                ui.spacing_mut().icon_width = 14.0;
                ui.spacing_mut().icon_spacing = 4.0;
                widgets::combo_value_box(ui, "studio.zoom", current, 76.0, |ui| {
                    if widgets::selectable(ui, self.fit, self.i18n.text("action-fit")).clicked() {
                        self.dispatch(Command::Fit, ui.ctx());
                    }
                    for percent in [25, 50, 75, 100, 150, 200, 400, 800] {
                        let zoom = percent as f32 / 100.0;
                        let label = self.i18n.format(
                            "zoom-value",
                            &[("value", &self.i18n.number(percent as f32, 0))],
                        );
                        if widgets::selectable(
                            ui,
                            !self.fit && (self.zoom - zoom).abs() < 0.001,
                            label,
                        )
                        .clicked()
                        {
                            self.zoom = zoom;
                            self.fit = false;
                            self.pan = egui::Vec2::ZERO;
                        }
                    }
                })
            })
            .inner;
        response.response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::ComboBox,
                true,
                self.i18n.text("field-stage-zoom"),
            )
        });
    }

    pub(super) fn navigator(&mut self, ui: &mut egui::Ui) {
        let p = self.prefs.theme.palette();
        let (r, response) =
            ui.allocate_exact_size(vec2(ui.available_width(), 120.0), Sense::click());
        ui.painter().rect_filled(r, 3.0, p.back);
        if let Some(texture) = &self.texture {
            let scale = ((r.width() - 18.0) / self.editor.doc.width)
                .min((r.height() - 18.0) / self.editor.doc.height);
            let image_rect = Rect::from_center_size(
                r.center(),
                vec2(self.editor.doc.width, self.editor.doc.height) * scale,
            );
            ui.painter().image(
                texture.id(),
                image_rect,
                Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        // Preview is an explicit fit shortcut, not an unimplemented pan affordance.
        response.clone().on_hover_text(self.i18n.text("action-fit"));
        if response.clicked() {
            self.dispatch(Command::Fit, ui.ctx());
        }
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                true,
                self.i18n.text("panel-navigator"),
            )
        });
    }

    fn open_commands(&mut self) {
        self.command_open = true;
        self.command_focus = true;
        self.command_query.clear();
    }

    pub(super) fn command_dialog(&mut self, ctx: &egui::Context) {
        if !self.command_open {
            return;
        }
        egui::Modal::new(egui::Id::new("studio.commands")).show(ctx, |ui| {
            ui.set_width(480.0);
            ui.heading(self.i18n.text("action-search"));
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.command_query)
                    .id(egui::Id::new("studio.command-query"))
                    .hint_text(self.i18n.text("search-hint"))
                    .desired_width(f32::INFINITY),
            );
            if self.command_focus {
                edit.request_focus();
                self.command_focus = false;
            }
            widgets::separator(ui);
            let query = self.command_query.to_lowercase();
            let commands: Vec<_> = Command::ALL
                .into_iter()
                .filter(|c| self.i18n.text(c.label()).to_lowercase().contains(&query))
                .collect();
            let first = commands.iter().copied().find(|c| self.enabled(*c));
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
            let mut chosen = if enter { first } else { None };
            egui::ScrollArea::vertical()
                .max_height(330.0)
                .show(ui, |ui| {
                    if commands.is_empty() {
                        ui.label(self.i18n.text("search-empty"));
                    }
                    for command in commands {
                        if ui
                            .add_enabled(
                                self.enabled(command),
                                egui::Button::new(self.i18n.text(command.label()))
                                    .shortcut_text(command.shortcut())
                                    .min_size(vec2(ui.available_width(), 30.0)),
                            )
                            .clicked()
                        {
                            chosen = Some(command);
                        }
                    }
                });
            widgets::separator(ui);
            if widgets::button(ui, self.i18n.text("action-cancel")).clicked() {
                self.command_open = false;
            }
            if let Some(command) = chosen {
                self.command_open = false;
                self.dispatch(command, ctx);
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::Queryable, Harness};

    #[test]
    fn zoom_combo_retains_keyboard_popup_and_fixed_geometry() {
        for locale in [Locale::English, Locale::French, Locale::Expanded] {
            for theme in [theme::Theme::Light, theme::Theme::Dark] {
                let mut h = Harness::builder()
                    .with_size(vec2(1440.0, 920.0))
                    .with_pixels_per_point(1.25)
                    .build_eframe(|cc| {
                        Studio::with_preferences(
                            &cc.egui_ctx,
                            Preferences {
                                locale,
                                theme,
                                ..Default::default()
                            },
                        )
                    });
                h.run();
                let before = h.state().editor.doc.clone();
                let i18n = Localizer::new(locale);
                let label = i18n.text("field-stage-zoom");
                let rect = h.get_by_label(&label).rect();
                assert!((rect.height() - 18.0).abs() < 0.01);
                h.get_by_label(&label).hover();
                for _ in 0..3 {
                    h.step();
                    assert_eq!(h.get_by_label(&label).rect(), rect);
                }
                h.get_by_label(&label).focus();
                h.run();
                assert_eq!(h.get_by_label(&label).rect(), rect);
                h.key_press(egui::Key::Enter);
                h.run();
                assert_eq!(h.get_by_label(&label).rect(), rect);
                let value = i18n.format("zoom-value", &[("value", &i18n.number(200.0, 0))]);
                h.get_by_label(&value).click();
                h.run();
                assert_eq!(h.state().zoom, 2.0);
                assert!(!h.state().fit);
                assert_eq!(h.state().editor.doc, before);
                assert_eq!(h.state().editor.undo_depth(), 0);
            }
        }
    }
}
