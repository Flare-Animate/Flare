//! Flipnote Studio 3D (KWZ, and the KWC comments that share its layout): a 320 by 240 animation
//! of three layers, each with two pens and a shared paper colour, in tile-coded layers. The file
//! is a run of sections: `KFH` (the header and metadata), `KTN` (thumbnail), `KMC` (the layer
//! data), `KMI` (one 28-byte record per frame), and `KSN` (sound; absent in comments), then a
//! 256-byte RSA signature that is not checked. Layouts follow flipnote.js (MIT); see
//! docs/FLIPNOTE.md.

pub(crate) mod bits;
pub(crate) mod convert;
pub mod lines;
#[cfg(test)]
mod synth;
#[cfg(test)]
mod tests;
pub mod tiles;

use super::error::FlipnoteError;
use super::reader::Reader;

pub use lines::LineTables;
pub use tiles::{LAYER_HEIGHT, LAYER_WIDTH};

/// Most frames read from one file; the frame counter is 16 bits.
pub const MAX_FRAMES: usize = 4096;
/// Frames a second for each speed, as (numerator, denominator).
pub const FRAME_RATES: [(u32, u32); 11] = [
    (1, 5),
    (1, 2),
    (1, 1),
    (2, 1),
    (4, 1),
    (6, 1),
    (8, 1),
    (12, 1),
    (20, 1),
    (24, 1),
    (30, 1),
];
/// Global palette as RGBA: index 6 is "no colour" (transparent).
pub const PALETTE: [[u8; 4]; 7] = [
    [0xFF, 0xFF, 0xFF, 0xFF],
    [0x10, 0x10, 0x10, 0xFF],
    [0xFF, 0x10, 0x10, 0xFF],
    [0xFF, 0xE7, 0x00, 0xFF],
    [0x00, 0x86, 0x31, 0xFF],
    [0x00, 0x38, 0xCE, 0xFF],
    [0xFF, 0xFF, 0xFF, 0x00],
];
const FRAME_META_LEN: usize = 28;
const LAYER_UNCHANGED: u32 = 38;
const SIGNATURE_LEN: usize = 256;
const MAX_SECTIONS: usize = 6;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub at: usize,
    pub len: usize,
}

/// A Flipnote author as the header records them: name, hex FSID and a 28-byte filename.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub fsid: String,
    pub filename: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub frame_count: usize,
    pub speed: u8,
    pub thumb_index: u16,
    pub locked: bool,
    pub looped: bool,
    /// Layer 1, 2 and 3 visibility as the flags word gives it (see docs/FLIPNOTE.md).
    pub layer_visible: [bool; 3],
    pub created_unix: i64,
    pub modified_unix: i64,
    pub root: Author,
    pub parent: Author,
    pub current: Author,
}

/// One frame's record: the palette nibbles (paper, then two pens per layer), the layer depths and
/// where each layer's bits lie in the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrameMeta {
    pub palette: [u8; 7],
    pub depths: [u8; 3],
    pub sizes: [usize; 3],
    pub data: [usize; 3],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sound {
    pub bgm_speed: u32,
    /// Byte lengths of the BGM and the four sound effect tracks.
    pub tracks: [usize; 5],
}

pub struct Kwz<'a> {
    bytes: &'a [u8],
    meta: Meta,
    frames: Vec<FrameMeta>,
    sound: Option<Sound>,
    sections: Vec<(String, Section)>,
}

/// The decoded layers of the last frame, one byte per pixel (0 is transparent, 1 and 2 are the
/// layer's pens).
pub struct KwzState {
    layers: [Vec<u8>; 3],
    tables: LineTables,
}

impl KwzState {
    pub fn new() -> Self {
        let n = LAYER_WIDTH * LAYER_HEIGHT;
        Self {
            layers: [vec![0; n], vec![0; n], vec![0; n]],
            tables: LineTables::new(),
        }
    }

    pub fn layers(&self) -> &[Vec<u8>; 3] {
        &self.layers
    }
}

impl Default for KwzState {
    fn default() -> Self {
        Self::new()
    }
}

/// What a decoded frame says besides its pixels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FrameInfo {
    /// Bytes of each layer's bits the decode read (0 for an unchanged layer).
    pub consumed: [usize; 3],
}

fn section_name(magic: &[u8]) -> String {
    magic.iter().take(3).map(|&b| char::from(b)).collect()
}

