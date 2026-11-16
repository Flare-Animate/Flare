//! Lip sync: which mouth shape each frame of a voice shows.
//!
//! The shapes are the ten of the classic animation chart (Preston Blair's, the set the archived
//! Flare used). A sound is read in one of two ways: [`analyze`], built in and always there, which
//! looks at loudness, zero crossings and four frequency bands of each frame (a heuristic that
//! blocks in the timing, not speech recognition), or the cues of Rhubarb Lip Sync
//! (github.com/DanielSWolf/rhubarb-lip-sync, MIT), a speech recognizer run as a program of its
//! own when it is installed, whose output [`parse_rhubarb`] reads. Running it is not done here:
//! this crate opens no process (see `flare-runtime`).

mod analyze;
mod fft;
mod rhubarb;

pub use analyze::analyze;
pub use rhubarb::{MAX_RHUBARB_OUTPUT_BYTES, parse_rhubarb, rhubarb_shape};

use thiserror::Error;

/// Most frames one lip sync covers: well over ten minutes, the longest sound, at 120 fps.
pub const MAX_LIP_SYNC_FRAMES: usize = 100_000;
/// Most cues read from an analysis tool's output.
pub const MAX_MOUTH_CUES: usize = 100_000;

/// A mouth shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Viseme {
    /// The mouth at rest, closed and relaxed: silence.
    Neutral,
    /// Lips pressed together: M, B and P.
    Mbp,
    /// Wide open: A and I.
    Ai,
    /// Open and spread: E.
    E,
    /// Rounded and open: O.
    O,
    /// Rounded and small: U and OO.
    U,
    /// Lower lip under the teeth: F and V.
    Fv,
    /// Tongue behind the teeth: L and TH.
    L,
    /// Puckered: W and Q.
    Wq,
    /// Slightly open with the teeth showing: most other consonants (C, D, G, K, N, R, S, T…).
    Etc,
}

impl Viseme {
    /// Every shape, in the order a mapping lists them.
    pub const ALL: [Self; 10] = [
        Self::Neutral,
        Self::Mbp,
        Self::Ai,
        Self::E,
        Self::O,
        Self::U,
        Self::Fv,
        Self::L,
        Self::Wq,
        Self::Etc,
    ];

    /// The name the command line, the MCP server and a mapping use.
    pub fn name(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Mbp => "mbp",
            Self::Ai => "ai",
            Self::E => "e",
            Self::O => "o",
            Self::U => "u",
            Self::Fv => "fv",
            Self::L => "l",
            Self::Wq => "wq",
            Self::Etc => "etc",
        }
    }

    /// The shape a name means, in any case and with spaces round it.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        Self::ALL
            .into_iter()
            .find(|v| v.name().eq_ignore_ascii_case(text))
    }

    /// Its place in [`Viseme::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }
}

/// A shape held from `start` to `end`, in seconds from the start of the sound.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouthCue {
    pub start: f64,
    pub end: f64,
    pub viseme: Viseme,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LipSyncError {
    #[error("the frame rate must be a positive number")]
    BadRate,
    #[error("a lip sync covers at most {MAX_LIP_SYNC_FRAMES} frames")]
    TooLong,
    #[error("Rhubarb Lip Sync's output could not be read: {0}")]
    Rhubarb(String),
}

/// The shape each of `frames` frames shows, frame `i` being the cue under `i / per_second`
/// seconds; a frame no cue covers is [`Viseme::Neutral`].
pub fn frames_from_cues(
    cues: &[MouthCue],
    per_second: f64,
    frames: usize,
) -> Result<Vec<Viseme>, LipSyncError> {
    if !(per_second.is_finite() && per_second > 0.0) {
        return Err(LipSyncError::BadRate);
    }
    if frames > MAX_LIP_SYNC_FRAMES {
        return Err(LipSyncError::TooLong);
    }
    let mut sorted = cues.to_vec();
    sorted.sort_by(|a, b| a.start.total_cmp(&b.start));
    // Cues come in order, so one walk along them finds every frame's.
    let mut next = 0;
    Ok((0..frames)
        .map(|frame| {
            let t = frame as f64 / per_second;
            while next < sorted.len() && sorted[next].end <= t {
                next += 1;
            }
            sorted
                .get(next)
                .filter(|cue| cue.start <= t)
                .map_or(Viseme::Neutral, |cue| cue.viseme)
        })
        .collect())
}

/// How many frames at `per_second` a sound of `seconds` covers, rounded up.
pub fn frames_for(seconds: f64, per_second: f64) -> Result<usize, LipSyncError> {
    if !(per_second.is_finite() && per_second > 0.0) {
        return Err(LipSyncError::BadRate);
    }
    let frames = (seconds.max(0.0) * per_second).ceil();
    if !frames.is_finite() || frames > MAX_LIP_SYNC_FRAMES as f64 {
        return Err(LipSyncError::TooLong);
    }
    Ok(frames as usize)
}

#[cfg(test)]
mod tests;
