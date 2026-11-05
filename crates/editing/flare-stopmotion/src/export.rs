use crate::error::StopMotionError;
use crate::session::{FrameId, MAX_TIMELINE_FRAMES, StopMotionSession};

/// One captured frame on the timeline: it shows from `start` for `length` timeline frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimelineSlot {
    frame: FrameId,
    start: u64,
    length: u32,
}

impl TimelineSlot {
    pub fn frame(&self) -> FrameId {
        self.frame
    }

    pub fn start(&self) -> u64 {
        self.start
    }

    pub fn length(&self) -> u32 {
        self.length
    }
}

/// Lays the session's frames end to end, each for its hold, in capture order. The caller
/// places them on a timeline (a keyframe per slot); this does not change any document.
pub fn timeline_slots(session: &StopMotionSession) -> Result<Vec<TimelineSlot>, StopMotionError> {
    if session.frames().is_empty() {
        return Err(StopMotionError::NoFrames);
    }
    let total = session.duration();
    if total > MAX_TIMELINE_FRAMES {
        return Err(StopMotionError::TimelineTooLong {
            frames: total,
            max: MAX_TIMELINE_FRAMES,
        });
    }
    let mut start = 0_u64;
    let slots = session
        .frames()
        .iter()
        .map(|frame| {
            let slot = TimelineSlot {
                frame: frame.id(),
                start,
                length: frame.hold(),
            };
            start += u64::from(frame.hold());
            slot
        })
        .collect();
    Ok(slots)
}
