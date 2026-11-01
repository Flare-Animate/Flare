//! Settings that outlive a document: what the Preferences window edits and the desktop saves.

use serde::{Deserialize, Serialize};

use std::path::PathBuf;

use super::*;

/// Largest preferences file accepted.
pub const MAX_PREFERENCES_BYTES: usize = 16 * 1024;

/// The settings a person keeps from one session to the next. Every field has a default, and a
/// value out of range is brought into range when applied, so an old or edited file cannot break
/// the workspace.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    /// Seconds between crash-recovery saves.
    pub autosave_seconds: f64,
    pub show_rulers: bool,
    pub show_grid: bool,
    pub snap_to_grid: bool,
    /// Grid spacing in stage units.
    pub grid_size: f32,
    pub loop_playback: bool,
    /// How far a smoothed pencil or brush line may stray, in screen pixels.
    pub smoothing: f32,
    /// The brush: its size and nib, so a session starts as the last one ended.
    pub brush_size: f32,
    pub brush_nib_square: bool,
    pub brush_nib_angle: f32,
    pub brush_nib_flatness: f32,
    /// Custom brushes, and the one in hand.
    pub brushes: Vec<BrushPreset>,
    pub brush_selected: Option<usize>,
    /// Compress SWF files when publishing.
    pub publish_compress: bool,
    /// Publish a Flash movie, and an HTML5 Canvas page, and how the page behaves.
    pub publish_swf: bool,
    pub publish_html5: bool,
    pub html5_loop: bool,
    pub html5_center: bool,
    pub html5_responsive: bool,
    pub publish_gif: bool,
    pub publish_mp4: bool,
    /// The interface's language: `system`, or a language's tag such as `de` or `pt-BR`.
    pub language: String,
    /// The look of the interface: `dark` or `light`.
    pub theme: String,
    /// Tools from the "…" list kept in the tool strip.
    pub pinned_tools: Vec<String>,
    /// Projects opened or saved lately, newest first.
    pub recent: Vec<String>,
}

/// How many recent projects are remembered.
pub const MAX_RECENT: usize = 10;

impl Default for Preferences {
    fn default() -> Self {
        Self {
            autosave_seconds: 30.0,
            show_rulers: true,
            show_grid: false,
            snap_to_grid: false,
            grid_size: 20.0,
            loop_playback: true,
            smoothing: 2.0,
            brush_size: 12.0,
            brush_nib_square: false,
            brush_nib_angle: 0.0,
            brush_nib_flatness: 100.0,
            brushes: Vec::new(),
            brush_selected: None,
            publish_compress: true,
            publish_swf: true,
            publish_html5: true,
            html5_loop: true,
            html5_center: true,
            html5_responsive: false,
            publish_gif: false,
            publish_mp4: false,
            language: "system".to_owned(),
            theme: "dark".to_owned(),
            pinned_tools: Vec::new(),
            recent: Vec::new(),
        }
    }
}

impl Workspace {
    /// The current settings.
    pub fn preferences(&self) -> Preferences {
        Preferences {
            autosave_seconds: self.recovery.interval,
            show_rulers: self.display.show_rulers,
            show_grid: self.display.show_grid,
            snap_to_grid: self.display.snap_to_grid,
            grid_size: self.display.grid_size,
            loop_playback: self.loop_playback,
            smoothing: self.smoothing,
            brush_size: self.brush_size,
            brush_nib_square: self.brush_shelf.nib().shape()
                == flare_tools::brush::NibShape::Square,
            brush_nib_angle: self.brush_shelf.nib().angle_degrees() as f32,
            brush_nib_flatness: self.brush_shelf.nib().flatness_percent() as f32,
            brushes: self.brush_shelf.presets().to_vec(),
            brush_selected: self.selected_brush(),
            publish_compress: self.publish.compress,
            publish_swf: self.publish.swf,
            publish_html5: self.publish.html5,
            html5_loop: self.publish.loop_timeline,
            html5_center: self.publish.center_stage,
            html5_responsive: self.publish.responsive,
            publish_gif: self.publish.gif,
            publish_mp4: self.publish.mp4,
            language: self.language_choice.code().to_owned(),
            theme: self.theme_mode.code().to_owned(),
            pinned_tools: self.pinned_tools.iter().map(|n| (*n).to_owned()).collect(),
            recent: self
                .recent
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        }
    }

