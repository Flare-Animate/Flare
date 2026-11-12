//! Which part of a sound to play: all of it, from a point in, or a short slice (a scrub).

use std::ops::Range;

/// A stretch of a sound, in seconds so that it means the same at any sample rate (a sound may be
/// resampled for the device between being asked for and being played).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Excerpt {
    /// How far into the sound to start.
    pub offset_seconds: f64,
    /// How long to play; to the end of the sound when `None`.
    pub length_seconds: Option<f64>,
}

impl Excerpt {
    /// The whole sound.
    pub const WHOLE: Self = Self {
        offset_seconds: 0.0,
        length_seconds: None,
    };

    /// From `offset_seconds` in to the end.
    pub fn from(offset_seconds: f64) -> Self {
        Self {
            offset_seconds,
            length_seconds: None,
        }
    }

    /// `length_seconds` of the sound, starting `offset_seconds` in.
    pub fn slice(offset_seconds: f64, length_seconds: f64) -> Self {
        Self {
            offset_seconds,
            length_seconds: Some(length_seconds),
        }
    }

    /// The frames this covers of a sound `frames` long at `rate`. Empty when it starts at or past
    /// the end; negative or non-finite values are taken as nothing, an infinite length as all.
    pub fn frames(self, rate: u32, frames: usize) -> Range<usize> {
        let to_frames = |seconds: f64| -> usize {
            if seconds.is_nan() || seconds <= 0.0 {
                0
            } else {
                // `as` saturates, so an infinite or huge value is simply the whole sound.
                ((seconds * f64::from(rate)).floor() as usize).min(frames)
            }
        };
        let start = to_frames(self.offset_seconds);
        let end = self
            .length_seconds
            .map_or(frames, |length| start.saturating_add(to_frames(length)))
            .min(frames);
        start..end.max(start)
    }
}
