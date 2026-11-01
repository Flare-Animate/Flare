//! Cut, Select All and Reverse Frames, and the menu on a right click of a frame.

use flare_editor::frames;

use super::*;
use flare_menubar::MenuAction;

impl Workspace {
    /// The frames the range commands act on: the selected ones, or with none selected the current
    /// frame, of the active layer.
    fn frames_in_play(&self) -> Option<(LayerId, u32, u32)> {
        match self.frame_range {
            Some(r) => Some((r.layer, r.from, r.to)),
            None => Some((self.active_layer_id()?, self.frame, self.frame)),
        }
    }

    /// Cut Frames: remembers the frames for Paste Frames and takes them out, so what came after
    /// moves left. One undo step.
    pub fn cut_frames(&mut self) {
        let (Some(scene), Some((id, from, to))) = (self.scene_id(), self.frames_in_play()) else {
            return;
        };
        let Some(layer) = self.layer(id) else { return };
        if layer.is_locked() {
            self.report("Cannot cut frames: the layer is locked");
            return;
        }
        let cut =
            frames::copy_frames(layer, from, to).zip(frames::remove_frames(scene, layer, from, to));
        let Some((clip, commands)) = cut else {
            self.report("There is nothing to cut in those frames");
            return;
        };
        self.run_batch(commands, "cut frames");
        if self.status.is_none() {
            self.frame_clipboard = Some(clip);
            self.frame_range = None;
            self.set_frame(from);
            self.validate_selection();
        }
    }

    /// Select All Frames: every frame of the active layer, from the first to the end of its last
    /// keyframe. (The selection holds one layer's frames.)
    pub fn select_all_frames(&mut self) {
        let Some(layer) = self.active_layer() else {
            return;
        };
        let id = layer.id();
        let last = layer
            .spans()
            .last()
            .and_then(|s| u32::try_from(s.end() - 1).ok());
        match last {
            Some(last) => self.select_frames(id, 0, last),
            None => self.report("There are no frames on this layer to select"),
        }
    }

    /// Reverse Frames: the keyframes of the selected frames in the opposite order, as one undo
    /// step. It needs frames selected, and whole keyframes.
    pub fn reverse_frames(&mut self) {
        let Some(range) = self.frame_range else {
            self.report("Select the frames to reverse first: click one, Shift-click another");
            return;
        };
        let Some(scene) = self.scene_id() else { return };
        let Some(layer) = self.layer(range.layer) else {
            return;
        };
        if layer.is_locked() {
            self.report("Cannot reverse frames: the layer is locked");
            return;
        }
        match frames::reverse_frames(scene, layer, range.from, range.to) {
            Ok(commands) => {
                self.run_batch(commands, "reverse frames");
                self.set_frame(self.frame);
                self.validate_selection();
            }
            Err(refusal) => self.report(format!("Cannot reverse those frames: {refusal}")),
        }
    }

    /// The menu on a right click of a frame: where the frame operations are, in the order of
    /// Animate's own.
    pub(super) fn frame_menu(&mut self, ui: &mut egui::Ui) {
        let groups: [&[MenuAction]; 3] = [
            &[
                MenuAction::InsertFrame,
                MenuAction::InsertKeyframe,
                MenuAction::InsertBlankKeyframe,
                MenuAction::ClearKeyframe,
                MenuAction::RemoveFrame,
            ],
            &[MenuAction::ToggleMotionTween, MenuAction::ToggleShapeTween],
            &[
                MenuAction::CutFrames,
                MenuAction::CopyFrames,
                MenuAction::PasteFrames,
                MenuAction::SelectAllFrames,
                MenuAction::ReverseFrames,
            ],
        ];
        for (n, group) in groups.into_iter().enumerate() {
            if n > 0 {
                ui.separator();
            }
            for &action in group {
                // Here the three inserts say what they do; the menu bar has them under Insert.
                let label = match action {
                    MenuAction::InsertFrame => "Insert Frame".to_owned(),
                    MenuAction::InsertKeyframe => "Insert Keyframe".to_owned(),
                    MenuAction::InsertBlankKeyframe => "Insert Blank Keyframe".to_owned(),
                    _ => self.label(action),
                };
                let button = egui::Button::new(label);
                if ui.add_enabled(self.is_enabled(action), button).clicked() {
                    self.perform(action);
                    ui.close();
                }
            }
        }
    }
}
