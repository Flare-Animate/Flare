//! Native `.flare` container: a SQLite file holding the saved document.
//!
//! Saving stages a complete database next to the target and atomically renames it into
//! place, so a failed save never damages the previous file. Loading treats the file as
//! untrusted: defensive SQLite settings, size limits, and full document revalidation
//! through the normal constructors. The on-disk schema is a draft, not a stable format.
#![forbid(unsafe_code)]

mod recovery;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;
#[cfg(target_arch = "wasm32")]
mod unsupported;
#[cfg(not(target_arch = "wasm32"))]
mod wire;

use flare_document::{Diagnostic, TimelineError};
use thiserror::Error;

pub use recovery::{
    RecoveryEntry, discard_recovery, list_recoveries, recovery_path, save_recovery,
};

#[cfg(not(target_arch = "wasm32"))]
pub use sqlite::{load, save};
#[cfg(target_arch = "wasm32")]
pub use unsupported::{load, save};

#[derive(Debug, Error)]
pub enum StorageError {
    #[cfg(not(target_arch = "wasm32"))]
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("{0}")]
    Unsupported(&'static str),
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),
    #[error("malformed payload: {0}")]
    Json(#[from] serde_json::Error),
    #[error("not a Flare project file")]
    NotFlareFile,
    #[error("project schema version {0} is not supported by this build")]
    UnsupportedVersion(i64),
    #[error("project exceeds a safety limit: {0}")]
    Limit(&'static str),
    #[error("corrupt project: {0}")]
    Corrupt(String),
    #[error(transparent)]
    Timeline(#[from] TimelineError),
    #[error("stored document is invalid: {0:?}")]
    Invalid(Vec<Diagnostic>),
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
