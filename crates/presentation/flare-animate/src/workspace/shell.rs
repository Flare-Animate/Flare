//! What the desktop shell asks of the workspace.

use flare_dock::{self, LayoutError};
use flare_document::Document;

use super::*;

impl Workspace {
    // ---- dock layout -----------------------------------------------------------------

    /// The workspaces, with the arrangement on screen as the active one's, as JSON, for saving
    /// between sessions.
    pub fn layout_json(&self) -> Result<String, serde_json::Error> {
        flare_dock::workspaces_to_json(&self.panels.workspaces, &self.panels.layout)
    }

    /// Restores saved workspaces (or a layout saved before there were any). An unusable file is
    /// refused and the current arrangement is kept, so a stale or damaged file can never leave the
    /// workspace without a panel.
    pub fn restore_layout(&mut self, json: &str) -> Result<(), LayoutError> {
        let (workspaces, layout) = flare_dock::workspaces_from_json(json)?;
        self.panels.workspaces = workspaces;
        self.panels.layout = layout;
        self.panels.flyout = None;
        Ok(())
    }

    /// The workspace in use back as it was built in or saved. Never touches the document.
    pub fn reset_layout(&mut self) {
        self.panels.layout = self.panels.workspaces.reset();
        self.panels.flyout = None;
    }

    // ---- menu actions ----------------------------------------------------------------

    /// Tells the workspace a native menu bar is present, so it does not draw its own menu
    /// or handle the menu's shortcuts a second time.
    pub fn set_native_menu(&mut self, native: bool) {
        self.native_menu = native;
    }

    /// Returns (and clears) a request to quit the application.
    pub fn take_quit_request(&mut self) -> bool {
        std::mem::take(&mut self.quit_requested)
    }

    /// What an image export needs, shared not copied: the saved document and the frame on show.
    pub fn image_export(&self) -> (Document, u32) {
        (self.main_document().clone(), self.frame)
    }

    /// Returns (and clears) a request to play the document in an external player.
    pub fn take_test_movie_request(&mut self) -> bool {
        std::mem::take(&mut self.test_movie_requested)
    }

    /// The document as saved, which is what a test movie plays: unlike `document()`, it is
    /// never a symbol's scratch document.
    pub fn main_document(&self) -> &Document {
        self.session.document()
    }

    /// Brings a panel's tab to the front of its stack, or opens it beside the strip when it is
    /// kept there.
    pub(super) fn show_pane(&mut self, pane: Pane) {
        if self.panels.layout.in_strip(pane) {
            self.panels.flyout = Some(pane);
            return;
        }
        self.panels
            .layout
            .tree
            .make_active(|_, tile| matches!(tile, egui_tiles::Tile::Pane(p) if *p == pane));
    }
}
