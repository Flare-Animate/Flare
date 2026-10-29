//! Cohesive parts of the workspace's state, each with its own defaults, so the workspace itself
//! holds a handful of concerns instead of every setting as a field of its own.

use egui::{Color32, Pos2};
use flare_core::SoundId;
use flare_document::{Cap, Join};
use flare_tools::symmetry::Symmetry;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use super::{AudioEvent, PendingText, guides};

/// The Text tool: how new text is made, and the text being typed or turned into outlines.
pub(super) struct TextTool {
    pub(super) family: String,
    pub(super) size: f32,
    pub(super) bold: bool,
    pub(super) italic: bool,
    /// Text being typed on the stage.
    pub(super) edit: Option<PendingText>,
    /// Text handed to the font worker: its job and where it goes.
    pub(super) pending: Option<(u64, Pos2)>,
    pub(super) editor_id: egui::Id,
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            family: "sans-serif".into(),
            size: 48.0,
            bold: false,
            italic: false,
            edit: None,
            pending: None,
            editor_id: egui::Id::unique("flare_text_editor"),
        }
    }
}

/// How new shapes are painted: the fill, an optional gradient, and the outline.
pub(super) struct Paint {
    pub(super) fill: Color32,
    pub(super) gradient_enabled: bool,
    pub(super) gradient_end: Color32,
    pub(super) gradient_vertical: bool,
    pub(super) stroke_enabled: bool,
    pub(super) stroke_color: Color32,
    pub(super) stroke_width: f32,
    pub(super) stroke_cap: Cap,
    pub(super) stroke_join: Join,
    /// Pencil and brush strokes are mirrored across these axes.
    pub(super) symmetry: Symmetry,
    /// Where the axes cross; the middle of the stage until one is set.
    pub(super) symmetry_center: Option<Pos2>,
    /// Pencil and brush strokes are straightened to a line between their ends.
    pub(super) straight_edge: bool,
    /// Shift is held, which straightens a stroke for as long as it is.
    pub(super) straight_held: bool,
    /// The stroke just committed, shown until the stage picture that has it arrives.
    pub(super) settling: Option<super::stroke_preview::Settling>,
}

impl Default for Paint {
    fn default() -> Self {
        Self {
            fill: Color32::from_rgb(232, 69, 60),
            gradient_enabled: false,
            gradient_end: Color32::from_rgb(60, 90, 220),
            gradient_vertical: false,
            stroke_enabled: false,
            stroke_color: Color32::BLACK,
            stroke_width: 4.0,
            stroke_cap: Cap::Round,
            stroke_join: Join::Round,
            symmetry: Symmetry::Off,
            symmetry_center: None,
            straight_edge: false,
            straight_held: false,
            settling: None,
        }
    }
}

/// What the stage view shows besides the artwork.
pub(super) struct Display {
    pub(super) show_rulers: bool,
    pub(super) show_grid: bool,
    /// Points drawn and moves are rounded to the grid.
    pub(super) snap_to_grid: bool,
    /// Grid spacing in stage units.
    pub(super) grid_size: f32,
    /// Guides pulled out of the rulers, and whether they show.
    pub(super) guides: Vec<guides::Guide>,
    pub(super) show_guides: bool,
    /// The guide being dragged, by index.
    pub(super) guide_drag: Option<usize>,
    /// The stage draws outlines in layer colors instead of the artwork.
    pub(super) outline_mode: bool,
    /// What the Ruler tool last measured: from one stage point to another.
    pub(super) measure: Option<(Pos2, Pos2)>,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            show_rulers: true,
            show_grid: false,
            snap_to_grid: false,
            grid_size: 20.0,
            guides: Vec::new(),
            show_guides: true,
            guide_drag: None,
            outline_mode: false,
            measure: None,
        }
    }
}

/// Which small windows are open.
#[derive(Default)]
pub(super) struct OpenWindows {
    pub(super) preferences: bool,
    pub(super) recent: bool,
    pub(super) about: bool,
    pub(super) shortcuts: bool,
    pub(super) publish_settings: bool,
}

/// Autosave and crash recovery of the document on show.
pub(super) struct Recovery {
    /// Where recovery files go; `None` turns autosave off.
    pub(super) dir: Option<PathBuf>,
    /// Seconds between autosaves.
    pub(super) interval: f64,
    pub(super) last: Option<f64>,
    /// A recovery file for the current changes exists.
    pub(super) written: bool,
    /// The autosave being written, by job number.
    pub(super) in_flight: Option<u64>,
}

impl Default for Recovery {
    fn default() -> Self {
        Self {
            dir: None,
            interval: 30.0,
            last: None,
            written: false,
            in_flight: None,
        }
    }
}

/// Sounds: what has been decoded and what is being worked on, and what the sound card is owed.
#[derive(Default)]
pub(super) struct AudioState {
    pub(super) decoded: HashMap<SoundId, Arc<flare_audio::Audio>>,
    pub(super) decoding: HashSet<SoundId>,
    pub(super) decode_failed: HashSet<SoundId>,
    pub(super) events: Vec<AudioEvent>,
    pub(super) importing: Option<u64>,
    pub(super) synced: Option<(bool, u32)>,
}
