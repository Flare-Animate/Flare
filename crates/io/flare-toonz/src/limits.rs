//! Bounds on everything a Toonz file can make grow. A file over a bound is refused with
//! [`crate::ToonzError::Limit`]; nothing is truncated silently.

/// Largest file accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 64 * 1024 * 1024;
/// Deepest element nesting accepted.
pub const MAX_DEPTH: usize = 64;
/// Most elements in one file.
pub const MAX_NODES: usize = 1_000_000;
/// Most text in one element, in bytes.
pub const MAX_NODE_TEXT: usize = 4 * 1024 * 1024;
/// Most level declarations in one scene.
pub const MAX_LEVELS: usize = 100_000;
/// Most xsheet columns in one scene.
pub const MAX_COLUMNS: usize = 10_000;
/// Most cell runs in one scene, across all columns.
pub const MAX_CELL_RUNS: usize = 1_000_000;
/// Rows in one xsheet; a cell past this row is refused.
pub const MAX_ROWS: u32 = 1_000_000;
/// Largest step between frames of one cell run.
pub const MAX_INCREMENT: u32 = 1_000_000;
/// Largest frame number in a cell.
pub const MAX_FRAME_NUMBER: u32 = 1_000_000;
/// Most styles, or style indices, in one palette.
pub const MAX_PALETTE_ITEMS: usize = 1_000_000;
/// Highest frame rate accepted.
pub const MAX_FPS: f64 = 1000.0;
