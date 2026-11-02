//! Layers, frames and dragging spans in the timeline.

use flare_commands::{Command, layers};
use flare_core::{LayerId, SceneId};
use flare_document::Layer;
use flare_editor::frames;

use super::*;

impl Workspace {
    // ---- layers and frames -----------------------------------------------------------

    /// The layer frame operations act on: the chosen one, else the first.
    pub(super) fn active_layer(&self) -> Option<&Layer> {
        let layers = self.document().scenes().first()?.timeline().layers();
        self.current_layer
            .and_then(|id| layers.iter().find(|layer| layer.id() == id))
            .or_else(|| layers.first())
    }

    pub fn active_layer_id(&self) -> Option<LayerId> {
        self.active_layer().map(Layer::id)
    }

    pub fn select_layer(&mut self, layer: LayerId) {
        self.current_layer = Some(layer);
    }

    pub fn toggle_layer_visible(&mut self, layer: LayerId) {
        self.layer_edit("change visibility", layer, layers::toggle_visible);
    }

    pub fn toggle_layer_locked(&mut self, layer: LayerId) {
        self.layer_edit("change the lock", layer, layers::toggle_locked);
        self.validate_selection();
    }

    /// Turns the layer into a mask for the layer below it, or back into a normal layer.
    pub fn toggle_layer_mask(&mut self, layer: LayerId) {
        self.layer_edit("change the mask", layer, layers::toggle_mask);
        self.validate_selection();
    }

    /// Turns the layer into a motion guide for the layer below it, or back into a normal layer.
    pub fn toggle_layer_guide(&mut self, layer: LayerId) {
        self.layer_edit("change the motion guide", layer, layers::toggle_guide);
        self.validate_selection();
    }

    /// Asks `flare_commands::layers` for the commands that do `build` to `layer`, and runs them. A
    /// layer that is no longer there is not worth a message; any other refusal is shown.
    fn layer_edit(
        &mut self,
        what: &str,
        layer: LayerId,
        build: impl FnOnce(&Document, SceneId, LayerId) -> Result<Vec<Command>, layers::LayerRefusal>,
    ) {
        let Some(scene) = self.scene_id() else { return };
        match build(self.document(), scene, layer) {
            Ok(commands) => self.run_batch(commands, what),
            Err(layers::LayerRefusal::NoSuchLayer) => {}
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    pub(super) fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.document()
            .scenes()
            .first()?
            .timeline()
            .layers()
            .iter()
            .find(|l| l.id() == id)
    }

    pub fn rename_active_layer(&mut self, name: &str) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        match layers::rename(scene, layer, name) {
            Ok(commands) => self.run_batch(commands, "rename the layer"),
            Err(refusal) => self.report(refusal.to_string()),
        }
        self.validate_selection();
    }

    pub fn delete_active_layer(&mut self) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        match layers::delete(self.document(), scene, layer) {
            Ok(commands) => {
                self.run_batch(commands, "delete the layer");
                self.validate_selection();
                self.current_layer = None;
            }
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// Moves the active layer up (`-1`) or down (`+1`) in the stacking order.
    pub fn move_active_layer(&mut self, direction: i32) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        match layers::move_by(self.document(), scene, layer, direction) {
            Ok(commands) => self.run_batch(commands, "move the layer"),
            Err(refusal) => self.report(refusal.to_string()),
        }
        self.validate_selection();
    }

    /// What a frame operation is called in a message, in the interface's language.
    fn frame_op_name(&self, op: FrameOp) -> String {
        self.tr(match op {
            FrameOp::InsertFrame => "msg-op-insert-frame",
            FrameOp::RemoveFrame => "msg-op-remove-frame",
            FrameOp::InsertKeyframe => "msg-op-insert-keyframe",
            FrameOp::InsertBlankKeyframe => "msg-op-insert-blank-keyframe",
            FrameOp::ClearKeyframe => "msg-op-clear-keyframe",
            FrameOp::ConvertToKeyframes => "msg-op-convert-keyframes",
        })
    }

