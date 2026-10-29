//! Settings that outlive a document: what the Preferences window edits and the desktop saves.

use serde::{Deserialize, Serialize};

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
}

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
