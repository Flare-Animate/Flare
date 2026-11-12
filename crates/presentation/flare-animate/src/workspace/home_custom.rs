//! What the New Document dialog (`new_document.rs`) edits: the width, height, frame rate and
//! background color a project is made with. File > New and the New File button open it on the
//! first preset (the sizes Flare has always started with) and the Custom Size card on a size that
//! is none of them.

use flare_core::Rgba8;

use super::home::PRESETS;
use super::*;

/// What the New Document window is editing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct NewDocument {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) fps: u32,
    pub(super) background: Rgba8,
}

impl NewDocument {
    /// The numbers of preset `index` of `PRESETS`, on a white stage.
    pub(super) fn preset(index: usize) -> Self {
        let (_, width, height, fps) = PRESETS.get(index).copied().unwrap_or(PRESETS[0]);
        Self {
            width,
            height,
            fps,
            background: Rgba8::WHITE,
        }
    }

    /// The size the Custom Size card starts on: none of the presets', so the window says Custom.
    pub(super) fn custom() -> Self {
        Self {
            width: 1024,
            height: 768,
            fps: 24,
            background: Rgba8::WHITE,
        }
    }

    /// The preset these numbers are exactly those of, if there is one.
    #[cfg(test)]
    pub(super) fn matching_preset(&self) -> Option<usize> {
        PRESETS
            .iter()
            .position(|&(_, w, h, fps)| (w, h, fps) == (self.width, self.height, self.fps))
    }
}

impl Workspace {
    /// Opens the New Document window on the first preset, as the New File button does.
    pub fn show_new_document(&mut self) {
        self.home.new_document = Some(NewDocument::preset(0));
        self.home.new_document_kind = 0;
        self.home.new_document_from_home = self.windows.home;
    }

    /// Opens the window on a custom size, as the Custom Size card does; on the size last
    /// chosen if it is already open.
    pub fn show_custom_size(&mut self) {
        if self.home.new_document.is_none() {
            self.home.new_document = Some(NewDocument::custom());
            // A size of the person's own is Advanced, as in Animate.
            self.home.new_document_kind = super::new_document_presets::CATEGORIES.len() - 1;
        }
        self.home.new_document_from_home = self.windows.home;
    }

    pub fn new_document_shown(&self) -> bool {
        self.home.new_document.is_some()
    }
}
