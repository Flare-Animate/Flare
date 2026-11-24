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
    /// Grid spacing in stage units: across, and down when it differs (files from before the two
    /// were apart have one spacing for both).
    pub grid_size: f32,
    pub grid_size_down: Option<f32>,
    /// The grid's color as red, green, blue and alpha; `None` follows the stage's background.
    pub grid_color: Option<[u8; 4]>,
    pub loop_playback: bool,
    /// How wide the timeline's column of layer names is, in points.
    pub timeline_names_width: f32,
    /// Onion skin: how many frames before and after it shows, as outlines or pictures, and
    /// whether they are tinted, in which colors (red, green, blue). Whether it is on is not kept:
    /// a session starts with it off, as Animate's does.
    pub onion_before: u32,
    pub onion_after: u32,
    pub onion_outlines: bool,
    pub onion_tinted: bool,
    pub onion_before_color: [u8; 3],
    pub onion_after_color: [u8; 3],
    /// How far a smoothed pencil or brush line may stray, in screen pixels.
    pub smoothing: f32,
    /// The brush: its size and nib, so a session starts as the last one ended.
    pub brush_size: f32,
    pub brush_nib_square: bool,
    pub brush_nib_angle: f32,
    pub brush_nib_flatness: f32,
    pub brush_pressure_min: f32,
    pub brush_pressure_gamma: f32,
    /// The stabilizer level and taper lengths in hand.
    pub brush_stabilizer: u8,
    pub brush_taper_start: f32,
    pub brush_taper_end: f32,
    /// The strands and grain of the brush in hand.
    pub brush_bristles: u8,
    pub brush_grain: f32,
    /// Object Drawing on the drawing tools; off is Merge Drawing. Files from before it read as
    /// on, which is how everything was drawn then.
    pub object_drawing: bool,
    /// Whether the Pencil follows a pen's pressure, and its lightest touch.
    pub pencil_pressure: bool,
    pub pencil_light: f32,
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
    pub publish_mov: bool,
    /// Export and publish with a transparent background instead of the stage colour.
    pub publish_transparent: bool,
    /// Which defaults the format choices above were saved under (`PUBLISH_DEFAULTS`). A file
    /// without it is from before, so it reads as 0 and not as the current value.
    #[serde(default)]
    pub publish_defaults: u32,
    /// The interface's language: `system`, or a language's tag such as `de` or `pt-BR`.
    pub language: String,
    /// The look of the interface, by a theme's code (`flare_ui::theme::Mode::code`).
    pub theme: String,
    /// How large the interface is drawn, as a factor of its design size (`flare_ui::scale`).
    pub interface_size: f32,
    /// Tools from the "…" list kept in the tool strip.
    pub pinned_tools: Vec<String>,
    /// The keyboard shortcuts as a `flare_menubar` shortcut file; empty is the Flare set.
    pub shortcuts: String,
    /// Projects opened or saved lately, newest first.
    pub recent: Vec<String>,
    /// The person's own swatches, as `[r, g, b, a]`.
    pub swatches: Vec<[u8; 4]>,
    /// The reference mannequin as it was last posed (`flare_mannequin`'s figure JSON); empty is
    /// the default figure.
    pub mannequin: String,
}

fn rgb(color: flare_core::Rgba8) -> [u8; 3] {
    [color.r, color.g, color.b]
}

/// How many recent projects are remembered.
pub const MAX_RECENT: usize = 10;

impl Default for Preferences {
    fn default() -> Self {
        let publish = PublishSettings::default();
        Self {
            autosave_seconds: 30.0,
            show_rulers: true,
            show_grid: false,
            snap_to_grid: false,
            grid_size: 20.0,
            grid_size_down: None,
            grid_color: None,
            loop_playback: true,
            timeline_names_width: crate::workspace::timeline::LABEL_W,
            onion_before: 1,
            onion_after: 1,
            onion_outlines: false,
            onion_tinted: true,
            onion_before_color: rgb(ONION_BEFORE_COLOR),
            onion_after_color: rgb(ONION_AFTER_COLOR),
            smoothing: 2.0,
            brush_size: 12.0,
            brush_nib_square: false,
            brush_nib_angle: 0.0,
            brush_nib_flatness: 100.0,
            brush_pressure_min: 0.15,
            brush_pressure_gamma: 1.0,
            brush_stabilizer: 0,
            brush_taper_start: 0.0,
            brush_taper_end: 0.0,
            brush_bristles: 0,
            brush_grain: 0.0,
            object_drawing: true,
            pencil_pressure: true,
            pencil_light: flare_tools::pencil::DEFAULT_PENCIL_LIGHT,
            brush_builtin_profile: None,
            brushes: Vec::new(),
            brush_selected: None,
            publish_compress: publish.compress,
            publish_swf: publish.swf,
            publish_html5: publish.html5,
            html5_loop: publish.loop_timeline,
            html5_center: publish.center_stage,
            html5_responsive: publish.responsive,
            publish_gif: publish.gif,
            publish_mp4: publish.mp4,
            publish_mov: publish.mov,
            publish_transparent: publish.transparent,
            publish_defaults: crate::workspace::publish_settings::PUBLISH_DEFAULTS,
            language: "system".to_owned(),
            theme: "dark".to_owned(),
            interface_size: flare_ui::scale::DEFAULT,
            pinned_tools: Vec::new(),
            shortcuts: String::new(),
            recent: Vec::new(),
            swatches: Vec::new(),
            mannequin: String::new(),
        }
    }
}
