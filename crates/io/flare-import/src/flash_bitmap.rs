//! Animate's own lossless bitmap payload (`bin/M *.dat` in an XFL or FLA).
//!
//! Layout, verified against real files (see docs/FLASH-FORMATS.md): the bytes `03 05`; the row
//! length in bytes, the width and the height, each a u16 little-endian; the frame in twips as four
//! i32 little-endian (left, right, top, bottom); a has-alpha byte and a compressed byte; then the
//! pixel rows as premultiplied ARGB. Compressed, the rows are one zlib stream cut into chunks that
//! each start with their u16 length, ended by a chunk of length 0.

use std::io::Read;

use flare_document::{MAX_BITMAP_PIXELS, MAX_BITMAP_SIDE};

use crate::bitmap::Picture;

/// The two bytes every payload starts with.
pub(crate) const SIGNATURE: [u8; 2] = [0x03, 0x05];
const HEADER_LEN: usize = 26;
/// Chunks a stream may have; a chunk is at most 65535 bytes, far above what a bitmap needs.
const MAX_CHUNKS: usize = 1 << 16;

#[derive(Debug, thiserror::Error)]
pub(crate) enum FlashBitmapError {
    #[error("the data ends before the header does")]
    TruncatedHeader,
    #[error("not Animate's bitmap data (it does not start with 03 05)")]
    BadSignature,
    #[error("{0}x{1} pixels is outside what a bitmap may be")]
    BadSize(u32, u32),
    #[error(
        "rows of {0} bytes for {1} pixels: only 32-bit rows are read (an 8-bit palette bitmap is not imported)"
    )]
    UnsupportedRows(u32, u32),
    #[error("the pixel data is cut short")]
    Truncated,
    #[error("the compressed chunks are more than a bitmap of this size can need")]
    TooMuchData,
    #[error("the compressed pixel data is damaged")]
    BadZlib,
    #[error("the pixel data is {0} bytes, not the {1} the header gives")]
    WrongLength(usize, usize),
}

fn le16(bytes: &[u8], at: usize) -> usize {
    usize::from(bytes[at]) | usize::from(bytes[at + 1]) << 8
}

/// Joins the length-prefixed chunks into the zlib stream they cut up.
fn join_chunks(body: &[u8], limit: usize) -> Result<Vec<u8>, FlashBitmapError> {
    let mut stream = Vec::new();
    let mut at = 0;
    for _ in 0..MAX_CHUNKS {
        if body.len() < at + 2 {
            return Err(FlashBitmapError::Truncated);
        }
        let len = le16(body, at);
        at += 2;
        if len == 0 {
            return Ok(stream);
        }
        let chunk = body.get(at..at + len).ok_or(FlashBitmapError::Truncated)?;
        if stream.len() + len > limit {
            return Err(FlashBitmapError::TooMuchData);
        }
        stream.extend_from_slice(chunk);
        at += len;
    }
    Err(FlashBitmapError::TooMuchData)
}

fn inflate(stream: &[u8], expected: usize) -> Result<Vec<u8>, FlashBitmapError> {
    let mut out = Vec::new();
    // One byte past the size shows data that does not fit, without reading it all.
    flate2::read::ZlibDecoder::new(stream)
        .take(expected as u64 + 1)
        .read_to_end(&mut out)
        .map_err(|_| FlashBitmapError::BadZlib)?;
    Ok(out)
}

/// Decodes a payload into straight-alpha RGBA8.
pub(crate) fn decode(bytes: &[u8]) -> Result<Picture, FlashBitmapError> {
    if bytes.len() < HEADER_LEN {
        return Err(FlashBitmapError::TruncatedHeader);
    }
    if bytes[..2] != SIGNATURE {
        return Err(FlashBitmapError::BadSignature);
    }
    let (row, width, height) = (le16(bytes, 2), le16(bytes, 4), le16(bytes, 6));
    let (w, h) = (width as u32, height as u32);
    if w == 0
        || h == 0
        || w > MAX_BITMAP_SIDE
        || h > MAX_BITMAP_SIDE
        || u64::from(w) * u64::from(h) > MAX_BITMAP_PIXELS
    {
        return Err(FlashBitmapError::BadSize(w, h));
    }
    if row != width * 4 {
        return Err(FlashBitmapError::UnsupportedRows(row as u32, w));
    }
    let (has_alpha, compressed) = (bytes[24] != 0, bytes[25] != 0);
    let expected = row * height;
    let body = &bytes[HEADER_LEN..];
    let mut pixels = if compressed {
        // Deflate data cannot be much larger than what it holds; this bounds hostile chunks.
        let stream = join_chunks(body, expected * 2 + 1024)?;
        inflate(&stream, expected)?
    } else {
        body.to_vec()
    };
    if compressed && pixels.len() < expected {
        return Err(FlashBitmapError::Truncated);
    }
    if pixels.len() != expected {
        return Err(FlashBitmapError::WrongLength(pixels.len(), expected));
    }
    for px in pixels.as_chunks_mut::<4>().0 {
        let (a, r, g, b) = (px[0], px[1], px[2], px[3]);
        let (r, g, b, a) = if !has_alpha {
            (r, g, b, 255)
        } else if a == 0 {
            (0, 0, 0, 0)
        } else {
            let straight =
                |c: u8| ((u32::from(c) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8;
            (straight(r), straight(g), straight(b), a)
        };
        px.copy_from_slice(&[r, g, b, a]);
    }
    Ok(Picture {
        width: w,
        height: h,
        rgba: pixels,
        delay_cs: 0,
    })
}

#[cfg(test)]
mod tests;
