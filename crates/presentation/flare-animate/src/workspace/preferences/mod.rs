//! Settings that outlive a document: what the Preferences window edits and the desktop saves.

mod apply;
mod settings;
mod window;

pub use settings::{MAX_PREFERENCES_BYTES, Preferences};
