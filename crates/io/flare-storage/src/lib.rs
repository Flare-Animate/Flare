//! Native `.flare` container: a SQLite file holding the saved document.
//!
//! Saving stages a complete database next to the target and atomically renames it into
//! place, so a failed save never damages the previous file. Loading treats the file as
//! untrusted: defensive SQLite settings, size limits, and full document revalidation
//! through the normal constructors. The on-disk schema is a draft, not a stable format.
#![forbid(unsafe_code)]

mod recovery;
mod wire;

use std::fs;
use std::path::Path;

use flare_document::{Diagnostic, Document, TimelineError};
use rusqlite::{Connection, OpenFlags, config::DbConfig, params};
use thiserror::Error;

pub use recovery::{
    RecoveryEntry, discard_recovery, list_recoveries, recovery_path, save_recovery,
};

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

    use flare_core::{
        DocumentId, DocumentPoint, FrameRate, InstanceId, LayerId, Rgba8, SceneId, ShapeId,
        SymbolId, Transform2D,
    };
    use flare_document::{
        Content, FrameSpan, Instance, Layer, PathSegment, Scene, Shape, Stage, Stroke, Symbol,
        Timeline, Tween,
    };
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
        let curve = Shape::new(
            ShapeId::new(),
            vec![
                PathSegment::MoveTo(DocumentPoint::new(0.0, 0.5).expect("finite")),
                PathSegment::CubicTo(
                    DocumentPoint::new(1.0, 2.0).expect("finite"),
                    DocumentPoint::new(3.0, -4.0).expect("finite"),
                    DocumentPoint::new(5.5, 6.25).expect("finite"),
                ),
                PathSegment::Close,
            ],
            Rgba8::new(10, 20, 30, 40),
        )
        .expect("shape")
        .with_stroke(Stroke::new(2.5, Rgba8::new(9, 8, 7, 255)).expect("stroke"));
        let drawn = FrameSpan::new(
            10,
            NonZeroU32::new(1).expect("nonzero"),
            Content::Shape(curve),
        );
        let layer = Layer::new(
            LayerId::new(),
            "Layer 1",
            vec![place(0).with_tween(Tween::Linear), place(4), empty, drawn],
        )
        .expect("layer")
        .with_visible(false)
        .with_locked(true);
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
        .with_stage(Stage::new(640, 360, Rgba8::new(1, 2, 3, 4)).expect("stage"))
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
    fn files_without_a_stage_load_with_the_default_and_bad_stages_are_rejected() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        save(&sample(), &path).expect("save");
        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE project SET settings_json = '{\"frame_rate\":[30000,1001]}'",
            [],
        )
        .expect("strip stage");
        drop(conn);
        assert_eq!(load(&path).expect("legacy load").stage(), Stage::default());

        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE project SET settings_json = '{\"frame_rate\":[24,1],\"stage\":{\"width\":0,\"height\":5,\"background\":[0,0,0,0]}}'",
            [],
        )
        .expect("bad stage");
        drop(conn);
        assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
    }

    #[test]
    fn legacy_layers_without_flags_load_visible_and_unlocked() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        save(&sample(), &path).expect("save");
        assert!(!load(&path).expect("load").scenes()[0].timeline().layers()[0].is_visible());
        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE entity SET payload_json = replace(replace(payload_json, '\"visible\":', '\"v\":'), '\"locked\":', '\"l\":')",
            [],
        )
        .expect("strip flags");
        drop(conn);
        let layer = &load(&path).expect("legacy load").scenes()[0]
            .timeline()
            .layers()[0]
            .clone();
        assert!(layer.is_visible() && !layer.is_locked());
    }

    #[test]
    fn strokes_are_revalidated_and_legacy_shapes_without_strokes_still_load() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        save(&sample(), &path).expect("save");

        let conn = Connection::open(&path).expect("open");
        conn.execute(
            "UPDATE entity SET payload_json = replace(payload_json, '\"stroke\":', '\"legacy\":')",
            [],
        )
        .expect("strip stroke key");
        drop(conn);
        assert!(load(&path).is_ok(), "an absent stroke is a fill-only shape");

        save(&sample(), &path).expect("save");
        let conn = Connection::open(&path).expect("open");
        conn.execute("UPDATE entity SET payload_json = replace(payload_json, '\"width\":2.5', '\"width\":-1.0')", [])
            .expect("corrupt width");
        drop(conn);
        assert!(matches!(load(&path), Err(StorageError::Corrupt(_))));
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

    /// Deterministic pseudo-random numbers, so a failure is reproducible.
    fn lcg(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state >> 33
    }

    #[test]
    fn corrupted_project_files_never_panic_and_corruption_is_usually_detected() {
        let dir = tempfile::tempdir().expect("dir");
        let good = dir.path().join("good.flare");
        save(&sample(), &good).expect("save");
        let original = fs::read(&good).expect("read");
        let mutant = dir.path().join("mutant.flare");

        let (mut errors, mut accepted) = (0, 0);
        let mut state = 0x5EED_u64;
        // Flip a run of bytes at many positions: header, schema pages and payloads alike.
        for _ in 0..400 {
            let mut bytes = original.clone();
            let at = (lcg(&mut state) as usize) % bytes.len();
            let run = 1 + (lcg(&mut state) as usize) % 8;
            for i in at..(at + run).min(bytes.len()) {
                bytes[i] ^= 0xFF;
            }
            fs::write(&mutant, &bytes).expect("write");
            match load(&mutant) {
                Ok(_) => accepted += 1,
                Err(_) => errors += 1,
            }
        }
        // Truncations at every kind of boundary.
        for keep in (0..original.len()).step_by(original.len() / 40 + 1) {
            fs::write(&mutant, &original[..keep]).expect("write");
            if load(&mutant).is_ok() {
                accepted += 1;
            } else {
                errors += 1;
            }
        }
        // Zeroed pages and garbage appended.
        let mut zeroed = original.clone();
        zeroed[100..original.len().min(4096)].fill(0);
        fs::write(&mutant, &zeroed).expect("write");
        assert!(
            load(&mutant).is_err(),
            "a zeroed first page is not a project"
        );
        let mut extended = original.clone();
        extended.extend(std::iter::repeat_n(0xAB, 10_000));
        fs::write(&mutant, &extended).expect("write");
        let _ = load(&mutant); // trailing garbage may or may not matter; it must not panic

        assert!(
            errors > 50,
            "corruption was caught often enough to mean something ({errors} vs {accepted})"
        );
        // Whatever was accepted is still a valid document, by construction of the loader.
    }

    #[test]
    fn a_project_with_hostile_payloads_is_refused_not_trusted() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        for payload in [
            "null",
            "[]",
            "{\"name\":1}",
            "{\"name\":\"x\",\"timeline\":{\"layers\":[{\"id\":\"nope\",\"name\":\"l\",\"spans\":[]}]}}",
            "{\"name\":\"x\",\"timeline\":{\"layers\":[{\"id\":\"00000000-0000-0000-0000-000000000001\",\"name\":\"l\",\"spans\":[{\"start\":4294967295,\"duration\":4294967295,\"content\":\"Empty\"}]}]}}",
            "{\"name\":\"x\",\"timeline\":{\"layers\":[{\"id\":\"00000000-0000-0000-0000-000000000001\",\"name\":\"l\",\"spans\":[{\"start\":0,\"duration\":0,\"content\":\"Empty\"}]}]}}",
        ] {
            save(&sample(), &path).expect("save");
            let conn = Connection::open(&path).expect("open");
            conn.execute(
                "UPDATE entity SET payload_json = ?1 WHERE kind = 'scene'",
                [payload],
            )
            .expect("tamper");
            drop(conn);
            // Some of these are well-formed but extreme (a span ending past u32); those load as
            // valid documents because frame arithmetic is overflow-safe. None may panic.
            let result = load(&path);
            if payload.contains("\"duration\":0") || payload == "null" || payload == "[]" {
                assert!(result.is_err(), "{payload}");
            }
        }
    }
}
