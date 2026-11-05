//! Bitmaps: a library of raster images, and instances that show one on a frame of a layer.
//!
//! Pixels are straight-alpha RGBA, 8 bits a channel, kept once per distinct image: the content
//! hash lets an importer reuse a bitmap that is already in the library instead of storing a
//! sequence's repeated frames again.

use std::sync::Arc;

use flare_core::{BitmapId, InstanceId, Transform2D};
use thiserror::Error;

/// Longest side of a bitmap, in pixels.
pub const MAX_BITMAP_SIDE: u32 = 8192;
/// Most pixels in one bitmap (64 MiB of RGBA).
pub const MAX_BITMAP_PIXELS: u64 = 16 * 1024 * 1024;
/// Most bitmaps one document may hold.
pub const MAX_BITMAPS: usize = 4096;
/// Most bytes of pixels in one document.
pub const MAX_TOTAL_BITMAP_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BitmapError {
    #[error("a bitmap needs at least one pixel and at most {MAX_BITMAP_SIDE} on a side")]
    Size,
    #[error("a bitmap is limited to {MAX_BITMAP_PIXELS} pixels")]
    TooManyPixels,
    #[error("a {width}x{height} bitmap needs {expected} bytes of RGBA, got {got}")]
    PixelLength {
        width: u32,
        height: u32,
        expected: u64,
        got: usize,
    },
}

/// An imported raster image.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bitmap {
    id: BitmapId,
    name: String,
    width: u32,
    height: u32,
    /// Shared, so a snapshot of the document never copies the pixels.
    pixels: Arc<[u8]>,
    hash: [u8; 32],
}

impl Bitmap {
    /// `rgba` is `width * height * 4` bytes, rows top to bottom, straight alpha.
    pub fn new(
        id: BitmapId,
        name: impl Into<String>,
        width: u32,
        height: u32,
        rgba: Vec<u8>,
    ) -> Result<Self, BitmapError> {
        if width == 0 || height == 0 || width > MAX_BITMAP_SIDE || height > MAX_BITMAP_SIDE {
            return Err(BitmapError::Size);
        }
        let count = u64::from(width) * u64::from(height);
        if count > MAX_BITMAP_PIXELS {
            return Err(BitmapError::TooManyPixels);
        }
        if rgba.len() as u64 != count * 4 {
            return Err(BitmapError::PixelLength {
                width,
                height,
                expected: count * 4,
                got: rgba.len(),
            });
        }
        let mut hasher = blake3::Hasher::new();
        hasher.update(&width.to_le_bytes());
        hasher.update(&height.to_le_bytes());
        hasher.update(&rgba);
        Ok(Self {
            id,
            name: name.into(),
            width,
            height,
            pixels: rgba.into(),
            hash: *hasher.finalize().as_bytes(),
        })
    }

    pub fn id(&self) -> BitmapId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// RGBA8, straight alpha, rows top to bottom.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn shared_pixels(&self) -> Arc<[u8]> {
        Arc::clone(&self.pixels)
    }

    /// Hash of the size and pixels: equal hashes mean the same image.
    pub fn content_hash(&self) -> &[u8; 32] {
        &self.hash
    }

    pub fn byte_len(&self) -> u64 {
        self.pixels.len() as u64
    }
}

/// A bitmap shown on a layer: its top-left corner sits at the transform's origin.
#[derive(Debug, Clone, PartialEq)]
pub struct BitmapInstance {
    id: InstanceId,
    bitmap: BitmapId,
    transform: Transform2D,
    opacity: f32,
}

impl BitmapInstance {
    pub fn new(id: InstanceId, bitmap: BitmapId, transform: Transform2D) -> Self {
        Self {
            id,
            bitmap,
            transform,
            opacity: 1.0,
        }
    }

    /// The same instance with opacity from 0 (clear) to 1 (opaque); a NaN counts as opaque.
    #[must_use]
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.opacity = if opacity.is_nan() {
            1.0
        } else {
            opacity.clamp(0.0, 1.0)
        };
        self
    }

    pub fn id(&self) -> InstanceId {
        self.id
    }

    pub fn bitmap(&self) -> BitmapId {
        self.bitmap
    }

    pub fn transform(&self) -> Transform2D {
        self.transform
    }

    pub fn opacity(&self) -> f32 {
        self.opacity
    }
}
