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

    /// Screen points per stage unit, for distances a user measures on screen: an arrow-key
    /// nudge moves one screen pixel however far in that means on the stage (issue #213).
    pub(super) fn stage_scale(&self) -> f32 {
        self.stage_view.scale(self.stage_panel, self.stage_extent())
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
            self.report(tr!(self, "msg-leave-symbol-fps"));
            return;
        }
        match flare_core::FrameRate::from_per_second(fps) {
            Ok(rate) if rate != self.document().frame_rate() => {
                self.run_batch(vec![Command::SetFrameRate(rate)], "change the frame rate");
            }
            Ok(_) => {}
            Err(_) => self.report(tr!(self, "msg-fps-range")),
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

    /// Window title: the document's saved name with an unsaved-changes marker, or just the
    /// application's name on the Home page, where no document is on show.
    pub fn title(&self) -> String {
        if self.windows.home {
            return "Flare".to_owned();
        }
        let name = self.path.as_deref().and_then(Path::file_name).map_or_else(
            || tr!(self, "untitled"),
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

    /// The pen pressure shared with a native source, for window systems that pass the pen on as
    /// a mouse: whoever holds a clone writes to it and the stage reads it.
    pub fn pen_feed(&self) -> flare_input::PenFeed {
        self.pen_feed.clone()
    }

    /// Draws the window's own minimize, maximize and close buttons in the top bar, for a window
    /// the system gives no title bar (Linux and Windows).
    pub fn set_window_controls(&mut self, shown: bool) {
        self.window_controls = shown;
    }

    /// Tells the top bar whether the window is maximized, so its button offers the other thing.
    pub fn set_window_maximized(&mut self, maximized: bool) {
        self.window_maximized = maximized;
    }

    /// Whether the interface is driven with fingers: the rail, the dock's tabs and egui's own
    /// widgets grow to sizes a thumb can land on. The surface decides (the web build detects a
    /// phone; the Android shell always says yes), and says so before the first frame.
    pub fn set_touch(&mut self, touch: bool) {
        self.touch = touch;
    }

    pub fn touch(&self) -> bool {
        self.touch
    }

    /// Returns (and clears) a window move or zoom the top bar asked for.
    pub fn take_window_request(&mut self) -> Option<WindowRequest> {
        self.window_request.take()
    }

    /// Returns (and clears) a dialog the shell should show.
    pub fn take_dialog(&mut self) -> Option<DialogRequest> {
        self.dialog.take()
    }

    /// Says something to the person through the status bar and the Output panel, for a shell
    /// that could not do what was asked (a page that cannot import sounds, a failed download).
    pub fn report_message(&mut self, message: &str) {
        self.report(message);
    }

    /// The message called `key` in the interface's language, for a shell that shows its own
    /// windows (native dialogs, the Test Movie window) and has no translator of its own.
    pub fn interface_text(&self, key: &str) -> String {
        self.tr(key)
    }

    /// Like [`Self::interface_text`] with the `{ $name }` placeholders filled from `args`.
    pub fn interface_text_with(&self, key: &str, args: &[(&str, &str)]) -> String {
        self.tr_with(key, args)
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

    /// Scrubbing in the timeline: the playhead may rest in the empty cells past the scene's
    /// end, as Animate's does, so a first keyframe can be placed there before anything is
    /// drawn. `shown` is how many cells the panel draws; playback still stops at the scene.
    pub(super) fn set_frame_in_view(&mut self, frame: u32, shown: u32) {
        let max = shown.max(self.scene_frames()).saturating_sub(1);
        self.frame = frame.min(max);
    }

    /// The steps of the history for a list: "Original" first, then each step in order, and
    /// which one the document is at now.
    pub(crate) fn history_entries(&self) -> (Vec<String>, usize) {
        let session = self.active();
        let past: Vec<String> = session.undo_labels().map(str::to_owned).collect();
        let current = past.len() + 1;
        let entries = std::iter::once(tr!(self, "msg-history-original"))
            .chain(past)
            .chain(session.redo_labels().map(str::to_owned))
            .collect();
        (entries, current - 1)
    }

    /// Returns the document to the state at `index` of [`Self::history_entries`] by undoing or
    /// redoing as many steps as it takes.
    pub(crate) fn jump_in_history(&mut self, index: usize) {
        let (entries, current) = self.history_entries();
        if index >= entries.len() {
            return;
        }
        for _ in index..current {
            self.undo();
        }
        for _ in current..index {
            self.redo();
        }
    }

    /// Forgets the undo and redo steps; the document stays as it is.
    pub(crate) fn clear_history(&mut self) {
        self.active_mut().clear_history();
    }
}
