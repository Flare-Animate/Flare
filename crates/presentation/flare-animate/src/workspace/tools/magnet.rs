//! The Magnet in the workspace: a drag on the stage pulls the lines and shapes near where it
//! started, by `flare_tools::magnet`. The stage shows the pull while the button is held, made with
//! the very batch the release commits, so what is let go is what was shown, as one undo step.
//! With shapes selected only they are pulled; otherwise those on the layer being drawn on.

use std::hash::{Hash, Hasher};
use std::sync::Arc;

use egui::{Color32, Painter, Pos2, Stroke, Ui};
use flare_commands::Command;
use flare_document::{Content, Document};
use flare_tools::magnet::{
    self, DEFAULT_MAGNET_RADIUS, MAX_MAGNET_RADIUS, MIN_MAGNET_RADIUS, MagnetError, Pull,
};
use flare_ui::chrome;
use glamx::DVec2;

use crate::workspace::*;

/// What the Magnet is doing.
pub(in crate::workspace) struct MagnetState {
    /// How far the pull reaches, in stage units.
    pub(in crate::workspace) radius: f32,
    /// A pull being dragged: where it started and where the pointer is, on the stage.
    drag: Option<(Pos2, Pos2)>,
    /// The document the pull would leave, with the stamp of the pull it was made for.
    preview: Option<(u64, Arc<Document>)>,
}

impl Default for MagnetState {
    fn default() -> Self {
        Self {
            radius: DEFAULT_MAGNET_RADIUS as f32,
            drag: None,
            preview: None,
        }
    }
}

fn stage(p: Pos2) -> DVec2 {
    DVec2::new(f64::from(p.x), f64::from(p.y))
}

impl Workspace {
    pub(in crate::workspace) fn magnet_held(&self) -> bool {
        self.springs.magnet.drag.is_some()
    }

    /// Starts a pull at `point`, with the Magnet in hand.
    pub(in crate::workspace) fn begin_magnet(&mut self, point: Pos2) {
        if self.tool == Tool::Magnet {
            self.springs.magnet.drag = Some((point, point));
        }
    }

    pub(in crate::workspace) fn update_magnet(&mut self, point: Pos2) {
        if let Some((_, to)) = &mut self.springs.magnet.drag {
            *to = point;
        }
    }

    /// Drops a pull without committing it.
    pub(in crate::workspace) fn cancel_magnet(&mut self) -> bool {
        self.springs.magnet.preview = None;
        self.springs.magnet.drag.take().is_some()
    }

    /// Commits the pull as one undo step, unless it was cancelled. A pull that reaches nothing
    /// says so.
    pub(in crate::workspace) fn end_magnet(&mut self, cancelled: bool) {
        self.springs.magnet.preview = None;
        let Some((from, to)) = self.springs.magnet.drag.take() else {
            return;
        };
        if cancelled || from == to {
            return;
        }
        match self.magnet_commands(from, to) {
            Ok(commands) => self.run_batch(commands, "bend the line"),
            Err(MagnetError::NothingInReach { .. }) => {
                let message = tr!(self, "tools-magnet-nothing");
                self.report(message);
            }
            Err(error) => self.report(error.to_string()),
        }
    }

    /// The batch a pull from `from` to `to` stands for.
    fn magnet_commands(&self, from: Pos2, to: Pos2) -> Result<Vec<Command>, MagnetError> {
        let pull = Pull::new(
            stage(from),
            stage(to),
            f64::from(self.springs.magnet.radius),
        )?;
        let scene = self.scene_id().ok_or(MagnetError::NotFinite)?;
        let document = self.document();
        let in_reach = |at: &SpanRef| {
            matches!(ops::find_element(document, scene, *at),
                Some(Content::Shape(shape)) if magnet::reaches(shape, &pull))
        };
        let chosen: Vec<SpanRef> = self.selection_all().into_iter().filter(in_reach).collect();
        let targets = if chosen.is_empty() {
            match self.active_layer_id() {
                Some(layer) => magnet::in_reach(document, scene, self.frame, Some(layer), &pull),
                None => Vec::new(),
            }
        } else {
            chosen
        };
        magnet::pull(document, scene, &targets, &pull)
    }

    /// The document to draw the stage from while a pull is held, with a stamp that tells one
    /// pull from another; `None` when nothing is held or the pull reaches nothing.
    pub(in crate::workspace) fn magnet_preview(&mut self) -> Option<(u64, Arc<Document>)> {
        let (from, to) = self.springs.magnet.drag.filter(|(a, b)| a != b)?;
        let stamp = {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (self.context_serial, self.active().revision(), self.frame).hash(&mut hasher);
            [from.x, from.y, to.x, to.y, self.springs.magnet.radius]
                .map(f32::to_bits)
                .hash(&mut hasher);
            hasher.finish()
        };
        if let Some((kept, document)) = &self.springs.magnet.preview
            && *kept == stamp
        {
            return Some((stamp, Arc::clone(document)));
        }
        let document = self
            .magnet_commands(from, to)
            .ok()
            .and_then(|commands| flare_commands::apply_all(self.document(), commands).ok())
            .flatten()
            .map(Arc::new);
        self.springs.magnet.preview = document.map(|d| (stamp, d));
        self.springs
            .magnet
            .preview
            .as_ref()
            .map(|(stamp, document)| (*stamp, Arc::clone(document)))
    }

    /// The circle the pull reaches over: at the pointer, or where a held pull started.
    pub(in crate::workspace) fn draw_magnet(
        &self,
        painter: &Painter,
        to_screen: impl Fn(Pos2) -> Pos2,
        scale: f32,
    ) {
        if self.tool != Tool::Magnet {
            return;
        }
        let centre = match self.springs.magnet.drag {
            Some((from, _)) => Some(to_screen(from)),
            None => painter.ctx().pointer_hover_pos(),
        };
        let Some(centre) = centre.filter(|c| painter.clip_rect().contains(*c)) else {
            return;
        };
        let radius = self.springs.magnet.radius * scale;
        painter.circle_stroke(centre, radius + 1.0, Stroke::new(1.0, Color32::BLACK));
        painter.circle_stroke(centre, radius, Stroke::new(1.0, Color32::WHITE));
        if let Some((_, to)) = self.springs.magnet.drag {
            painter.line_segment(
                [centre, to_screen(to)],
                Stroke::new(1.0, crate::workspace::ANCHOR_INK),
            );
        }
    }

    /// The Magnet's controls in Properties: how far a pull reaches.
    pub(in crate::workspace) fn magnet_options(&mut self, ui: &mut Ui) {
        let label = tr!(self, "opt-size");
        let range = MIN_MAGNET_RADIUS as f32..=MAX_MAGNET_RADIUS as f32;
        chrome::slider(
            ui,
            &label,
            &mut self.springs.magnet.radius,
            range,
            " px",
            true,
        );
        ui.weak(tr!(self, "tools-help-magnet"));
    }
}
