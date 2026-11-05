//! The Bone tool and posing on the stage.
//!
//! With the Bone tool a drag from one symbol instance to another links them with a bone (see
//! `flare_tools::bones::connect`); from an instance in an armature it adds a bone below it. With
//! the Selection tool, a press on an armature's joint, bone or instance poses it: the drag shows
//! the bones where they would go, and letting go keys the pose at the current frame. The bones of
//! every visible armature are drawn over the stage, and Properties edits the joint last touched.

use egui::{Color32, Pos2, Stroke, Ui, Vec2};
use flare_core::{DocumentPoint, InstanceId};
use flare_tools::bones::{self, Bond, JointRef};
use glamx::DVec2;

use crate::workspace::*;

/// Animate draws bones in a pale yellow-green.
const BONE_INK: Color32 = Color32::from_rgb(200, 220, 120);

/// A bone gesture in progress.
#[derive(Clone, Copy, Debug)]
pub(in crate::workspace) enum BoneDrag {
    /// The Bone tool, from an instance (and where its joint goes) toward another.
    Link {
        from: InstanceId,
        at: Pos2,
        current: Pos2,
    },
    /// Posing: a joint pulled by the pointer, which went down at `grab`; the joint follows the
    /// pointer's motion, so grabbing an instance off its joint does not make it jump.
    Pose {
        joint: JointRef,
        grab: Pos2,
        current: Pos2,
    },
}

fn doc_point(p: Pos2) -> Option<DocumentPoint> {
    DocumentPoint::new(f64::from(p.x), f64::from(p.y)).ok()
}

fn vec(p: Pos2) -> DVec2 {
    DVec2::new(f64::from(p.x), f64::from(p.y))
}

impl Workspace {
    /// The top-level instance drawn topmost under `point`, if the topmost thing there is one.
    fn instance_under(&self, point: Pos2) -> Option<InstanceId> {
        let scene = self.scene_id()?;
        let frame =
            flare_eval::evaluate_frame_without_camera(self.document(), scene, self.frame).ok()?;
        let index = *flare_render::hit_test_all(&frame, vec(point)).first()?;
        frame.shapes.get(index)?.path.first().copied()
    }

    /// The joint a press at `point` grabs: a joint's circle first, then the instance a joint
    /// carries (the art is what the press is on), then a bone's line, by its tip.
    fn joint_under(&self, point: Pos2) -> Option<JointRef> {
        let scene = self.scene_id()?;
        let radius = f64::from(self.handle_radius.max(f32::EPSILON));
        let document = self.document();
        let at = doc_point(point)?;
        bones::joint_near(document, scene, self.frame, at, radius)
            .or_else(|| {
                let instance = self.instance_under(point)?;
                bones::joint_of(document, scene, self.frame, instance)
            })
            .or_else(|| bones::bone_near(document, scene, self.frame, at, radius))
    }

    /// Where the Bone tool puts a joint on the instance under `point`: the middle of the
    /// instance, as Animate's transformation point is by default.
    fn bond_under(&self, point: Pos2) -> Option<(InstanceId, Pos2)> {
        let scene = self.scene_id()?;
        let at = self.hit(point)?;
        match ops::find_element(self.document(), scene, at)? {
            flare_document::Content::Instance(instance) => {
                let middle = ops::span_bounds(self.document(), scene, at)
                    .map_or(point, |bounds| bounds.center());
                Some((instance.id(), middle))
            }
            _ => None,
        }
    }

    /// A press on the stage. Returns whether a bone gesture started (the Bone tool always takes
    /// the press; the Selection tool only when it lands on an armature).
    pub(in crate::workspace) fn bone_press(&mut self, point: Pos2) -> bool {
        match self.tool {
            Tool::Bone => {
                let joint = self.joint_under(point);
                if let Some(joint) = joint {
                    self.bone_joint = Some(joint);
                }
                let from = joint
                    .and_then(|j| self.joint_instance(j))
                    .or_else(|| self.bond_under(point));
                self.bone_drag = from.map(|(from, at)| BoneDrag::Link {
                    from,
                    at,
                    current: point,
                });
                true
            }
            Tool::Select => match self.joint_under(point) {
                Some(joint) => {
                    self.bone_joint = Some(joint);
                    self.bone_drag = Some(BoneDrag::Pose {
                        joint,
                        grab: point,
                        current: point,
                    });
                    true
                }
                None => false,
            },
            _ => false,
        }
    }

