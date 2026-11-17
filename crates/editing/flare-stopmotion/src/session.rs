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

impl StopMotionSession {
    /// Inserts a captured frame at `position` (shifting the frames after it right), with the
    /// default hold, and returns its id.
    pub fn insert_frame(&mut self, position: usize) -> Result<FrameId, StopMotionError> {
        if self.frames.len() >= MAX_FRAMES {
            return Err(StopMotionError::TooManyFrames { max: MAX_FRAMES });
        }
        if position > self.frames.len() {
            return Err(StopMotionError::IndexOutOfRange {
                index: position,
                len: self.frames.len(),
            });
        }
        let id = FrameId(self.next_id);
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(StopMotionError::IdsExhausted)?;
        self.frames.insert(
            position,
            Frame {
                id,
                hold: self.default_hold,
            },
        );
        Ok(id)
    }

    /// Duplicates the frame at `id` right after it: the copy gets a new id and the same
    /// hold, and returns the copy's id.
    pub fn duplicate_frame(&mut self, id: FrameId) -> Result<FrameId, StopMotionError> {
        let index = self
            .frames
            .iter()
            .position(|frame| frame.id == id)
            .ok_or(StopMotionError::UnknownFrame(id))?;
        if self.frames.len() >= MAX_FRAMES {
            return Err(StopMotionError::TooManyFrames { max: MAX_FRAMES });
        }
        let copy = Frame {
            id: FrameId(self.next_id),
            hold: self.frames[index].hold,
        };
        self.next_id = self
            .next_id
            .checked_add(1)
            .ok_or(StopMotionError::IdsExhausted)?;
        let copy_id = copy.id;
        self.frames.insert(index + 1, copy);
        Ok(copy_id)
    }

    /// Moves the frame at `id` to `position` in the list the move produces (the frame is
    /// taken out first, then put back). Returns the position it came from.
    pub fn move_frame(&mut self, id: FrameId, position: usize) -> Result<usize, StopMotionError> {
        let from = self
            .frames
            .iter()
            .position(|frame| frame.id == id)
            .ok_or(StopMotionError::UnknownFrame(id))?;
        if position >= self.frames.len() {
            return Err(StopMotionError::IndexOutOfRange {
                index: position,
                len: self.frames.len(),
            });
        }
        let frame = self.frames.remove(from);
        self.frames.insert(position, frame);
        Ok(from)
    }

    /// Removes every frame. Ids stay spent: a later capture never reuses one.
    pub fn clear(&mut self) -> Vec<Frame> {
        std::mem::take(&mut self.frames)
    }

    /// The position of the frame at `id`, or an error when no such frame is in the session.
    pub fn position_of(&self, id: FrameId) -> Result<usize, StopMotionError> {
        self.frames
            .iter()
            .position(|frame| frame.id == id)
            .ok_or(StopMotionError::UnknownFrame(id))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod ops_tests {
    use super::*;

    fn session_with(n: u32) -> StopMotionSession {
        let mut session = StopMotionSession::new(12, 1).unwrap();
        for _ in 0..n {
            session.add_frame().unwrap();
        }
        session
    }

    #[test]
    fn an_insert_lands_at_the_position_with_the_default_hold() {
        let mut session = session_with(2);
        let id = session.insert_frame(1).unwrap();
        let positions: Vec<u32> = session.frames().iter().map(|f| f.id().get()).collect();
        assert_eq!(positions, [0, 2, 1]);
        assert_eq!(session.position_of(id).unwrap(), 1);
        assert_eq!(session.frames()[2].hold(), session.default_hold());
    }

    #[test]
    fn an_insert_past_the_end_is_refused_and_changes_nothing() {
        let mut session = session_with(2);
        assert_eq!(
            session.insert_frame(3),
            Err(StopMotionError::IndexOutOfRange { index: 3, len: 2 })
        );
        assert_eq!(session.frames().len(), 2);
        // The end itself is allowed: appending through insert.
        assert!(session.insert_frame(2).is_ok());
    }

    #[test]
    fn a_duplicate_copies_the_hold_and_follows_the_original() {
        let mut session = session_with(2);
        let original = session.frames()[0].id();
        session.set_hold(original, 5).unwrap();
        let copy = session.duplicate_frame(original).unwrap();
        assert_ne!(copy, original);
        assert_eq!(session.position_of(copy).unwrap(), 1);
        assert_eq!(session.frames()[1].hold(), 5);
        assert_eq!(session.frames()[0].hold(), 5);
    }

    #[test]
    fn duplicating_an_unknown_frame_is_refused() {
        let mut session = session_with(1);
        assert_eq!(
            session.duplicate_frame(FrameId(99)),
            Err(StopMotionError::UnknownFrame(FrameId(99)))
        );
    }

    #[test]
    fn a_move_splices_and_reports_where_it_came_from() {
        let mut session = session_with(3);
        let middle = session.frames()[1].id();
        let from = session.move_frame(middle, 0).unwrap();
        assert_eq!(from, 1);
        let order: Vec<u32> = session.frames().iter().map(|f| f.id().get()).collect();
        assert_eq!(order, [1, 0, 2]);
        // The last position of the list the move produces is the end.
        session.move_frame(middle, 2).unwrap();
        let order: Vec<u32> = session.frames().iter().map(|f| f.id().get()).collect();
        assert_eq!(order, [0, 2, 1]);
    }

    #[test]
    fn a_move_past_the_end_is_refused_and_changes_nothing() {
        let mut session = session_with(2);
        let first = session.frames()[0].id();
        assert_eq!(
            session.move_frame(first, 2),
            Err(StopMotionError::IndexOutOfRange { index: 2, len: 2 })
        );
        let order: Vec<u32> = session.frames().iter().map(|f| f.id().get()).collect();
        assert_eq!(order, [0, 1]);
    }

    #[test]
    fn clearing_empties_but_never_reuses_ids() {
        let mut session = session_with(2);
        let cleared = session.clear();
        assert_eq!(cleared.len(), 2);
        assert!(session.frames().is_empty());
        assert_eq!(session.duration(), 0);
        let next = session.add_frame().unwrap();
        assert_ne!(next.get(), cleared[0].id().get());
        assert_ne!(next.get(), cleared[1].id().get());
    }
}
