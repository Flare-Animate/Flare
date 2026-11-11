//! The selection following a move, scale, rotate or skew drag while the button is held.
//!
//! The stage is drawn from the document the drag would leave if it were let go now, made with
//! the very batch the release commits, so the artwork moves with the handles and what is let go
//! is what was shown. The copy is never put in the history: only the release makes an undo step.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use egui::Vec2;
use flare_commands::Command;
use flare_tools::free_transform::handles;
use glamx::{DAffine2, DVec2};

use super::*;

/// What a move or transform drag does to the selection if it is let go now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum DragEdit {
    /// Moved by this much, in stage units.
    Move(Vec2),
    /// Mapped through this matrix, in stage coordinates.
    Transform(DAffine2),
}

/// The document a held drag would leave, kept while the drag stays where it is.
pub(super) struct DragPreview {
    stamp: u64,
    document: Arc<Document>,
}

impl Workspace {
    /// What `drag` does to the selection if it is let go now; `None` for a drag that draws a
    /// shape, and for a scale, rotation or skew that would change nothing.
    pub(super) fn drag_edit(&self, drag: &Drag) -> Option<DragEdit> {
        match drag.mode {
            DragMode::Draw => None,
            DragMode::Move => {
                let mut delta = drag.current - drag.start;
                if self.display.snap_to_grid {
                    delta = snap_point(delta.to_pos2(), self.display.grid_size).to_vec2();
                }
                Some(DragEdit::Move(delta))
            }
            DragMode::Scale(handle) => {
                handles::scale_for(drag.bounds, handle, drag.current, self.uniform_scale)
                    .filter(|scale| *scale != (1.0, 1.0))
                    .map(|scale| DragEdit::Transform(handles::matrix(drag.bounds, handle, scale)))
            }
            DragMode::Rotate | DragMode::Skew(_) => self.drag_matrix(drag).map(DragEdit::Transform),
        }
    }

    /// Commits `edit` to the selection as one undo step.
    pub(super) fn commit_drag_edit(&mut self, edit: DragEdit) {
        match edit {
            DragEdit::Move(delta) => self.move_selection(delta),
            DragEdit::Transform(matrix) => self.apply_to_selection(matrix),
        }
    }

    /// The batch that `edit` stands for, or `None` when it would change nothing.
    fn drag_edit_commands(&self, edit: DragEdit) -> Option<Vec<Command>> {
        let scene = self.scene_id()?;
        let selection = self.selection_all();
        match edit {
            DragEdit::Move(delta) => {
                let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
                ops::translate_many(self.document(), scene, &selection, delta)
            }
            DragEdit::Transform(matrix) => {
                ops::transform_many(self.document(), scene, &selection, matrix)
            }
        }
    }

    /// The document to draw the stage from while a move or transform drag is held, with a stamp
    /// that tells one held state from another, or `None` when the stage shows the document as it
    /// is. A batch that would be refused previews nothing; the release reports why.
    ///
    /// Making the copy costs what the release costs (a move of 100 spans in a 5 000-layer
    /// document is 0.5 ms, see docs/PERFORMANCE.md) and is redone only when the drag moves.
    pub(super) fn previewed_document(&mut self) -> Option<(u64, Arc<Document>)> {
        let edit = self
            .drag
            .filter(|_| !self.drag_cancelled)
            .and_then(|drag| self.drag_edit(&drag))
            .filter(|edit| *edit != DragEdit::Move(Vec2::ZERO));
        let Some(edit) = edit else {
            self.drag_preview = None;
            return None;
        };
        let stamp = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (self.context_serial, self.active().revision()).hash(&mut hasher);
            match edit {
                DragEdit::Move(delta) => [delta.x, delta.y].map(f32::to_bits).hash(&mut hasher),
                DragEdit::Transform(m) => m.to_cols_array().map(f64::to_bits).hash(&mut hasher),
            }
            hasher.finish()
        };
        if let Some(kept) = self.drag_preview.as_ref().filter(|p| p.stamp == stamp) {
            return Some((stamp, Arc::clone(&kept.document)));
        }
        let document = self
            .drag_edit_commands(edit)
            .and_then(|commands| flare_commands::apply_all(self.document(), commands).ok())
            .flatten()
            .map(Arc::new);
        self.drag_preview = document.map(|document| DragPreview { stamp, document });
        self.drag_preview
            .as_ref()
            .map(|p| (p.stamp, Arc::clone(&p.document)))
    }
}
