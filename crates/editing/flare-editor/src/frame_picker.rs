//! Picking a frame to go to: the cells a grid of frames shows, and where a request such as "the
//! next keyframe" or "frame 12" lands. The window's frame picker, `flare goto` and the MCP
//! server's `goto_frame` all ask here, so they agree on what a keyframe is and where the ends are.

use std::fmt;
use std::str::FromStr;

use flare_document::{Content, Timeline};

/// The most cells one call describes, so a very long timeline cannot make a huge answer.
pub const MAX_CELLS: u32 = 2_000;

/// One frame as the picker shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameCell {
    /// Zero-based.
    pub frame: u32,
    /// How many layers start a keyframe here.
    pub keyframes: usize,
    /// Whether any layer shows art (a drawing, an instance or a picture) at this frame, held or
    /// not.
    pub shows_art: bool,
}

/// The cells of frames `first` up to `count` of them (at most [`MAX_CELLS`]), cut short at the
/// timeline's end.
pub fn cells(timeline: &Timeline, first: u32, count: u32) -> Vec<FrameCell> {
    let end = timeline
        .frame_count()
        .min(u64::from(first) + u64::from(count.min(MAX_CELLS)));
    let first = u64::from(first);
    if end <= first {
        return Vec::new();
    }
    let len = usize::try_from(end - first).unwrap_or(0);
    let mut keyframes = vec![0_usize; len];
    let mut shows_art = vec![false; len];
    let index = |frame: u64| {
        usize::try_from(frame.checked_sub(first)?)
            .ok()
            .filter(|i| *i < len)
    };
    // One pass over each layer's spans that touch the window, not a search per cell: a big
    // document with a long window would otherwise cost layers x spans x cells.
    for layer in timeline.layers() {
        let spans = layer.spans();
        let from = spans.partition_point(|s| s.end() <= first);
        for span in spans[from..]
            .iter()
            .take_while(|s| u64::from(s.start()) < end)
        {
            if let Some(i) = index(u64::from(span.start())) {
                keyframes[i] += 1;
            }
            if span.elements().any(is_art) {
                for frame in u64::from(span.start()).max(first)..span.end().min(end) {
                    if let Some(i) = index(frame) {
                        shows_art[i] = true;
                    }
                }
            }
        }
        // Past its last keyframe a layer that holds its drawing still shows it.
        if let Some(last) = spans.last() {
            let held_from = last.end().max(first);
            if held_from < end
                && let Some(span) = u32::try_from(held_from).ok().and_then(|f| layer.span_at(f))
                && span.elements().any(is_art)
            {
                for frame in held_from..end {
                    if let Some(i) = index(frame) {
                        shows_art[i] = true;
                    }
                }
            }
        }
    }
    (first..end)
        .zip(keyframes.into_iter().zip(shows_art))
        .filter_map(|(frame, (keyframes, shows_art))| {
            Some(FrameCell {
                frame: u32::try_from(frame).ok()?,
                keyframes,
                shows_art,
            })
        })
        .collect()
}

fn is_art(content: &Content) -> bool {
    matches!(
        content,
        Content::Shape(_) | Content::Instance(_) | Content::Bitmap(_)
    )
}

/// Where to go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Goto {
    /// A frame number, zero-based.
    Frame(u32),
    First,
    Last,
    Next,
    Previous,
    /// The next frame after this one where any layer starts a keyframe.
    NextKeyframe,
    PreviousKeyframe,
}

/// Why a request was not understood or cannot be met.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GotoError {
    /// The text is no frame number or one of the words.
    Unknown(String),
    /// The scene has no frames at all.
    NoFrames,
    /// The frame is after the scene's last (`frames` long).
    PastTheEnd { frame: u32, frames: u64 },
    /// There is no keyframe in that direction, or no frame (a first or last step off the end).
    NothingThere,
}

impl fmt::Display for GotoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unknown(text) => write!(
                f,
                "{text:?} is no frame: give a zero-based number, or first, last, next, previous, next-keyframe or previous-keyframe"
            ),
            Self::NoFrames => f.write_str("the scene has no frames"),
            Self::PastTheEnd { frame, frames } => {
                write!(
                    f,
                    "frame {frame} is past the end: the scene has {frames} frames"
                )
            }
            Self::NothingThere => f.write_str("there is no frame or keyframe that way"),
        }
    }
}

impl std::error::Error for GotoError {}

impl FromStr for Goto {
    type Err = GotoError;

    fn from_str(text: &str) -> Result<Self, GotoError> {
        let word = text.trim().to_ascii_lowercase();
        Ok(match word.as_str() {
            "first" | "start" => Self::First,
            "last" | "end" => Self::Last,
            "next" => Self::Next,
            "previous" | "prev" => Self::Previous,
            "next-keyframe" => Self::NextKeyframe,
            "previous-keyframe" | "prev-keyframe" => Self::PreviousKeyframe,
            other => Self::Frame(
                other
                    .parse()
                    .map_err(|_| GotoError::Unknown(text.to_owned()))?,
            ),
        })
    }
}

/// The frame `goto` leads to from `current`, which has to exist in the scene.
pub fn resolve(timeline: &Timeline, current: u32, goto: Goto) -> Result<u32, GotoError> {
    let frames = timeline.frame_count();
    let last = u32::try_from(frames.checked_sub(1).ok_or(GotoError::NoFrames)?)
        .map_err(|_| GotoError::NoFrames)?;
    let starts_keyframe = |frame: u32| {
        timeline
            .layers()
            .iter()
            .any(|l| l.spans().iter().any(|s| s.start() == frame))
    };
    match goto {
        Goto::Frame(frame) if frame <= last => Ok(frame),
        Goto::Frame(frame) => Err(GotoError::PastTheEnd { frame, frames }),
        Goto::First => Ok(0),
        Goto::Last => Ok(last),
        Goto::Next => current
            .checked_add(1)
            .filter(|f| *f <= last)
            .ok_or(GotoError::NothingThere),
        // From a frame past the end the step back lands on the last frame, not on another one
        // past it.
        Goto::Previous => current
            .checked_sub(1)
            .map(|f| f.min(last))
            .ok_or(GotoError::NothingThere),
        Goto::NextKeyframe => (current.saturating_add(1)..=last)
            .find(|f| starts_keyframe(*f))
            .ok_or(GotoError::NothingThere),
        Goto::PreviousKeyframe => (0..current)
            .rev()
            .find(|f| starts_keyframe(*f))
            .ok_or(GotoError::NothingThere),
    }
}

#[cfg(test)]
mod tests;
