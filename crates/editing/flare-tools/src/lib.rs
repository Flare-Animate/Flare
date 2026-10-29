//! What the drawing and selection tools compute, with no window in sight: handle geometry for
//! scaling, rotating and skewing, the pen's path building, freehand smoothing, and the
//! selection operations that turn a gesture into commands. Everything here is a pure function
//! of a document and numbers, so it is tested without a UI.
#![forbid(unsafe_code)]

pub mod brush;
pub mod freehand;
pub mod handles;
pub mod ops;
pub mod pen;
pub mod rulers;
pub mod symmetry;
