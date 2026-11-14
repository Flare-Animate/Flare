//! What the drawing and selection tools compute, with no window in sight. Each tool has a folder
//! of its own with its code and its tests: the Pen and its path building, the Pencil's smoothing,
//! the Brush and its nib, the Free Transform handles, the Selection and Lasso picking, the
//! Subselection, Add, Delete and Convert Anchor Point edits, the Ruler and the Mirror, the Magic
//! Wand's picking and clearing of bitmap pixels, Blur and Smudge on those pixels, and the operations of the Paint Bucket and Ink Bottle, the Eraser, Text, grouping, the clipboard,
//! alignment, tweens and Graphic playback, placing the reference mannequin, and a video brought in
//! to draw over. What several of them share is in `common`.
//!
//! Everything here is a pure function of a document and numbers, so it is tested without a UI.
//! [`ops`] gathers the operations that turn a gesture into commands under one name.
#![forbid(unsafe_code)]

pub mod add_anchor;
pub mod arrange;
pub mod blend;
pub mod bones;
pub mod brush;
pub mod clipboard;
mod common;
pub mod convert_anchor;
pub mod delete_anchor;
pub mod drawing;
#[cfg(test)]
mod elements_tests;
pub mod eraser;
pub mod free_transform;
pub mod game;
pub mod grouping;
pub mod lasso;
pub mod library;
pub mod line;
pub mod magic_wand;
pub mod mannequin;
pub mod mirror;
pub mod multi_frame;
pub mod not_yet;
pub mod ops;
pub mod paint;
pub mod pen;
pub mod pencil;
pub mod playback;
pub mod ruler;
pub mod selection;
pub mod stacking;
pub mod subselection;
#[cfg(test)]
mod test_support;
pub mod text;
pub mod tween;
pub mod video;
