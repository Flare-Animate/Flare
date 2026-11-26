//! A stop-motion session kept in the project: the rate and hold new frames get, and the captured
//! pictures in order, each a library bitmap shown for a number of timeline frames. The pictures
//! themselves are ordinary bitmaps (stored once, whatever shows them); the session only names
//! them. `flare_editor::stop_motion_library` makes every change.

use flare_core::BitmapId;

/// Highest capture rate. Above this a camera cannot keep up and a player stutters.
pub const MAX_STOP_MOTION_FPS: u32 = 120;
/// Most timeline frames one captured picture may be held for.
pub const MAX_STOP_MOTION_HOLD: u32 = 60;
/// Most pictures in one session.
pub const MAX_STOP_MOTION_FRAMES: usize = 10_000;

/// Why a session could not be made or changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum StopMotionError {
    #[error("the capture rate must be 1 to {MAX_STOP_MOTION_FPS} frames a second")]
    Rate,
    #[error("a hold must be 1 to {MAX_STOP_MOTION_HOLD} timeline frames")]
    Hold,
    #[error("a session holds at most {MAX_STOP_MOTION_FRAMES} frames")]
    TooManyFrames,
}

/// One captured picture and how many timeline frames it shows for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopFrame {
    bitmap: BitmapId,
    hold: u32,
}

impl StopFrame {
    pub fn new(bitmap: BitmapId, hold: u32) -> Result<Self, StopMotionError> {
        check_hold(hold)?;
        Ok(Self { bitmap, hold })
    }

    pub fn bitmap(&self) -> BitmapId {
        self.bitmap
    }

    pub fn hold(&self) -> u32 {
        self.hold
    }
}

/// The session: rate, the hold a new frame gets, and the frames in capture order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopMotion {
    fps: u32,
    default_hold: u32,
    frames: Vec<StopFrame>,
}

fn check_hold(hold: u32) -> Result<(), StopMotionError> {
    if (1..=MAX_STOP_MOTION_HOLD).contains(&hold) {
        Ok(())
    } else {
        Err(StopMotionError::Hold)
    }
}

impl StopMotion {
    pub fn new(fps: u32, default_hold: u32) -> Result<Self, StopMotionError> {
        if !(1..=MAX_STOP_MOTION_FPS).contains(&fps) {
            return Err(StopMotionError::Rate);
        }
        check_hold(default_hold)?;
        Ok(Self {
            fps,
            default_hold,
            frames: Vec::new(),
        })
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn default_hold(&self) -> u32 {
        self.default_hold
    }

    pub fn frames(&self) -> &[StopFrame] {
        &self.frames
    }

    /// Timeline frames the whole session covers.
    pub fn duration(&self) -> u64 {
        self.frames.iter().map(|f| u64::from(f.hold)).sum()
    }

    pub fn with_rate(mut self, fps: u32, default_hold: u32) -> Result<Self, StopMotionError> {
        let checked = Self::new(fps, default_hold)?;
        self.fps = checked.fps;
        self.default_hold = checked.default_hold;
        Ok(self)
    }

    pub fn with_frames(mut self, frames: Vec<StopFrame>) -> Result<Self, StopMotionError> {
        if frames.len() > MAX_STOP_MOTION_FRAMES {
            return Err(StopMotionError::TooManyFrames);
        }
        self.frames = frames;
        Ok(self)
    }
}
