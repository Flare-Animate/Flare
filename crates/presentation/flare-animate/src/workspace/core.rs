//! Accessors and small state changes.

use egui::Vec2;
use flare_commands::Command;
use flare_core::Rgba8;
use flare_document::{Document, Stage};
use flare_history::EditorSession;
use std::path::Path;

use super::*;

impl Workspace {
    /// Zooms the stage about the center of its panel (menu commands).
    pub(super) fn zoom_stage(&mut self, factor: f32) {
        let extent = self.stage_extent();
        self.stage_view
            .zoom_center(factor, self.stage_panel, extent);
    }

    pub(super) fn stage_extent(&self) -> Vec2 {
        let stage = self.document().stage();
        Vec2::new(stage.width() as f32, stage.height() as f32)
    }

    /// Changes the stage size as one undo step, keeping the background.
    pub fn set_stage_size(&mut self, width: u32, height: u32) {
        let current = self.document().stage();
        match Stage::new(width, height, current.background()) {
            Ok(stage) => self.run_batch(vec![Command::SetStage(stage)], "resize the stage"),
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Changes the stage background as one undo step, keeping the size.
    pub fn set_stage_background(&mut self, background: Rgba8) {
        let current = self.document().stage();
        if current.background() == background {
            return;
        }
        match Stage::new(current.width(), current.height(), background) {
            Ok(stage) => self.run_batch(vec![Command::SetStage(stage)], "change the background"),
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Sets the document's frame rate (0.01 to 240 fps) as one undo step.
    pub fn set_frame_rate(&mut self, fps: f64) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the frame rate");
            return;
        }
        if !fps.is_finite() || !(0.01..=240.0).contains(&fps) {
            self.report("The frame rate must be between 0.01 and 240 fps");
            return;
        }
        let milli = (fps * 1000.0).round() as u32;
        match flare_core::FrameRate::new(milli, 1000) {
            Ok(rate) if rate != self.document().frame_rate() => {
                self.run_batch(vec![Command::SetFrameRate(rate)], "change the frame rate");
            }
            Ok(_) => {}
            Err(error) => self.report(error.to_string()),
        }
    }

    /// The session edits go to: the innermost symbol being edited, else the document's.
    pub(super) fn active(&self) -> &EditorSession {
        self.contexts.last().map_or(&self.session, |c| &c.session)
    }

    pub(super) fn active_mut(&mut self) -> &mut EditorSession {
        match self.contexts.last_mut() {
            Some(context) => &mut context.session,
            None => &mut self.session,
        }
    }

    /// The document being edited right now (a symbol's scratch document in symbol context).
    pub fn document(&self) -> &Document {
        self.active().document()
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty() || self.contexts.iter().any(|c| c.session.is_dirty())
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Window title with an unsaved-changes marker.
    pub fn title(&self) -> String {
        let name = self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "Untitled".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let dirty = if self.is_dirty() { "*" } else { "" };
        format!("{name}{dirty} - Flare")
    }

    /// Width at the left of the top bar the window's own buttons cover; non-zero when the shell
    /// draws no title bar of its own.
    pub fn set_titlebar_inset(&mut self, inset: f32) {
        self.titlebar_inset = inset;
    }

    /// Returns (and clears) a window move or zoom the top bar asked for.
    pub fn take_window_request(&mut self) -> Option<WindowRequest> {
        self.window_request.take()
    }

    /// Returns (and clears) a dialog the shell should show.
    pub fn take_dialog(&mut self) -> Option<DialogRequest> {
        self.dialog.take()
    }

    pub(super) fn report(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self.log.last() != Some(&message) {
            self.log.push(message.clone());
            if self.log.len() > MAX_LOG_LINES {
                self.log.remove(0);
            }
        }
        self.status = Some(message);
    }

    pub(super) fn scene_frames(&self) -> u32 {
        let frames = self
            .document()
            .scenes()
            .first()
            .map_or(0, |scene| scene.timeline().frame_count());
        u32::try_from(frames).unwrap_or(u32::MAX).max(1)
    }

    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame.min(self.scene_frames() - 1);
    }
}
