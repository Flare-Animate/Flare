//! The modern Animate-style workspace, built on egui.
//!
//! This crate owns presentation state and interaction only. It has no windowing or GPU
//! code (the desktop shell supplies that) and mutates the document exclusively through
//! `flare-commands` commands, so UI gestures and the CLI share the same semantics.
//! A drag-to-draw gesture is tool state until it is released; Escape cancels it.
#![forbid(unsafe_code)]

mod actions;
mod chrome;
mod export_image;
mod icons;
mod layout;
mod theme;
mod timeline_view;
mod view;
mod workspace;

pub use actions::{MENU_LAYOUT, MenuAction, MenuEntry};
pub use export_image::export_png;
pub use layout::{LayoutError, MAX_LAYOUT_BYTES, Pane};
pub use workspace::{
    AudioEvent, DialogRequest, FrameOp, MAX_ONION_FRAMES, OnionSkin, Tool, WindowRequest,
    Workspace, onion_ghosts,
};
