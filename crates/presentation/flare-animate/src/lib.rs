//! The modern Animate-style workspace, built on egui.
//!
//! This crate owns presentation state and interaction only. It has no windowing or GPU
//! code (the desktop shell supplies that) and mutates the document exclusively through
//! `flare-commands` commands, so UI gestures and the CLI share the same semantics.
//! A drag-to-draw gesture is tool state until it is released; Escape cancels it.
#![forbid(unsafe_code)]

#[cfg(test)]
mod icon_names;
mod workspace;

pub use flare_dock::{LayoutError, MAX_LAYOUT_BYTES, Pane};
pub use flare_input::PenFeed;
pub use workspace::{
    AudioEvent, DialogRequest, FrameOp, MAX_ONION_FRAMES, MAX_PREFERENCES_BYTES, OnionSkin,
    Preferences, PublishSettings, ShareTarget, Tool, WindowRequest, Workspace, onion_ghosts,
};
