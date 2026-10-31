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

use crate::codec::{Packer, from_cbor, to_cbor, unpack};
use crate::{StorageError, wire};

/// ASCII "FLAR".
pub(crate) const APPLICATION_ID: i64 = 0x464C_4152;
pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const MAX_ENTITIES: i64 = 1_000_000;
/// The most one record (a whole scene, at most) may be once decompressed: 256 MiB, which is
/// hundreds of thousands of shapes. A record's size is read from the file and used to size the
/// buffer, so some bound has to hold; this one is far above what drawing produces.
pub(crate) const MAX_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;
/// The most a sound record may be: the largest sound file, and its fields.
pub(crate) const MAX_SOUND_PAYLOAD_BYTES: usize = flare_document::MAX_SOUND_BYTES + 64 * 1024 + 4;
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
    kind TEXT NOT NULL CHECK (kind IN ('scene', 'symbol', 'attachment', 'sound')),
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
        let settings = serde_json::to_string(&wire::Settings::of(document))?;
        tx.execute(
            "INSERT INTO project VALUES (1, ?1, ?2, ?3)",
            params![
                document.id().as_uuid().to_string(),
                SCHEMA_VERSION,
                settings
            ],
        )?;
        let mut packer = Packer::new()?;
        let mut insert = tx.prepare("INSERT INTO entity VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)")?;
        let mut put =
            |id: &str, kind: &str, ordinal: i64, raw: &[u8]| -> Result<(), StorageError> {
                let stored = packer.pack(raw)?;
                insert.execute(params![
                    id,
                    kind,
                    ordinal,
                    stored.codec.name(),
                    i64::try_from(stored.size).unwrap_or(i64::MAX),
                    stored.digest.as_slice(),
                    stored.bytes.as_ref()
                ])?;
                Ok(())
            };
        for (ordinal, scene) in (0_i64..).zip(document.scenes()) {
            let raw = to_cbor(&wire::SceneWire::of(scene))?;
            put(&scene.id().as_uuid().to_string(), "scene", ordinal, &raw)?;
        }
        for (ordinal, attachment) in (0_i64..).zip(document.attachments()) {
            let raw = to_cbor(&wire::AttachmentWire::of(attachment))?;
            // Attachments have no identity of their own; the row id only has to be unique.
            put(
                &uuid::Uuid::now_v7().to_string(),
                "attachment",
                ordinal,
                &raw,
            )?;
        }
        for (ordinal, sound) in (0_i64..).zip(document.sounds()) {
            let raw = wire::SoundMeta::frame(sound)?;
            put(&sound.id().as_uuid().to_string(), "sound", ordinal, &raw)?;
        }
        for (ordinal, symbol) in (0_i64..).zip(document.symbols()) {
            let raw = to_cbor(&wire::SymbolWire::of(symbol))?;
            put(&symbol.id().as_uuid().to_string(), "symbol", ordinal, &raw)?;
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
    if count > MAX_ENTITIES {
        return Err(StorageError::Limit("entity count"));
    }
    let (project_id, settings): (String, String) = conn.query_row(
        "SELECT project_id, settings_json FROM project WHERE singleton = 1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    let mut scenes = Vec::new();
    let mut symbols = Vec::new();
    let mut attachments = Vec::new();
    let mut sounds = Vec::new();
    let mut statement = conn.prepare(
        "SELECT id, kind, codec, size, digest, payload FROM entity ORDER BY kind, ordinal",
    )?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let kind: String = row.get(1)?;
        let codec: String = row.get(2)?;
        let size: i64 = row.get(3)?;
        let digest: Vec<u8> = row.get(4)?;
        let stored: Vec<u8> = row.get(5)?;
        // A sound record holds a whole audio file, so it is allowed to be bigger.
        let limit = if kind == "sound" {
            MAX_SOUND_PAYLOAD_BYTES
        } else {
            MAX_PAYLOAD_BYTES
        };
        let record = unpack(&codec, size, &digest, &stored, limit)?;
        match kind.as_str() {
            "scene" => scenes.push(from_cbor::<wire::SceneWire>(&record)?.build(&id)?),
            "symbol" => symbols.push(from_cbor::<wire::SymbolWire>(&record)?.build(&id)?),
            "attachment" => attachments.push(from_cbor::<wire::AttachmentWire>(&record)?.build()?),
            "sound" => sounds.push(wire::SoundMeta::unframe(&id, &record)?),
            other => {
                return Err(StorageError::Corrupt(format!(
                    "unknown entity kind {other}"
                )));
            }
        }
    }
    wire::assemble(&project_id, &settings, scenes, symbols, attachments, sounds)
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
