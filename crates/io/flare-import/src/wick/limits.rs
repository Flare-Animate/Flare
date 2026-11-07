//! Bounds on what a Wick file may ask for. Each is checked before the memory or the work it guards
//! is spent, so a hostile file cannot make the import grow without limit.

/// Most zip entries one project may hold.
pub const MAX_ENTRIES: usize = 4_096;
/// Most bytes one entry may hold, stored or inflated.
pub const MAX_ENTRY_BYTES: u64 = 32 * 1024 * 1024;
/// Most bytes `project.json` may hold once inflated.
pub const MAX_PROJECT_BYTES: u64 = 64 * 1024 * 1024;
/// Most bytes all entries may hold together, stored or inflated.
pub const MAX_TOTAL_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
/// Most objects in the project's table.
pub const MAX_OBJECTS: usize = 200_000;
/// Most children one object may list.
pub const MAX_CHILDREN: usize = 100_000;
/// Most clips one clip may hold inside another, counted down the tree.
pub const MAX_DEPTH: usize = 64;
/// Most symbols (clips and buttons) one project may hold.
pub const MAX_SYMBOLS: usize = 10_000;
/// Most layers one timeline may hold.
pub const MAX_LAYERS: usize = 1_024;
/// The last frame a project may name.
pub const MAX_FRAME: u64 = 100_000;
/// Fastest frame rate a project may ask for.
pub const MAX_FPS: u32 = 120;
/// Most decoded picture bytes the library may hold, after duplicates merge.
pub const MAX_DECODED_BYTES: u64 = 512 * 1024 * 1024;
/// Largest coordinate, in either direction, that is taken as it is.
pub const MAX_COORDINATE: f64 = 1.0e8;
/// Longest name kept from the file, in characters.
pub const MAX_NAME_CHARS: usize = 200;
