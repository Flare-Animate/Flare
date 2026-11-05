//! Bounds on what a FlipaClip file may ask for. Each is checked before the memory or the work it
//! guards is spent, so a hostile file cannot make the import grow without limit.

/// Most zip entries one project may hold.
pub const MAX_ENTRIES: usize = 4_096;
/// Most bytes one entry may hold, stored or inflated. A 1080p picture is about 1 MB stored.
pub const MAX_ENTRY_BYTES: u64 = 16 * 1024 * 1024;
/// Most bytes all entries may hold together, stored or inflated.
pub const MAX_TOTAL_ENTRY_BYTES: u64 = 256 * 1024 * 1024;
/// Most bytes the `meta` document, and its `layersState` text, may hold.
pub const MAX_META_BYTES: u64 = 1024 * 1024;
/// Most layers one project may hold.
pub const MAX_LAYERS: usize = 64;
/// Most frames one project may hold; the same bound as an animated GIF.
pub const MAX_FRAMES: u32 = 2_000;
/// Fastest frame rate a project may ask for.
pub const MAX_FPS: u32 = 120;
/// Most bytes all pictures may decode to together, counted before each is cropped. This bounds
/// the work, not the memory: each picture is cropped to what it draws as soon as it is decoded.
pub const MAX_DECODED_BYTES: u64 = 4 * 1024 * 1024 * 1024;
/// Most bytes all kept pictures may hold once each is cropped to what it draws. The same bound
/// as the bitmap importer's, since a web build holds the whole document in memory.
pub const MAX_STORED_BYTES: u64 = 512 * 1024 * 1024;
