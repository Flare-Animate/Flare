//! One PPM frame: its header byte (keyframe, translation, palette choice), the line types of both
//! layers, the line bitmaps, and the XOR against the previous frame for a delta frame.

use super::{HEIGHT, WIDTH};
use crate::flipnote::error::FlipnoteError;
use crate::flipnote::reader::Reader;

pub const PIXELS: usize = WIDTH * HEIGHT;

/// The decoded layers of the last frame: one byte per pixel, 0 or 1, row by row from the top.
pub struct FrameDecoder {
    layers: [Vec<u8>; 2],
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            layers: [vec![0; PIXELS], vec![0; PIXELS]],
        }
    }

    pub fn layers(&self) -> &[Vec<u8>; 2] {
        &self.layers
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// What the frame header says about the frame besides its pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    pub keyframe: bool,
    /// Palette indices (into `PALETTE`): paper, layer 1 pen, layer 2 pen.
    pub palette: [u8; 3],
    /// Where the frame's data ends in the file, for checking it against the next frame.
    pub end: usize,
}

/// Decodes the frame at `at` in `bytes`, leaving its layers in `dec`.
pub(crate) fn decode(
    bytes: &[u8],
    at: usize,
    dec: &mut FrameDecoder,
) -> Result<FrameInfo, FlipnoteError> {
    let mut r = Reader::at(bytes, at)?;
    let header = r.u8("a frame header")?;
    let keyframe = (header >> 7) & 1 == 1;
    let translated = (header >> 5) & 3 != 0;
    let (tx, ty) = if translated {
        (
            i64::from(r.i8("the frame translation")?),
            i64::from(r.i8("the frame translation")?),
        )
    } else {
        (0, 0)
    };
    let palette = palette(header);

    // Every layer's line types come first, four to a byte; a zero byte is four blank lines.
    let mut types = [[0_u8; HEIGHT]; 2];
    for layer in types.iter_mut() {
        let mut y = 0;
        while y < HEIGHT {
            let byte = r.u8("the line types")?;
            if byte != 0 {
                for k in 0..4 {
                    if let Some(slot) = layer.get_mut(y + k) {
                        *slot = (byte >> (2 * k)) & 3;
                    }
                }
            }
            y += 4;
        }
    }

    let mut current = [vec![0_u8; PIXELS], vec![0_u8; PIXELS]];
    for (layer, line_types) in current.iter_mut().zip(types.iter()) {
        decode_layer(&mut r, layer, line_types)?;
    }
    if !keyframe {
        xor_previous(&mut current, &dec.layers, tx, ty);
    }
    dec.layers = current;
    Ok(FrameInfo {
        keyframe,
        palette,
        end: r.pos(),
    })
}

/// Bit 0 set means the paper is white and the pens are not inverted; the two pen fields are two
/// bits each, mapped through the global palette as flipnote.js does.
fn palette(header: u8) -> [u8; 3] {
    let inverted = header & 1 == 0;
    let pen = |field: u8| -> u8 {
        match field {
            0 => {
                if inverted {
                    0
                } else {
                    1
                }
            }
            1 => {
                if inverted {
                    0
                } else {
                    1
                }
            }
            2 => 2,
            _ => 3,
        }
    };
    [
        u8::from(inverted),
        pen((header >> 1) & 3),
        pen((header >> 3) & 3),
    ]
}

fn set(layer: &mut [u8], at: usize, value: u8) {
    if let Some(p) = layer.get_mut(at) {
        *p = value;
    }
}

fn decode_layer(
    r: &mut Reader<'_>,
    layer: &mut [u8],
    types: &[u8; HEIGHT],
) -> Result<(), FlipnoteError> {
    for (y, &kind) in types.iter().enumerate() {
        let row = y * WIDTH;
        match kind {
            // Blank line.
            0 => {}
            // Compressed line: a 32-bit header with one bit per 8-pixel chunk, the chunks that
            // follow are non-zero bits only.
            1 | 2 => {
                // Kind 2 starts with every pixel set and each chunk writes all eight of its bits.
                if let (2, Some(row_px)) = (kind, layer.get_mut(row..row + WIDTH)) {
                    row_px.fill(1);
                }
                let mut bits = r.u32_be("a line header")?;
                let mut base = row;
                while bits != 0 {
                    if bits & 0x8000_0000 != 0 {
                        let mut chunk = r.u8("a line chunk")?;
                        if kind == 2 {
                            for px in 0..8 {
                                set(layer, base + px, chunk & 1);
                                chunk >>= 1;
                            }
                        } else {
                            for px in 0..8 {
                                if chunk == 0 {
                                    break;
                                }
                                set(layer, base + px, chunk & 1);
                                chunk >>= 1;
                            }
                        }
                    }
                    bits <<= 1;
                    base += 8;
                }
            }
            // Raw line: 256 pixels as 32 bytes, least significant bit first.
            _ => {
                let mut chunk = 0_u8;
                for x in 0..WIDTH {
                    if x % 8 == 0 {
                        chunk = r.u8("a raw line")?;
                    }
                    set(layer, row + x, chunk & 1);
                    chunk >>= 1;
                }
            }
        }
    }
    Ok(())
}

/// A delta frame XORs each pixel with the pixel the translation maps it from. Pixels the
/// translation does not reach keep the value they were decoded with.
fn xor_previous(current: &mut [Vec<u8>; 2], prev: &[Vec<u8>; 2], tx: i64, ty: i64) {
    let w = WIDTH as i64;
    let h = HEIGHT as i64;
    for y in ty.max(0)..(h + ty).min(h) {
        for x in tx.max(0)..(w + tx).min(w) {
            let dest = (y * w + x) as usize;
            let src = ((y - ty) * w + (x - tx)) as usize;
            for (cur, old) in current.iter_mut().zip(prev.iter()) {
                let before = old.get(src).copied().unwrap_or(0);
                if let Some(p) = cur.get_mut(dest) {
                    *p ^= before;
                }
            }
        }
    }
}
