//! Flipnote Studio PPM (DSi): a 256 by 192 animation of two 1-bit layers over a paper colour,
//! with a frame table, a sound header and four IMA ADPCM tracks. Read-only; frames are decoded
//! one at a time, in order, because a delta frame is the XOR of the frame before it.

pub mod adpcm;
pub(crate) mod frame;
pub mod header;
#[cfg(test)]
pub(crate) mod synth;
#[cfg(test)]
mod tests;

use crate::flipnote::error::FlipnoteError;
use crate::flipnote::reader::Reader;
pub use frame::{FrameDecoder, FrameInfo};
pub use header::{Author, Meta, Section, SoundHeader};

pub const WIDTH: usize = 256;
pub const HEIGHT: usize = 192;
/// Most frames read from one file. The format's counter is 16 bits; no Flipnote is near this.
pub const MAX_FRAMES: usize = 4096;
/// The global palette, as RGBA: paper and pen colours are indices into it.
pub const PALETTE: [[u8; 4]; 4] = [
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0x0E, 0x0E, 0x0E, 0xFF],
    [0xFF, 0x2A, 0x2A, 0xFF],
    [0x0A, 0x39, 0xFF, 0xFF],
];
/// Frames a second for each in-app speed, as (numerator, denominator). Speed 0 is not used by the
/// app; it is listed with speed 1 as the table has it.
const FRAME_RATES: [(u32, u32); 9] = [
    (1, 2),
    (1, 2),
    (1, 1),
    (2, 1),
    (4, 1),
    (6, 1),
    (12, 1),
    (20, 1),
    (30, 1),
];

pub struct Ppm<'a> {
    bytes: &'a [u8],
    header: header::Header,
    sound: SoundHeader,
    frame_offsets: Vec<usize>,
    frame_speed: u8,
    bgm_speed: u8,
}

impl<'a> Ppm<'a> {
    /// Reads the header, the frame table and the sound header. The frame data itself is only
    /// checked when it is decoded.
    pub fn parse(bytes: &'a [u8]) -> Result<Self, FlipnoteError> {
        let header = header::read(bytes, MAX_FRAMES)?;
        let frame_offsets = read_frame_table(bytes, header.frame_count)?;
        let sound = header::read_sound(bytes, &header)?;
        let frame_speed = speed(sound.frame_speed_byte)?;
        let bgm_speed = speed(sound.bgm_speed_byte)?;
        Ok(Self {
            bytes,
            header,
            sound,
            frame_offsets,
            frame_speed,
            bgm_speed,
        })
    }

    pub fn frame_count(&self) -> usize {
        self.header.frame_count
    }

    pub fn meta(&self) -> Option<&Meta> {
        self.header.meta.as_ref()
    }

    /// The frame rate as (numerator, denominator).
    pub fn frame_rate(&self) -> (u32, u32) {
        FRAME_RATES
            .get(usize::from(self.frame_speed))
            .copied()
            .unwrap_or((1, 2))
    }

    pub fn bgm_speed(&self) -> u8 {
        self.bgm_speed
    }

    pub fn sound(&self) -> &SoundHeader {
        &self.sound
    }

    /// The bytes of one sound section; empty for a track the file does not have.
    pub fn section_bytes(&self, section: Section) -> Result<&'a [u8], FlipnoteError> {
        let end = section
            .at
            .checked_add(section.len)
            .filter(|&e| e <= self.bytes.len());
        match end {
            Some(end) => Ok(self.bytes.get(section.at..end).unwrap_or_default()),
            None => Err(FlipnoteError::SectionOutside {
                what: "a sound track",
                at: section.at,
                len: section.len,
                file_len: self.bytes.len(),
            }),
        }
    }

    /// Whether the sound effect `index` (0 to 2) plays at `frame`, from the per-frame flag byte.
    pub fn effect_on(&self, frame: usize, index: usize) -> Result<bool, FlipnoteError> {
        let at = self.sound.effect_flags_at + frame;
        let byte = self.byte(at, "a sound effect flag")?;
        Ok(byte & (1 << index) != 0)
    }

    /// Decodes frame `index` into `dec`, which must have decoded every frame before it.
    pub fn decode_frame(
        &self,
        dec: &mut FrameDecoder,
        index: usize,
    ) -> Result<FrameInfo, FlipnoteError> {
        let at = *self
            .frame_offsets
            .get(index)
            .ok_or(FlipnoteError::FrameOutside {
                frame: index,
                offset: 0,
                len: self.bytes.len(),
            })?;
        frame::decode(self.bytes, at, dec)
    }

    /// The file offset where frame `index` starts.
    pub fn frame_offset(&self, index: usize) -> Option<usize> {
        self.frame_offsets.get(index).copied()
    }

    /// The 1536 bytes of the thumbnail.
    pub fn thumbnail_bytes(&self) -> Result<&'a [u8], FlipnoteError> {
        header::thumbnail_bytes(self.bytes)
    }

    fn byte(&self, at: usize, what: &'static str) -> Result<u8, FlipnoteError> {
        self.bytes.get(at).copied().ok_or(FlipnoteError::Truncated {
            what,
            at,
            need: 1,
            len: self.bytes.len(),
        })
    }
}

/// The offset table: its length is at `0x6A0`, then one pointer per entry relative to the end of
/// the table. The first `frame_count` entries are the frames.
fn read_frame_table(bytes: &[u8], frame_count: usize) -> Result<Vec<usize>, FlipnoteError> {
    let mut r = Reader::at(bytes, header::ANIMATION_AT)?;
    let table_len = usize::from(r.u16("the frame table length")?);
    if table_len == 0 {
        return Err(FlipnoteError::NoFrames);
    }
    let entries = table_len / 4;
    if entries < frame_count {
        return Err(FlipnoteError::FrameTableShort {
            entries,
            frames: frame_count,
        });
    }
    r.skip_to(header::FRAME_TABLE_AT, "the frame pointers")?;
    let base = header::FRAME_TABLE_AT + table_len;
    let mut out = Vec::with_capacity(frame_count);
    for frame in 0..frame_count {
        let rel = r.u32("a frame pointer")? as usize;
        let at = base.saturating_add(rel);
        if at >= bytes.len() {
            return Err(FlipnoteError::FrameOutside {
                frame,
                offset: at,
                len: bytes.len(),
            });
        }
        out.push(at);
    }
    Ok(out)
}

/// The in-app speed for a stored speed byte (`8 - byte`); bytes above 8 are refused.
fn speed(byte: u8) -> Result<u8, FlipnoteError> {
    8_u8.checked_sub(byte).ok_or(FlipnoteError::BadSpeed(byte))
}
