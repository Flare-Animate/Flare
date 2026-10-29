//! Undo and redo over validated edits.
//!
//! Every mutation goes through [`EditorSession::apply`]: a batch of commands runs against a
//! working copy, and the session only changes if the whole batch succeeds. History keeps
//! whole-document snapshots (shared via `Arc`), so undo/redo restore IDs and references
//! exactly. Structural sharing can replace this if profiling demands it.
#![forbid(unsafe_code)]

use std::collections::VecDeque;
use std::sync::Arc;

use flare_document::Document;

use flare_commands::{Command, EditError, apply_all};

const DEFAULT_HISTORY_DEPTH: usize = 1000;

/// Owns the current document, undo/redo history and clean/dirty tracking.
pub struct EditorSession {
    current: Arc<Document>,
    undo: VecDeque<Arc<Document>>,
    redo: Vec<Arc<Document>>,
    /// What each undo step did, oldest first (one per entry of `undo`).
    undo_labels: VecDeque<String>,
    /// What each redo step does, the next one to redo last (one per entry of `redo`).
    redo_labels: Vec<String>,
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
            undo_labels: VecDeque::new(),
            redo_labels: Vec::new(),
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
        self.apply_labeled(commands, "Edit")
    }

    /// Like [`Self::apply`], naming the step for a history list.
    pub fn apply_labeled(
        &mut self,
        commands: impl IntoIterator<Item = Command>,
        label: &str,
    ) -> Result<(), EditError> {
        let Some(working) = apply_all(&self.current, commands)? else {
            return Ok(());
        };
        let previous = std::mem::replace(&mut self.current, Arc::new(working));
        self.undo.push_back(previous);
        self.undo_labels.push_back(label.to_owned());
        if self.undo.len() > self.max_depth {
            self.undo.pop_front();
            self.undo_labels.pop_front();
        }
        self.redo.clear();
        self.redo_labels.clear();
        self.revision += 1;
        Ok(())
    }

    /// What each step that can be undone did, oldest first.
    pub fn undo_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.undo_labels.iter().map(String::as_str)
    }

    /// What each step that can be redone does, the next one first.
    pub fn redo_labels(&self) -> impl ExactSizeIterator<Item = &str> {
        self.redo_labels.iter().rev().map(String::as_str)
    }

    /// Forgets every undo and redo step; the document stays as it is.
    pub fn clear_history(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.undo_labels.clear();
        self.redo_labels.clear();
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
        if let Some(label) = self.undo_labels.pop_back() {
            self.redo_labels.push(label);
        }
        self.revision += 1;
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo
            .push_back(std::mem::replace(&mut self.current, next));
        if let Some(label) = self.redo_labels.pop() {
            self.undo_labels.push_back(label);
        }
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
