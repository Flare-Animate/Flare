//! Shared foundation for all Flare crates: validated types and common utilities.
//!
//! Keep this crate free of GUI, GPU, database, async and I/O dependencies. Domain
//! logic (document model, commands, rendering) belongs in its own crate.
#![forbid(unsafe_code)]

pub mod color;
pub mod convolve;
pub mod curves;
pub mod fx_params;
pub mod geometry;
pub mod id;
pub mod image_ops;
pub mod resample;
pub mod time;

pub use color::Rgba8;
pub use geometry::{DocumentPoint, NonFiniteCoordinate, Transform2D};
pub use id::{BitmapId, DocumentId, InstanceId, LayerId, SceneId, ShapeId, SoundId, SymbolId};
pub use time::{FrameRate, RateError};
