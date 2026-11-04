//! Bounds on what a Moho project may contain. Each is far above what a real project needs and
//! small enough that a hostile file cannot exhaust memory or the stack.

/// Largest project file read, the ZIP wrapper included.
pub const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
/// Largest JSON document, after any ZIP wrapper is removed.
pub const MAX_DOCUMENT_BYTES: u64 = 32 * 1024 * 1024;
/// Deepest JSON nesting walked.
pub const MAX_JSON_DEPTH: usize = 96;
/// Deepest layer nesting (groups inside groups, switch cells inside switches).
pub const MAX_LAYER_DEPTH: usize = 32;
/// Layers in the whole project, at every depth.
pub const MAX_LAYERS: usize = 10_000;
pub const MAX_BONES: usize = 4_096;
/// Alternatives named by one switch layer; more are dropped.
pub const MAX_SWITCH_ALTERNATIVES: usize = 1_024;
pub const MAX_TRACKS: usize = 200_000;
pub const MAX_KEYFRAMES: usize = 2_000_000;
/// Components kept from one key's vector value.
pub const MAX_KEY_VECTOR: usize = 16;
/// Distinct image paths, and distinct audio paths, kept.
pub const MAX_REFERENCES: usize = 100_000;
