//! Validated, atomic document edits with undo/redo.
//!
//! Every mutation goes through [`EditorSession::apply`]: a batch of [`Command`]s runs
//! against a working copy, and the session only changes if the whole batch succeeds.
//! History keeps whole-document snapshots (shared via `Arc`), so undo/redo restore IDs
//! and references exactly. Structural sharing can replace this if profiling demands it.
#![forbid(unsafe_code)]

mod command;
pub mod frames;
mod session;

pub use command::{Command, EditError, LayerChange};
pub use session::EditorSession;
