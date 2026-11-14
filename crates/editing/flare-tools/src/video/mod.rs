//! A video brought in to draw over: its pictures, taken at a rate the person picks, become the
//! frames of one Graphic symbol, and one instance of it plays on a layer of its own at the bottom
//! of the scene, one picture to a frame of the timeline, drawn see-through while editing. Moving,
//! scaling or turning that instance moves every picture alike, so the video is lined up once.
//!
//! Pictures are kept decoded, so a video is costly: [`plan`] says before anything is read how
//! many frames it makes, how large they are, how much of the document's room for pictures they
//! take, how long the animation runs at the document's rate, and, when it is too large, how long
//! or how small it would have to be to fit. [`place`] turns the pictures read into one batch, one
//! undo step. The desktop app, `flare video-import` and the MCP server's `import_video` all call
//! these. Reading the movie itself is `flare-import`'s, through ffmpeg.

mod place;
mod report;

pub use place::{Placed, place};
pub use report::{Asked, plan_json};

use std::fmt;

use flare_core::FrameRate;
use flare_document::{Document, MAX_BITMAPS, MAX_TOTAL_BITMAP_BYTES};

/// Longest side a picture of the video may be read at, in pixels.
pub const MAX_VIDEO_SIDE: u32 = 3840;
/// Shortest side asked for, in pixels; smaller is no use to draw over.
pub const MIN_VIDEO_SIDE: u32 = 16;
/// The longest stretch of a video one import may ask for, in seconds.
pub const MAX_VIDEO_SECONDS: f64 = 3600.0;
/// The longest side a picture is read at unless the person asks for more: enough to trace, and a
/// quarter of the memory of a full HD picture.
pub const DEFAULT_SIDE: u32 = 960;
/// How much of the video is read unless the person asks for more, in seconds.
pub const DEFAULT_SECONDS: f64 = 60.0;
/// How opaque the stage draws the video while editing, in percent, unless asked otherwise.
pub const DEFAULT_OPACITY: u8 = 50;
/// Above this many bytes of pictures an import is large: the project is slow to save and open.
pub const LARGE_BYTES: u64 = 256 * 1024 * 1024;

/// What the movie is, as ffprobe says.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Source {
    /// Its pictures' size as shown (a movie stored on its side already turned upright).
    pub width: u32,
    pub height: u32,
    /// Its own frames a second.
    pub rate: FrameRate,
    /// How long it runs, when the file says.
    pub seconds: Option<f64>,
}

/// What the person chose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Pictures taken from each second of the video; each takes one frame of the timeline.
    pub rate: FrameRate,
    /// The most of the video read, from its start, in seconds.
    pub max_seconds: f64,
    /// The longest side of a picture, in pixels; a smaller video is never enlarged.
    pub max_side: u32,
    /// How opaque the stage draws the video's layer while editing, 0 to 100; exports draw it whole.
    pub opacity: u8,
}

impl Settings {
    /// Where the dialog starts: the document's own rate, so the video keeps its timing, pictures
    /// at most [`DEFAULT_SIDE`] long, and as much of the first [`DEFAULT_SECONDS`] as fits.
    pub fn suggested(document: &Document, source: &Source) -> Self {
        let rate = document.frame_rate();
        let mut settings = Self {
            rate,
            max_seconds: source
                .seconds
                .unwrap_or(DEFAULT_SECONDS)
                .clamp(1.0 / rate.per_second(), DEFAULT_SECONDS),
            max_side: DEFAULT_SIDE.clamp(MIN_VIDEO_SIDE, MAX_VIDEO_SIDE),
            opacity: DEFAULT_OPACITY,
        };
        if let Ok(plan) = plan(document, source, &settings)
            && !plan.fits()
            && plan.fitting_seconds > 0.0
        {
            // Whole seconds, so the dialog shows a round length that does fit.
            settings.max_seconds = plan
                .fitting_seconds
                .floor()
                .max(plan.fitting_seconds.min(1.0));
        }
        settings
    }
}

/// Why the video does not fit in the document.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TooLarge {
    /// More pictures than the library has room for.
    Frames { needed: u32, room: u32 },
    /// More bytes of pictures than the document has room for.
    Bytes { needed: u64, room: u64 },
}

