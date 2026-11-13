//! Editing operations built on commands: frame and keyframe operations, and in-place symbol
//! editing support. Each turns what the user did into the commands that do it.
#![forbid(unsafe_code)]

mod camera;
mod context;
pub mod frames;
pub mod loop_range;
pub mod timeline_view;

pub use camera::{MAX_CAMERA_FRAME, camera_now, set_camera_key, set_camera_key_named};
pub use context::symbol_workspace;
