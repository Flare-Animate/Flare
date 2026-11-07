//! The settings a person keeps from one session to the next.

use serde::{Deserialize, Serialize};

use crate::workspace::*;

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
    pub brush_pressure_min: f32,
    pub brush_pressure_gamma: f32,
    pub brush_builtin_profile: Option<flare_tools::brush::BuiltinBrushProfile>,
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
    /// The keyboard shortcuts as a `flare_menubar` shortcut file; empty is the Flare set.
    pub shortcuts: String,
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
            brush_pressure_min: 0.15,
            brush_pressure_gamma: 1.0,
            brush_builtin_profile: None,
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
            shortcuts: String::new(),
            recent: Vec::new(),
        }
    }
}
