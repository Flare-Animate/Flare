//! The reasons a Flipnote Studio file is refused. Every read is bounded, so a damaged or hostile
//! file ends in one of these and never in a panic or an allocation the file size does not justify.

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FlipnoteError {
    #[error("not a Flipnote Studio PPM file (the magic is not PARA)")]
    NotPpm,
    #[error("the file is truncated: {what} needs {need} bytes at {at}, the file has {len}")]
    Truncated {
        what: &'static str,
        at: usize,
        need: usize,
        len: usize,
    },
    #[error("the file holds no frames")]
    NoFrames,
    #[error("the file holds {0} frames, more than the {1} this importer reads")]
    TooManyFrames(usize, usize),
    #[error("the frame table has {entries} entries for {frames} frames")]
    FrameTableShort { entries: usize, frames: usize },
    #[error("frame {frame} starts at {offset}, outside the {len} byte file")]
    FrameOutside {
        frame: usize,
        offset: usize,
        len: usize,
    },
    #[error("the frame speed {0} is not one of the speeds the format has")]
    BadSpeed(u8),
    #[error("the file has no {0} section")]
    MissingSection(&'static str),
    #[error("the file has {0} section bytes but its sections need {1}")]
    SectionsShort(usize, usize),
    #[error("the {what} at {at} with {len} bytes lies outside the {file_len} byte file")]
    SectionOutside {
        what: &'static str,
        at: usize,
        len: usize,
        file_len: usize,
    },
    #[error("a KWZ line code {0} is outside the table")]
    BadLineCode(u32),
    #[error("the frames decode to more than {0} bytes of pictures")]
    TooLarge(u64),
}