    pub(in crate::workspace) fn bone_move(&mut self, point: Pos2) {
        if let Some(BoneDrag::Link { current, .. } | BoneDrag::Pose { current, .. }) =
            &mut self.bone_drag
        {
            *current = point;
        }
    }

    /// The release that ends a bone gesture: the bone is made, or the pose keyed, as one undo
    /// step.
    pub(in crate::workspace) fn bone_release(&mut self, point: Pos2) {
        let Some(drag) = self.bone_drag.take() else {
            return;
        };
        let Some(scene) = self.scene_id() else {
            return;
        };
        let document = self.document();
        let (made, label) = match drag {
            BoneDrag::Link { from, at, .. } => {
                let Some((to, to_at)) = self.bond_under(point) else {
                    let message = tr!(self, "tools-bone-drop-on-instance");
                    return self.report(message);
                };
                let count = document.scenes()[0]
                    .timeline()
                    .layers()
                    .iter()
                    .filter(|l| bones::is_armature_layer(l))
                    .count();
                let name = tr!(
                    self,
                    "name-new-armature-layer",
                    &[("number", &(count + 1).to_string())]
                );
                let made = bones::connect(
                    document,
                    scene,
                    self.frame,
                    Bond {
                        instance: from,
                        at: doc_point(at),
                    },
                    Bond {
                        instance: to,
                        at: doc_point(to_at),
                    },
                    &name,
                );
                (made, tr!(self, "tools-bone-add"))
            }
            BoneDrag::Pose { joint, grab, .. } => {
                let Some(target) = self.pose_target(joint, grab, point) else {
                    return;
                };
                let made = bones::pose_joint(
                    self.document(),
                    scene,
                    joint.layer,
                    self.frame,
                    joint.joint,
                    target,
                );
                (made, tr!(self, "tools-bone-pose"))
            }
        };
        self.apply_bones(made, &label);
    }

    /// Applies what a bone operation made, or says why it could not be made.
    pub(in crate::workspace) fn apply_bones(
        &mut self,
        made: Result<Vec<flare_commands::Command>, bones::BoneRefusal>,
        label: &str,
    ) {
        let result = made.map_err(|e| e.to_string()).and_then(|batch| {
            self.active_mut()
                .apply_labeled(batch, label)
                .map_err(|e| e.to_string())
        });
        match result {
            Ok(()) => self.status = None,
            Err(error) => {
                let message = tr!(self, "tools-bone-error", &[("error", &error)]);
                self.report(message);
            }
        }
    }

    /// Where a pose drag puts `joint`: where it is now, moved as far as the pointer has from
    /// `grab` to `point`.
    fn pose_target(&self, joint: JointRef, grab: Pos2, point: Pos2) -> Option<DVec2> {
        let (_, at) = self.joint_instance(joint)?;
        Some(vec(at) + vec(point) - vec(grab))
    }

    /// The instance `joint` carries and where the joint is now.
    fn joint_instance(&self, joint: JointRef) -> Option<(InstanceId, Pos2)> {
        let scene = self.scene_id()?;
        let (posed, _) =
            bones::armature_at(self.document(), scene, joint.layer, self.frame).ok()?;
        let at = posed.posed().get(joint.joint)?.0;
        let instance = posed.joint(joint.joint)?.instance().id();
        Some((instance, Pos2::new(at.x as f32, at.y as f32)))
    }