    /// Runs a frame operation on the active layer at the current frame.
    pub fn frame_edit(&mut self, op: FrameOp) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer()) else {
            return;
        };
        if layer.is_locked() {
            let text = tr!(
                self,
                "msg-cannot-locked",
                &[("op", &self.frame_op_name(op))]
            );
            self.report(text);
            return;
        }
        let commands = match op {
            FrameOp::InsertFrame => frames::insert_frame(scene, layer, self.frame),
            FrameOp::RemoveFrame => frames::remove_frame(scene, layer, self.frame),
            FrameOp::InsertKeyframe => frames::insert_keyframe(scene, layer, self.frame),
            FrameOp::InsertBlankKeyframe => frames::insert_blank_keyframe(scene, layer, self.frame),
            FrameOp::ClearKeyframe => frames::clear_keyframe(scene, layer, self.frame),
            FrameOp::ConvertToKeyframes => frames::convert_to_keyframes(scene, layer, self.frame),
        };
        match commands {
            Some(commands) => {
                self.run_batch(commands, op.label());
                self.set_frame(self.frame);
                self.validate_selection();
            }
            None => {
                let text = tr!(
                    self,
                    "msg-cannot-at-frame",
                    &[
                        ("op", &self.frame_op_name(op)),
                        ("frame", &self.frame.to_string()),
                    ],
                );
                self.report(text);
            }
        }
    }

    // ---- dragging spans in the timeline -----------------------------------------------

    /// Starts dragging the span of `layer` under the fractional frame position `at`: its right
    /// edge (within `edge` frames) resizes, the rest moves. False when there is no span there
    /// or the layer is locked, in which case the drag should scrub instead.
    pub(super) fn begin_span_drag(&mut self, layer: LayerId, at: f32, edge: f32) -> bool {
        if !at.is_finite() || at < 0.0 {
            return false;
        }
        let frame = at.floor() as u32;
        let Some(found) = self.layer(layer) else {
            return false;
        };
        if found.is_locked() {
            return false;
        }
        let Some(span) = found
            .spans()
            .iter()
            .find(|s| s.start() <= frame && u64::from(frame) < s.end())
        else {
            return false;
        };
        let grab = if span.end() as f32 - at <= edge {
            SpanGrab::Resize
        } else {
            SpanGrab::Move {
                offset: frame - span.start(),
            }
        };
        self.span_drag = Some(SpanDrag {
            layer,
            start: span.start(),
            grab,
            frame,
        });
        true
    }

    pub(super) fn update_span_drag(&mut self, frame: u32) {
        if let Some(drag) = &mut self.span_drag {
            drag.frame = frame;
        }
    }

    /// Where the dragged span would land: (layer, start, duration).
    pub(super) fn span_drag_preview(&self) -> Option<(LayerId, u32, u32)> {
        let drag = self.span_drag?;
        let span = self
            .layer(drag.layer)?
            .spans()
            .iter()
            .find(|s| s.start() == drag.start)?;
        Some(match drag.grab {
            SpanGrab::Move { offset } => (
                drag.layer,
                drag.frame.saturating_sub(offset),
                span.duration().get(),
            ),
            SpanGrab::Resize => (
                drag.layer,
                drag.start,
                drag.frame
                    .saturating_add(1)
                    .saturating_sub(drag.start)
                    .max(1),
            ),
        })
    }

    pub(super) fn end_span_drag(&mut self) {
        let Some((layer, start, duration)) = self.span_drag_preview() else {
            self.span_drag = None;
            return;
        };
        let Some(drag) = self.span_drag.take() else {
            return;
        };
        let (Some(scene), Some(found)) = (self.scene_id(), self.layer(layer)) else {
            return;
        };
        let (commands, what) = match drag.grab {
            SpanGrab::Move { .. } => (frames::move_span(scene, found, drag.start, start), "move"),
            SpanGrab::Resize => (
                frames::resize_span(scene, found, drag.start, duration),
                "resize",
            ),
        };
        let unchanged = match drag.grab {
            SpanGrab::Move { .. } => start == drag.start,
            SpanGrab::Resize => found
                .spans()
                .iter()
                .find(|s| s.start() == drag.start)
                .is_some_and(|s| s.duration().get() == duration),
        };
        match commands {
            Some(commands) => {
                self.run_batch(commands, what);
                self.set_frame(self.frame);
                self.validate_selection();
            }
            None if !unchanged => {
                let key = if what == "move" {
                    "msg-cannot-move-span"
                } else {
                    "msg-cannot-resize-span"
                };
                self.report(self.tr(key));
            }
            None => {}
        }
    }

    /// Selects frames `a..=b` (either order) of `layer` for copying.
    pub fn select_frames(&mut self, layer: LayerId, a: u32, b: u32) {
        if self.layer(layer).is_some() {
            self.frame_range = Some(FrameRange {
                layer,
                from: a.min(b),
                to: a.max(b),
            });
        }
    }

    /// Remembers the selected frames (or, with none selected, the current frame) of the active
    /// layer for Paste Frames.
    pub fn copy_frames(&mut self) {
        let (layer, from, to) = match self.frame_range {
            Some(r) => (r.layer, r.from, r.to),
            None => match self.active_layer_id() {
                Some(id) => (id, self.frame, self.frame),
                None => return,
            },
        };
        match self
            .layer(layer)
            .and_then(|l| frames::copy_frames(l, from, to))
        {
            Some(clip) => self.frame_clipboard = Some(clip),
            None => self.report(tr!(self, "msg-nothing-to-copy")),
        }
    }

    /// Pastes the remembered frames onto the active layer starting at the current frame,
    /// replacing what is there, and selects what was pasted.
    pub fn paste_frames(&mut self) {
        let (Some(scene), Some(layer), Some(clip)) = (
            self.scene_id(),
            self.active_layer(),
            self.frame_clipboard.clone(),
        ) else {
            return;
        };
        if layer.is_locked() {
            self.report(tr!(self, "msg-cannot-paste-locked"));
            return;
        }
        let id = layer.id();
        match frames::paste_frames(scene, layer, self.frame, &clip) {
            Some(commands) => {
                self.run_batch(commands, "paste frames");
                if self.status.is_none() {
                    self.select_frames(
                        id,
                        self.frame,
                        self.frame.saturating_add(clip.length() - 1),
                    );
                    self.set_frame(self.frame);
                    self.validate_selection();
                }
            }
            None => self.report(tr!(self, "msg-cannot-paste-past-end")),
        }
    }

    /// Adds a folder just above the active layer, in the same folder if it is in one, and puts
    /// the active layer in it. With no layer to hold, the folder goes at the top, empty.
    pub fn add_layer_folder(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        match layers::add_folder(self.document(), scene, self.active_layer_id()) {
            Ok((commands, folder)) => {
                self.run_batch(commands, "add a folder");
                if self.status.is_none() {
                    self.current_layer = Some(folder);
                }
            }
            Err(refusal) => self.report(refusal.to_string()),
        }
    }

    /// Puts the active layer in `folder` (a folder above it), or takes it out with `None`.
    pub fn move_active_layer_to_folder(&mut self, folder: Option<LayerId>) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        self.run_batch(
            layers::set_folder(scene, layer, folder),
            "move the layer into a folder",
        );
        self.validate_selection();
    }

    /// The folders the active layer could go into: those above it, other than the one it is in.
    pub(crate) fn folders_for_active_layer(&self) -> Vec<(LayerId, String)> {
        match (self.scene_id(), self.active_layer_id()) {
            (Some(scene), Some(layer)) => self
                .document()
                .scenes()
                .iter()
                .find(|s| s.id() == scene)
                .map(|s| s.timeline().folders_for(layer))
                .unwrap_or_default()
                .into_iter()
                .map(|l| (l.id(), l.name().to_owned()))
                .collect(),
            _ => Vec::new(),
        }
    }
}
