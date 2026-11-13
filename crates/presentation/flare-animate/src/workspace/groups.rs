//! Cohesive parts of the workspace's state, each with its own defaults, so the workspace itself
//! holds a handful of concerns instead of every setting as a field of its own.

use egui::{Color32, Pos2};
use flare_core::{SoundId, SymbolId};
use flare_document::{Cap, Join};
use flare_menubar::MenuAction;
use flare_tools::mirror::Symmetry;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use super::{AudioEvent, PendingText, TextTarget, guides};

/// The Text tool: how new text is made, and the text being typed or turned into outlines.
pub(super) struct TextTool {
    pub(super) family: String,
    pub(super) size: f32,
    pub(super) bold: bool,
    pub(super) italic: bool,
    /// Text being typed on the stage.
    pub(super) edit: Option<PendingText>,
    /// Text handed to the font worker: its job and where it goes.
    pub(super) pending: Option<(u64, TextTarget)>,
    pub(super) editor_id: egui::Id,
    /// The font families there are, once the worker has listed them.
    pub(super) families: Vec<String>,
    /// The list was asked for, so it is not asked for again.
    pub(super) families_asked: bool,
    /// Typed in the family list's filter box.
    pub(super) family_filter: String,
    /// A size being dragged or typed for the selected text, applied when it is let go.
    pub(super) size_edit: Option<f32>,
    /// The text as typed so far, shown on the stage in place of the stored letters.
    pub(super) preview: Option<super::text_preview::TextPreview>,
    /// What the preview was last asked for, and the request in flight.
    pub(super) preview_asked: Option<super::text_preview::PreviewAsk>,
    pub(super) preview_pending: Option<u64>,
    /// Counts the changes of the preview, so the stage is drawn again for each.
    pub(super) preview_epoch: u64,
    /// The identity of the previewed letters of new text.
    pub(super) preview_shape: flare_core::ShapeId,
}

