//! The stop-motion domain as data: a capture session with its frames, frame rate, hold count
//! and onion skin, the ghost frames shown while drawing, and the timeline the captured frames
//! export to. No camera is opened here. Capture and live view go through [`CameraSource`],
//! and the only source built today, [`NoCamera`], answers with a named error.
//!
//! Everything is a pure function of the session and numbers, so it is tested without a
//! window or a device. A session keeps frame numbers and holds, not pixels: the caller stores
//! the captured images, and the export says which frame shows for how long.
#![forbid(unsafe_code)]

mod camera;
mod error;
mod export;
mod onion;
mod session;

pub use camera::{
    CameraError, CameraSource, MAX_CAPTURE_BYTES, NOT_YET_CAPTURE, NOT_YET_LIVE_VIEW, NoCamera,
    accept_capture,
};
pub use error::StopMotionError;
pub use export::{TimelineSlot, timeline_slots};
pub use onion::{MAX_ONION_FRAMES, MAX_ONION_OPACITY, OnionSkin, ghost_frames};
pub use session::{
    Frame, FrameId, MAX_FPS, MAX_FRAMES, MAX_HOLD, MAX_TIMELINE_FRAMES, StopMotionSession,
};
