use crate::error::StopMotionError;
use crate::onion::OnionSkin;

/// Highest capture rate. Above this a camera cannot keep up and a player stutters.
pub const MAX_FPS: u32 = 120;
/// Most timeline frames one captured frame may be held for.
pub const MAX_HOLD: u32 = 60;
/// Most captured frames in one session.
pub const MAX_FRAMES: usize = 10_000;
/// Longest timeline a session may export to.
pub const MAX_TIMELINE_FRAMES: u64 = 100_000;

/// Names a captured frame. Ids are never reused within a session, so a removed frame's id
/// cannot silently point at a later one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameId(u32);

impl FrameId {
    pub fn get(self) -> u32 {
        self.0
    }
}

/// One captured frame and how many timeline frames it shows for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    id: FrameId,
    hold: u32,
}

impl Frame {
    pub fn id(&self) -> FrameId {
        self.id
    }

    pub fn hold(&self) -> u32 {
        self.hold
    }
}

/// A capture session: the rate, the hold each new frame gets, the onion skin, and the frames
/// in capture order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopMotionSession {
    fps: u32,
    default_hold: u32,
    onion: OnionSkin,
    frames: Vec<Frame>,
    next_id: u32,
}

impl StopMotionSession {
    pub fn new(fps: u32, default_hold: u32) -> Result<Self, StopMotionError> {
        Ok(Self {
            fps: check_fps(fps)?,
            default_hold: check_hold(default_hold)?,
            onion: OnionSkin::default(),
            frames: Vec::new(),
            next_id: 0,
        })
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn default_hold(&self) -> u32 {
        self.default_hold
    }

    /// Sets the hold that frames captured from now on get. Frames already captured keep theirs.
    pub fn set_default_hold(&mut self, hold: u32) -> Result<(), StopMotionError> {
        self.default_hold = check_hold(hold)?;
        Ok(())
    }

    pub fn onion(&self) -> OnionSkin {
        self.onion
    }

    pub fn set_onion(&mut self, onion: OnionSkin) {
        self.onion = onion;
    }

    pub fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Adds a captured frame at the end, with the default hold, and returns its id.
    pub fn add_frame(&mut self) -> Result<FrameId, StopMotionError> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(StopMotionError::TooManyFrames { max: MAX_FRAMES });
        }
        let id = FrameId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(StopMotionError::IdsExhausted)?;
        self.frames.push(Frame {
            id,
            hold: self.default_hold,
        });
        Ok(id)
    }

    pub fn set_hold(&mut self, id: FrameId, hold: u32) -> Result<(), StopMotionError> {
        let hold = check_hold(hold)?;
        let frame = self
            .frames
            .iter_mut()
            .find(|frame| frame.id == id)
            .ok_or(StopMotionError::UnknownFrame(id))?;
        frame.hold = hold;
        Ok(())
    }

    /// Removes a frame and returns it. The frames after it keep their ids and holds.
    pub fn remove_frame(&mut self, id: FrameId) -> Result<Frame, StopMotionError> {
        let index = self
            .frames
            .iter()
            .position(|frame| frame.id == id)
            .ok_or(StopMotionError::UnknownFrame(id))?;
        Ok(self.frames.remove(index))
    }

    /// Timeline frames the whole session covers: the sum of the holds.
    pub fn duration(&self) -> u64 {
        self.frames.iter().map(|frame| u64::from(frame.hold)).sum()
    }
}

fn check_fps(fps: u32) -> Result<u32, StopMotionError> {
    if (1..=MAX_FPS).contains(&fps) {
        Ok(fps)
    } else {
        Err(StopMotionError::FrameRate {
            got: fps,
            max: MAX_FPS,
        })
    }
}

fn check_hold(hold: u32) -> Result<u32, StopMotionError> {
    if (1..=MAX_HOLD).contains(&hold) {
        Ok(hold)
    } else {
        Err(StopMotionError::Hold {
            got: hold,
            max: MAX_HOLD,
        })
    }
}
