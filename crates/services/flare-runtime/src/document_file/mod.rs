//! The formats a document is stored in, and reading and writing them by the name of the file.
//!
//! Animate's own formats are first-class: a person can open a `.fla` or `.xfl`, work, and save it
//! back to that file for somebody who still uses Animate, without a `.flare` ever being made.
//! The extension decides the format; everything else (the atomic replacement, what an unchanged
//! import writes back, what is reported) is the library's, so the desktop app, the browser build,
//! `flare-cli` and `flare-mcp` all do the same thing.

mod format;
mod load;
mod save;

pub use format::{FileFormat, resolve_save_path, xfl_folder};
pub use load::{LoadError, Loaded, load, saves_in_place};
pub use save::{ExportSummary, MAX_NAMED_FILES, SaveError, Saved, save, summarize};
