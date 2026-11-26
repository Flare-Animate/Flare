//! The project as the records the file holds, apart from where they are kept.
//!
//! [`to_parts`] turns a document into rows (a scene, a symbol, a sound or an attachment each, as
//! CBOR, compressed where that is smaller, with its size and digest), and [`from_parts`] checks and
//! rebuilds a document from rows. Neither touches a database or a compression library: the
//! desktop app keeps the rows in SQLite and brings zstd with it, the browser keeps them in a
//! SQLite WebAssembly module and a zstd one, and both get the same checks from here.

use flare_document::Document;

use crate::StorageError;
use crate::codec::{Packer, Zstd, from_cbor, to_cbor, unpack};
use crate::wire;

/// The one version of the format.
pub const SCHEMA_VERSION: i64 = 1;
/// Written instead of [`SCHEMA_VERSION`] by a file that holds bitmaps, so a build that does not
/// know them refuses the file by name rather than calling it corrupt. Files without bitmaps stay
/// at version 1 and open in older builds.
pub const BITMAP_SCHEMA_VERSION: i64 = 2;
/// Written by a file that holds a game description, for the same reason as
/// [`BITMAP_SCHEMA_VERSION`]: a build without games refuses it by name. Files without one keep the
/// older version. Version 4 is the same format with trigger zones, hooks and named input actions;
/// a build of version 3 refuses a v4 file rather than misreading a hook as corrupt.
pub const GAME_SCHEMA_VERSION: i64 = 4;
/// Written by a project containing shader graphs; older readers refuse it instead of discarding them.
pub const SHADER_SCHEMA_VERSION: i64 = 5;
/// Written by a project with a bitmap kept as a photo (a JPEG); older readers refuse it by name.
pub const PHOTO_SCHEMA_VERSION: i64 = 6;

/// The tables of a project file, the same for the desktop app and the browser (which gets them
/// from here, so the two cannot drift apart).
pub const TABLES: &str = "
CREATE TABLE project (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    project_id TEXT NOT NULL,
    schema_version INTEGER NOT NULL,
    settings_json TEXT NOT NULL
) STRICT;
CREATE TABLE entity (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('scene', 'symbol', 'attachment', 'sound', 'bitmap', 'photo', 'origin', 'game', 'shader')),
    ordinal INTEGER NOT NULL,
    payload_version INTEGER NOT NULL CHECK (payload_version > 0),
    codec TEXT NOT NULL CHECK (codec IN ('cbor', 'cbor+zstd')),
    size INTEGER NOT NULL CHECK (size >= 0),
    digest BLOB NOT NULL CHECK (length(digest) = 32),
    payload BLOB NOT NULL
) STRICT, WITHOUT ROWID;
";

/// Whether this build reads a file of schema `version`, or the error that says it was made by a
/// newer Flare.
pub fn check_schema_version(version: i64) -> Result<(), StorageError> {
    match version {
        SCHEMA_VERSION
        | BITMAP_SCHEMA_VERSION
        | GAME_SCHEMA_VERSION
        | SHADER_SCHEMA_VERSION
        | PHOTO_SCHEMA_VERSION => Ok(()),
        other => Err(StorageError::UnsupportedVersion(other)),
    }
}

impl Parts {
    /// The schema version these rows need to be read.
    pub fn schema_version(&self) -> i64 {
        if self.records.iter().any(|r| r.kind == "photo") {
            PHOTO_SCHEMA_VERSION
        } else if self.records.iter().any(|r| r.kind == "shader") {
            SHADER_SCHEMA_VERSION
        } else if self.records.iter().any(|r| r.kind == "game") {
            GAME_SCHEMA_VERSION
        } else if self.records.iter().any(|r| r.kind == "bitmap") {
            BITMAP_SCHEMA_VERSION
        } else {
            SCHEMA_VERSION
        }
    }
}
/// Most records a project may have.
pub const MAX_ENTITIES: usize = 1_000_000;
/// The most one record (a whole scene, at most) may be once decompressed: 256 MiB, which is
/// hundreds of thousands of shapes. A record's size is read from the file and used to size the
/// buffer, so some bound has to hold; this one is far above what drawing produces.
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024 * 1024;
/// The most a sound record may be: the largest sound file, and its fields.
pub const MAX_SOUND_PAYLOAD_BYTES: usize = flare_document::MAX_SOUND_BYTES + 64 * 1024 + 4;
pub const MAX_SHADER_PAYLOAD_BYTES: usize = 1024 * 1024;