impl<'a> Kwz<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, FlipnoteError> {
        let sections = read_sections(bytes)?;
        let find = |name: &str| sections.iter().find(|(n, _)| n == name).map(|(_, s)| *s);
        let kfh = find("KFH").ok_or(FlipnoteError::MissingSection("KFH"))?;
        let kmc = find("KMC").ok_or(FlipnoteError::MissingSection("KMC"))?;
        let kmi = find("KMI").ok_or(FlipnoteError::MissingSection("KMI"))?;
        let meta = read_meta(bytes, kfh)?;
        if meta.frame_count == 0 {
            return Err(FlipnoteError::NoFrames);
        }
        if meta.frame_count > MAX_FRAMES {
            return Err(FlipnoteError::TooManyFrames(meta.frame_count, MAX_FRAMES));
        }
        let frames = read_frames(bytes, kmc, kmi, meta.frame_count)?;
        let sound = match find("KSN") {
            Some(ksn) => Some(read_sound(bytes, ksn)?),
            None => None,
        };
        Ok(Self {
            bytes,
            meta,
            frames,
            sound,
            sections,
        })
    }

    pub fn meta(&self) -> &Meta {
        &self.meta
    }

    pub fn frame_count(&self) -> usize {
        self.frames.len()
    }

    pub fn frame(&self, index: usize) -> Option<&FrameMeta> {
        self.frames.get(index)
    }

    pub fn sound(&self) -> Option<&Sound> {
        self.sound.as_ref()
    }

    pub fn sections(&self) -> &[(String, Section)] {
        &self.sections
    }

    /// The frame rate as (numerator, denominator).
    pub fn frame_rate(&self) -> (u32, u32) {
        FRAME_RATES
            .get(usize::from(self.meta.speed))
            .copied()
            .unwrap_or((1, 1))
    }

    /// Decodes frame `index` into `state`. Layers the frame leaves unchanged keep their pixels
    /// from the frame before, so frames must be decoded in order from 0.
    pub fn decode_frame(
        &self,
        state: &mut KwzState,
        index: usize,
    ) -> Result<FrameInfo, FlipnoteError> {
        let frame = self.frames.get(index).ok_or(FlipnoteError::FrameOutside {
            frame: index,
            offset: 0,
            len: self.bytes.len(),
        })?;
        let mut consumed = [0_usize; 3];
        for (k, layer) in state.layers.iter_mut().enumerate() {
            let size = frame.sizes.get(k).copied().unwrap_or(0);
            if size as u32 == LAYER_UNCHANGED {
                continue;
            }
            let at = frame.data.get(k).copied().unwrap_or(0);
            let data = self
                .bytes
                .get(at..at + size)
                .ok_or(FlipnoteError::SectionOutside {
                    what: "a KWZ layer",
                    at,
                    len: size,
                    file_len: self.bytes.len(),
                })?;
            let used = tiles::decode_layer(data, layer, &state.tables)?;
            if let Some(slot) = consumed.get_mut(k) {
                *slot = used;
            }
        }
        Ok(FrameInfo { consumed })
    }
}

fn read_sections(bytes: &[u8]) -> Result<Vec<(String, Section)>, FlipnoteError> {
    let file_len = bytes.len();
    let body_end = file_len.saturating_sub(SIGNATURE_LEN);
    let mut out: Vec<(String, Section)> = Vec::new();
    let mut ptr = 0_usize;
    while ptr < body_end && out.len() < MAX_SECTIONS {
        let mut r = Reader::at(bytes, ptr)?;
        let magic = r.take(4, "a KWZ section header")?;
        let len = r.u32("a KWZ section length")? as usize;
        let end = ptr
            .checked_add(8)
            .and_then(|v| v.checked_add(len))
            .filter(|&e| e <= file_len)
            .ok_or(FlipnoteError::SectionOutside {
                what: "a KWZ section",
                at: ptr,
                len,
                file_len,
            })?;
        let name = section_name(magic);
        let section = Section { at: ptr, len };
        match out.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = section,
            None => out.push((name, section)),
        }
        ptr = end;
    }
    Ok(out)
}

fn fixed_wide(bytes: &[u8], at: usize, units: usize) -> Result<String, FlipnoteError> {
    let mut r = Reader::at(bytes, at)?;
    let raw = r.take(units * 2, "a KWZ author name")?;
    let (pairs, _) = raw.as_chunks::<2>();
    let text = pairs
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&u| u != 0);
    Ok(char::decode_utf16(text)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect())
}

fn fixed_chars(bytes: &[u8], at: usize, len: usize) -> Result<String, FlipnoteError> {
    let mut r = Reader::at(bytes, at)?;
    let raw = r.take(len, "a KWZ filename")?;
    Ok(raw
        .iter()
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b))
        .collect())
}

fn fsid_hex(bytes: &[u8], at: usize) -> Result<String, FlipnoteError> {
    let mut r = Reader::at(bytes, at)?;
    let raw = r.take(10, "a KWZ FSID")?;
    Ok(raw.iter().map(|b| format!("{b:02x}")).collect())
}

fn author(
    bytes: &[u8],
    base: usize,
    fsid_at: usize,
    name_at: usize,
    file_at: usize,
) -> Result<Author, FlipnoteError> {
    Ok(Author {
        name: fixed_wide(bytes, base + name_at, 11)?,
        fsid: fsid_hex(bytes, base + fsid_at)?,
        filename: fixed_chars(bytes, base + file_at, 28)?,
    })
}

