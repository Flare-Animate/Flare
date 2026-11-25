//! The selection following a move, scale, rotate or skew drag while the button is held.
//!
//! The stage is drawn from the document the drag would leave if it were let go now, made with
//! the very batch the release commits, so the artwork moves with the handles and what is let go
//! is what was shown. The copy is never put in the history: only the release makes an undo step.

use std::sync::Arc;

use egui::{Pos2, Vec2};
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

/// What a held drag's preview was made from. It is compared by value, not hashed: a hash could
/// collide and show, then commit, the picture of one held state for another.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum PreviewKey {
    Drag {
        context: u64,
        revision: u64,
        edit: DragEdit,
    },
    Magnet {
        context: u64,
        revision: u64,
        frame: u32,
        from: Pos2,
        to: Pos2,
        radius: f32,
    },
    EdgeBend {
        context: u64,
        revision: u64,
        frame: u32,
        from: Pos2,
        to: Pos2,
    },
    /// A layer effect setting held in the Effects panel; `serial` numbers each held value.
    Effects {
        context: u64,
        revision: u64,
        serial: u64,
    },
}

/// The document a held drag would leave, kept while the drag stays where it is, with the stamp
/// the stage's render cache knows it by.
pub(super) struct HeldPreview {
    pub(super) key: PreviewKey,
    pub(super) stamp: u64,
    pub(super) document: Arc<Document>,
}

impl HeldPreview {
    /// The kept document, if it was made from `key`.
    pub(super) fn matching(&self, key: &PreviewKey) -> Option<(u64, Arc<Document>)> {
        (self.key == *key).then(|| (self.stamp, Arc::clone(&self.document)))
    }
}

/// The preview of a move or transform drag, and the numbering every held preview's stamp comes
/// from, so two previews never share a stamp and the render cache never mixes their pictures.
#[derive(Default)]
pub(super) struct DragPreviews {
    pub(super) held: Option<HeldPreview>,
    serial: u64,
}

impl DragPreviews {
    /// A stamp no preview made before has had.
    pub(super) fn next_stamp(&mut self) -> u64 {
        self.serial = self.serial.wrapping_add(1);
        self.serial
    }
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
                ops::translate_many_at(self.document(), scene, &selection, self.frame, delta)
                    .map(|(commands, _)| commands)
            }
            DragEdit::Transform(matrix) => {
                ops::transform_many_at(self.document(), scene, &selection, self.frame, matrix)
                    .map(|(commands, _)| commands)
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
        if self.effects.held.is_some() {
            return self.effects_preview();
        }
        if self.edge_bend_held() {
            return self.edge_bend_preview();
        }
        if self.magnet_held() {
            return self.magnet_preview();
        }
        let edit = self
            .drag
            .filter(|_| !self.drag_cancelled)
            .and_then(|drag| self.drag_edit(&drag))
            .filter(|edit| *edit != DragEdit::Move(Vec2::ZERO));
        let Some(edit) = edit else {
            self.drag_preview.held = None;
            return None;
        };
        let key = PreviewKey::Drag {
            context: self.context_serial,
            revision: self.active().revision(),
            edit,
        };
        if let Some(kept) = self
            .drag_preview
            .held
            .as_ref()
            .and_then(|p| p.matching(&key))
        {
            return Some(kept);
        }
        let document = self
            .drag_edit_commands(edit)
            .and_then(|commands| flare_commands::apply_all(self.document(), commands).ok())
            .flatten()
            .map(Arc::new);
        let stamp = self.drag_preview.next_stamp();
        self.drag_preview.held = document.map(|document| HeldPreview {
            key,
            stamp,
            document,
        });
        self.drag_preview
            .held
            .as_ref()
            .map(|p| (p.stamp, Arc::clone(&p.document)))
    }
}
