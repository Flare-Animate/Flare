//! The timeline toolbar.

use crate::workspace::{Command, Layer, LayerId, MAX_ONION_FRAMES, Workspace};
use egui::Ui;
use flare_menubar::MenuAction;
use flare_ui::chrome;

use super::*;

/// Height of the row of controls above the timeline.
const TOOLBAR_ROW: f32 = 24.0;

/// The space at the toolbar's left, the same on every side.
const TOOLBAR_MARGIN: f32 = 8.0;

impl Workspace {
    // ---- timeline toolbar --------------------------------------------------------------

    /// Layer buttons, transport, frame buttons, onion skin and zoom above the timeline.
    pub(crate) fn timeline_toolbar(&mut self, ui: &mut Ui) {
        // The same space on every side, and one row height with everything centred in it, so the
        // buttons, number fields and text box do not sit at different heights.
        let margin = egui::Margin::symmetric(TOOLBAR_MARGIN as i8, 6);
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            let size = egui::vec2(ui.available_width(), TOOLBAR_ROW);
            // A timeline too narrow for every control carries the rest onto a second row of the
            // same height, rather than losing them past its edge.
            ui.allocate_ui_with_layout(
                size,
                egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true),
                |ui| {
                    ui.spacing_mut().interact_size.y = TOOLBAR_ROW;
                    self.timeline_toolbar_row(ui);
                },
            );
        });
    }

    fn timeline_toolbar_row(&mut self, ui: &mut Ui) {
        // The layer buttons sit over the column of layer names, as in Animate, and the rest
        // starts over the frames; a column narrower than the buttons pushes the rest along.
        let row_left = ui.cursor().min.x;
        self.timeline_layer_buttons(ui);
        let frames_left = row_left - TOOLBAR_MARGIN + self.display.timeline_names;
        let gap = frames_left - ui.cursor().min.x - ui.spacing().item_spacing.x;
        if gap > 0.0 {
            ui.add_space(gap);
        }
        ui.separator();
        let last = self.scene_frames() - 1;
        if chrome::icon_button(
            ui,
            "transport-first",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-first"),
        )
        .clicked()
        {
            self.set_frame(0);
        }
        if chrome::icon_button(
            ui,
            "transport-prev",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-prev"),
        )
        .clicked()
        {
            self.set_frame(self.frame.saturating_sub(1));
        }
        let (icon, hint) = if self.playing {
            ("transport-pause", tr!(self, "timeline-pause"))
        } else {
            ("transport-play", tr!(self, "timeline-play"))
        };
        if chrome::icon_button(ui, icon, BAR_ICON, self.playing, true, &hint).clicked() {
            self.playing = !self.playing;
        }
        if chrome::icon_button(
            ui,
            "transport-next",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-next"),
        )
        .clicked()
        {
            self.set_frame(self.frame.saturating_add(1));
        }
        if chrome::icon_button(
            ui,
            "transport-last",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-last"),
        )
        .clicked()
        {
            self.set_frame(last);
        }
        if chrome::icon_button(
            ui,
            "transport-loop",
            BAR_ICON,
            self.looping.on,
            true,
            &tr!(self, "timeline-loop"),
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleLoop);
        }
        ui.separator();
        // The playhead may rest on an empty cell past the scene's end (a click there, to give
        // the layer frames up to it), so the field's range reaches it: a range that stopped at the
        // scene clamped the playhead back to it on the next repaint.
        let mut frame = self.frame;
        let changed = ui
            .add(
                egui::DragValue::new(&mut frame)
                    .range(0..=last.max(self.frame))
                    .prefix(format!("{} ", tr!(self, "timeline-frame"))),
            )
            .changed();
        if changed && frame != self.frame {
            self.set_frame(frame);
        }
        // The rate can be changed here, as in Animate, without going to the document's settings.
        self.fps_field(ui);
        ui.separator();
        self.timeline_frame_buttons(ui);
        ui.separator();
        let mut onion = self.onion.enabled;
        if chrome::icon_button(
            ui,
            "onion-skin",
            BAR_ICON,
            onion,
            true,
            &tr!(self, "timeline-onion"),
        )
        .clicked()
        {
            onion = !onion;
        }
        self.set_onion_skin(onion);
        ui.add_enabled_ui(onion, |ui| {
            if chrome::icon_button(
                ui,
                "onion-outline",
                BAR_ICON,
                self.onion.outlines,
                true,
                &tr!(self, "timeline-onion-outline"),
            )
            .clicked()
            {
                self.set_onion_outlines(!self.onion.outlines);
            }
        });
        ui.add_enabled_ui(onion, |ui| {
            let (mut before, mut after) = (self.onion.before, self.onion.after);
            ui.add(
                egui::DragValue::new(&mut before)
                    .range(0..=MAX_ONION_FRAMES)
                    .prefix("\u{2190} "),
            );
            ui.add(
                egui::DragValue::new(&mut after)
                    .range(0..=MAX_ONION_FRAMES)
                    .suffix(" \u{2192}"),
            );
            self.set_onion_range(before, after);
        });
        self.onion_settings_button(ui, BAR_ICON);
        if chrome::icon_button(
            ui,
            "edit-multiple-frames",
            BAR_ICON,
            self.editing_multiple_frames(),
            true,
            &tr!(self, "timeline-multiple-frames"),
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleEditMultipleFrames);
        }
        ui.separator();
        for (icon, hint, factor) in [
            (
                "zoom-out",
                &tr!(self, "timeline-zoom-out"),
                1.0 / flare_editor::timeline_view::STEP,
            ),
            (
                "zoom-in",
                &tr!(self, "timeline-zoom-in"),
                flare_editor::timeline_view::STEP,
            ),
        ] {
            if chrome::icon_button(ui, icon, BAR_ICON, false, true, hint).clicked() {
                self.zoom_timeline(factor);
            }
        }
        if chrome::icon_button(
            ui,
            "zoom-fit",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-fit"),
        )
        .clicked()
        {
            self.timeline_fit = true;
        }
    }

    /// New layer, new folder, the Folder menu, motion guide, mask and delete: what is done to
    /// layers, at the left of the toolbar over their names.
    fn timeline_layer_buttons(&mut self, ui: &mut Ui) {
        if chrome::icon_button(
            ui,
            "layer-new",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-new-layer"),
        )
        .clicked()
        {
            self.add_layer();
        }
        if chrome::icon_button(
            ui,
            "layer-folder",
            BAR_ICON,
            false,
            true,
            &tr!(self, "timeline-new-folder"),
        )
        .clicked()
        {
            self.perform(MenuAction::NewLayerFolder);
        }
        let folders = self.folders_for_active_layer();
        let in_folder = self.active_layer().is_some_and(|l| l.parent().is_some());
        ui.add_enabled_ui(!folders.is_empty() || in_folder, |ui| {
            ui.menu_button(tr!(self, "timeline-folder"), |ui| {
                for (id, name) in &folders {
                    if ui
                        .button(tr!(self, "timeline-move-into", &[("name", name)]))
                        .clicked()
                    {
                        self.move_active_layer_to_folder(Some(*id));
                        ui.close();
                    }
                }
                if in_folder && ui.button(tr!(self, "timeline-move-out")).clicked() {
                    self.move_active_layer_to_folder(None);
                    ui.close();
                }
            });
        });
        if chrome::icon_button(
            ui,
            "layer-motion-guide",
            BAR_ICON,
            self.active_layer().is_some_and(Layer::is_guide),
            self.is_enabled(MenuAction::ToggleLayerGuide),
            &tr!(self, "timeline-guide"),
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleLayerGuide);
        }
        if chrome::icon_button(
            ui,
            "layer-mask",
            BAR_ICON,
            self.active_layer().is_some_and(Layer::is_mask),
            self.is_enabled(MenuAction::ToggleLayerMask),
            &tr!(self, "timeline-mask"),
        )
        .clicked()
        {
            self.perform(MenuAction::ToggleLayerMask);
        }
        if chrome::icon_button(
            ui,
            "layer-delete",
            BAR_ICON,
            false,
            self.is_enabled(MenuAction::DeleteLayer),
            &tr!(self, "timeline-delete-layer"),
        )
        .clicked()
        {
            self.perform(MenuAction::DeleteLayer);
        }
    }

    /// A new empty layer above the others, made the active one: one undo step.
    pub fn add_layer(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        let id = LayerId::new();
        let layer = match Layer::new(
            id,
            tr!(self, "name-new-layer", &[("n", &(count + 1).to_string())]),
            vec![],
        ) {
            Ok(layer) => layer,
            Err(error) => return self.report(error.to_string()),
        };
        self.run_batch(vec![Command::AddLayer { scene, layer }], "add a layer");
        if self.status.is_none() {
            self.current_layer = Some(id);
        }
    }
}
