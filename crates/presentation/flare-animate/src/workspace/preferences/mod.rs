//! Settings that outlive a document: what the Preferences window edits and the desktop saves.

mod apply;
mod settings;
pub(in crate::workspace) mod window;

pub use settings::{MAX_PREFERENCES_BYTES, Preferences};
