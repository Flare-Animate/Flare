//! Properties for the joint the Bone or Selection tool last touched: its turn and its limits,
//! taking its bone away, and letting the whole armature go.

use egui::Ui;
use flare_tools::bones::{self, JointRef};
use flare_ui::chrome;

use crate::workspace::*;

impl Workspace {
    /// The joint last touched, if it is still there at this frame.
    pub(in crate::workspace) fn bone_joint_here(
        &self,
    ) -> Option<(JointRef, flare_document::Armature)> {
        let joint = self.bone_joint?;
        let scene = self.scene_id()?;
        let (posed, _) =
            bones::armature_at(self.document(), scene, joint.layer, self.frame).ok()?;
        posed.joint(joint.joint)?;
        Some((joint, posed))
    }

    pub(in crate::workspace) fn bone_joint_options(&mut self, ui: &mut Ui) {
        let (Some(scene), Some((joint, posed))) = (self.scene_id(), self.bone_joint_here()) else {
            ui.weak(tr!(self, "tools-bone-none"));
            return;
        };
        let Some(rig) = posed.joint(joint.joint).cloned() else {
            return;
        };
        let title = tr!(
            self,
            "tools-bone-joint",
            &[("number", &(joint.joint + 1).to_string())]
        );
        ui.label(title);
        let mut turn = posed.pose()[joint.joint].turn.to_degrees();
        let turn_label = tr!(self, "tools-bone-turn");
        let mut turned = false;
        chrome::row(ui, &turn_label, |ui| {
            turned = ui
                .add(egui::DragValue::new(&mut turn).speed(0.5).suffix("°"))
                .changed();
        });
        if turned {
            let made = bones::turn_joint(
                self.document(),
                scene,
                joint.layer,
                self.frame,
                joint.joint,
                turn.to_radians(),
            );
            let label = tr!(self, "tools-bone-pose");
            self.apply_bones(made, &label);
        }

        let mut limited = rig.limit().is_some();
        let (mut min, mut max) = rig
            .limit()
            .map_or((-45.0, 45.0), |(a, b)| (a.to_degrees(), b.to_degrees()));
        let mut changed = ui
            .checkbox(&mut limited, tr!(self, "tools-bone-limit"))
            .changed();
        if limited {
            for (key, value) in [("tools-bone-min", &mut min), ("tools-bone-max", &mut max)] {
                let label = self.tr(key);
                chrome::row(ui, &label, |ui| {
                    changed |= ui
                        .add(
                            egui::DragValue::new(value)
                                .speed(0.5)
                                .range(-360.0..=360.0)
                                .suffix("°"),
                        )
                        .changed();
                });
            }
        }
        if changed {
            let limit = limited.then(|| (min.min(max).to_radians(), max.max(min).to_radians()));
            let made = bones::set_limit(self.document(), scene, joint.layer, joint.joint, limit);
            let label = tr!(self, "tools-bone-limit");
            self.apply_bones(made, &label);
        }

        let has_bone = rig.parent().is_some();
        if ui
            .add_enabled(has_bone, egui::Button::new(tr!(self, "tools-bone-detach")))
            .clicked()
        {
            let made = bones::detach(self.document(), scene, joint.layer, joint.joint);
            let label = tr!(self, "tools-bone-detach");
            self.apply_bones(made, &label);
        }
        if ui.button(tr!(self, "tools-bone-release")).clicked() {
            let made = bones::release(self.document(), scene, joint.layer);
            let label = tr!(self, "tools-bone-release");
            self.apply_bones(made, &label);
            self.bone_joint = None;
        }
    }
}