impl fmt::Display for TooLarge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Frames { needed, room } => write!(
                f,
                "{needed} pictures, and the document has room for {room} more"
            ),
            Self::Bytes { needed, room } => write!(
                f,
                "{} MB of pictures, and the document has room for {} MB more",
                megabytes(*needed),
                megabytes(*room)
            ),
        }
    }
}

/// Bytes as whole megabytes (of 1,048,576), rounded up so a little is never called nothing.
pub fn megabytes(bytes: u64) -> u64 {
    bytes.div_ceil(1024 * 1024)
}

/// Why a video cannot be imported as asked.
#[derive(Debug, Clone, PartialEq)]
pub enum VideoRefusal {
    /// The movie has no size, or none that can be read.
    NoSize,
    /// The rate, the length or the size asked for is outside what an import takes.
    Setting(&'static str),
    NoSuchScene,
    /// The movie gave no pictures.
    NoPictures,
    /// A picture is not the size the plan said.
    WrongSize {
        index: usize,
    },
    TooLarge(TooLarge),
    /// The layer, the symbol or a picture could not be built.
    Build(String),
}

impl fmt::Display for VideoRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSize => f.write_str("the video's pictures have no size"),
            Self::Setting(what) => f.write_str(what),
            Self::NoSuchScene => f.write_str("no such scene"),
            Self::NoPictures => f.write_str("the video gave no pictures"),
            Self::WrongSize { index } => {
                write!(f, "picture {} is not the size asked for", index + 1)
            }
            Self::TooLarge(why) => write!(f, "the video is too large to import: {why}"),
            Self::Build(why) => write!(f, "the video's layer cannot be made: {why}"),
        }
    }
}

impl std::error::Error for VideoRefusal {}

/// What an import of `source` with `settings` makes, worked out before anything is read.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plan {
    pub source: Source,
    pub settings: Settings,
    /// Each picture's size in pixels.
    pub size: (u32, u32),
    /// Pictures read, so frames of the timeline; at most this when the movie's length is unknown.
    pub frames: u32,
    /// Seconds of the video they cover.
    pub seconds: f64,
    /// Whether that is the whole video (false when it is cut short, or its length is unknown).
    pub whole: bool,
    /// Bytes of pictures, before pictures that repeat are kept once.
    pub bytes: u64,
    /// The document's frame rate, which the timeline plays the pictures at.
    pub document_rate: FrameRate,
    /// How long the frames last on the timeline at the document's rate, in seconds.
    pub length_seconds: f64,
    /// How much faster than the video the timeline shows it: 1 keeps its timing, 2 is twice as
    /// fast (pictures taken at 12 a second shown at 24).
    pub speed: f64,
    pub too_large: Option<TooLarge>,
    /// The most seconds of the video that fit at this rate and size (the whole video at most).
    pub fitting_seconds: f64,
    /// The largest `max_side` at which these frames fit; `None` when they do not fit even at
    /// [`MIN_VIDEO_SIDE`] (too many pictures, rather than too large ones).
    pub fitting_side: Option<u32>,
}

impl Plan {
    pub fn fits(&self) -> bool {
        self.too_large.is_none()
    }

    /// An import that fits but makes the project slow to save and open.
    pub fn is_large(&self) -> bool {
        self.bytes > LARGE_BYTES
    }

    /// Whether the timeline shows the video at its own speed.
    pub fn keeps_timing(&self) -> bool {
        (self.speed - 1.0).abs() < 1e-3
    }

    fn frame_bytes(&self) -> u64 {
        u64::from(self.size.0) * u64::from(self.size.1) * 4
    }
}

