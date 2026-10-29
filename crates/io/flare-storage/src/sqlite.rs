//! The SQLite container itself: saving and loading a project file.

use std::fs;
use std::path::Path;

use flare_document::Document;
use rusqlite::{Connection, OpenFlags, config::DbConfig, params};

use crate::{StorageError, wire};

/// ASCII "FLAR".
pub(crate) const APPLICATION_ID: i64 = 0x464C_4152;
pub(crate) const SCHEMA_VERSION: i64 = 1;
pub(crate) const MAX_ENTITIES: i64 = 1_000_000;
pub(crate) const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;
/// Base64 of the largest sound file, with room for the rest of the row.
pub(crate) const MAX_SOUND_PAYLOAD_BYTES: usize = flare_document::MAX_SOUND_BYTES / 3 * 4 + 4096;

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
    payload_json TEXT NOT NULL
) STRICT;
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
        let mut insert = tx.prepare("INSERT INTO entity VALUES (?1, ?2, ?3, 1, ?4)")?;
        for (ordinal, scene) in (0_i64..).zip(document.scenes()) {
            let payload = serde_json::to_string(&wire::SceneWire::of(scene))?;
            insert.execute(params![
                scene.id().as_uuid().to_string(),
                "scene",
                ordinal,
                payload
            ])?;
        }
        for (ordinal, attachment) in (0_i64..).zip(document.attachments()) {
            let payload = serde_json::to_string(&wire::AttachmentWire::of(attachment))?;
            // Attachments have no identity of their own; the row id only has to be unique.
            let id = uuid::Uuid::now_v7().to_string();
            insert.execute(params![id, "attachment", ordinal, payload])?;
        }
        for (ordinal, sound) in (0_i64..).zip(document.sounds()) {
            let payload = serde_json::to_string(&wire::SoundWire::of(sound))?;
            insert.execute(params![
                sound.id().as_uuid().to_string(),
                "sound",
                ordinal,
                payload
            ])?;
        }
        for (ordinal, symbol) in (0_i64..).zip(document.symbols()) {
            let payload = serde_json::to_string(&wire::SymbolWire::of(symbol))?;
            insert.execute(params![
                symbol.id().as_uuid().to_string(),
                "symbol",
                ordinal,
                payload
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
    let mut statement =
        conn.prepare("SELECT id, kind, payload_json FROM entity ORDER BY kind, ordinal")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (id, kind, payload): (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
        // A sound row holds a whole audio file as text, so it is allowed to be bigger.
        let limit = if kind == "sound" {
            MAX_SOUND_PAYLOAD_BYTES
        } else {
            MAX_PAYLOAD_BYTES
        };
        if payload.len() > limit {
            return Err(StorageError::Limit("entity payload size"));
        }
        match kind.as_str() {
            "scene" => scenes.push(serde_json::from_str::<wire::SceneWire>(&payload)?.build(&id)?),
            "symbol" => {
                symbols.push(serde_json::from_str::<wire::SymbolWire>(&payload)?.build(&id)?)
            }
            "attachment" => {
                attachments.push(serde_json::from_str::<wire::AttachmentWire>(&payload)?.build()?)
            }
            "sound" => sounds.push(serde_json::from_str::<wire::SoundWire>(&payload)?.build(&id)?),
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