    /// The bones of every armature on the stage, the pose being dragged in place of its own, and
    /// the line of a bone being drawn. Wires the pointer to the gestures above; returns whether
    /// a bone gesture has the pointer, so the other tools leave it alone.
    pub(in crate::workspace) fn bone_stage(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        to_screen: &dyn Fn(Pos2) -> Pos2,
        to_doc: &dyn Fn(Pos2) -> Pos2,
    ) -> bool {
        let primary = egui::PointerButton::Primary;
        if matches!(self.tool, Tool::Bone | Tool::Select)
            && response.drag_started_by(primary)
            && let Some(p) = response.ctx.input(|i| i.pointer.press_origin())
        {
            self.bone_press(to_doc(p));
        }
        let active = self.bone_drag.is_some();
        if active
            && response.dragged_by(primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.bone_move(to_doc(p));
        }
        self.draw_bones(painter, to_screen);
        if active && response.drag_stopped_by(primary) {
            let at = response
                .interact_pointer_pos()
                .map(to_doc)
                .or(match self.bone_drag {
                    Some(BoneDrag::Link { current, .. } | BoneDrag::Pose { current, .. }) => {
                        Some(current)
                    }
                    None => None,
                });
            if let Some(at) = at {
                self.bone_release(at);
            }
        }
        // The Bone tool has nothing else to do with the pointer.
        active || self.tool == Tool::Bone
    }

    fn draw_bones(&self, painter: &egui::Painter, to_screen: &dyn Fn(Pos2) -> Pos2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let document = self.document();
        let mut skeletons = bones::skeletons(document, scene, self.frame);
        // The pose being dragged is shown where it would go.
        if let Some(BoneDrag::Pose {
            joint,
            grab,
            current,
        }) = self.bone_drag
            && let Some(target) = self.pose_target(joint, grab, current)
            && let Some(skeleton) = skeletons.iter_mut().find(|s| s.layer == joint.layer)
            && let Ok((posed, _)) = bones::armature_at(document, scene, joint.layer, self.frame)
        {
            let moved = if posed
                .joint(joint.joint)
                .is_some_and(|j| j.parent().is_none())
            {
                let mut pose = posed.pose().to_vec();
                if let Some(at) = posed.posed().get(joint.joint) {
                    pose[joint.joint].shift += target - at.0;
                }
                posed.with_pose(pose).unwrap_or(posed)
            } else {
                bones::reach(&posed, joint.joint, target)
            };
            for (place, (at, _)) in skeleton.joints.iter_mut().zip(moved.posed()) {
                place.0 = at;
            }
        }
        let screen = |p: DVec2| to_screen(Pos2::new(p.x as f32, p.y as f32));
        for skeleton in &skeletons {
            for (index, (at, parent)) in skeleton.joints.iter().enumerate() {
                if let Some(base) = parent.and_then(|p| skeleton.joints.get(p)) {
                    bone_shape(painter, screen(base.0), screen(*at));
                }
                let chosen = self.bone_joint
                    == Some(JointRef {
                        layer: skeleton.layer,
                        joint: index,
                    });
                let ink = if chosen {
                    flare_ui::theme::accent()
                } else {
                    BONE_INK
                };
                painter.circle_stroke(screen(*at), 4.0, Stroke::new(1.5, ink));
            }
        }
        if let Some(BoneDrag::Link { at, current, .. }) = self.bone_drag {
            bone_shape(painter, to_screen(at), to_screen(current));
        }
    }
}

/// A bone as Animate draws one: a narrow wedge from its base to its tip.
fn bone_shape(painter: &egui::Painter, base: Pos2, tip: Pos2) {
    let along = tip - base;
    let length = along.length();
    if length < 1.0 {
        return;
    }
    let side = Vec2::new(-along.y, along.x) / length * (length * 0.08).clamp(2.0, 6.0);
    let shoulder = base + along * 0.15;
    let outline = vec![base, shoulder + side, tip, shoulder - side];
    painter.add(egui::Shape::convex_polygon(
        outline,
        BONE_INK.gamma_multiply(0.35),
        Stroke::new(1.0, BONE_INK),
    ));
}

mod options;

#[allow(unused_imports)]
pub(in crate::workspace) use options::*;

impl Workspace {
    /// Properties for the Bone tool.
    pub(in crate::workspace) fn bone_options(&mut self, ui: &mut Ui) {
        self.bone_joint_options(ui);
        ui.weak(tr!(self, "tools-help-bone"));
    }
}
