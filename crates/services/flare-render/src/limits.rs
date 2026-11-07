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
    #[error("JPEG quality {0} is outside 1 to 100")]
    InvalidQuality(u8),
    #[error("a JPEG cannot hold transparency, and the image has pixels that are not opaque")]
    NotOpaque,
    #[error("{format} holds at most {side} pixels a side")]
    TooLargeFor { format: &'static str, side: u32 },
    #[error("writing the image failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("image encoding failed: {0}")]
    Encode(#[from] ::image::ImageError),
}
