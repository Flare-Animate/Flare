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

use crate::workspace::*;

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

    pub(in crate::workspace) fn has_camera(&self) -> bool {
        self.camera_layer().is_some()
    }

    /// The camera at the current frame: the layer's, or the one that shows the stage as it is.
    fn camera_here(&self) -> Option<Camera> {
        flare_editor::camera_now(self.document(), self.scene_id()?, self.frame)
    }

    /// Makes the camera `camera` from `frame` on (see `flare_editor::set_camera_key`), as one
    /// undo step.
    pub(in crate::workspace) fn key_camera(
        &mut self,
        frame: u32,
        camera: Camera,
        motion: Option<(Tween, i8)>,
        label: &str,
    ) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let name = tr!(self, "name-new-camera-layer");
        let Some(batch) = flare_editor::set_camera_key_named(
            self.document(),
            scene,
            frame,
            camera,
            motion,
            &name,
        ) else {
            let message = tr!(
                self,
                "tools-camera-max-frame",
                &[("frame", &flare_editor::MAX_CAMERA_FRAME.to_string())],
            );
            return self.report(message);
        };
        match self.active_mut().apply_labeled(batch, label) {
            Ok(()) => self.status = None,
            Err(error) => {
                let message = tr!(
                    self,
                    "tools-camera-move-error",
                    &[("error", &error.to_string())]
                );
                self.report(message);
            }
        }
    }

    /// The Camera tool was picked: a scene without a camera gets one, looking at the stage as it
    /// is.
    pub(in crate::workspace) fn ensure_camera(&mut self) {
        if self.has_camera() {
            return;
        }
        let centered = Camera::centered(&self.document().stage());
        let label = tr!(self, "tools-camera-add");
        self.key_camera(0, centered, None, &label);
    }

    /// Shows the stage through the camera, or as it is edited. The picture has to be drawn again.
    pub(in crate::workspace) fn set_camera_preview(&mut self, on: bool) {
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
    pub(in crate::workspace) fn pan_camera(&mut self, shift: Vec2) {
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
            Some(moved) => {
                let label = tr!(self, "tools-camera-move");
                self.key_camera(self.frame, moved, None, &label);
            }
            None => {
                let message = tr!(self, "tools-camera-cannot-go");
                self.report(message);
            }
        }
    }

    /// What the stage does for the camera: with the Camera tool, mark what the camera sees and
    /// let a drag move it; while previewing, say so and ignore the pointer.
    pub(in crate::workspace) fn camera_stage(
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
                tr!(self, "tools-camera-previewing"),
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
    pub(in crate::workspace) fn camera_options(&mut self, ui: &mut Ui) {
        let Some(camera) = self.camera_here() else {
            ui.weak(tr!(self, "tools-no-scene-period"));
            return;
        };
        let (mut x, mut y) = (camera.center().as_vec().x, camera.center().as_vec().y);
        let mut zoom = camera.zoom() * 100.0;
        let mut turn = camera.rotation().to_degrees();
        let mut changed = false;
        for (label, value, speed, suffix) in [
            ("X", &mut x, 1.0, ""),
            ("Y", &mut y, 1.0, ""),
            ("opt-zoom", &mut zoom, 1.0, "%"),
            ("opt-rotation", &mut turn, 0.5, "°"),
        ] {
            // The axes' letters are not words; the other two are looked up.
            let label = if label.starts_with("opt-") {
                self.tr(label)
            } else {
                label.to_owned()
            };
            chrome::row(ui, &label, |ui| {
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
                Ok(camera) => {
                    let label = tr!(self, "tools-camera-move");
                    self.key_camera(self.frame, camera, None, &label);
                }
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
                .checkbox(&mut moving, tr!(self, "tools-camera-moving"))
                .changed()
            {
                let tween = if moving { Tween::Linear } else { Tween::None };
                let label = tr!(self, "tools-camera-motion");
                self.key_camera(start, held, Some((tween, ease)), &label);
            }
            if moving {
                let mut ease = i32::from(ease);
                let ease_label = tr!(self, "tools-camera-ease");
                chrome::row(ui, &ease_label, |ui| {
                    if ui
                        .add(egui::DragValue::new(&mut ease).range(-100..=100))
                        .changed()
                    {
                        let ease = i8::try_from(ease).unwrap_or(0);
                        let label = tr!(self, "tools-camera-ease-label");
                        self.key_camera(start, held, Some((Tween::Linear, ease)), &label);
                    }
                });
            }
        }
        let mut preview = self.camera_preview;
        if ui
            .checkbox(&mut preview, tr!(self, "bar-camera-preview"))
            .changed()
        {
            self.set_camera_preview(preview);
        }
        if ui.button(tr!(self, "tools-camera-remove")).clicked()
            && let (Some(scene), Some(layer)) =
                (self.scene_id(), self.camera_layer().map(Layer::id))
        {
            let label = tr!(self, "tools-camera-remove");
            let removed = self
                .active_mut()
                .apply_labeled(vec![Command::RemoveLayer { scene, layer }], &label);
            if let Err(error) = removed {
                let message = tr!(
                    self,
                    "tools-camera-remove-error",
                    &[("error", &error.to_string())],
                );
                self.report(message);
            }
            self.set_camera_preview(false);
        }
        ui.weak(tr!(self, "tools-camera-help"));
    }
}