/// One stored record, as its row holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: String,
    /// `scene`, `symbol`, `attachment`, `sound`, `bitmap`, `origin`, `game` or `shader`.
    pub kind: String,
    pub ordinal: i64,
    /// `cbor` or `cbor+zstd`.
    pub codec: String,
    /// The length once decompressed.
    pub size: i64,
    /// BLAKE3 of the payload as stored.
    pub digest: Vec<u8>,
    pub payload: Vec<u8>,
}

/// A whole project as rows.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parts {
    pub project_id: String,
    /// The frame rate and stage, as JSON text.
    pub settings: String,
    pub records: Vec<Record>,
}

/// The most a record of `kind` may hold once unpacked. Loading refuses a bigger one, so saving
/// checks the same number: a project that saves always opens.
pub(crate) fn limit_of(kind: &str) -> usize {
    match kind {
        "sound" => MAX_SOUND_PAYLOAD_BYTES,
        // A photo is kept only while it is smaller than its pixels, so a bitmap's room holds it.
        "bitmap" | "photo" => wire::MAX_BITMAP_RECORD_BYTES,
        "origin" => wire::MAX_ORIGIN_RECORD_BYTES,
        "game" => wire::MAX_GAME_RECORD_BYTES,
        "shader" => MAX_SHADER_PAYLOAD_BYTES,
        _ => MAX_PAYLOAD_BYTES,
    }
}

/// An error when a record of `kind` that unpacks to `size` bytes is more than loading takes.
#[cfg(test)]
pub(crate) fn fits(kind: &str, size: usize) -> Result<(), StorageError> {
    fits_within(kind, size, limit_of)
}

fn fits_within(kind: &str, size: usize, limit_of: fn(&str) -> usize) -> Result<(), StorageError> {
    let limit = limit_of(kind);
    if size > limit {
        return Err(StorageError::TooLarge {
            what: what_is(kind),
            size,
            limit,
        });
    }
    Ok(())
}

/// What a record of `kind` holds, as a person would call it.
fn what_is(kind: &str) -> &'static str {
    match kind {
        "scene" => "a scene's drawings and timeline",
        "symbol" => "a symbol",
        "bitmap" | "photo" => "a bitmap",
        "sound" => "a sound",
        "attachment" => "an attachment",
        "origin" => "the original file kept with the project",
        "game" => "the game description",
        "shader" => "a shader",
        _ => "a part of the project",
    }
}

/// `document` as rows, compressed with `zstd`.
pub fn to_parts(document: &Document, zstd: &mut dyn Zstd) -> Result<Parts, StorageError> {
    to_parts_within(document, zstd, limit_of)
}

/// [`to_parts`] with the limits given, so a test can reach them without a huge document.
pub(crate) fn to_parts_within(
    document: &Document,
    zstd: &mut dyn Zstd,
    limit_of: fn(&str) -> usize,
) -> Result<Parts, StorageError> {
    let settings = serde_json::to_string(&wire::Settings::of(document))?;
    let mut packer = Packer::new(zstd);
    let mut records = Vec::new();
    let mut put = |id: String, kind: &str, ordinal: i64, raw: &[u8]| -> Result<(), StorageError> {
        fits_within(kind, raw.len(), limit_of)?;
        let stored = packer.pack(raw)?;
        records.push(Record {
            id,
            kind: kind.to_owned(),
            ordinal,
            codec: stored.codec.name().to_owned(),
            size: i64::try_from(stored.size).unwrap_or(i64::MAX),
            digest: stored.digest.to_vec(),
            payload: stored.bytes.into_owned(),
        });
        Ok(())
    };
    for (ordinal, scene) in (0_i64..).zip(document.scenes()) {
        let raw = to_cbor(&wire::SceneWire::of(scene))?;
        put(scene.id().as_uuid().to_string(), "scene", ordinal, &raw)?;
    }
    for (ordinal, attachment) in (0_i64..).zip(document.attachments()) {
        let raw = to_cbor(&wire::AttachmentWire::of(attachment))?;
        // Attachments have no identity of their own; the id only has to be unique.
        put(
            uuid::Uuid::now_v7().to_string(),
            "attachment",
            ordinal,
            &raw,
        )?;
    }
    for (ordinal, sound) in (0_i64..).zip(document.sounds()) {
        let raw = wire::SoundMeta::frame(sound)?;
        put(sound.id().as_uuid().to_string(), "sound", ordinal, &raw)?;
    }
    for (ordinal, bitmap) in (0_i64..).zip(document.bitmaps()) {
        let id = bitmap.id().as_uuid().to_string();
        match wire::photo::frame(bitmap)? {
            Some(raw) => put(id, "photo", ordinal, &raw)?,
            None => put(id, "bitmap", ordinal, &wire::BitmapMeta::frame(bitmap)?)?,
        }
    }
    for (ordinal, symbol) in (0_i64..).zip(document.symbols()) {
        let raw = to_cbor(&wire::SymbolWire::of(symbol))?;
        put(symbol.id().as_uuid().to_string(), "symbol", ordinal, &raw)?;
    }
    if let Some(origin) = document.origin() {
        let raw = wire::OriginMeta::frame(origin)?;
        put("origin".to_owned(), "origin", 0, &raw)?;
    }
    if let Some(game) = document.game() {
        let raw = wire::game::frame(game)?;
        put("game".to_owned(), "game", 0, &raw)?;
    }
    for (ordinal, shader) in (0_i64..).zip(document.shader_assets()) {
        let raw = to_cbor(&wire::ShaderGraphWire::of(shader))?;
        put(format!("shader-{ordinal}"), "shader", ordinal, &raw)?;
    }
    Ok(Parts {
        project_id: document.id().as_uuid().to_string(),
        settings,
        records,
    })
}

