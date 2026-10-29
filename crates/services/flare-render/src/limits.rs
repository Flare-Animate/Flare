//! What a render may ask for, and how it can fail.

use thiserror::Error;

/// Per-side limit (vello_cpu uses `u16`; we stay well below).
pub const MAX_DIMENSION: u32 = 16_384;
/// Total pixel budget, so a huge request cannot exhaust memory.
pub const MAX_PIXELS: u64 = 100_000_000;

#[derive(Debug, Error)]
pub enum RenderError {
    #[error(
        "image size {0}x{1} is invalid (each side 1..={MAX_DIMENSION}, at most {MAX_PIXELS} pixels)"
    )]
    InvalidSize(u32, u32),
    #[error("png encoding failed: {0}")]
    Png(#[from] png::EncodingError),
}
