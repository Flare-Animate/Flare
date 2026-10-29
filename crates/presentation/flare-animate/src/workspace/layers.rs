//! Layers, frames and dragging spans in the timeline.

use flare_commands::{Command, LayerChange};
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

    pub(super) fn layer_command(
        &mut self,
        what: &str,
        make: impl FnOnce(SceneId, LayerId) -> Command,
    ) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        self.run_batch(vec![make(scene, layer)], what);
        self.validate_selection();
    }

    pub fn toggle_layer_visible(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_visible) else {
            return;
        };
        let change = LayerChange::Visible(!current);
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change,
            }],
            "change visibility",
        );
    }

    pub fn toggle_layer_locked(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_locked) else {
            return;
        };
        let change = LayerChange::Locked(!current);
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change,
            }],
            "change the lock",
        );
        self.validate_selection();
    }

    /// Turns the layer into a mask for the layer below it, or back into a normal layer.
    pub fn toggle_layer_mask(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_mask) else {
            return;
        };
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change: LayerChange::Mask(!current),
            }],
            "change the mask",
        );
        self.validate_selection();
    }

    /// Turns the layer into a motion guide for the layer below it, or back into a normal layer.
    pub fn toggle_layer_guide(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_guide) else {
            return;
        };
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change: LayerChange::Guide(!current),
            }],
            "change the motion guide",
        );
        self.validate_selection();
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
        let name = name.trim();
        if name.is_empty() {
            self.report("A layer needs a name");
            return;
        }
        let name = name.to_owned();
        self.layer_command("rename the layer", |scene, layer| Command::UpdateLayer {
            scene,
            layer,
            change: LayerChange::Name(name),
        });
    }

    pub fn delete_active_layer(&mut self) {
        let count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        if count <= 1 {
            self.report("A scene needs at least one layer");
            return;
        }
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        // What a folder held is let out of it into the folder's own parent first.
        let outer = self.layer(layer).and_then(Layer::parent);
        let mut commands: Vec<Command> = self
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
            .unwrap_or_default()
            .iter()
            .filter(|l| l.parent() == Some(layer))
            .map(|held| Command::UpdateLayer {
                scene,
                layer: held.id(),
                change: LayerChange::Parent(outer),
            })
            .collect();
        // Sounds only this layer played go with it, so deleting it leaves nothing behind.
        commands.push(Command::RemoveLayer { scene, layer });
        commands.extend(
            self.orphaned_sounds(layer)
                .into_iter()
                .map(Command::RemoveSound),
        );
        self.run_batch(commands, "delete the layer");
        self.validate_selection();
        self.current_layer = None;
    }

    /// Moves the active layer up (`-1`) or down (`+1`) in the stacking order.
    pub fn move_active_layer(&mut self, direction: i32) {
        let Some(id) = self.active_layer_id() else {
            return;
        };
        let Some(layers) = self
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
        else {
            return;
        };
        let Some(from) = layers.iter().position(|l| l.id() == id) else {
            return;
        };
        let to = if direction < 0 {
            from.checked_sub(1)
        } else {
            Some(from + 1)
        };
        match to {
            Some(index) if index < layers.len() => {
                self.layer_command("move the layer", |scene, layer| Command::MoveLayer {
                    scene,
                    layer,
                    index,
                });
            }
            _ => self.report("The layer is already at the edge"),
        }
    }

    /// Runs a frame operation on the active layer at the current frame.
    pub fn frame_edit(&mut self, op: FrameOp) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer()) else {
            return;
        };
        if layer.is_locked() {
            self.report(format!("Cannot {}: the layer is locked", op.label()));
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
            None => self.report(format!(
                "Cannot {} at frame {} of this layer",
                op.label(),
                self.frame
            )),
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
            None if !unchanged => self.report(format!(
                "Cannot {what} the span there: it would overlap another one"
            )),
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
            None => self.report("There is nothing to copy in those frames"),
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
            self.report("Cannot paste frames: the layer is locked");
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
            None => self.report("Cannot paste frames there: they would run past the last frame"),
        }
    }

    /// Adds a folder just above the active layer, in the same folder if it is in one, and puts
    /// the active layer in it. With no layer to hold, the folder goes at the top, empty.
    pub fn add_layer_folder(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let layers = self
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
            .unwrap_or_default();
        let held = self
            .active_layer_id()
            .and_then(|id| layers.iter().position(|l| l.id() == id));
        let folders = layers.iter().filter(|l| l.is_folder()).count();
        let id = LayerId::new();
        let outer = held.and_then(|i| layers[i].parent());
        let folder = match Layer::new(id, format!("Folder {}", folders + 1), vec![]) {
            Ok(layer) => layer.with_folder(true).with_parent(outer),
            Err(error) => return self.report(error.to_string()),
        };
        let mut commands = vec![
            Command::AddLayer {
                scene,
                layer: folder,
            },
            Command::MoveLayer {
                scene,
                layer: id,
                index: held.unwrap_or(0),
            },
        ];
        if let Some(i) = held {
            commands.push(Command::UpdateLayer {
                scene,
                layer: layers[i].id(),
                change: LayerChange::Parent(Some(id)),
            });
        }
        self.run_batch(commands, "add a folder");
        if self.status.is_none() {
            self.current_layer = Some(id);
        }
    }

    /// Puts the active layer in `folder` (a folder above it), or takes it out with `None`.
    pub fn move_active_layer_to_folder(&mut self, folder: Option<LayerId>) {
        self.layer_command("move the layer into a folder", |scene, layer| {
            Command::UpdateLayer {
                scene,
                layer,
                change: LayerChange::Parent(folder),
            }
        });
    }

    /// The folders the active layer could go into: those above it, other than the one it is in.
    pub(crate) fn folders_for_active_layer(&self) -> Vec<(LayerId, String)> {
        let Some(active) = self.active_layer_id() else {
            return Vec::new();
        };
        let layers = self
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
            .unwrap_or_default();
        let Some(position) = layers.iter().position(|l| l.id() == active) else {
            return Vec::new();
        };
        let current = layers[position].parent();
        layers[..position]
            .iter()
            .filter(|l| l.is_folder() && Some(l.id()) != current)
            .map(|l| (l.id(), l.name().to_owned()))
            .collect()
    }
}
