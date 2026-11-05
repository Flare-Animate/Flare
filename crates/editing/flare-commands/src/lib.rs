//! The vocabulary of document edits: [`Command`]s, applying a batch of them to a document as one
//! validated step, and the builders that turn a wish ("delete this layer") into the commands that
//! do it. History and sessions live in `flare-history`.
#![forbid(unsafe_code)]

mod apply;
mod command;
mod error;
mod game;
pub mod layers;

pub use apply::apply_all;
pub use command::{Command, LayerChange};
pub use error::{EditError, MAX_SCENE_NAME_CHARS};
