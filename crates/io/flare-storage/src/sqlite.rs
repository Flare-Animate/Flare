//! The SQLite container itself: saving and loading a project file.
//!
//! Each record (a scene, a symbol, a sound, an attachment) is a row whose payload is the record's
//! fields as CBOR, compressed with zstd where that is smaller, beside the digest of what is stored
//! (see `codec`). The pages are 1 KiB and the records table has no second index, so the database
//! itself costs about 2 KiB and about 1% over the payload, and the file is as small as a binary
//! file of our own would be, with SQLite's atomic writes and tooling kept.

use std::fs;
use std::path::Path;

use flare_document::Document;
use rusqlite::{Connection, OpenFlags, config::DbConfig, params};

use crate::StorageError;
use crate::codec::NativeZstd;
use crate::records::{MAX_ENTITIES, Parts, Record, SCHEMA_VERSION, from_parts, to_parts};

/// ASCII "FLAR".
pub(crate) const APPLICATION_ID: i64 = 0x464C_4152;
/// Small pages waste less on the many small records and the empty space after the last one.
pub(crate) const PAGE_SIZE: i64 = 1024;

pub(crate) const SCHEMA: &str = "
CREATE TABLE project (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    project_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    settings_json TEXT NOT NULL
) STRICT;
CREATE TABLE entity (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('scene', 'symbol', 'attachment', 'sound', 'bitmap', 'origin')),
    ordinal INTEGER NOT NULL,
    payload_version INTEGER NOT NULL CHECK (payload_version > 0),
    codec TEXT NOT NULL CHECK (codec IN ('cbor', 'cbor+zstd')),
    size INTEGER NOT NULL CHECK (size >= 0),
    digest BLOB NOT NULL CHECK (length(digest) = 32),
    payload BLOB NOT NULL
) STRICT, WITHOUT ROWID;
";

/// Writes `document` to `path`, replacing any existing file only on full success.
pub fn save(document: &Document, path: &Path) -> Result<(), StorageError> {
    let directory = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staged = tempfile::NamedTempFile::new_in(directory)?;
    {
        let mut conn = Connection::open(staged.path())?;
        // The page size can only be chosen before the first table exists.
        conn.pragma_update(None, "page_size", PAGE_SIZE)?;
        conn.pragma_update(None, "journal_mode", "DELETE")?;
        conn.pragma_update(None, "application_id", APPLICATION_ID)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        let tx = conn.transaction()?;
        tx.execute_batch(SCHEMA)?;
        let parts = to_parts(document, &mut NativeZstd::new()?)?;
        tx.execute(
            "INSERT INTO project VALUES (1, ?1, ?2, ?3)",
            params![parts.project_id, SCHEMA_VERSION, parts.settings],
        )?;
        let mut insert = tx.prepare("INSERT INTO entity VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)")?;
        for record in &parts.records {
            insert.execute(params![
                record.id,
                record.kind,
                record.ordinal,
                record.codec,
                record.size,
                record.digest,
                record.payload
            ])?;
        }
        drop(insert);
        tx.commit()?;
        conn.close().map_err(|(_, error)| error)?;
    }
    // Directory fsync after rename is not done yet; see the storage durability gate.
    staged
        .persist(path)
        .map_err(|error| StorageError::Io(error.error))?;
    Ok(())
}

/// Reads and fully revalidates a project file.
pub fn load(path: &Path) -> Result<Document, StorageError> {
    fs::metadata(path)?;
    let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)?;
    conn.set_db_config(DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false)?;

    let application_id: i64 = conn
        .pragma_query_value(None, "application_id", |row| row.get(0))
        .map_err(not_flare)?;
    if application_id != APPLICATION_ID {
        return Err(StorageError::NotFlareFile);
    }
    let version: i64 = conn.pragma_query_value(None, "user_version", |row| row.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(StorageError::UnsupportedVersion(version));
    }

    let count: i64 = conn.query_row("SELECT count(*) FROM entity", [], |row| row.get(0))?;
    if usize::try_from(count).map_or(true, |n| n > MAX_ENTITIES) {
        return Err(StorageError::Limit("entity count"));
    }
    let (project_id, settings): (String, String) = conn.query_row(
        "SELECT project_id, settings_json FROM project WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut statement = conn.prepare(
        "SELECT id, kind, ordinal, codec, size, digest, payload FROM entity ORDER BY kind, ordinal",
    )?;
    let records = statement
        .query_map([], |row| {
            Ok(Record {
                id: row.get(0)?,
                kind: row.get(1)?,
                ordinal: row.get(2)?,
                codec: row.get(3)?,
                size: row.get(4)?,
                digest: row.get(5)?,
                payload: row.get(6)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    from_parts(
        Parts {
            project_id,
            settings,
            records,
        },
        &mut NativeZstd::new()?,
    )
}

pub(crate) fn not_flare(error: rusqlite::Error) -> StorageError {
    // A non-database file fails on the first read with "file is not a database" (or, for a
    // damaged one, "malformed"). Busy, permission and I/O failures are real errors about a file
    // that may well be a project, so they are reported as such.
    match error {
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(
                failure.code,
                rusqlite::ErrorCode::NotADatabase | rusqlite::ErrorCode::DatabaseCorrupt
            ) =>
        {
            StorageError::NotFlareFile
        }
        other => StorageError::Sqlite(other),
    }
}
