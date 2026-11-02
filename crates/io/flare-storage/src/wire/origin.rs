//! The record of the file a document was imported from.
//!
//! Like a sound, it is one record whose fields are CBOR and whose bulk is bytes, not text: the
//! length of the fields (4 bytes, little endian), the fields, then the source's bytes (for a
//! folder, its files one after another, as the fields say how long each is).

use crate::StorageError;
use crate::codec::{from_cbor, to_cbor};
use flare_core::SymbolId;
use flare_document::{
    Baseline, MAX_ORIGIN_BYTES, MAX_ORIGIN_FILES, Origin, OriginData, OriginFile, OriginKind,
    SymbolBaseline,
};
use serde::{Deserialize, Serialize};

use super::{hex, unhex, uuid};

/// The most the fields of an origin may take: names of up to `MAX_ORIGIN_FILES` files and the
/// symbols' file names.
const MAX_META_BYTES: usize = 16 * 1024 * 1024;

/// The most an origin record may be once decompressed.
pub(crate) const MAX_ORIGIN_RECORD_BYTES: usize = MAX_ORIGIN_BYTES + MAX_META_BYTES + 4;

#[derive(Serialize, Deserialize)]
struct SymbolMeta {
    id: String,
    name: String,
    digest: String,
    file: String,
}

#[derive(Serialize, Deserialize)]
struct FileMeta {
    name: String,
    len: u64,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct OriginMeta {
    kind: String,
    name: String,
    /// Lowercase hex of the content's digest.
    digest: String,
    main: String,
    symbols: Vec<SymbolMeta>,
    /// The files of a folder; empty for a single file.
    files: Vec<FileMeta>,
}

impl OriginMeta {
    /// An origin as one record.
    pub(crate) fn frame(origin: &Origin) -> Result<Vec<u8>, StorageError> {
        let baseline = origin.baseline();
        let files = match origin.data() {
            OriginData::Bytes(_) => Vec::new(),
            OriginData::Files(files) => files
                .iter()
                .map(|f| FileMeta {
                    name: f.name.clone(),
                    len: f.bytes.len() as u64,
                })
                .collect(),
        };
        let meta = to_cbor(&Self {
            kind: origin.kind().name().to_owned(),
            name: origin.name().to_owned(),
            digest: hex(&origin.digest()),
            main: hex(&baseline.main),
            symbols: baseline
                .symbols
                .iter()
                .map(|s| SymbolMeta {
                    id: s.id.as_uuid().to_string(),
                    name: s.name.clone(),
                    digest: hex(&s.digest),
                    file: s.file.clone(),
                })
                .collect(),
            files,
        })?;
        let mut out = Vec::with_capacity(4 + meta.len() + origin.len());
        out.extend_from_slice(&u32::try_from(meta.len()).unwrap_or(u32::MAX).to_le_bytes());
        out.extend_from_slice(&meta);
        match origin.data() {
            OriginData::Bytes(bytes) => out.extend_from_slice(bytes),
            OriginData::Files(files) => {
                for file in files {
                    out.extend_from_slice(&file.bytes);
                }
            }
        }
        Ok(out)
    }

    /// Takes a record made by [`OriginMeta::frame`] apart and rebuilds the origin, checking its
    /// digest and every size it states against the bytes that are there.
    pub(crate) fn unframe(record: &[u8]) -> Result<Origin, StorageError> {
        let corrupt = |why: &str| StorageError::Corrupt(format!("the source record {why}"));
        let (length, rest) = record
            .split_first_chunk::<4>()
            .ok_or_else(|| corrupt("is too short"))?;
        let length = u32::from_le_bytes(*length) as usize;
        if length > MAX_META_BYTES || length > rest.len() {
            return Err(corrupt("has fields that do not fit it"));
        }
        let (meta, payload) = rest.split_at(length);
        let meta: Self = from_cbor(meta)?;
        let kind = OriginKind::from_name(&meta.kind)
            .ok_or_else(|| corrupt("names a kind of source this build does not know"))?;
        if meta.files.len() > MAX_ORIGIN_FILES {
            return Err(StorageError::Limit("source file count"));
        }
        let digest = unhex(&meta.digest).ok_or_else(|| corrupt("has a bad digest"))?;
        let data = match kind {
            OriginKind::Fla | OriginKind::Swf => {
                if !meta.files.is_empty() {
                    return Err(corrupt("lists files for a single file"));
                }
                OriginData::Bytes(payload.to_vec())
            }
            OriginKind::Xfl => {
                let mut files = Vec::with_capacity(meta.files.len());
                let mut at = 0_usize;
                for file in meta.files {
                    let len = usize::try_from(file.len)
                        .map_err(|_| corrupt("states a file size that does not fit"))?;
                    let end = at
                        .checked_add(len)
                        .filter(|end| *end <= payload.len())
                        .ok_or_else(|| corrupt("states files longer than its bytes"))?;
                    files.push(OriginFile {
                        name: file.name,
                        bytes: payload[at..end].to_vec(),
                    });
                    at = end;
                }
                if at != payload.len() {
                    return Err(corrupt("has bytes no file accounts for"));
                }
                OriginData::Files(files)
            }
        };
        let mut symbols = Vec::with_capacity(meta.symbols.len().min(1 << 16));
        for symbol in &meta.symbols {
            symbols.push(SymbolBaseline {
                id: SymbolId::from_uuid(uuid(&symbol.id)?),
                name: symbol.name.clone(),
                digest: unhex(&symbol.digest).ok_or_else(|| corrupt("has a bad symbol digest"))?,
                file: symbol.file.clone(),
            });
        }
        let baseline = Baseline {
            main: unhex(&meta.main).ok_or_else(|| corrupt("has a bad baseline"))?,
            symbols,
        };
        Origin::from_stored(kind, meta.name, data, digest, baseline)
            .map_err(|error| StorageError::Corrupt(format!("the source record: {error}")))
    }
}
