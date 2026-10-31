//! How the selected Graphic instance plays its symbol: Animate's Looping section of Properties.

use flare_document::{Content, PlayMode, Playback, SymbolKind};

use super::*;

impl Workspace {
    /// The frame setting of the selection when it is one instance of a Graphic, with how many
    /// frames that symbol has. A Movie Clip, a Button, artwork or several things have none.
    pub fn selected_playback(&self) -> Option<(Playback, u32)> {
        if !self.also_selected.is_empty() {
            return None;
        }
        let (selected, scene) = (self.selected?, self.scene_id()?);
        let span = ops::find_span(self.document(), scene, selected)?;
        let Content::Instance(instance) = span.content() else {
            return None;
        };
        let symbol = self.document().symbol(instance.symbol())?;
        if symbol.kind() != SymbolKind::Graphic {
            return None;
        }
        let frames = u32::try_from(symbol.timeline().frame_count()).unwrap_or(u32::MAX);
        Some((instance.playback(), frames.max(1)))
    }

    /// Gives the selected Graphic another frame setting: one undo step.
    pub fn set_selection_playback(&mut self, playback: Playback) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        match ops::set_playback(self.document(), scene, selected, playback) {
            Some(commands) => self.run_batch(commands, "change how the graphic plays"),
            None => {
                self.report("Only a Graphic plays by a Loop, Play Once or Single Frame setting")
            }
        }
    }

    /// The Looping section of the Object tab, for a selected Graphic.
    pub(super) fn looping_section(&mut self, ui: &mut egui::Ui) {
        let Some((current, frames)) = self.selected_playback() else {
            return;
        };
        flare_ui::chrome::section(ui, "transport-loop", "Looping");
        let mut mode = current.mode;
        flare_ui::chrome::row(ui, "Options", |ui| {
            egui::ComboBox::from_id_salt("graphic_play_mode")
                .selected_text(mode.label())
                .show_ui(ui, |ui| {
                    for option in PlayMode::ALL {
                        ui.selectable_value(&mut mode, option, option.label());
                    }
                });
        });
        if mode != current.mode {
            self.set_selection_playback(Playback { mode, ..current });
        }
        // Counted from 1 on screen, as the timeline counts. Applied on release or Enter, so
        // scrubbing the number is one undo step.
        let mut first = self.playback_first_edit.unwrap_or(current.first_frame) + 1;
        flare_ui::chrome::row(ui, "First", |ui| {
            let response = ui.add(egui::DragValue::new(&mut first).range(1..=frames));
            if response.changed() {
                self.playback_first_edit = Some(first - 1);
            }
            if (response.drag_stopped() || response.lost_focus())
                && let Some(edited) = self.playback_first_edit.take()
            {
                self.set_selection_playback(Playback {
                    first_frame: edited,
                    ..current
                });
            }
        });
    }
}