/// What importing `source` into `document` with `settings` would make, and whether it fits in
/// the room the document has left for pictures.
pub fn plan(
    document: &Document,
    source: &Source,
    settings: &Settings,
) -> Result<Plan, VideoRefusal> {
    if source.width == 0 || source.height == 0 {
        return Err(VideoRefusal::NoSize);
    }
    if !(settings.max_seconds.is_finite()
        && settings.max_seconds > 0.0
        && settings.max_seconds <= MAX_VIDEO_SECONDS)
    {
        return Err(VideoRefusal::Setting(
            "the most of the video to read is above 0 and at most 60 minutes",
        ));
    }
    if !(MIN_VIDEO_SIDE..=MAX_VIDEO_SIDE).contains(&settings.max_side) {
        return Err(VideoRefusal::Setting(
            "the pictures' longest side is 16 to 3840 pixels",
        ));
    }
    if settings.opacity > 100 {
        return Err(VideoRefusal::Setting("the opacity is 0 to 100 percent"));
    }
    let fps = settings.rate.per_second();
    let video_seconds = source.seconds.filter(|s| s.is_finite() && *s > 0.0);
    let seconds = video_seconds.map_or(settings.max_seconds, |s| s.min(settings.max_seconds));
    let whole = video_seconds.is_some_and(|s| s <= settings.max_seconds);
    let frames = frames_in(seconds, fps);
    let size = picture_size(source, settings.max_side);
    let frame_bytes = u64::from(size.0) * u64::from(size.1) * 4;
    let bytes = frame_bytes * u64::from(frames);
    let (room_count, room_bytes) = room(document);
    let too_large = if frames > room_count {
        Some(TooLarge::Frames {
            needed: frames,
            room: room_count,
        })
    } else if bytes > room_bytes {
        Some(TooLarge::Bytes {
            needed: bytes,
            room: room_bytes,
        })
    } else {
        None
    };
    let fitting_frames = u64::from(room_count).min(room_bytes / frame_bytes.max(1));
    let fitting_seconds =
        (fitting_frames as f64 / fps).min(video_seconds.unwrap_or(MAX_VIDEO_SECONDS));
    let document_rate = document.frame_rate();
    let mut plan = Plan {
        source: *source,
        settings: *settings,
        size,
        frames,
        seconds,
        whole,
        bytes,
        document_rate,
        length_seconds: f64::from(frames) / document_rate.per_second(),
        speed: document_rate.per_second() / fps,
        too_large,
        fitting_seconds,
        fitting_side: None,
    };
    plan.fitting_side = fitting_side(&plan, room_count, room_bytes);
    Ok(plan)
}

/// Pictures in `seconds` of video taken `fps` times a second: at least one.
fn frames_in(seconds: f64, fps: f64) -> u32 {
    let frames = (seconds * fps).round();
    if frames < 1.0 {
        1
    } else if frames >= f64::from(u32::MAX) {
        u32::MAX
    } else {
        frames as u32
    }
}

/// The source's size, scaled down so its longest side is `max_side` at most, never up.
pub fn picture_size(source: &Source, max_side: u32) -> (u32, u32) {
    let longest = source.width.max(source.height);
    if longest <= max_side {
        return (source.width, source.height);
    }
    let scale = f64::from(max_side) / f64::from(longest);
    let side = |n: u32| ((f64::from(n) * scale).round() as u32).clamp(1, max_side);
    (side(source.width), side(source.height))
}

/// Pictures and bytes of pictures the document can still take.
fn room(document: &Document) -> (u32, u64) {
    let held: u64 = document.bitmaps().iter().map(|b| b.byte_len()).sum();
    let count = MAX_BITMAPS.saturating_sub(document.bitmaps().len());
    (
        u32::try_from(count).unwrap_or(u32::MAX),
        MAX_TOTAL_BITMAP_BYTES.saturating_sub(held),
    )
}

/// The largest longest side, up to the one asked for, at which the plan's frames fit.
fn fitting_side(plan: &Plan, room_count: u32, room_bytes: u64) -> Option<u32> {
    if plan.frames > room_count {
        return None;
    }
    let fits = |side: u32| {
        let (w, h) = picture_size(&plan.source, side);
        u64::from(w) * u64::from(h) * 4 * u64::from(plan.frames) <= room_bytes
    };
    if plan.frame_bytes() * u64::from(plan.frames) <= room_bytes {
        return Some(plan.settings.max_side);
    }
    let (mut lo, mut hi) = (MIN_VIDEO_SIDE, plan.settings.max_side);
    if !fits(lo) {
        return None;
    }
    // The bytes grow with the side, so the last side that fits is found by halving.
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if fits(mid) {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    Some(lo)
}

#[cfg(test)]
mod tests;