/// The header fields at fixed offsets from the start of the `KFH` section.
fn read_meta(bytes: &[u8], kfh: Section) -> Result<Meta, FlipnoteError> {
    let base = kfh.at;
    // Offsets are from the section's first byte, and the section is its length plus 8.
    if kfh.len + 8 < 0xD4 {
        return Err(FlipnoteError::SectionOutside {
            what: "the KWZ header",
            at: base,
            len: 0xD4,
            file_len: kfh.len + 8,
        });
    }
    let mut r = Reader::at(bytes, base + 12)?;
    let created = r.u32("the creation time")?;
    let modified = r.u32("the modified time")?;
    r.take(4, "the application version")?;
    let root = author(bytes, base, 24, 54, 120)?;
    let parent = author(bytes, base, 34, 76, 148)?;
    let current = author(bytes, base, 44, 98, 176)?;
    let mut r = Reader::at(bytes, base + 204)?;
    let frame_count = usize::from(r.u16("the frame count")?);
    let thumb_index = r.u16("the thumbnail index")?;
    let flags = r.u16("the flags")?;
    let speed = r.u8("the frame speed")?;
    let layer_flags = r.u8("the layer flags")?;
    if usize::from(speed) >= FRAME_RATES.len() {
        return Err(FlipnoteError::BadSpeed(speed));
    }
    let nibble = |bit: u8| layer_flags & (1 << bit) == 0;
    Ok(Meta {
        frame_count,
        speed,
        thumb_index,
        locked: flags & 0x1 != 0,
        looped: flags & 0x2 != 0,
        layer_visible: [nibble(0), nibble(1), layer_flags & 0x3 == 0],
        created_unix: i64::from(created) + 946_684_800,
        modified_unix: i64::from(modified) + 946_684_800,
        root,
        parent,
        current,
    })
}

/// The `KMI` records (one per frame) and the byte ranges they point at in `KMC`.
fn read_frames(
    bytes: &[u8],
    kmc: Section,
    kmi: Section,
    frame_count: usize,
) -> Result<Vec<FrameMeta>, FlipnoteError> {
    if kmi.len / FRAME_META_LEN < frame_count {
        return Err(FlipnoteError::FrameTableShort {
            entries: kmi.len / FRAME_META_LEN,
            frames: frame_count,
        });
    }
    let kmc_end = kmc.at + 8 + kmc.len;
    let mut data_at = kmc.at + 12;
    let mut out = Vec::with_capacity(frame_count);
    for index in 0..frame_count {
        let rec = kmi.at + 8 + index * FRAME_META_LEN;
        let mut r = Reader::at(bytes, rec)?;
        let flags = r.u32("a frame record")?;
        let mut sizes_reader = Reader::at(bytes, rec + 4)?;
        let sizes = [
            usize::from(sizes_reader.u16("a layer size")?),
            usize::from(sizes_reader.u16("a layer size")?),
            usize::from(sizes_reader.u16("a layer size")?),
        ];
        let mut depth_reader = Reader::at(bytes, rec + 0x14)?;
        let depths = [
            depth_reader.u8("a layer depth")?,
            depth_reader.u8("a layer depth")?,
            depth_reader.u8("a layer depth")?,
        ];
        let palette = [
            (flags & 0xF) as u8,
            ((flags >> 8) & 0xF) as u8,
            ((flags >> 12) & 0xF) as u8,
            ((flags >> 16) & 0xF) as u8,
            ((flags >> 20) & 0xF) as u8,
            ((flags >> 24) & 0xF) as u8,
            ((flags >> 28) & 0xF) as u8,
        ];
        let mut data = [0_usize; 3];
        for (slot, size) in data.iter_mut().zip(sizes) {
            *slot = data_at;
            data_at = data_at.saturating_add(size);
        }
        if data_at > kmc_end {
            return Err(FlipnoteError::SectionOutside {
                what: "a frame's layers",
                at: data_at,
                len: 0,
                file_len: kmc_end,
            });
        }
        out.push(FrameMeta {
            palette,
            depths,
            sizes,
            data,
        });
    }
    Ok(out)
}

fn read_sound(bytes: &[u8], ksn: Section) -> Result<Sound, FlipnoteError> {
    let mut r = Reader::at(bytes, ksn.at + 8)?;
    let bgm_speed = r.u32("the BGM speed")?;
    let mut tracks = [0_usize; 5];
    for slot in tracks.iter_mut() {
        *slot = r.u32("a sound track length")? as usize;
    }
    let total: usize = tracks.iter().sum();
    if 28_usize.saturating_add(total) > ksn.len {
        return Err(FlipnoteError::SectionOutside {
            what: "the sound tracks",
            at: ksn.at,
            len: total,
            file_len: ksn.len,
        });
    }
    Ok(Sound { bgm_speed, tracks })
}