/// Checks `parts` (untrusted) and rebuilds the document through its validating constructors.
pub fn from_parts(mut parts: Parts, zstd: &mut dyn Zstd) -> Result<Document, StorageError> {
    if parts.records.len() > MAX_ENTITIES {
        return Err(StorageError::Limit("entity count"));
    }
    parts
        .records
        .sort_by(|a, b| a.kind.cmp(&b.kind).then(a.ordinal.cmp(&b.ordinal)));
    let mut scenes = Vec::new();
    let mut symbols = Vec::new();
    let mut attachments = Vec::new();
    let mut sounds = Vec::new();
    let mut bitmaps = Vec::new();
    let mut origin = None;
    let mut game = None;
    let mut shader_assets = Vec::new();
    for record in &parts.records {
        let raw = unpack(
            zstd,
            &record.codec,
            record.size,
            &record.digest,
            &record.payload,
            limit_of(&record.kind),
        )?;
        match record.kind.as_str() {
            "scene" => scenes.push(from_cbor::<wire::SceneWire>(&raw)?.build(&record.id)?),
            "symbol" => symbols.push(from_cbor::<wire::SymbolWire>(&raw)?.build(&record.id)?),
            "attachment" => attachments.push(from_cbor::<wire::AttachmentWire>(&raw)?.build()?),
            "sound" => sounds.push(wire::SoundMeta::unframe(&record.id, &raw)?),
            "bitmap" => {
                bitmaps.push((record.ordinal, wire::BitmapMeta::unframe(&record.id, &raw)?))
            }
            "photo" => bitmaps.push((record.ordinal, wire::photo::unframe(&record.id, &raw)?)),
            "origin" => {
                if origin.replace(wire::OriginMeta::unframe(&raw)?).is_some() {
                    return Err(StorageError::Corrupt("two source records".into()));
                }
            }
            "game" => {
                if game.replace(wire::game::unframe(&raw)?).is_some() {
                    return Err(StorageError::Corrupt("two game records".into()));
                }
            }
            "shader" => {
                if shader_assets.len() >= flare_document::MAX_SHADER_ASSETS {
                    return Err(StorageError::Limit("shader graph count"));
                }
                shader_assets.push(from_cbor::<wire::ShaderGraphWire>(&raw)?.build()?);
            }
            other => {
                return Err(StorageError::Corrupt(format!(
                    "unknown entity kind {other}"
                )));
            }
        }
    }
    // Records come sorted by kind, so the lossless bitmaps and the photos are put back in the order
    // the library had them.
    bitmaps.sort_by_key(|(ordinal, _)| *ordinal);
    let bitmaps = bitmaps.into_iter().map(|(_, b)| b).collect();
    wire::assemble(
        &parts.project_id,
        &parts.settings,
        scenes,
        symbols,
        attachments,
        sounds,
        bitmaps,
        origin,
        game,
        shader_assets,
    )
}
