//! The fixed header, the metadata block and the sound header of a PPM file. The offsets follow
//! the Flipnote Studio PPM layout as flipnote.js (MIT) reads it; see docs/FLIPNOTE.md.

use crate::flipnote::error::FlipnoteError;
use crate::flipnote::reader::Reader;

pub(crate) const MAGIC: &[u8; 4] = b"PARA";
/// The animation section starts here: its offset table length, then the frame pointers.
pub(crate) const ANIMATION_AT: usize = 0x06A0;
/// The first frame pointer. Its table is `ANIMATION_AT + 8` onwards when the table is empty.
pub(crate) const FRAME_TABLE_AT: usize = 0x06A8;
/// The metadata block is only read when the version's high nibble is set; it ends here.
const META_LEN: usize = 0x06A8;
const THUMB_AT: usize = 0xA0;
pub(crate) const THUMB_LEN: usize = 1536;
/// Nintendo timestamps count seconds from 2000-01-01 UTC.
const NINTENDO_EPOCH_UNIX: i64 = 946_684_800;
const SOUND_HEADER_LEN: usize = 32;

/// One author in the metadata: display name, FSID (hex, as Flipnote Studio shows it) and, for
/// the parent and the current author, the filename the Flipnote was saved under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Author {
    pub name: String,
    pub fsid: String,
    pub filename: Option<String>,
}

/// The metadata block (present when the version's high nibble is set).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub locked: bool,
    pub looped: bool,
    pub thumb_index: i16,
    /// Whether layer 1 and layer 2 are shown, from the flags word.
    pub layer_visible: [bool; 2],
    pub created_unix: i64,
    pub root: Author,
    pub parent: Author,
    pub current: Author,
}

/// A byte range of the file: where a sound track (or the sound header) starts and its length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Section {
    pub at: usize,
    pub len: usize,
}

/// The sound header: four sections (BGM, then three sound effects) and the two speed bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SoundHeader {
    pub bgm: Section,
    pub effects: [Section; 3],
    /// Raw speed bytes: the in-app speed is `8 - byte`.
    pub frame_speed_byte: u8,
    pub bgm_speed_byte: u8,
    /// Where the per-frame sound effect flags start (one byte per frame).
    pub effect_flags_at: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub frame_count: usize,
    pub version: u16,
    pub frame_data_len: usize,
    pub sound_data_len: usize,
    /// Where the sound header starts: after the frame data and the one-byte-per-frame flags.
    pub sound_at: usize,
    pub meta: Option<Meta>,
}

pub(crate) fn read(bytes: &[u8], max_frames: usize) -> Result<Header, FlipnoteError> {
    if bytes.get(..4) != Some(&MAGIC[..]) {
        return Err(FlipnoteError::NotPpm);
    }
    let mut r = Reader::at(bytes, 4)?;
    let frame_data_len = r.u32("the frame data length")? as usize;
    let sound_data_len = r.u32("the sound data length")? as usize;
    let frame_count = usize::from(r.u16("the frame count")?) + 1;
    let version = r.u16("the version")?;
    if frame_count > max_frames {
        return Err(FlipnoteError::TooManyFrames(frame_count, max_frames));
    }
    // The sound header follows the frame data and one flag byte per frame, aligned to 4 bytes.
    let mut sound_at = ANIMATION_AT
        .checked_add(frame_data_len)
        .and_then(|v| v.checked_add(frame_count))
        .ok_or(FlipnoteError::SectionOutside {
            what: "the sound header",
            at: usize::MAX,
            len: 0,
            file_len: bytes.len(),
        })?;
    if sound_at % 4 != 0 {
        sound_at += 4 - sound_at % 4;
    }
    if sound_at >= bytes.len() {
        return Err(FlipnoteError::SectionOutside {
            what: "the sound header",
            at: sound_at,
            len: SOUND_HEADER_LEN,
            file_len: bytes.len(),
        });
    }
    let meta = if (version >> 4) & 0xF != 0 {
        Some(read_meta(bytes)?)
    } else {
        None
    };
    Ok(Header {
        frame_count,
        version,
        frame_data_len,
        sound_data_len,
        sound_at,
        meta,
    })
}

