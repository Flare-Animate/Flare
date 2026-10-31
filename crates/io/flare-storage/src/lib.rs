//! Native `.flare` container: a SQLite file holding the saved document, its records compressed.
//!
//! Saving stages a complete database next to the target and atomically renames it into
//! place, so a failed save never damages the previous file. Each record is CBOR, zstd-compressed
//! where that is smaller, with a BLAKE3 digest. Loading treats the file as untrusted: defensive
//! SQLite settings, digests, size limits checked before anything is allocated or decompressed,
//! and full document revalidation through the normal constructors.
#![forbid(unsafe_code)]

mod codec;
mod records;
mod recovery;
#[cfg(not(target_arch = "wasm32"))]
mod sqlite;
#[cfg(target_arch = "wasm32")]
mod unsupported;
mod wire;

use flare_document::{Diagnostic, TimelineError};
use thiserror::Error;

#[cfg(not(target_arch = "wasm32"))]
pub use codec::NativeZstd;
pub use codec::{Codec, Packer, Stored, Zstd, unpack};
pub use records::{
    MAX_ENTITIES, MAX_PAYLOAD_BYTES, MAX_SOUND_PAYLOAD_BYTES, Parts, Record, SCHEMA_VERSION,
    from_parts, to_parts,
};
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
    /// A library the page brings (SQLite or zstd as WebAssembly) failed.
    #[error("{0}")]
    Host(String),
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
