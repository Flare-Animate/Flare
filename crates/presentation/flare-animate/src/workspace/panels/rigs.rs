//! The Rigs panel: the skeleton rig of a library symbol, kept in the project. Bones, poses keyed on
//! the parent's frames and the rig's removal are `flare_editor::rig_library`'s, the same
//! `flare rig project` and the MCP server make, so each is one undo step and the library refuses
//! what is not valid.

use egui::Ui;
use flare_core::SymbolId;
use flare_editor::rig_library;
use flare_rig::{Bone, PoseKey, Rig, RigCommand};
use flare_ui::{chrome, theme};

use crate::workspace::Workspace;

/// What the panel remembers while it is open.
pub(crate) struct RigPanel {
    /// The symbol chosen here; without one the Library's choice is used.
    pub(crate) selected: Option<SymbolId>,
    pub(crate) bone_name: String,
    /// The index of the new bone's parent, as typed; `None` for a root.
    pub(crate) bone_parent: Option<usize>,
    pub(crate) bone_length: f64,
    pub(crate) bone_angle: f64,
    pub(crate) pose_frame: u32,
    pub(crate) pose_bone: usize,
    pub(crate) pose_angle: f64,
}

impl Default for RigPanel {
    fn default() -> Self {
        Self {
            selected: None,
            bone_name: String::new(),
            bone_parent: None,
            bone_length: 40.0,
            bone_angle: 0.0,
            pose_frame: 0,
            pose_bone: 0,
            pose_angle: 0.0,
        }
    }
}

const LABEL: &str = "Edit a rig";

impl Workspace {
    fn rig_failed(&mut self, error: impl std::fmt::Display) {
        self.report(tr!(self, "rigs-cannot", &[("error", &error.to_string())]));
    }

    fn rig_apply(
        &mut self,
        command: Result<flare_commands::Command, rig_library::RigLibraryError>,
    ) -> bool {
        match command {
            Ok(command) => match self.active_mut().apply_labeled([command], LABEL) {
                Ok(()) => true,
                Err(error) => {
                    self.rig_failed(error);
                    false
                }
            },
            Err(error) => {
                self.rig_failed(error);
                false
            }
        }
    }

    /// The symbol the panel works on: its own choice, else the Library's.
    pub(crate) fn rig_symbol(&self) -> Option<SymbolId> {
        let known = |id: &SymbolId| self.document().symbols().iter().any(|s| s.id() == *id);
        self.rigs
            .selected
            .filter(known)
            .or(self.library.selected.filter(known))
    }

    /// Gives `symbol` an empty rig and chooses it.
    pub(crate) fn rig_create(&mut self, symbol: SymbolId) {
        let command = rig_library::create(self.document(), symbol);
        if self.rig_apply(command) {
            self.rigs.selected = Some(symbol);
        }
    }

    pub(crate) fn rig_remove(&mut self, symbol: SymbolId) {
        let command = rig_library::remove(self.document(), symbol);
        self.rig_apply(command);
    }

    pub(crate) fn rig_add_bone(&mut self, symbol: SymbolId, bone: Bone) {
        let command = rig_library::edit(self.document(), symbol, &[RigCommand::AddBone(bone)]);
        self.rig_apply(command);
    }

    /// Keys `bone` at `angle` radians on `frame`, keeping what that key already poses.
    pub(crate) fn rig_set_pose(&mut self, symbol: SymbolId, frame: u32, bone: usize, angle: f64) {
        let mut key = match rig_library::find(self.document(), symbol) {
            Ok(Some(rig)) => rig
                .keys
                .get(&frame)
                .cloned()
                .unwrap_or_else(PoseKey::default),
            Ok(None) => return self.rig_failed(rig_library::RigLibraryError::NoRig),
            Err(error) => return self.rig_failed(error),
        };
        key.pose.rotations.insert(bone, angle);
        let command = rig_library::edit(
            self.document(),
            symbol,
            &[RigCommand::SetRigPose { frame, key }],
        );
        self.rig_apply(command);
    }

    pub(crate) fn rig_remove_pose(&mut self, symbol: SymbolId, frame: u32) {
        let command = rig_library::edit(
            self.document(),
            symbol,
            &[RigCommand::RemoveRigPose { frame }],
        );
        self.rig_apply(command);
    }

