use std::collections::VecDeque;
use std::sync::Arc;

use flare_document::Document;

use crate::command::{Command, EditError, apply_all};

const DEFAULT_HISTORY_DEPTH: usize = 1000;

/// Owns the current document, undo/redo history and clean/dirty tracking.
pub struct EditorSession {
    current: Arc<Document>,
    undo: VecDeque<Arc<Document>>,
    redo: Vec<Arc<Document>>,
    saved: Option<Arc<Document>>,
    max_depth: usize,
    revision: u64,
}

impl EditorSession {
    pub fn new(document: Document) -> Self {
        Self::with_history_depth(document, DEFAULT_HISTORY_DEPTH)
    }

    /// `max_depth` bounds the undo stack; the oldest entries are dropped first.
    pub fn with_history_depth(document: Document, max_depth: usize) -> Self {
        let current = Arc::new(document);
        Self {
            saved: Some(Arc::clone(&current)),
            current,
            undo: VecDeque::new(),
            redo: Vec::new(),
            max_depth,
            revision: 0,
        }
    }

    /// Increases on every change to the current document (apply, undo, redo). Use it
    /// as a cache key for derived data; it is not a dirty indicator, see [`Self::is_dirty`].
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn document(&self) -> &Document {
        &self.current
    }

    /// Applies the batch all-or-nothing as a single undo step.
    pub fn apply(&mut self, commands: impl IntoIterator<Item = Command>) -> Result<(), EditError> {
        let Some(working) = apply_all(&self.current, commands)? else {
            return Ok(());
        };
        let previous = std::mem::replace(&mut self.current, Arc::new(working));
        self.undo.push_back(previous);
        if self.undo.len() > self.max_depth {
            self.undo.pop_front();
        }
        self.redo.clear();
        self.revision += 1;
        Ok(())
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo
            .push(std::mem::replace(&mut self.current, previous));
        self.revision += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        self.revision += 1;
        true
    }

    /// The current document as a shared, immutable snapshot. Cheap, and safe to hand to another
    /// thread: later edits make new snapshots and leave this one alone.
    pub fn snapshot(&self) -> Arc<Document> {
        Arc::clone(&self.current)
    }

    /// Records that `snapshot`, taken earlier, was durably saved. Edits made since then keep the
    /// session dirty, and undoing back to exactly this content makes it clean again.
    pub fn mark_snapshot_saved(&mut self, snapshot: &Arc<Document>) {
        self.saved = Some(Arc::clone(snapshot));
    }

    /// Records that the current content was durably saved.
    pub fn mark_saved(&mut self) {
        self.saved = Some(Arc::clone(&self.current));
    }

    /// Marks the current content as never saved (for example work recovered after a
    /// crash), so it counts as dirty until it is saved to a project file.
    pub fn mark_unsaved(&mut self) {
        self.saved = None;
    }

    /// Dirty means the content differs from what was saved; undoing back to the saved
    /// content is clean again, unlike a monotonically increasing revision counter.
    pub fn is_dirty(&self) -> bool {
        !self
            .saved
            .as_ref()
            .is_some_and(|saved| Arc::ptr_eq(saved, &self.current))
    }
}

#[cfg(test)]
mod tests;
