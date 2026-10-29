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
    /// Compress SWF files when publishing.
    pub publish_compress: bool,
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
            publish_compress: true,
            recent: Vec::new(),
        }
    }
}

impl Workspace {
    /// The current settings.
    pub fn preferences(&self) -> Preferences {
        Preferences {
            autosave_seconds: self.autosave_interval,
            show_rulers: self.show_rulers,
            show_grid: self.show_grid,
            snap_to_grid: self.snap_to_grid,
            grid_size: self.grid_size,
            loop_playback: self.loop_playback,
            smoothing: self.smoothing,
            publish_compress: self.publish_compress,
            recent: self
                .recent
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
        }
    }

    /// Takes `preferences` as the current settings, keeping each in its sensible range.
    pub fn apply_preferences(&mut self, preferences: &Preferences) {
        let finite = |v: f64, fallback: f64| if v.is_finite() { v } else { fallback };
        self.autosave_interval = finite(preferences.autosave_seconds, 30.0).clamp(5.0, 3600.0);
        self.show_rulers = preferences.show_rulers;
        self.show_grid = preferences.show_grid;
        self.snap_to_grid = preferences.snap_to_grid;
        self.grid_size = finite(f64::from(preferences.grid_size), 20.0).clamp(1.0, 500.0) as f32;
        self.loop_playback = preferences.loop_playback;
        self.smoothing = finite(f64::from(preferences.smoothing), 2.0).clamp(0.0, 20.0) as f32;
        self.publish_compress = preferences.publish_compress;
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
        self.publish_compress
    }

    /// The small windows opened from the File menu: recent projects and publish settings.
    pub(super) fn file_windows(&mut self, ctx: &egui::Context) {
        if self.recent_open {
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
            self.recent_open = open && chosen.is_none();
            if let Some(path) = chosen {
                self.open(&path);
            }
        }
        if self.publish_settings_open {
            let mut open = true;
            egui::Window::new("Publish Settings")
                .open(&mut open)
                .collapsible(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.checkbox(&mut self.publish_compress, "Compress the SWF");
                    ui.weak(
                        "A compressed file is smaller; players older than Flash 6 cannot read it.",
                    );
                });
            self.publish_settings_open = open;
        }
    }

    /// The Preferences window, while it is open.
    pub(super) fn preferences_window(&mut self, ctx: &egui::Context) {
        if !self.preferences_open {
            return;
        }
        let mut open = true;
        let mut edited = self.preferences();
        egui::Window::new("Preferences")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .show(ctx, |ui| {
                flare_ui::chrome::section(ui, "settings", "General");
                flare_ui::chrome::row(ui, "Autosave every", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut edited.autosave_seconds)
                            .range(5.0..=3600.0)
                            .suffix(" s"),
                    );
                });
                flare_ui::chrome::section(ui, "grid", "Stage");
                ui.checkbox(&mut edited.show_rulers, "Show rulers");
                ui.checkbox(&mut edited.show_grid, "Show grid");
                ui.checkbox(&mut edited.snap_to_grid, "Snap to grid");
                flare_ui::chrome::row(ui, "Grid size", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut edited.grid_size)
                            .range(1.0..=500.0)
                            .suffix(" px"),
                    );
                });
                flare_ui::chrome::section(ui, "transport-loop", "Timeline and drawing");
                ui.checkbox(&mut edited.loop_playback, "Loop playback");
                flare_ui::chrome::row(ui, "Line smoothing", |ui| {
                    ui.add(
                        egui::DragValue::new(&mut edited.smoothing)
                            .range(0.0..=20.0)
                            .speed(0.1),
                    );
                });
                ui.add_space(6.0);
                if ui.button("Restore defaults").clicked() {
                    edited = Preferences::default();
                }
            });
        self.apply_preferences(&edited);
        self.preferences_open = open;
    }
}