    pub(crate) fn rigs_panel(&mut self, ui: &mut Ui) {
        let symbols: Vec<(SymbolId, String, bool)> = self
            .document()
            .symbols()
            .iter()
            .map(|s| {
                let rigged = self
                    .document()
                    .rig_assets()
                    .iter()
                    .any(|r| r.symbol() == s.id());
                (s.id(), s.name().to_owned(), rigged)
            })
            .collect();
        if symbols.is_empty() {
            chrome::empty_state(ui, "panel-rigs", &tr!(self, "rigs-empty"));
            return;
        }
        let chosen = self.rig_symbol();
        for (id, name, rigged) in &symbols {
            let label = if *rigged {
                format!("{name}  ({})", tr!(self, "rigs-has-rig"))
            } else {
                name.clone()
            };
            if ui.selectable_label(chosen == Some(*id), label).clicked() {
                self.rigs.selected = Some(*id);
            }
        }
        ui.separator();
        let Some(symbol) = chosen else {
            ui.label(tr!(self, "rigs-select"));
            return;
        };
        match rig_library::find(self.document(), symbol) {
            Ok(Some(rig)) => self.rig_form(ui, symbol, &rig),
            Ok(None) => {
                if ui.button(tr!(self, "rigs-add-rig")).clicked() {
                    self.rig_create(symbol);
                }
            }
            Err(error) => {
                ui.label(error.to_string());
            }
        }
        ui.add_space(6.0);
        ui.label(egui::RichText::new(tr!(self, "rigs-note")).color(theme::text_dim()));
    }

    fn rig_form(&mut self, ui: &mut Ui, symbol: SymbolId, rig: &Rig) {
        for (index, bone) in rig.bones.iter().enumerate() {
            let parent = bone
                .parent
                .map_or_else(|| "-".to_owned(), |p| p.to_string());
            ui.label(format!(
                "{index}  {}  ({parent})  {:.1}",
                bone.name, bone.length
            ));
        }
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.rigs.bone_name).desired_width(90.0));
            let parent = self.rigs.bone_parent;
            egui::ComboBox::from_id_salt("rig-parent")
                .selected_text(parent.map_or_else(|| "-".to_owned(), |p| p.to_string()))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.rigs.bone_parent, None, "-");
                    for i in 0..rig.bones.len() {
                        ui.selectable_value(&mut self.rigs.bone_parent, Some(i), i.to_string());
                    }
                });
            ui.add(egui::DragValue::new(&mut self.rigs.bone_length).range(0.0..=10_000.0));
            ui.add(egui::DragValue::new(&mut self.rigs.bone_angle).speed(0.01));
            if ui.button(tr!(self, "rigs-add-bone")).clicked() {
                let bone = Bone {
                    name: std::mem::take(&mut self.rigs.bone_name),
                    parent: self.rigs.bone_parent,
                    length: self.rigs.bone_length,
                    rest_angle: self.rigs.bone_angle,
                    limit: None,
                    target: None,
                };
                self.rig_add_bone(symbol, bone);
            }
        });
        ui.separator();
        for (frame, key) in &rig.keys {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{frame}: {} {}",
                    key.pose.rotations.len(),
                    tr!(self, "rigs-rotations")
                ));
                if ui.small_button(tr!(self, "rigs-remove-pose")).clicked() {
                    self.rig_remove_pose(symbol, *frame);
                }
            });
        }
        ui.horizontal(|ui| {
            ui.add(egui::DragValue::new(&mut self.rigs.pose_frame));
            ui.add(
                egui::DragValue::new(&mut self.rigs.pose_bone)
                    .range(0..=rig.bones.len().saturating_sub(1)),
            );
            ui.add(egui::DragValue::new(&mut self.rigs.pose_angle).speed(0.01));
            if ui.button(tr!(self, "rigs-set-pose")).clicked() {
                let (frame, bone, angle) = (
                    self.rigs.pose_frame,
                    self.rigs.pose_bone,
                    self.rigs.pose_angle,
                );
                self.rig_set_pose(symbol, frame, bone, angle);
            }
        });
        ui.separator();
        if ui.button(tr!(self, "rigs-remove-rig")).clicked() {
            self.rig_remove(symbol);
        }
    }
}
