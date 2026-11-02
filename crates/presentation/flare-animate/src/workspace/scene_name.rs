//! Renaming the scene by double clicking its name in the bar above the stage.

use egui::{Key, Ui};

use super::*;

impl Workspace {
    /// Starts editing the scene's name in place. Inside a symbol the first crumb is not the scene
    /// being edited, so nothing starts.
    pub(super) fn begin_scene_rename(&mut self) {
        if self.in_symbol() {
            return;
        }
        let Some(name) = self
            .document()
            .scenes()
            .first()
            .map(|s| s.name().to_owned())
        else {
            return;
        };
        // A box with a new id each time, so one that was left with Escape does not report that it
        // has lost focus the moment the next edit appears.
        self.scene_rename_serial = self.scene_rename_serial.wrapping_add(1);
        self.scene_renaming = Some(name);
    }

    /// The text box that stands in for the scene's name while it is edited: Enter or leaving the
    /// box keeps the name, Escape drops it.
    pub(super) fn scene_name_editor(&mut self, ui: &mut Ui) {
        let serial = self.scene_rename_serial;
        let Some(text) = &mut self.scene_renaming else {
            return;
        };
        let edit = ui.add(
            egui::TextEdit::singleline(text)
                .id(ui.make_persistent_id(("scene_name", serial)))
                .desired_width(160.0),
        );
        if !edit.has_focus() && !edit.lost_focus() {
            edit.request_focus();
        }
        if ui.input(|i| i.key_pressed(Key::Escape)) {
            self.scene_renaming = None;
        } else if edit.lost_focus()
            && let Some(name) = self.scene_renaming.take()
        {
            self.rename_scene(&name);
        }
    }

    /// Renames the scene as one undo step. A blank name is refused, and the same name is no edit.
    pub fn rename_scene(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.report(tr!(self, "msg-scene-needs-name"));
            return;
        }
        let Some(scene) = self.document().scenes().first() else {
            return;
        };
        if scene.name() == name || self.in_symbol() {
            return;
        }
        let command = Command::RenameScene {
            scene: scene.id(),
            name: name.to_owned(),
        };
        self.run_batch(vec![command], "rename the scene");
    }
}
