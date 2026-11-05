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

impl Parts {
    /// The schema version these rows need to be read.
    pub fn schema_version(&self) -> i64 {
        if self.records.iter().any(|r| r.kind == "bitmap") {
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

/// One stored record, as its row holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Record {
    pub id: String,
    /// `scene`, `symbol`, `attachment`, `sound`, `bitmap` or `origin`.
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

fn limit_of(kind: &str) -> usize {
    match kind {
        "sound" => MAX_SOUND_PAYLOAD_BYTES,
        "bitmap" => wire::MAX_BITMAP_RECORD_BYTES,
        "origin" => wire::MAX_ORIGIN_RECORD_BYTES,
        _ => MAX_PAYLOAD_BYTES,
    }
}

/// `document` as rows, compressed with `zstd`.
pub fn to_parts(document: &Document, zstd: &mut dyn Zstd) -> Result<Parts, StorageError> {
    let settings = serde_json::to_string(&wire::Settings::of(document))?;
    let mut packer = Packer::new(zstd);
    let mut records = Vec::new();
    let mut put = |id: String, kind: &str, ordinal: i64, raw: &[u8]| -> Result<(), StorageError> {
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
        let raw = wire::BitmapMeta::frame(bitmap)?;
        put(bitmap.id().as_uuid().to_string(), "bitmap", ordinal, &raw)?;
    }
    for (ordinal, symbol) in (0_i64..).zip(document.symbols()) {
        let raw = to_cbor(&wire::SymbolWire::of(symbol))?;
        put(symbol.id().as_uuid().to_string(), "symbol", ordinal, &raw)?;
    }
    if let Some(origin) = document.origin() {
        let raw = wire::OriginMeta::frame(origin)?;
        put("origin".to_owned(), "origin", 0, &raw)?;
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
            "bitmap" => bitmaps.push(wire::BitmapMeta::unframe(&record.id, &raw)?),
            "origin" => {
                if origin.replace(wire::OriginMeta::unframe(&raw)?).is_some() {
                    return Err(StorageError::Corrupt("two source records".into()));
                }
            }
            other => {
                return Err(StorageError::Corrupt(format!(
                    "unknown entity kind {other}"
                )));
            }
        }
    }
    wire::assemble(
        &parts.project_id,
        &parts.settings,
        scenes,
        symbols,
        attachments,
        sounds,
        bitmaps,
        origin,
    )
}
