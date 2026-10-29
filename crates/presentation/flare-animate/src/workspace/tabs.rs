//! Several documents in one window: tabs. One is being edited (`self`); the others are parked,
//! whole, and swapped in when their tab is chosen.

use flare_document::Document;
use std::path::PathBuf;

use super::*;

/// What a tab shows about its document.
pub(crate) struct TabInfo {
    pub title: String,
    pub dirty: bool,
    pub current: bool,
}

impl Workspace {
    fn document_title(&self) -> String {
        self.path
            .as_deref()
            .and_then(std::path::Path::file_name)
            .map_or_else(
                || "Untitled".to_owned(),
                |n| n.to_string_lossy().into_owned(),
            )
    }

    /// Every tab, in order, with the current one marked.
    pub(crate) fn tabs(&self) -> Vec<TabInfo> {
        let mut parked = self.parked.iter();
        (0..=self.parked.len())
            .map(|index| {
                if index == self.position {
                    TabInfo {
                        title: self.document_title(),
                        dirty: self.is_dirty(),
                        current: true,
                    }
                } else {
                    let other = parked.next().map(|w| (w.document_title(), w.is_dirty()));
                    let (title, dirty) = other.unwrap_or_default();
                    TabInfo {
                        title,
                        dirty,
                        current: false,
                    }
                }
            })
            .collect()
    }

    /// Whether any tab has unsaved work, not just the one being edited.
    pub fn any_dirty(&self) -> bool {
        self.is_dirty() || self.parked.iter().any(|w| w.is_dirty())
    }

    /// Whether the document being edited is an untouched new one, which a newly opened project
    /// may take the place of instead of opening beside it.
    pub(super) fn is_blank(&self) -> bool {
        self.path.is_none()
            && !self.is_dirty()
            && !self.session.can_undo()
            && self.contexts.is_empty()
    }

    /// Shows `document` in a new tab beside the current one, and makes it the current one.
    pub(super) fn open_in_new_tab(&mut self, document: Document, path: Option<PathBuf>) {
        let mut fresh = Self::new(document, path);
        fresh.adopt_environment(self);
        fresh.parked = std::mem::take(&mut self.parked);
        let mut old = std::mem::replace(self, fresh);
        // The Home page was a view over that document, not part of it.
        old.windows.home = false;
        // The new tab goes right after the one it was opened from.
        let at = old.position;
        self.parked.insert(at, old);
        self.position = at + 1;
    }

    /// Shows a newly read document: in place of an untouched new one, else in a new tab.
    pub(super) fn show_document(&mut self, document: Document, path: Option<PathBuf>) {
        if self.is_blank() {
            self.install_document(document, path);
        } else {
            self.open_in_new_tab(document, path);
        }
    }

    /// A new empty document in a new tab.
    pub fn new_tab(&mut self) {
        if self.windows.home && self.is_blank() {
            return self.new_document_in_place();
        }
        match Self::untitled() {
            Ok(fresh) => {
                let document = fresh.session.document().clone();
                self.open_in_new_tab(document, None);
            }
            Err(error) => self.report(error),
        }
    }

    /// Makes tab `index` the one being edited.
    pub fn switch_to_tab(&mut self, index: usize) {
        if index == self.position || index > self.parked.len() {
            return;
        }
        if self.has_pending_io() {
            self.report("Wait for the file work in progress before switching documents");
            return;
        }
        let slot = if index < self.position {
            index
        } else {
            index - 1
        };
        let mut other = self.parked.remove(slot);
        other.adopt_environment(self);
        other.parked = std::mem::take(&mut self.parked);
        let before = self.position;
        let old = std::mem::replace(self, other);
        self.position = index;
        let slot = if before < index { before } else { before - 1 };
        self.parked.insert(slot, old);
        self.validate_selection();
    }

    /// Closes tab `index`, refusing while it has unsaved work. The last tab is replaced by an
    /// empty one rather than leaving the window with nothing.
    pub fn close_tab(&mut self, index: usize) {
        if index > self.parked.len() {
            return;
        }
        if index != self.position {
            let slot = if index < self.position {
                index
            } else {
                index - 1
            };
            if self.parked[slot].is_dirty() {
                self.report("Save or undo the changes in that document before closing it");
                return;
            }
            self.parked.remove(slot);
            if slot < self.position {
                self.position -= 1;
            }
            return;
        }
        if self.is_dirty() {
            self.report("Save or undo your changes before closing the document");
            return;
        }
        if self.parked.is_empty() {
            self.new_document_in_place();
            return;
        }
        // Another tab takes over: the one on the left, or on the right if this was the first.
        let neighbour = if index > 0 { index - 1 } else { 1 };
        self.switch_to_tab(neighbour);
        // After the switch the closed document is parked where its tab was.
        let slot = if index < self.position {
            index
        } else {
            index - 1
        };
        if slot < self.parked.len() {
            self.parked.remove(slot);
            if slot < self.position {
                self.position -= 1;
            }
        }
    }
}
