use crate::session::FrameId;

/// Why a stop-motion session refused a change. A refused change leaves the session as it was.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum StopMotionError {
    #[error("frame rate must be 1 to {max} frames per second, not {got}")]
    FrameRate { got: u32, max: u32 },
    #[error("a frame is held for 1 to {max} timeline frames, not {got}")]
    Hold { got: u32, max: u32 },
    #[error("a session holds at most {max} frames")]
    TooManyFrames { max: usize },
    #[error("no frame has id {}", .0.get())]
    UnknownFrame(FrameId),
    #[error("frame ids are used up")]
    IdsExhausted,
    #[error("onion skin shows 0 to {max} frames each way, not {got}")]
    OnionFrames { got: u8, max: u8 },
    #[error("onion skin opacity is 0 to {max} percent, not {got}")]
    OnionOpacity { got: u8, max: u8 },
    #[error("frame position {index} is past the {len} frames of the session")]
    IndexOutOfRange { index: usize, len: usize },
    #[error("the session has no frames to export")]
    NoFrames,
    #[error("the timeline would need {frames} frames; the limit is {max}")]
    TimelineTooLong { frames: u64, max: u64 },
}
