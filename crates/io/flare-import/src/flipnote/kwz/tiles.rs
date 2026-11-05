//! One KWZ layer: 320 by 240 pixels in 128-pixel tiles of 8 by 8 blocks. Each block is a tile
//! type (three bits) and the line codes it needs. Blocks are read in tile order, then row by row
//! inside a tile; a skip code passes over the blocks that follow it, which keep the pixels they
//! had. Ported from flipnote.js (MIT); see ENGINEERING.md.

use super::bits::Bits;
use super::lines::{Line, LineTables};
use crate::flipnote::error::FlipnoteError;

pub const LAYER_WIDTH: usize = 320;
pub const LAYER_HEIGHT: usize = 240;

/// Which line of a block's pair each row takes, for the pattern tile (type 7): true is `a`.
const PATTERNS: [[bool; 8]; 4] = [
    [true, false, true, false, true, false, true, false],
    [true, true, false, true, true, false, true, true],
    [true, false, true, true, false, true, true, false],
    [true, false, false, true, false, false, true, false],
];

/// Decodes one layer's bits into `out` (values 0 to 2, row-major). Pixels no block writes keep
/// their old value: a skipped block and a layer the frame does not change both depend on that.
pub(crate) fn decode_layer(
    data: &[u8],
    out: &mut [u8],
    tables: &LineTables,
) -> Result<usize, FlipnoteError> {
    let mut bits = Bits::new(data);
    let mut skip = 0_u32;
    for tile_y in [0_usize, 128] {
        for tile_x in [0_usize, 128, 256] {
            for sub_y in (0..128).step_by(8) {
                let y = tile_y + sub_y;
                if y >= LAYER_HEIGHT {
                    break;
                }
                for sub_x in (0..128).step_by(8) {
                    let x = tile_x + sub_x;
                    if x >= LAYER_WIDTH {
                        break;
                    }
                    if skip > 0 {
                        skip -= 1;
                        continue;
                    }
                    let base = y * LAYER_WIDTH + x;
                    block(&mut bits, out, tables, base, &mut skip)?;
                }
            }
        }
    }
    if !bits.within() {
        return Err(FlipnoteError::Truncated {
            what: "a KWZ layer",
            at: bits.consumed(),
            need: data.len(),
            len: data.len(),
        });
    }
    Ok(bits.consumed())
}

fn common(t: &LineTables, code: u32) -> Result<Line, FlipnoteError> {
    t.common
        .get(code as usize)
        .copied()
        .ok_or(FlipnoteError::BadLineCode(code))
}

fn full(t: &LineTables, code: u32) -> Result<(Line, Line), FlipnoteError> {
    match (t.full.get(code as usize), t.full_shift.get(code as usize)) {
        (Some(a), Some(b)) => Ok((*a, *b)),
        _ => Err(FlipnoteError::BadLineCode(code)),
    }
}

fn block(
    bits: &mut Bits<'_>,
    out: &mut [u8],
    t: &LineTables,
    base: usize,
    skip: &mut u32,
) -> Result<(), FlipnoteError> {
    match bits.read(3) {
        0 => {
            let line = common(t, bits.read(5))?;
            rows(out, base, [line; 8]);
        }
        1 => {
            let (line, _) = full(t, bits.read(13))?;
            rows(out, base, [line; 8]);
        }
        2 => {
            let i = bits.read(5);
            let (a, b) = (common(t, i)?, common_shifted(t, i)?);
            rows(out, base, [a, b, a, b, a, b, a, b]);
        }
        3 => {
            let (a, b) = full(t, bits.read(13))?;
            rows(out, base, [a, b, a, b, a, b, a, b]);
        }
        4 => {
            let flags = bits.read(8);
            let mut lines = [[0_u8; 8]; 8];
            for (row, slot) in lines.iter_mut().enumerate() {
                *slot = if flags & (1 << row) != 0 {
                    common(t, bits.read(5))?
                } else {
                    full(t, bits.read(13))?.0
                };
            }
            rows(out, base, lines);
        }
        5 => *skip = bits.read(5),
        // Type 6 is not used by the format; the block keeps its pixels.
        6 => {}
        _ => {
            let pattern = bits.read(2);
            let common_pair = bits.read(1) != 0;
            let (a, b, pattern) = if common_pair {
                let ia = bits.read(5);
                let ib = bits.read(5);
                (common(t, ia)?, common(t, ib)?, pattern + 1)
            } else {
                let ia = bits.read(13);
                let ib = bits.read(13);
                (full(t, ia)?.0, full(t, ib)?.0, pattern)
            };
            let order = PATTERNS[(pattern % 4) as usize];
            let mut lines = [[0_u8; 8]; 8];
            for (slot, use_a) in lines.iter_mut().zip(order) {
                *slot = if use_a { a } else { b };
            }
            rows(out, base, lines);
        }
    }
    Ok(())
}

fn common_shifted(t: &LineTables, code: u32) -> Result<Line, FlipnoteError> {
    t.common_shift
        .get(code as usize)
        .copied()
        .ok_or(FlipnoteError::BadLineCode(code))
}

/// Writes eight rows of a block, starting at `base` and stepping one layer row at a time.
fn rows(out: &mut [u8], base: usize, lines: [Line; 8]) {
    for (r, line) in lines.iter().enumerate() {
        let start = base + r * LAYER_WIDTH;
        if let Some(dst) = out.get_mut(start..start + 8) {
            dst.copy_from_slice(line);
        }
    }
}
