//! Reading the settings off the workspace, and applying saved ones to it.

use std::path::PathBuf;

use crate::workspace::*;

use super::settings::MAX_RECENT;

impl Workspace {
    /// The current settings.
    pub fn preferences(&self) -> Preferences {
        Preferences {
            autosave_seconds: self.recovery.interval,
            show_rulers: self.display.show_rulers,
            show_grid: self.display.show_grid,
            snap_to_grid: self.display.snap_to_grid,
            grid_size: self.display.grid_size.x,
            grid_size_down: Some(self.display.grid_size.y),
            grid_color: self.display.grid_color.map(|c| c.to_srgba_unmultiplied()),
            loop_playback: self.looping.on,
            timeline_names_width: self.display.timeline_names,
            onion_before: self.onion.before,
            onion_after: self.onion.after,
            onion_outlines: self.onion.outlines,
            onion_tinted: self.onion.tinted,
            onion_before_color: [
                self.onion.before_color.r,
                self.onion.before_color.g,
                self.onion.before_color.b,
            ],
            onion_after_color: [
                self.onion.after_color.r,
                self.onion.after_color.g,
                self.onion.after_color.b,
            ],
            smoothing: self.smoothing,
            brush_size: self.brush_size,
            brush_nib_square: self.brush_shelf.nib().shape()
                == flare_tools::brush::NibShape::Square,
            brush_nib_angle: self.brush_shelf.nib().angle_degrees() as f32,
            brush_nib_flatness: self.brush_shelf.nib().flatness_percent() as f32,
            brush_pressure_min: self.brush_light,
            brush_pressure_gamma: self.brush_pressure_gamma,
            brush_stabilizer: self.brush_shelf.stabilizer(),
            brush_taper_start: self.brush_shelf.taper().start() as f32,
            brush_taper_end: self.brush_shelf.taper().end() as f32,
            brush_bristles: self.brush_shelf.texture().bristles(),
            brush_grain: self.brush_shelf.texture().grain(),
            pencil_pressure: self.pencil_pressure.is_on(),
            pencil_light: self.pencil_pressure.light(),
            brush_builtin_profile: self.brush_shelf.builtin(),
            brushes: self.brush_shelf.presets().to_vec(),
            brush_selected: self.brush_shelf.selected(),
            publish_compress: self.publish.compress,
            publish_swf: self.publish.swf,
            publish_html5: self.publish.html5,
            html5_loop: self.publish.loop_timeline,
            html5_center: self.publish.center_stage,
            html5_responsive: self.publish.responsive,
            publish_gif: self.publish.gif,
            publish_mp4: self.publish.mp4,
            publish_mov: self.publish.mov,
            publish_defaults: crate::workspace::publish_settings::PUBLISH_DEFAULTS,
            language: self.language_choice.code().to_owned(),
            theme: self.theme_mode.code().to_owned(),
            pinned_tools: self.pinned_tools.iter().map(|n| (*n).to_owned()).collect(),
            shortcuts: if self.shortcuts == flare_menubar::ShortcutMap::default() {
                String::new()
            } else {
                self.shortcuts.to_json()
            },
            recent: self
                .home
                .recent
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            swatches: self
                .paint
                .swatches
                .colors()
                .iter()
                .map(|c| [c.r, c.g, c.b, c.a])
                .collect(),
            mannequin: self.mannequin_preferences(),
        }
    }