    /// Takes `preferences` as the current settings, keeping each in its sensible range.
    pub fn apply_preferences(&mut self, preferences: &Preferences) {
        self.set_language_choice(flare_i18n::Choice::from_code(&preferences.language));
        self.theme_mode = flare_ui::theme::Mode::from_code(&preferences.theme);
        // Only names the list has, each once; a file naming anything else changes nothing.
        self.pinned_tools.clear();
        for name in &preferences.pinned_tools {
            self.pin_tool(name);
        }
        let finite = |v: f64, fallback: f64| if v.is_finite() { v } else { fallback };
        self.recovery.interval = finite(preferences.autosave_seconds, 30.0).clamp(5.0, 3600.0);
        self.display.show_rulers = preferences.show_rulers;
        self.display.show_grid = preferences.show_grid;
        self.display.snap_to_grid = preferences.snap_to_grid;
        self.display.grid_size =
            finite(f64::from(preferences.grid_size), 20.0).clamp(1.0, 500.0) as f32;
        self.loop_playback = preferences.loop_playback;
        self.smoothing = finite(f64::from(preferences.smoothing), 2.0).clamp(0.0, 20.0) as f32;
        self.brush_size = finite(f64::from(preferences.brush_size), 12.0).clamp(1.0, 200.0) as f32;
        let loose = flare_tools::brush::Nib::new(
            if preferences.brush_nib_square {
                flare_tools::brush::NibShape::Square
            } else {
                flare_tools::brush::NibShape::Round
            },
            f64::from(preferences.brush_nib_angle),
            f64::from(preferences.brush_nib_flatness),
        );
        self.brush_shelf = flare_tools::brush::BrushShelf::restore(
            loose,
            &preferences.brushes,
            preferences.brush_selected,
        );
        self.publish = PublishSettings {
            swf: preferences.publish_swf,
            compress: preferences.publish_compress,
            html5: preferences.publish_html5,
            loop_timeline: preferences.html5_loop,
            center_stage: preferences.html5_center,
            responsive: preferences.html5_responsive,
            gif: preferences.publish_gif,
            mp4: preferences.publish_mp4,
        };
        self.recent = preferences
            .recent
            .iter()
            .filter(|p| !p.is_empty())
            .take(MAX_RECENT)
            .map(PathBuf::from)
            .collect();
    }

    /// The settings as text to keep between sessions.
    pub fn preferences_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.preferences())
    }

    /// Restores saved settings. Text that is too long or not understood is refused and the
    /// current settings stay.
    pub fn restore_preferences(&mut self, json: &str) -> Result<(), String> {
        if json.len() > MAX_PREFERENCES_BYTES {
            return Err("the preferences file is too large".to_owned());
        }
        let preferences: Preferences = serde_json::from_str(json).map_err(|e| e.to_string())?;
        self.apply_preferences(&preferences);
        Ok(())
    }

    /// Remembers `path` as the most recently used project.
    pub(super) fn note_recent(&mut self, path: &std::path::Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_owned());
        self.recent.truncate(MAX_RECENT);
    }

    /// Projects opened or saved lately, newest first.
    pub fn recent_files(&self) -> &[PathBuf] {
        &self.recent
    }

    /// Whether published SWF files are compressed.
    pub fn publish_compress(&self) -> bool {
        self.publish.compress
    }

    /// The small windows opened from the File menu: recent projects and publish settings.
    pub(super) fn file_windows(&mut self, ctx: &egui::Context) {
        if self.windows.recent {
            let mut open = true;
            let mut chosen = None;
            egui::Window::new("Open Recent")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    if self.recent.is_empty() {
                        ui.weak("Nothing opened or saved yet.");
                    }
                    for path in &self.recent {
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
                    if !self.recent.is_empty() && ui.small_button("Clear the list").clicked() {
                        self.recent.clear();
                    }
                });
            self.windows.recent = open && chosen.is_none();
            if let Some(path) = chosen {
                self.open(&path);
            }
        }
        if self.windows.publish_settings {
            let mut open = true;
            egui::Window::new("Publish Settings")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    self.publish_settings_ui(ui);
                    ui.weak("File > Publish writes each format that is ticked, side by side.");
                });
            self.windows.publish_settings = open;
        }
    }

    /// The Preferences window, while it is open.
    pub(super) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.windows.preferences {
            return;
        }
        let mut edited = self.preferences();
        let tr = &self.translator;
        let system_label = self.system_choice_label();
        // A dialog in the middle of the window over a dimmed backdrop, as PaintCraft's are:
        // Escape or a click outside closes it.
        let response = egui::Modal::new(self.preferences_id).show(ctx, |ui| {
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
