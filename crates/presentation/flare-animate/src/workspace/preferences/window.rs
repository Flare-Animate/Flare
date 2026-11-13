//! The Preferences window, and the Open Recent window.

use crate::workspace::*;

impl Workspace {
    /// The small windows opened from the File menu: recent projects and publish settings.
    pub(in crate::workspace) fn file_windows(&mut self, ctx: &egui::Context) {
        if self.windows.recent {
            let mut open = true;
            let mut chosen = None;
            let (title, nothing, clear) = (
                tr!(self, "start-open-recent"),
                tr!(self, "start-nothing-recent"),
                tr!(self, "start-clear-list"),
            );
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if self.home.recent.is_empty() {
                        ui.weak(nothing);
                    }
                    for path in &self.home.recent {
                        let name = path.file_name().map_or_else(
                            || path.display().to_string(),
                            |n| n.to_string_lossy().into_owned(),
                        );
                        if ui
                            .button(name)
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            chosen = Some(path.clone());
                        }
                    }
                    if !self.home.recent.is_empty() && ui.small_button(clear).clicked() {
                        self.home.recent.clear();
                    }
                });
            self.windows.recent = open && chosen.is_none();
            if let Some(path) = chosen {
                self.open(&path);
            }
        }
        if self.windows.publish_settings {
            let mut open = true;
            let (title, hint) = (
                tr!(self, "doc-publish-settings"),
                tr!(self, "start-publish-hint"),
            );
            egui::Window::new(title)
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    self.publish_settings_ui(ui);
                    ui.weak(hint);
                });
            self.windows.publish_settings = open;
        }
    }

    /// The Preferences window, while it is open.
    pub(in crate::workspace) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.windows.preferences {
            return;
        }
        let mut edited = self.preferences();
        let tr = &self.translator;
        let system_label = self.system_choice_label();
        // A dialog in the middle of the window over a dimmed backdrop, as PaintCraft's are:
        // Escape or a click outside closes it.
        let response = flare_ui::dialog::modal(ctx, self.preferences_id).show(ctx, |ui| {
            ui.set_width(380.0);
            egui::Sides::new().show(
                ui,
                |ui| {
                    ui.heading(tr.text("prefs-title"));
                },
                |ui| {
                    if flare_ui::chrome::icon_button(
                        ui,
                        "close",
                        14.0,
                        false,
                        true,
                        &tr.text("close"),
                    )
                    .clicked()
                    {
                        ui.close();
                    }
                },
            );
            ui.add_space(4.0);
            // A language's name in itself, in the font that draws it whatever the interface speaks.
            let native_name = |language: flare_i18n::Language| {
                let text = egui::RichText::new(language.native_name());
                match flare_ui::theme::cjk_family(language.code()) {
                    Some(family) => text.family(family),
                    None => text,
                }
            };
            // A label and its control on one line; the label is as wide as the language needs.
            let labelled = |ui: &mut egui::Ui, key: &str, add: &mut dyn FnMut(&mut egui::Ui)| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(tr.text(key)).color(flare_ui::theme::text_dim()));
                    add(ui);
                });
            };
            flare_ui::chrome::group(ui, "settings", &tr.text("prefs-general"), |ui| {
                labelled(ui, "prefs-language", &mut |ui| {
                    let shown = match flare_i18n::Choice::from_code(&edited.language) {
                        flare_i18n::Choice::System => egui::RichText::new(system_label.clone()),
                        flare_i18n::Choice::Language(l) => native_name(l),
                    };
                    egui::ComboBox::from_id_salt("language")
                        .selected_text(shown)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut edited.language,
                                "system".to_owned(),
                                system_label.clone(),
                            );
                            for language in flare_i18n::Language::ALL {
                                ui.selectable_value(
                                    &mut edited.language,
                                    language.code().to_owned(),
                                    native_name(language),
                                );
                            }
                        });
                });
                labelled(ui, "prefs-theme", &mut |ui| {
                    for (code, key) in
                        [("dark", "prefs-theme-dark"), ("light", "prefs-theme-light")]
                    {
                        ui.selectable_value(&mut edited.theme, code.to_owned(), tr.text(key));
                    }
                });
                labelled(ui, "prefs-autosave", &mut |ui| {
                    ui.add(
                        egui::DragValue::new(&mut edited.autosave_seconds)
                            .range(5.0..=3600.0)
                            .suffix(" s"),
                    );
                });
            });
            flare_ui::chrome::group(ui, "grid", &tr.text("prefs-stage"), |ui| {
                ui.checkbox(&mut edited.show_rulers, tr.text("prefs-show-rulers"));
                ui.checkbox(&mut edited.show_grid, tr.text("prefs-show-grid"));
                ui.checkbox(&mut edited.snap_to_grid, tr.text("prefs-snap-to-grid"));
                labelled(ui, "prefs-grid-size", &mut |ui| {
                    ui.add(
                        egui::DragValue::new(&mut edited.grid_size)
                            .range(1.0..=500.0)
                            .suffix(" px"),
                    );
                });
            });
            flare_ui::chrome::group(ui, "transport-loop", &tr.text("prefs-timeline"), |ui| {
                ui.checkbox(&mut edited.loop_playback, tr.text("prefs-loop-playback"));
                labelled(ui, "prefs-line-smoothing", &mut |ui| {
                    ui.add(
                        egui::DragValue::new(&mut edited.smoothing)
                            .range(0.0..=20.0)
                            .speed(0.1),
                    );
                });
            });
            flare_ui::chrome::group(ui, "file-open", &tr.text("prefs-recent"), |ui| {
                labelled(ui, "prefs-remembered", &mut |ui| {
                    ui.label(edited.recent.len().to_string());
                });
                let forget = ui.add_enabled(
                    !edited.recent.is_empty(),
                    egui::Button::new(tr.text("prefs-clear-recent")),
                );
                if forget
                    .on_hover_text(tr.text("prefs-clear-recent-hint"))
                    .clicked()
                {
                    edited.recent.clear();
                }
            });
            ui.add_space(10.0);
            // Restore defaults is the one thing here that throws work away, so it is red; Close is
            // at the bottom right, where a dialog's way out is.
            // One row of a fixed height, so the two ends are centered on the same line.
            let row = egui::vec2(ui.available_width(), ui.spacing().interact_size.y + 6.0);
            ui.allocate_ui_with_layout(
                row,
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let restore = egui::Button::new(
                        egui::RichText::new(tr.text("prefs-restore-defaults"))
                            .color(egui::Color32::WHITE),
                    )
                    .fill(flare_ui::theme::danger());
                    if ui.add(restore).clicked() {
                        edited = Preferences {
                            recent: edited.recent.clone(),
                            ..Preferences::default()
                        };
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(tr.text("close")).clicked() {
                            ui.close();
                        }
                    });
                },
            );
        });
        self.apply_preferences(&edited);
        self.windows.preferences = !response.should_close();
    }
}
