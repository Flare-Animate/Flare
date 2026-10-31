//! The Camera tool: pointing the scene's camera, and looking through it.
//!
//! The camera is a layer of keyframes (see `flare_editor::set_camera_key`). While the stage is
//! edited it shows the scene as drawn, with a frame marking what the camera sees; the preview
//! shows the picture itself, through the camera, and cannot be edited.

use egui::{Color32, Pos2, Stroke, Ui, Vec2};
use flare_commands::Command;
use flare_core::DocumentPoint;
use flare_document::{Camera, Content, Layer, Tween};
use flare_ui::chrome;
use glamx::{DAffine2, DVec2};

use super::*;

/// Animate's camera orange.
const CAMERA_INK: Color32 = Color32::from_rgb(235, 150, 60);

impl Workspace {
    /// The camera layer of the scene being edited.
    fn camera_layer(&self) -> Option<&Layer> {
        self.document()
            .scenes()
            .first()?
            .timeline()
            .layers()
            .iter()
            .find(|l| l.is_camera())
    }

    pub(super) fn has_camera(&self) -> bool {
        self.camera_layer().is_some()
    }

    /// The camera at the current frame: the layer's, or the one that shows the stage as it is.
    fn camera_here(&self) -> Option<Camera> {
        flare_editor::camera_now(self.document(), self.scene_id()?, self.frame)
    }

