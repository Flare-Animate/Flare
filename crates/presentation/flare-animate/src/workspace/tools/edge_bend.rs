//! Dragging the edge of a drawn line with the Selection tool: pressing on the edge of merged
//! drawing that is not selected takes hold of it, and the drag curves that edge through the
//! pointer while its ends stay, by `flare_tools::reshape`. The stage shows the bend while the
//! button is held, made with the very batch the release commits, so what is let go is what was
//! shown, as one undo step.

use std::sync::Arc;

use egui::Pos2;
use flare_commands::Command;
use flare_document::Document;
use flare_tools::reshape::{self, EdgeGrab};
use glamx::DVec2;

use crate::workspace::drag_preview::{HeldPreview, PreviewKey};
use crate::workspace::*;

/// An edge in hand.
#[derive(Default)]
pub(in crate::workspace) struct EdgeBendState {
    /// The edge taken, with where the press was and where the pointer is now.
    held: Option<(EdgeGrab, Pos2, Pos2)>,
    /// The document the bend would leave.
    preview: Option<HeldPreview>,
}

fn stage(p: Pos2) -> DVec2 {
    DVec2::new(f64::from(p.x), f64::from(p.y))
}

impl Workspace {
    pub(in crate::workspace) fn edge_bend_held(&self) -> bool {
        self.springs.edge_bend.held.is_some()
    }

    /// Takes hold of the merged edge under `point`, if there is one. Pressing on what is already
    /// selected moves it instead, so the caller asks only when nothing selected is under the
    /// pointer.
    pub(in crate::workspace) fn begin_edge_bend(&mut self, point: Pos2) -> bool {
        let Some(scene) = self.scene_id() else {
            return false;
        };
        let grab = reshape::edge_at(
            self.document(),
            scene,
            self.frame,
            None,
            stage(point),
            f64::from(self.handle_radius),
        );
        self.springs.edge_bend.held = grab.map(|grab| (grab, point, point));
        self.springs.edge_bend.held.is_some()
    }

    pub(in crate::workspace) fn update_edge_bend(&mut self, point: Pos2) {
        if let Some((_, _, to)) = &mut self.springs.edge_bend.held {
            *to = point;
        }
    }

    /// Lets go of the edge without committing it.
    pub(in crate::workspace) fn cancel_edge_bend(&mut self) -> bool {
        self.springs.edge_bend.preview = None;
        self.springs.edge_bend.held.take().is_some()
    }

    /// Commits the bend as one undo step, unless it was cancelled or moved nothing.
    pub(in crate::workspace) fn end_edge_bend(&mut self, cancelled: bool) {
        self.springs.edge_bend.preview = None;
        let Some((grab, from, to)) = self.springs.edge_bend.held.take() else {
            return;
        };
        if cancelled || from == to {
            return;
        }
        match self.edge_bend_commands(&grab, to) {
            Ok(commands) => self.run_batch(commands, "bend the line"),
            Err(error) => self.report(error.to_string()),
        }
    }

    fn edge_bend_commands(
        &self,
        grab: &EdgeGrab,
        to: Pos2,
    ) -> Result<Vec<Command>, reshape::ReshapeError> {
        let scene = self.scene_id().ok_or(reshape::ReshapeError::Refused)?;
        reshape::bend(self.document(), scene, grab, stage(to))
    }

    /// The document to draw the stage from while an edge is held, with a stamp that tells one
    /// bend from another; `None` when nothing is held or the bend is refused.
    pub(in crate::workspace) fn edge_bend_preview(&mut self) -> Option<(u64, Arc<Document>)> {
        let (grab, from, to) = self.springs.edge_bend.held.filter(|(_, a, b)| a != b)?;
        let key = PreviewKey::EdgeBend {
            context: self.context_serial,
            revision: self.active().revision(),
            frame: self.frame,
            from,
            to,
        };
        if let Some(kept) = self
            .springs
            .edge_bend
            .preview
            .as_ref()
            .and_then(|p| p.matching(&key))
        {
            return Some(kept);
        }
        let document = self
            .edge_bend_commands(&grab, to)
            .ok()
            .and_then(|commands| flare_commands::apply_all(self.document(), commands).ok())
            .flatten()
            .map(Arc::new);
        let stamp = self.drag_preview.next_stamp();
        self.springs.edge_bend.preview = document.map(|document| HeldPreview {
            key,
            stamp,
            document,
        });
        self.springs
            .edge_bend
            .preview
            .as_ref()
            .map(|p| (p.stamp, Arc::clone(&p.document)))
    }
}