fn read_meta(bytes: &[u8]) -> Result<Meta, FlipnoteError> {
    if bytes.len() < META_LEN {
        return Err(FlipnoteError::Truncated {
            what: "the metadata",
            at: 0x10,
            need: META_LEN,
            len: bytes.len(),
        });
    }
    let mut r = Reader::at(bytes, 0x10)?;
    let lock = r.u16("the lock flag")?;
    let thumb_index = r.i16("the thumbnail index")?;
    let root_name = wide_name(r.take(22, "the root author name")?);
    let parent_name = wide_name(r.take(22, "the parent author name")?);
    let current_name = wide_name(r.take(22, "the current author name")?);
    let parent_fsid = fsid(r.take(8, "the parent FSID")?);
    let current_fsid = fsid(r.take(8, "the current FSID")?);
    let parent_file = filename(r.take(18, "the parent filename")?);
    let current_file = filename(r.take(18, "the current filename")?);
    let root_fsid = fsid(r.take(8, "the root FSID")?);
    r.skip_to(0x9A, "the timestamp")?;
    let timestamp = r.i32("the timestamp")?;
    r.skip_to(0x6A6, "the flags")?;
    let flags = r.u16("the flags")?;
    Ok(Meta {
        locked: lock == 1,
        looped: (flags >> 1) & 1 == 1,
        thumb_index,
        layer_visible: [flags & 0x10 == 0, flags & 0x20 == 0],
        created_unix: i64::from(timestamp) + NINTENDO_EPOCH_UNIX,
        root: Author {
            name: root_name,
            fsid: root_fsid,
            filename: None,
        },
        parent: Author {
            name: parent_name,
            fsid: parent_fsid,
            filename: Some(parent_file),
        },
        current: Author {
            name: current_name,
            fsid: current_fsid,
            filename: Some(current_file),
        },
    })
}

/// Eleven UTF-16 units, read up to the first zero unit.
fn wide_name(raw: &[u8]) -> String {
    let (pairs, _) = raw.as_chunks::<2>();
    let units = pairs
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .take_while(|&u| u != 0);
    char::decode_utf16(units)
        .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
        .collect()
}

/// Eight bytes, shown as hex with the byte order reversed.
fn fsid(raw: &[u8]) -> String {
    raw.iter().rev().map(|b| format!("{b:02x}")).collect()
}

/// `mac_random_edits`: three MAC bytes, thirteen characters, and a decimal edit count.
fn filename(raw: &[u8]) -> String {
    let mac: String = raw.iter().take(3).map(|b| format!("{b:02x}")).collect();
    let random: String = raw
        .iter()
        .skip(3)
        .take(13)
        .take_while(|&&b| b != 0)
        .map(|&b| char::from(b))
        .collect();
    let edits = match raw.get(16..18) {
        Some(&[a, b]) => u16::from_le_bytes([a, b]),
        _ => 0,
    };
    format!("{mac}_{random}_{edits:03}")
}

/// The sound header at `at`: the four section lengths, the two speed bytes, and the effect flags
/// position. Sections are checked to lie inside the file.
pub(crate) fn read_sound(bytes: &[u8], header: &Header) -> Result<SoundHeader, FlipnoteError> {
    let mut r = Reader::at(bytes, header.sound_at)?;
    let lens = [
        r.u32("the BGM length")? as usize,
        r.u32("the sound effect 1 length")? as usize,
        r.u32("the sound effect 2 length")? as usize,
        r.u32("the sound effect 3 length")? as usize,
    ];
    let frame_speed_byte = r.u8("the frame speed")?;
    let bgm_speed_byte = r.u8("the BGM speed")?;
    let mut at = header.sound_at + SOUND_HEADER_LEN;
    let mut sections = [Section { at: 0, len: 0 }; 4];
    for (slot, len) in sections.iter_mut().zip(lens) {
        if len > 0 {
            let end = at.checked_add(len).filter(|&e| e <= bytes.len());
            if end.is_none() {
                return Err(FlipnoteError::SectionOutside {
                    what: "a sound track",
                    at,
                    len,
                    file_len: bytes.len(),
                });
            }
        }
        *slot = Section { at, len };
        at = at.saturating_add(len);
    }
    let [bgm, e1, e2, e3] = sections;
    Ok(SoundHeader {
        bgm,
        effects: [e1, e2, e3],
        frame_speed_byte,
        bgm_speed_byte,
        effect_flags_at: ANIMATION_AT + header.frame_data_len,
    })
}

/// The thumbnail's 1536 bytes of 4-bit tiles.
pub(crate) fn thumbnail_bytes(bytes: &[u8]) -> Result<&[u8], FlipnoteError> {
    let mut r = Reader::at(bytes, THUMB_AT)?;
    r.take(THUMB_LEN, "the thumbnail")
}