    /// Makes the camera `camera` from `frame` on (see `flare_editor::set_camera_key`), as one
    /// undo step.
    pub(super) fn key_camera(
        &mut self,
        frame: u32,
        camera: Camera,
        motion: Option<(Tween, i8)>,
        label: &str,
    ) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let Some(batch) =
            flare_editor::set_camera_key(self.document(), scene, frame, camera, motion)
        else {
            return self.report(format!(
                "The camera can be keyed up to frame {}",
                flare_editor::MAX_CAMERA_FRAME
            ));
        };
        match self.active_mut().apply_labeled(batch, label) {
            Ok(()) => self.status = None,
            Err(error) => self.report(format!("Cannot move the camera: {error}")),
        }
    }

    /// The Camera tool was picked: a scene without a camera gets one, looking at the stage as it
    /// is.
    pub(super) fn ensure_camera(&mut self) {
        if self.has_camera() {
            return;
        }
        let centered = Camera::centered(&self.document().stage());
        self.key_camera(0, centered, None, "Add camera");
    }

    /// Shows the stage through the camera, or as it is edited. The picture has to be drawn again.
    pub(super) fn set_camera_preview(&mut self, on: bool) {
        if self.camera_preview == on {
            return;
        }
        self.camera_preview = on;
        self.cancel_drag();
        self.texture = None;
        self.pending_render = None;
    }

    /// Moves the camera so the picture follows a drag of `shift` stage units, keying it at the
    /// current frame.
    pub(super) fn pan_camera(&mut self, shift: Vec2) {
        let Some(camera) = self.camera_here() else {
            return;
        };
        let picture = DVec2::new(f64::from(shift.x), f64::from(shift.y));
        // What the picture moves by is the scene's shift, turned and magnified; the camera goes
        // the other way.
        let scene =
            DAffine2::from_angle(camera.rotation()).transform_vector2(picture) / camera.zoom();
        let center = camera.center().as_vec() - scene;
        let moved = DocumentPoint::new(center.x, center.y)
            .ok()
            .and_then(|c| Camera::new(c, camera.zoom(), camera.rotation()).ok());
        match moved {
            Some(moved) => self.key_camera(self.frame, moved, None, "Move camera"),
            None => self.report("The camera cannot go there"),
        }
    }

    /// What the stage does for the camera: with the Camera tool, mark what the camera sees and
    /// let a drag move it; while previewing, say so and ignore the pointer.
    pub(super) fn camera_stage(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        scale: f32,
        to_screen: &dyn Fn(Pos2) -> Pos2,
    ) -> bool {
        if self.camera_preview {
            painter.text(
                response.rect.left_top() + Vec2::new(10.0, 8.0),
                egui::Align2::LEFT_TOP,
                "Camera preview: turn it off to edit",
                egui::FontId::proportional(13.0),
                CAMERA_INK,
            );
            return true;
        }
        if self.tool != Tool::Camera || !self.has_camera() {
            return false;
        }
        if let Some(camera) = self.camera_here() {
            let stage = self.document().stage();
            let inverse = camera.view(&stage).inverse();
            let corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].map(|(u, v)| {
                let at = inverse.transform_point2(DVec2::new(
                    u * f64::from(stage.width()),
                    v * f64::from(stage.height()),
                ));
                to_screen(Pos2::new(at.x as f32, at.y as f32))
            });
            painter.add(egui::Shape::closed_line(
                corners.to_vec(),
                Stroke::new(2.0, CAMERA_INK),
            ));
        }
        let primary = egui::PointerButton::Primary;
        if response.drag_stopped_by(primary)
            && let (Some(from), Some(to)) = (
                response.ctx.input(|i| i.pointer.press_origin()),
                response.interact_pointer_pos(),
            )
        {
            self.pan_camera((to - from) / scale.max(f32::EPSILON));
        }
        false
    }

    /// The Camera tool's controls in Properties: where the camera looks at this frame, and how
    /// its keyframe moves to the next.
    pub(super) fn camera_options(&mut self, ui: &mut Ui) {
        let Some(camera) = self.camera_here() else {
            ui.weak("This document has no scene.");
            return;
        };
        let (mut x, mut y) = (camera.center().as_vec().x, camera.center().as_vec().y);
        let mut zoom = camera.zoom() * 100.0;
        let mut turn = camera.rotation().to_degrees();
        let mut changed = false;
        for (label, value, speed, suffix) in [
            ("X", &mut x, 1.0, ""),
            ("Y", &mut y, 1.0, ""),
            ("Zoom", &mut zoom, 1.0, "%"),
            ("Rotation", &mut turn, 0.5, "°"),
        ] {
            chrome::row(ui, label, |ui| {
                changed |= ui
                    .add(egui::DragValue::new(value).speed(speed).suffix(suffix))
                    .changed();
            });
        }
        if changed {
            let made = DocumentPoint::new(x, y)
                .map_err(|e| e.to_string())
                .and_then(|c| {
                    Camera::new(c, zoom / 100.0, turn.to_radians()).map_err(|e| e.to_string())
                });
            match made {
                Ok(camera) => self.key_camera(self.frame, camera, None, "Move camera"),
                Err(message) => self.report(message),
            }
        }
        // How the keyframe the camera is in moves on to the next one.
        let keyframe = self.camera_layer().and_then(|layer| {
            let span = layer.span_at(self.frame)?;
            match span.content() {
                Content::Camera(c) => Some((span.start(), *c, span.tween(), span.ease())),
                _ => None,
            }
        });
        if let Some((start, held, tween, ease)) = keyframe {
            let mut moving = tween == Tween::Linear;
            if ui
                .checkbox(&mut moving, "Move to the next keyframe")
                .changed()
            {
                let tween = if moving { Tween::Linear } else { Tween::None };
                self.key_camera(start, held, Some((tween, ease)), "Camera motion");
            }
            if moving {
                let mut ease = i32::from(ease);
                chrome::row(ui, "Ease", |ui| {
                    if ui
                        .add(egui::DragValue::new(&mut ease).range(-100..=100))
                        .changed()
                    {
                        let ease = i8::try_from(ease).unwrap_or(0);
                        self.key_camera(start, held, Some((Tween::Linear, ease)), "Camera ease");
                    }
                });
            }
        }
        let mut preview = self.camera_preview;
        if ui
            .checkbox(&mut preview, "Preview through the camera")
            .changed()
        {
            self.set_camera_preview(preview);
        }
        if ui.button("Remove camera").clicked()
            && let (Some(scene), Some(layer)) =
                (self.scene_id(), self.camera_layer().map(Layer::id))
        {
            let removed = self
                .active_mut()
                .apply_labeled(vec![Command::RemoveLayer { scene, layer }], "Remove camera");
            if let Err(error) = removed {
                self.report(format!("Cannot remove the camera: {error}"));
            }
            self.set_camera_preview(false);
        }
        ui.weak("Drag on the stage to move the camera, or type where it looks. Each change keys the camera at this frame.");
    }
}
