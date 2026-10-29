//! Native `.flare` container: a SQLite file holding the saved document.
//!
//! Saving stages a complete database next to the target and atomically renames it into
//! place, so a failed save never damages the previous file. Loading treats the file as
//! untrusted: defensive SQLite settings, size limits, and full document revalidation
//! through the normal constructors. The on-disk schema is a draft, not a stable format.
#![forbid(unsafe_code)]

mod wire;

use std::fs;
use std::path::Path;

use flare_document::{Diagnostic, Document, TimelineError};
use rusqlite::{Connection, OpenFlags, config::DbConfig, params};
use thiserror::Error;

/// ASCII "FLAR".
const APPLICATION_ID: i64 = 0x464C_4152;
const SCHEMA_VERSION: i64 = 1;
const MAX_ENTITIES: i64 = 1_000_000;
const MAX_PAYLOAD_BYTES: usize = 16 * 1024 * 1024;

const SCHEMA: &str = "
CREATE TABLE project (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    project_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    settings_json TEXT NOT NULL
) STRICT;
CREATE TABLE entity (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('scene', 'symbol')),
    ordinal INTEGER NOT NULL,
    payload_version INTEGER NOT NULL CHECK (payload_version > 0),
    payload_json TEXT NOT NULL
) STRICT;
";

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
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
    let mut statement =
        conn.prepare("SELECT id, kind, payload_json FROM entity ORDER BY kind, ordinal")?;
    let mut rows = statement.query([])?;
    while let Some(row) = rows.next()? {
        let (id, kind, payload): (String, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
        if payload.len() > MAX_PAYLOAD_BYTES {
            return Err(StorageError::Limit("entity payload size"));
        }
        match kind.as_str() {
            "scene" => scenes.push(serde_json::from_str::<wire::SceneWire>(&payload)?.build(&id)?),
            "symbol" => {
                symbols.push(serde_json::from_str::<wire::SymbolWire>(&payload)?.build(&id)?)
            }
            other => {
                return Err(StorageError::Corrupt(format!(
                    "unknown entity kind {other}"
                )));
            }
        }
    }
    wire::assemble(&project_id, &settings, scenes, symbols)
}

fn not_flare(error: rusqlite::Error) -> StorageError {
    // A non-database file fails on the first read with "file is not a database".
    match error {
        rusqlite::Error::SqliteFailure(_, _) => StorageError::NotFlareFile,
        other => StorageError::Sqlite(other),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::num::NonZeroU32;

    use flare_core::{DocumentId, FrameRate, InstanceId, LayerId, SceneId, SymbolId, Transform2D};
    use flare_document::{Content, FrameSpan, Instance, Layer, Scene, Symbol, Timeline};
    use glamx::DAffine2;

    use super::*;

    fn sample() -> Document {
        let art = SymbolId::new();
        let transform = Transform2D::new(DAffine2::from_cols_array(&[
            1.0, 0.5, -0.25, 0.0, 3.5, -2.0,
        ]))
        .expect("finite");
        let place = |start| {
            let instance = Instance::new(InstanceId::new(), art, transform);
            FrameSpan::new(
                start,
                NonZeroU32::new(4).expect("nonzero"),
                Content::Instance(instance),
            )
        };
        let empty = FrameSpan::new(8, NonZeroU32::new(2).expect("nonzero"), Content::Empty);
        let layer =
            Layer::new(LayerId::new(), "Layer 1", vec![place(0), place(4), empty]).expect("layer");
        let scenes = vec![
            Scene::new(SceneId::new(), "One", Timeline::new(vec![layer])),
            Scene::new(SceneId::new(), "Two", Timeline::default()),
        ];
        let symbol = Symbol::new(art, "Art", Timeline::default());
        Document::new(
            DocumentId::new(),
            FrameRate::new(30_000, 1_001).expect("rate"),
            scenes,
            vec![symbol],
        )
        .expect("document")
    }

    #[test]
    fn round_trip_preserves_identity_order_and_matrices() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        let document = sample();
        save(&document, &path).expect("save");
        assert_eq!(load(&path).expect("load"), document);
    }

    #[test]
    fn resaving_replaces_the_file_atomically() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        save(&sample(), &path).expect("first");
        let second = sample();
        save(&second, &path).expect("second");
        assert_eq!(load(&path).expect("load"), second);
        assert_eq!(
            fs::read_dir(dir.path()).expect("list").count(),
            1,
            "no stray temp files"
        );
    }

    #[test]
    fn failed_save_leaves_previous_file_intact() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        let original = sample();
        save(&original, &path).expect("save");
        let missing = dir.path().join("no-such-dir").join("b.flare");
        assert!(matches!(
            save(&sample(), &missing),
            Err(StorageError::Io(_))
        ));
        assert_eq!(load(&path).expect("load"), original);
    }

    #[test]
    fn non_flare_and_truncated_files_are_rejected() {
        let dir = tempfile::tempdir().expect("dir");
        let text = dir.path().join("t.flare");
        fs::write(
            &text,
            b"this is definitely not sqlite, just some long enough text....",
        )
        .expect("w");
        assert!(matches!(load(&text), Err(StorageError::NotFlareFile)));

        let plain = dir.path().join("p.db");
        Connection::open(&plain)
            .expect("open")
            .execute_batch("CREATE TABLE x(a);")
            .expect("x");
        assert!(matches!(load(&plain), Err(StorageError::NotFlareFile)));

        let good = dir.path().join("g.flare");
        save(&sample(), &good).expect("save");
        let bytes = fs::read(&good).expect("read");
        fs::write(&good, &bytes[..bytes.len() / 2]).expect("truncate");
        assert!(load(&good).is_err());
        assert!(load(&dir.path().join("missing.flare")).is_err());
    }

    #[test]
    fn newer_schema_versions_are_refused() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        save(&sample(), &path).expect("save");
        Connection::open(&path)
            .expect("open")
            .pragma_update(None, "user_version", 2)
            .expect("set");
        assert!(matches!(
            load(&path),
            Err(StorageError::UnsupportedVersion(2))
        ));
    }

    #[test]
    fn tampered_content_is_revalidated_on_load() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        save(&sample(), &path).expect("save");
        let conn = Connection::open(&path).expect("open");
        conn.execute("DELETE FROM entity WHERE kind = 'symbol'", [])
            .expect("delete");
        drop(conn);
        assert!(matches!(load(&path), Err(StorageError::Invalid(_))));

        let conn = Connection::open(&path).expect("open");
        conn.execute("UPDATE entity SET payload_json = '{nope'", [])
            .expect("update");
        drop(conn);
        assert!(matches!(load(&path), Err(StorageError::Json(_))));
    }
}
