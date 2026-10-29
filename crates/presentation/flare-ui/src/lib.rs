//! The modern Animate-style workspace, built on egui.
//!
//! This crate owns presentation state and interaction only. It has no windowing or GPU
//! code (the desktop shell supplies that) and mutates the document exclusively through
//! `flare-edit` commands, so UI gestures and the CLI share the same semantics.
//! A drag-to-draw gesture is tool state until it is released; Escape cancels it.
#![forbid(unsafe_code)]

mod actions;
mod ops;
mod view;
mod workspace;

pub use actions::{MENU_LAYOUT, MenuAction, MenuEntry};
pub use workspace::{DialogRequest, FrameOp, Tool, Workspace};