    /// Takes `preferences` as the current settings, keeping each in its sensible range.
    pub fn apply_preferences(&mut self, preferences: &Preferences) {
        self.set_language_choice(flare_i18n::Choice::from_code(&preferences.language));
        self.theme_mode = flare_ui::theme::Mode::from_code(&preferences.theme);
        // Only names the list has, each once; a file naming anything else changes nothing.
        self.paint.swatches = flare_core::swatches::CustomSwatches::from_colors(
            preferences
                .swatches
                .iter()
                .map(|[r, g, b, a]| flare_core::Rgba8::new(*r, *g, *b, *a)),
        );
        if !preferences.mannequin.is_empty() {
            self.apply_mannequin_preferences(&preferences.mannequin);
        }
        self.pinned_tools.clear();
        for name in &preferences.pinned_tools {
            self.pin_tool(name);
        }
        // A file that cannot be used gives the default set, so a person is never left keyless.
        self.shortcuts = if preferences.shortcuts.is_empty() {
            flare_menubar::ShortcutMap::default()
        } else {
            flare_menubar::ShortcutMap::from_json_or_default(&preferences.shortcuts).0
        };
        let finite = |v: f64, fallback: f64| if v.is_finite() { v } else { fallback };
        self.recovery.interval = finite(preferences.autosave_seconds, 30.0).clamp(5.0, 3600.0);
        self.display.show_rulers = preferences.show_rulers;
        self.display.show_grid = preferences.show_grid;
        self.display.snap_to_grid = preferences.snap_to_grid;
        self.set_grid_spacing(
            preferences.grid_size,
            preferences.grid_size_down.unwrap_or(preferences.grid_size),
        );
        self.set_grid_color(
            preferences
                .grid_color
                .map(|[r, g, b, a]| egui::Color32::from_rgba_unmultiplied(r, g, b, a)),
        );
        self.looping.on = preferences.loop_playback;
        self.display.timeline_names =
            super::super::timeline_names_edge::names_width(preferences.timeline_names_width);
        self.set_onion_range(preferences.onion_before, preferences.onion_after);
        self.set_onion_outlines(preferences.onion_outlines);
        self.set_onion_tinted(preferences.onion_tinted);
        let [r, g, b] = preferences.onion_before_color;
        let [r2, g2, b2] = preferences.onion_after_color;
        self.set_onion_colors(
            flare_core::Rgba8::opaque(r, g, b),
            flare_core::Rgba8::opaque(r2, g2, b2),
        );
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
        self.brush_shelf.set_pressure_curve(
            finite(f64::from(preferences.brush_pressure_min), 0.15) as f32,
            finite(f64::from(preferences.brush_pressure_gamma), 1.0) as f32,
        );
        // The shelf keeps both in range, so a file with odd numbers cannot break the brush.
        self.brush_shelf
            .set_stabilizer(preferences.brush_stabilizer);
        self.brush_shelf.set_taper(
            f64::from(preferences.brush_taper_start),
            f64::from(preferences.brush_taper_end),
        );
        self.brush_shelf
            .set_texture(preferences.brush_bristles, preferences.brush_grain);
        self.pencil_pressure = flare_tools::pencil::PencilPressure::new(
            preferences.pencil_pressure,
            preferences.pencil_light,
        );
        if preferences.brush_selected.is_none()
            && let Some(profile) = preferences.brush_builtin_profile
        {
            self.brush_shelf.choose_builtin(profile);
        }
        self.brush_light = self.brush_shelf.pressure_min();
        self.brush_pressure_gamma = self.brush_shelf.pressure_gamma();
        self.publish = PublishSettings {
            swf: preferences.publish_swf,
            compress: preferences.publish_compress,
            html5: preferences.publish_html5,
            loop_timeline: preferences.html5_loop,
            center_stage: preferences.html5_center,
            responsive: preferences.html5_responsive,
            gif: preferences.publish_gif,
            mp4: preferences.publish_mp4,
            mov: preferences.publish_mov,
        }
        .update_old_defaults(preferences.publish_defaults);
        self.home.recent = preferences
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
    pub(in crate::workspace) fn note_recent(&mut self, path: &std::path::Path) {
        self.home.recent.retain(|p| p != path);
        self.home.recent.insert(0, path.to_owned());
        self.home.recent.truncate(MAX_RECENT);
    }

    /// Projects opened or saved lately, newest first.
    pub fn recent_files(&self) -> &[PathBuf] {
        &self.home.recent
    }

    /// Whether published SWF files are compressed.
    pub fn publish_compress(&self) -> bool {
        self.publish.compress
    }
}
