//! A movie's tag stream, walked at the byte level so its script blocks can be found and one
//! replaced without re-encoding any other tag.

use crate::{MAX_BLOCKS, MAX_MOVIE_BYTES, MAX_UNCOMPRESSED_BYTES, ScriptError};
use serde::Serialize;
use swf::avm2::types::AbcFile;

/// `DoABC` without flags or a name (Flash Player 9's first form).
const TAG_DO_ABC: u16 = 72;
/// `DoABC2`: flags, a name, then the block.
const TAG_DO_ABC2: u16 = 82;
const TAG_END: u16 = 0;

/// A Flash movie, decompressed, with its script blocks found.
pub struct Movie {
    header: swf::Header,
    /// The tag stream after the header, as decompressed.
    tags: Vec<u8>,
    blocks: Vec<Block>,
}

/// Where one script block sits in the tag stream.
struct Block {
    /// `TAG_DO_ABC` or `TAG_DO_ABC2`.
    code: u16,
    /// The tag's first byte (its code and length).
    tag_start: usize,
    /// The first byte of the tag's body, after its length.
    body_start: usize,
    /// The first byte after the tag.
    tag_end: usize,
    /// The first byte of the ABC data; before it are DoABC2's flags and name.
    abc_start: usize,
    name: String,
}

/// What a surface shows of one block.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BlockInfo {
    pub index: usize,
    /// The name DoABC2 gives it (often the frame or the main class); empty for DoABC.
    pub name: String,
    pub bytes: usize,
}

impl Movie {
    /// Reads a movie from its bytes: uncompressed, zlib or LZMA, any version.
    pub fn read(bytes: &[u8]) -> Result<Self, ScriptError> {
        if bytes.len() as u64 > MAX_MOVIE_BYTES {
            return Err(ScriptError::TooLarge {
                what: "on disk",
                limit: MAX_MOVIE_BYTES,
            });
        }
        if !matches!(bytes.get(..3), Some(b"FWS" | b"CWS" | b"ZWS")) {
            return Err(ScriptError::NotSwf(
                "it does not start like a Flash movie".into(),
            ));
        }
        // Bytes 4..8 are the length once decompressed, which the reader would allocate for.
        let claimed = bytes
            .get(4..8)
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
            .map_or(0, u32::from_le_bytes);
        if claimed > MAX_UNCOMPRESSED_BYTES {
            return Err(ScriptError::TooLarge {
                what: "once decompressed",
                limit: u64::from(MAX_UNCOMPRESSED_BYTES),
            });
        }
        let buf = swf::decompress_swf(bytes).map_err(|e| ScriptError::NotSwf(e.to_string()))?;
        let header = buf.header.swf_header().clone();
        let tags = buf.data;
        let blocks = find_blocks(&tags)?;
        if blocks.is_empty() {
            return Err(ScriptError::NoScripts);
        }
        Ok(Self {
            header,
            tags,
            blocks,
        })
    }

    /// The script blocks, in the order the player runs them.
    pub fn blocks(&self) -> Vec<BlockInfo> {
        self.blocks
            .iter()
            .enumerate()
            .map(|(index, block)| BlockInfo {
                index,
                name: block.name.clone(),
                bytes: block.tag_end - block.abc_start,
            })
            .collect()
    }

    pub(crate) fn block_count(&self) -> usize {
        self.blocks.len()
    }

    /// The ABC bytes of block `index`.
    pub(crate) fn abc_bytes(&self, index: usize) -> Option<&[u8]> {
        let block = self.blocks.get(index)?;
        self.tags.get(block.abc_start..block.tag_end)
    }

    /// Block `index` read into its constant pool, methods, classes and scripts.
    pub(crate) fn abc(&self, index: usize) -> Result<AbcFile, ScriptError> {
        let bytes = self.abc_bytes(index).ok_or(ScriptError::Unreadable {
            block: index,
            reason: "it is not in the movie".into(),
        })?;
        swf::avm2::read::Reader::new(bytes)
            .read()
            .map_err(|e| ScriptError::Unreadable {
                block: index,
                reason: e.to_string(),
            })
    }

