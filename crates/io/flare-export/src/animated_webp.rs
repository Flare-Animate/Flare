//! An animated WebP written in pure Rust. Each frame is a lossless picture of its own, in an `ANMF`
//! chunk with its time, and the frames replace the canvas one after the other, so the colours come
//! back exactly. Lossless is the only pure-Rust WebP encoder there is; the frames are added one by
//! one, so the pictures are never all held at once.

use image_webp::{ColorType, WebPEncoder};
use thiserror::Error;

/// The most the output may hold: a file past this is better written as an APNG or a GIF.
pub const MAX_OUTPUT_BYTES: usize = 512 * 1024 * 1024;
/// The largest side a WebP canvas can have.
pub const MAX_SIDE: u32 = 16_384;

#[derive(Debug, Error)]
pub enum AnimatedWebpError {
    #[error("a WebP canvas is at most {MAX_SIDE} pixels on a side; this one is {0}x{1}")]
    TooLarge(u32, u32),
    #[error("a frame has {got} bytes; a {width}x{height} picture has {want}")]
    FrameSize {
        width: u32,
        height: u32,
        got: usize,
        want: usize,
    },
    #[error("the animation is larger than {MAX_OUTPUT_BYTES} bytes as lossless WebP")]
    OutputTooLarge,
    #[error("a frame could not be encoded: {0}")]
    Encode(#[from] image_webp::EncodingError),
}

/// Builds an animated WebP one frame at a time.
pub struct AnimatedWebp {
    width: u32,
    height: u32,
    /// The frames' chunks, after the file's header and the animation's two chunks.
    body: Vec<u8>,
}

impl AnimatedWebp {
    /// An animation on a canvas of `width` by `height`, with no frames yet.
    pub fn new(width: u32, height: u32) -> Result<Self, AnimatedWebpError> {
        if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE {
            return Err(AnimatedWebpError::TooLarge(width, height));
        }
        let mut body = Vec::new();
        // The extended header: animation and alpha flags, then the canvas size minus one.
        let mut vp8x = vec![0x12, 0, 0, 0];
        vp8x.extend_from_slice(&le24(width - 1));
        vp8x.extend_from_slice(&le24(height - 1));
        push_chunk(&mut body, b"VP8X", &vp8x);
        // The background (transparent black) and the loop count (0: forever).
        push_chunk(&mut body, b"ANIM", &[0, 0, 0, 0, 0, 0]);
        Ok(Self {
            width,
            height,
            body,
        })
    }

    /// Adds a frame of straight-alpha RGBA pixels shown for `millis` milliseconds.
    pub fn push_frame(&mut self, rgba: &[u8], millis: u32) -> Result<(), AnimatedWebpError> {
        let want = self.width as usize * self.height as usize * 4;
        if rgba.len() != want {
            return Err(AnimatedWebpError::FrameSize {
                width: self.width,
                height: self.height,
                got: rgba.len(),
                want,
            });
        }
        let mut picture = Vec::new();
        WebPEncoder::new(&mut picture).encode(rgba, self.width, self.height, ColorType::Rgba8)?;
        // The still file's 12-byte header is dropped; what follows is one lossless chunk.
        let lossless = &picture[12..];
        let mut frame = Vec::with_capacity(16 + lossless.len());
        frame.extend_from_slice(&le24(0)); // x / 2
        frame.extend_from_slice(&le24(0)); // y / 2
        frame.extend_from_slice(&le24(self.width - 1));
        frame.extend_from_slice(&le24(self.height - 1));
        frame.extend_from_slice(&le24(millis.min(0xFF_FFFF)));
        // Bit 1 set: the frame replaces the canvas rather than being blended onto it.
        frame.push(0x02);
        frame.extend_from_slice(lossless);
        push_chunk(&mut self.body, b"ANMF", &frame);
        if self.body.len() > MAX_OUTPUT_BYTES {
            return Err(AnimatedWebpError::OutputTooLarge);
        }
        Ok(())
    }

    /// The whole file.
    pub fn finish(self) -> Vec<u8> {
        let mut out = Vec::with_capacity(12 + self.body.len());
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((self.body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(&self.body);
        out
    }
}

fn le24(value: u32) -> [u8; 3] {
    let b = value.to_le_bytes();
    [b[0], b[1], b[2]]
}

/// A RIFF chunk: its name, its size, its payload, and a pad byte when the payload is odd.
fn push_chunk(out: &mut Vec<u8>, name: &[u8; 4], payload: &[u8]) {
    out.extend_from_slice(name);
    out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    out.extend_from_slice(payload);
    if payload.len() % 2 == 1 {
        out.push(0);
    }
}
