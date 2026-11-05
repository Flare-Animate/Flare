//! Repairs a movie's tag stream before the reader sees it, the way Flash Player reads it.
//!
//! Flash Player plays movies whose tag blocks lack their End record: the movie's own, or a
//! sprite's. A sprite's block ends at its declared length either way. The reader in the `swf`
//! crate needs the record, so this adds it where it is missing. The same walk refuses sprites
//! nested deeper than [`MAX_SPRITE_NESTING`]: the reader recurses once per level, and a file of
//! nested sprite headers would run a 1 MiB stack out (the Windows main thread). The walk keeps
//! its own stack and never recurses.

use thiserror::Error;

/// How deep sprites may nest. Real movies nest a few levels.
pub const MAX_SPRITE_NESTING: usize = 32;

const END: u16 = 0;
const DEFINE_SPRITE: u16 = 39;
const LONG_LENGTH: u16 = 0x3f;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RepairError {
    #[error("a tag runs past the end of its block at byte {0}")]
    TagOverrun(usize),
    #[error("sprites nest deeper than {MAX_SPRITE_NESTING} levels")]
    TooDeep,
    #[error("the movie's tags grow past {0} bytes when their End records are added")]
    TooLarge(usize),
}

#[derive(Debug, PartialEq, Eq)]
pub struct Repaired {
    pub data: Vec<u8>,
    /// How many blocks had no End record and were given one.
    pub ends_added: u32,
}

/// A tag block open on the walk: the movie itself, or a sprite inside it.
struct Block {
    /// Where the block's tags end in the input.
    input_end: usize,
    /// Output offset of a sprite's four-byte length, patched when the block closes.
    length_at: Option<usize>,
    /// Output offset of the first byte after that length (the sprite's id).
    body_at: usize,
    saw_end: bool,
}

/// Returns `data` with the End records that a block lacks, or `data` unchanged when it needs none.
///
/// `max_bytes` bounds the output. Errors name the problem; nothing is guessed about a tag that
/// does not fit its block, since the reader would reject it too.
pub fn repair(data: &[u8], max_bytes: usize) -> Result<Repaired, RepairError> {
    let mut out = Vec::with_capacity(data.len().min(max_bytes));
    let mut blocks = vec![Block {
        input_end: data.len(),
        length_at: None,
        body_at: 0,
        saw_end: false,
    }];
    let mut ends_added = 0_u32;
    let mut pos = 0_usize;
    while let Some(block) = blocks.last() {
        let block_end = block.input_end;
        // Fewer than two bytes left cannot hold a tag header: the block is over. Flash Player
        // ignores such a remainder, and so does this.
        if block_end.saturating_sub(pos) < 2 {
            let Some(closed) = blocks.pop() else { break };
            if !closed.saw_end {
                out.extend_from_slice(&[0, 0]);
                ends_added += 1;
            }
            if let Some(at) = closed.length_at {
                let length = u32::try_from(out.len() - closed.body_at).unwrap_or(u32::MAX);
                patch_length(&mut out, at, length);
            }
            if blocks.is_empty() {
                break;
            }
            pos = closed.input_end;
            continue;
        }
        let code_length = u16::from_le_bytes([data[pos], data[pos + 1]]);
        let code = code_length >> 6;
        let short = code_length & LONG_LENGTH;
        let (header_len, length) = if short == LONG_LENGTH {
            let bytes = data
                .get(pos + 2..pos + 6)
                .ok_or(RepairError::TagOverrun(pos))?;
            let length = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            (6_usize, length as usize)
        } else {
            (2_usize, short as usize)
        };
        let body = pos + header_len;
        let tag_end = body
            .checked_add(length)
            .filter(|&end| end <= block_end)
            .ok_or(RepairError::TagOverrun(pos))?;
        match code {
            END => {
                out.extend_from_slice(&data[pos..pos + 2]);
                if let Some(block) = blocks.last_mut() {
                    block.saw_end = true;
                }
                // The reader stops at the End record, so anything after it in the block is unread.
                pos = block_end;
            }
            DEFINE_SPRITE if length >= 4 => {
                if blocks.len() > MAX_SPRITE_NESTING {
                    return Err(RepairError::TooDeep);
                }
                let length_at = out.len() + 2;
                out.extend_from_slice(&((DEFINE_SPRITE << 6) | LONG_LENGTH).to_le_bytes());
                out.extend_from_slice(&[0; 4]);
                let body_at = out.len();
                out.extend_from_slice(&data[body..body + 4]);
                blocks.push(Block {
                    input_end: tag_end,
                    length_at: Some(length_at),
                    body_at,
                    saw_end: false,
                });
                pos = body + 4;
            }
            _ => {
                out.extend_from_slice(&data[pos..tag_end]);
                pos = tag_end;
            }
        }
        if out.len() > max_bytes {
            return Err(RepairError::TooLarge(max_bytes));
        }
    }
    Ok(Repaired {
        data: out,
        ends_added,
    })
}

fn patch_length(out: &mut [u8], at: usize, length: u32) {
    if let Some(slot) = out.get_mut(at..at + 4) {
        slot.copy_from_slice(&length.to_le_bytes());
    }
}

#[cfg(test)]
#[path = "repair_tests.rs"]
mod tests;
