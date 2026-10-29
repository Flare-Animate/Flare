//! Shared foundation for all Flare crates: validated types and common utilities.
//!
//! Keep this crate free of GUI, GPU, database, async and I/O dependencies. Domain
//! logic (document model, commands, rendering) belongs in its own crate.
#![forbid(unsafe_code)]

pub mod color;
pub mod geometry;
pub mod id;
pub mod time;

pub use color::Rgba8;
pub use geometry::{DocumentPoint, NonFiniteCoordinate, Transform2D};
pub use id::{DocumentId, InstanceId, LayerId, SceneId, ShapeId, SymbolId};
pub use time::{FrameRate, RateError};
