//! Shared foundation for all Flare crates: validated types and common utilities.
//!
//! Keep this crate free of GUI, GPU, database, async and I/O dependencies. Domain
//! logic (document model, commands, rendering) belongs in its own crate.
#![forbid(unsafe_code)]

pub mod geometry;
pub mod id;
pub mod time;

pub use geometry::{DocumentPoint, NonFiniteCoordinate, Transform2D};
pub use id::{DocumentId, InstanceId, LayerId, SceneId, SymbolId};
pub use time::{FrameRate, RateError};