    /// The movie written back with the ABC of some blocks replaced, in the movie's own
    /// compression. Every other tag keeps its bytes; a replaced block keeps its flags and name.
    pub(crate) fn rebuilt(&self, replaced: &[(usize, Vec<u8>)]) -> Result<Vec<u8>, ScriptError> {
        let mut tags = Vec::with_capacity(self.tags.len());
        let mut at = 0;
        for (index, block) in self.blocks.iter().enumerate() {
            let Some((_, abc)) = replaced.iter().find(|(i, _)| *i == index) else {
                continue;
            };
            tags.extend_from_slice(&self.tags[at..block.tag_start]);
            let prefix = &self.tags[block.body_start..block.abc_start];
            write_tag_header(&mut tags, block.code, prefix.len() + abc.len())?;
            tags.extend_from_slice(prefix);
            tags.extend_from_slice(abc);
            at = block.tag_end;
        }
        tags.extend_from_slice(&self.tags[at..]);
        let mut out = Vec::with_capacity(tags.len() / 2);
        swf::write::write_swf_raw_tags(&self.header, &tags, &mut out)
            .map_err(|e| ScriptError::NotSwf(e.to_string()))?;
        Ok(out)
    }
}

fn write_tag_header(out: &mut Vec<u8>, code: u16, len: usize) -> Result<(), ScriptError> {
    let len = u32::try_from(len).map_err(|_| ScriptError::TooLarge {
        what: "in one script block",
        limit: u64::from(u32::MAX),
    })?;
    // The long form is always valid; the short one only below 63 bytes.
    if len < 0x3f {
        out.extend_from_slice(&((code << 6) | len as u16).to_le_bytes());
    } else {
        out.extend_from_slice(&((code << 6) | 0x3f).to_le_bytes());
        out.extend_from_slice(&len.to_le_bytes());
    }
    Ok(())
}

/// Walks the top-level tags up to End (or the end of the data) and notes each script block.
fn find_blocks(tags: &[u8]) -> Result<Vec<Block>, ScriptError> {
    let mut blocks = Vec::new();
    let mut at = 0usize;
    while at + 2 <= tags.len() {
        let code_and_len = u16::from_le_bytes([tags[at], tags[at + 1]]);
        let code = code_and_len >> 6;
        let mut len = usize::from(code_and_len & 0x3f);
        let mut body = at + 2;
        if len == 0x3f {
            let long = tags.get(body..body + 4).ok_or(ScriptError::Truncated(at))?;
            len = u32::from_le_bytes([long[0], long[1], long[2], long[3]]) as usize;
            body += 4;
        }
        let end = body
            .checked_add(len)
            .filter(|end| *end <= tags.len())
            .ok_or(ScriptError::Truncated(at))?;
        match code {
            TAG_END => break,
            TAG_DO_ABC => blocks.push(Block {
                code,
                tag_start: at,
                body_start: body,
                tag_end: end,
                abc_start: body,
                name: String::new(),
            }),
            TAG_DO_ABC2 => {
                // Flags (4 bytes), then a name ending in a zero byte.
                let name_start = body + 4;
                let name_len = tags
                    .get(name_start..end)
                    .and_then(|rest| rest.iter().position(|b| *b == 0))
                    .ok_or(ScriptError::Truncated(at))?;
                blocks.push(Block {
                    code,
                    tag_start: at,
                    body_start: body,
                    tag_end: end,
                    abc_start: name_start + name_len + 1,
                    name: String::from_utf8_lossy(&tags[name_start..name_start + name_len])
                        .into_owned(),
                });
            }
            _ => {}
        }
        if blocks.len() > MAX_BLOCKS {
            return Err(ScriptError::TooManyBlocks);
        }
        at = end;
    }
    Ok(blocks)
}