impl Default for TextTool {
    fn default() -> Self {
        Self {
            family: flare_text::DEFAULT_FAMILY.into(),
            size: 48.0,
            bold: false,
            italic: false,
            edit: None,
            pending: None,
            editor_id: egui::Id::unique("flare_text_editor"),
            families: Vec::new(),
            families_asked: false,
            family_filter: String::new(),
            size_edit: None,
            preview: None,
            preview_asked: None,
            preview_pending: None,
            preview_epoch: 0,
            preview_shape: flare_core::ShapeId::new(),
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
    pub(super) stroke_style: flare_document::StrokeStyle,
    /// How wide a gap in the lines the Paint Bucket fills across.
    pub(super) gap: flare_tools::paint::GapSize,
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
    /// The person's own swatches, kept with the preferences.
    pub(super) swatches: flare_core::swatches::CustomSwatches,
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
            stroke_style: flare_document::StrokeStyle::Solid,
            gap: flare_tools::paint::GapSize::default(),
            symmetry: Symmetry::Off,
            symmetry_center: None,
            straight_edge: false,
            straight_held: false,
            settling: None,
            swatches: flare_core::swatches::CustomSwatches::default(),
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
    /// Artwork off the stage shows on the pasteboard around it, as Animate's View > Pasteboard;
    /// off, the view is cut to the stage, as the published movie is.
    pub(super) show_pasteboard: bool,
    /// What the Ruler tool last measured: from one stage point to another.
    pub(super) measure: Option<(Pos2, Pos2)>,
    /// How wide the timeline's column of layer names is, in points.
    pub(super) timeline_names: f32,
    /// Edit Multiple Frames: whether it is on and the range its bracket marks.
    pub(super) multi_frame: super::timeline_labels::MultiFrames,
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
            show_pasteboard: true,
            measure: None,
            timeline_names: super::timeline::LABEL_W,
            multi_frame: Default::default(),
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
    /// The Publish popup: formats to write, and where to share them.
    pub(super) publish: bool,
    /// The Home page covers the dock.
    pub(super) home: bool,
    /// The Increment Instances popup: the symbol to act on, and the two numbers.
    pub(super) increment_instances: Option<IncrementInstances>,
    /// The Create New Symbol or Convert to Symbol dialog.
    pub(super) symbol: Option<super::symbol_dialog::SymbolDialog>,
    /// The start-up splash: the egui time it appeared (`None` until its first frame) while shown.
    pub(super) splash: Option<Option<f64>>,
}

/// The work of the Increment Instances popup (issue #11).
pub(super) struct IncrementInstances {
    /// The symbol whose playing instances are moved.
    pub(super) symbol: SymbolId,
    /// Instances whose playback begins here or later are the ones that move.
    pub(super) from: u32,
    /// How far they move; may be negative.
    pub(super) by: i32,
}

impl super::Workspace {
    pub(super) fn show_increment_instances(&mut self) {
        let Some(symbol) = self.selected_instance_symbol() else {
            return;
        };
        self.windows.increment_instances = Some(IncrementInstances {
            symbol,
            from: 0,
            by: 1,
        });
    }

    pub(super) fn increment_instances_window(&mut self, ctx: &egui::Context) {
        let Some(mut form) = self.windows.increment_instances.take() else {
            return;
        };
        let (from, by, apply, close) = (
            self.tr("inc-from"),
            self.tr("inc-by"),
            self.tr("inc-run"),
            self.tr("close"),
        );
        let mut run = false;
        let title = self.label(MenuAction::IncrementInstances);
        let response =
            flare_ui::dialog::modal(ctx, egui::Id::unique("increment_instances")).show(ctx, |ui| {
                use flare_ui::dialog;
                ui.set_width(320.0);
                dialog::title(ui, &title);
                dialog::field(ui, &from, |ui| {
                    ui.add(egui::DragValue::new(&mut form.from).range(0..=u32::MAX));
                });
                dialog::field(ui, &by, |ui| {
                    ui.add(egui::DragValue::new(&mut form.by).range(i32::MIN..=i32::MAX));
                });
                dialog::footer(
                    ui,
                    |_| {},
                    |ui| {
                        if dialog::primary_button(ui, &apply, true).clicked() {
                            run = true;
                            ui.close();
                        }
                        if dialog::button(ui, &close).clicked() {
                            ui.close();
                        }
                    },
                );
            });
        if run {
            match flare_tools::library::increment_instances(
                self.document(),
                form.symbol,
                form.from,
                i64::from(form.by),
            ) {
                Ok(commands) => self.run_batch(commands, "increment instances"),
                Err(error) => self.report(error.to_string()),
            }
        } else if !response.should_close() {
            self.windows.increment_instances = Some(form);
        }
    }

    pub(super) fn add_selection_to_symbol(&mut self) {
        let (Some(symbol), Some(scene), Some(at)) =
            (self.library.selected, self.scene_id(), self.selected)
        else {
            self.report(tr!(self, "lib-select-first"));
            return;
        };
        match flare_tools::library::add_instance_to_symbol(self.document(), symbol, scene, at) {
            Ok(commands) => self.run_batch(commands, "add instance to symbol"),
            Err(error) => self.report(error.to_string()),
        }
    }

    pub(super) fn bake_selection_transform(&mut self) {
        let (Some(scene), Some(at)) = (self.scene_id(), self.selected) else {
            return;
        };
        match flare_tools::library::bake_transform(self.document(), scene, at) {
            Ok(commands) => self.run_batch(commands, "bake transform"),
            Err(error) => self.report(error.to_string()),
        }
    }
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
    /// Each decoded sound with its waveform, by the project's sound.
    pub(super) decoded: HashMap<SoundId, flare_audio::Decoded>,
    pub(super) decoding: HashSet<SoundId>,
    pub(super) decode_failed: HashSet<SoundId>,
    pub(super) events: Vec<AudioEvent>,
    pub(super) importing: Option<u64>,
    pub(super) synced: Option<(bool, u32)>,
    /// Whether a scrub slice may still be sounding, so the next move of the playhead stops it.
    pub(super) scrubbing: bool,
}
