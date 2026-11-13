//! The Mannequin panel: a 3D figure to pose and draw from, as drawing programs give an artist
//! one. A row of view buttons and the view (see `mannequin_view`), then the body, a built-in
//! pose, the pinned hands and feet, the selected joint's angles, the proportions, the view, and
//! the figure on the stage.

use egui::Ui;
use flare_mannequin::{Angles, Body, Joint, Proportion, Vec3};
use flare_ui::chrome;

use crate::workspace::Workspace;
use crate::workspace::mannequin::PlaceInto;

/// The message key of a joint's name: `mannequin-joint-left-shoulder`.
pub(crate) fn joint_key(joint: Joint) -> String {
    format!("mannequin-joint-{}", joint.id().replace('_', "-"))
}

/// The message key of a built-in pose's name: `mannequin-pose-t-pose`.
pub(crate) fn pose_key(id: &str) -> String {
    format!("mannequin-pose-{}", id.replace('_', "-"))
}

/// The message key of a proportion's name: `mannequin-proportion-legs`.
pub(crate) fn proportion_key(p: Proportion) -> String {
    format!("mannequin-proportion-{}", p.id())
}

impl Workspace {
    pub(crate) fn mannequin_panel(&mut self, ui: &mut Ui) {
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                self.mannequin_toolbar(ui);
                ui.add_space(2.0);
                self.mannequin_view(ui);
                ui.add_space(4.0);
                ui.weak(tr!(self, "mannequin-hint"));
                ui.add_space(6.0);
                self.mannequin_figure_row(ui);
                ui.add_space(6.0);
                self.mannequin_pins(ui);
                ui.add_space(6.0);
                self.mannequin_joint(ui);
                ui.add_space(6.0);
                let title = tr!(self, "mannequin-proportions");
                ui.collapsing(title, |ui| self.mannequin_proportions(ui));
                let title = tr!(self, "mannequin-view");
                ui.collapsing(title, |ui| self.mannequin_view_settings(ui));
                ui.add_space(6.0);
                self.mannequin_stage_settings(ui);
            });
    }

    /// Above the view: back to the first view, the guide lines, pinning both feet, and making
    /// the panel large.
    fn mannequin_toolbar(&mut self, ui: &mut Ui) {
        let hints = [
            tr!(self, "mannequin-reset-view"),
            tr!(self, "mannequin-guides"),
            tr!(self, "mannequin-pin-feet"),
            if self.mannequin.expanded {
                tr!(self, "mannequin-shrink")
            } else {
                tr!(self, "mannequin-expand")
            },
        ];
        let feet = [Joint::LeftAnkle, Joint::RightAnkle];
        let pinned = feet.iter().all(|f| self.mannequin.held.contains(f));
        ui.horizontal(|ui| {
            if chrome::icon_button(ui, "zoom-fit", 16.0, false, true, &hints[0]).clicked() {
                self.reset_mannequin_view();
            }
            let guides = self.mannequin.guides;
            if chrome::icon_button(ui, "grid", 16.0, guides, true, &hints[1]).clicked() {
                self.mannequin.guides = !guides;
            }
            if chrome::icon_button(ui, "pin", 16.0, pinned, true, &hints[2]).clicked() {
                for foot in feet {
                    if self.mannequin.held.contains(&foot) == pinned {
                        self.toggle_mannequin_pin(foot);
                    }
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let expanded = self.mannequin.expanded;
                if chrome::icon_button(ui, "fullscreen", 16.0, expanded, true, &hints[3]).clicked()
                {
                    self.set_mannequin_expanded(!expanded);
                }
            });
        });
    }

    /// The hands and feet that stay put while the body is posed, each a toggle.
    fn mannequin_pins(&mut self, ui: &mut Ui) {
        ui.label(
            egui::RichText::new(tr!(self, "mannequin-pinned")).color(flare_ui::theme::text_dim()),
        );
        let ends = [
            Joint::LeftWrist,
            Joint::RightWrist,
            Joint::LeftAnkle,
            Joint::RightAnkle,
        ];
        let names: Vec<String> = ends.iter().map(|j| self.tr(&joint_key(*j))).collect();
        let mut flip = None;
        ui.horizontal_wrapped(|ui| {
            for (joint, name) in ends.iter().zip(&names) {
                let on = self.mannequin.held.contains(joint);
                if ui.selectable_label(on, name).clicked() {
                    flip = Some(*joint);
                }
            }
        });
        if let Some(joint) = flip {
            self.toggle_mannequin_pin(joint);
        }
    }

    /// The body, a built-in pose, Mirror and Reset.
    fn mannequin_figure_row(&mut self, ui: &mut Ui) {
        let (male, female) = (
            tr!(self, "mannequin-body-male"),
            tr!(self, "mannequin-body-female"),
        );
        let current = usize::from(self.mannequin.figure.body == Body::Female);
        if let Some(choice) = chrome::segmented(ui, &[&male, &female], current) {
            self.set_mannequin_body(if choice == 1 {
                Body::Female
            } else {
                Body::Male
            });
        }
        ui.add_space(4.0);
        let names: Vec<(String, String)> = flare_mannequin::presets()
            .iter()
            .map(|p| (p.id.clone(), self.tr(&pose_key(&p.id))))
            .collect();
        let mut chosen = None;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("mannequin-pose")
                .selected_text(tr!(self, "mannequin-pose"))
                .show_ui(ui, |ui| {
                    for (id, name) in &names {
                        if ui.selectable_label(false, name).clicked() {
                            chosen = Some(id.clone());
                        }
                    }
                });
            if ui.button(tr!(self, "mannequin-mirror")).clicked() {
                self.mirror_mannequin();
            }
            if ui.button(tr!(self, "mannequin-reset-pose")).clicked() {
                self.reset_mannequin_pose();
            }
        });
        if let Some(id) = chosen {
            self.set_mannequin_pose(&id);
        }
    }

    /// The selected joint's three angles, each within its limits, and the figure's height off
    /// the ground for the pelvis.
    fn mannequin_joint(&mut self, ui: &mut Ui) {
        let Some(joint) = self.mannequin.selected else {
            ui.weak(tr!(self, "mannequin-joint-none"));
            return;
        };
        ui.strong(self.tr(&joint_key(joint)));
        let limits = joint.limits();
        let Angles {
            mut bend,
            mut twist,
            mut side,
        } = self.mannequin.figure.pose.angles(joint);
        let labels = [
            tr!(self, "mannequin-bend"),
            tr!(self, "mannequin-twist"),
            tr!(self, "mannequin-side"),
        ];
        for ((label, value), range) in labels.iter().zip([&mut bend, &mut twist, &mut side]).zip([
            limits.bend,
            limits.twist,
            limits.side,
        ]) {
            // An axis the joint does not turn about has nothing to set.
            if range.0 < range.1 {
                chrome::slider(ui, label, value, range.0..=range.1, "\u{b0}", false);
            }
        }
        self.mannequin
            .figure
            .pose
            .set(joint, Angles::new(bend, twist, side));
        if joint == Joint::Pelvis {
            let lift = self.mannequin.figure.pose.lift();
            let mut height = lift.y;
            let label = tr!(self, "mannequin-raise");
            chrome::slider(ui, &label, &mut height, -0.6..=0.6, "", false);
            self.mannequin
                .figure
                .pose
                .set_lift(Vec3::new(lift.x, height, lift.z));
        }
    }

    fn mannequin_proportions(&mut self, ui: &mut Ui) {
        for p in Proportion::ALL {
            let label = self.tr(&proportion_key(p));
            let mut value = self.mannequin.figure.proportions.get(p);
            let (min, max) = p.range();
            chrome::slider(ui, &label, &mut value, min..=max, "\u{d7}", false);
            self.mannequin.figure.proportions.set_clamped(p, value);
        }
        if ui
            .button(tr!(self, "mannequin-reset-proportions"))
            .clicked()
        {
            self.reset_mannequin_proportions();
        }
    }

    fn mannequin_view_settings(&mut self, ui: &mut Ui) {
        let view = &mut self.mannequin.figure.view;
        let (mut yaw, mut pitch, mut fov) = (view.yaw(), view.pitch(), view.fov());
        let labels = [
            tr!(self, "mannequin-view-turn"),
            tr!(self, "mannequin-view-tilt"),
            tr!(self, "mannequin-view-perspective"),
            tr!(self, "mannequin-view-zoom"),
        ];
        chrome::slider(ui, &labels[0], &mut yaw, -180.0..=180.0, "\u{b0}", false);
        let most = flare_mannequin::MAX_PITCH;
        chrome::slider(ui, &labels[1], &mut pitch, -most..=most, "\u{b0}", false);
        chrome::slider(
            ui,
            &labels[2],
            &mut fov,
            0.0..=flare_mannequin::MAX_FOV,
            "\u{b0}",
            false,
        );
        let view = &mut self.mannequin.figure.view;
        view.set_yaw(yaw);
        view.set_pitch(pitch);
        view.set_fov(fov);
        let (low, high) = crate::workspace::mannequin::ZOOM_RANGE;
        chrome::slider(
            ui,
            &labels[3],
            &mut self.mannequin.zoom,
            low..=high,
            "\u{d7}",
            false,
        );
        let sides = [
            tr!(self, "mannequin-view-front"),
            tr!(self, "mannequin-view-side"),
            tr!(self, "mannequin-view-back"),
        ];
        ui.horizontal(|ui| {
            for (label, yaw) in sides.iter().zip([0.0, 90.0, 180.0]) {
                if ui.button(label).clicked() {
                    self.mannequin.figure.view.set_yaw(yaw);
                    self.mannequin.figure.view.set_pitch(0.0);
                }
            }
        });
    }

    /// Showing the figure over the stage, where and how large, and placing it on a layer.
    fn mannequin_stage_settings(&mut self, ui: &mut Ui) {
        ui.strong(tr!(self, "mannequin-stage"));
        let show = tr!(self, "mannequin-show-on-stage");
        ui.checkbox(&mut self.mannequin.on_stage, show);
        let Some(at) = self.mannequin_on_stage() else {
            return;
        };
        let (mut center, mut height) = (at.center, at.height);
        let labels = [
            tr!(self, "mannequin-opacity"),
            tr!(self, "mannequin-stage-x"),
            tr!(self, "mannequin-stage-y"),
            tr!(self, "mannequin-stage-height"),
        ];
        chrome::slider(
            ui,
            &labels[0],
            &mut self.mannequin.opacity,
            0.1..=1.0,
            "",
            false,
        );
        let stage = self.document().stage();
        let (w, h) = (f64::from(stage.width()), f64::from(stage.height()));
        chrome::slider(ui, &labels[1], &mut center[0], -w..=2.0 * w, "", false);
        chrome::slider(ui, &labels[2], &mut center[1], -h..=2.0 * h, "", false);
        chrome::slider(ui, &labels[3], &mut height, 1.0..=4.0 * h.max(w), "", false);
        if (center, height) != (at.center, at.height) {
            self.mannequin.stage_at = Some((center, height));
        }
        ui.add_space(4.0);
        ui.label(
            egui::RichText::new(tr!(self, "mannequin-place-into"))
                .color(flare_ui::theme::text_dim()),
        );
        let names = [
            tr!(self, "mannequin-place-new-layer"),
            tr!(self, "mannequin-place-this-keyframe"),
            tr!(self, "mannequin-place-new-keyframe"),
        ];
        let labels: Vec<&str> = names.iter().map(String::as_str).collect();
        let current = PlaceInto::ALL
            .iter()
            .position(|p| *p == self.mannequin.into)
            .unwrap_or(0);
        // Its own id, apart from the body's choice above.
        let choice = ui
            .push_id("mannequin-place-into", |ui| {
                chrome::segmented(ui, &labels, current)
            })
            .inner;
        if let Some(choice) = choice {
            self.set_mannequin_place_into(PlaceInto::ALL[choice]);
        }
        ui.add_space(2.0);
        if ui.button(tr!(self, "mannequin-place")).clicked() {
            self.place_mannequin();
        }
    }
}
