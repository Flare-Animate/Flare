//! The file a document was imported from, kept so that it can be written back unchanged.
//!
//! Flare does not edit a `.fla`, an XFL folder or a `.swf` in place: it converts one into a
//! document, which cannot hold everything the file does (bitmaps, publish settings, the exact
//! bytes of the XML). An [`Origin`] is the file itself, as it was read, plus a fingerprint of what
//! the import produced ([`Baseline`]). An exporter that finds the document still matching that
//! fingerprint writes the original back byte for byte; one that finds it edited uses the
//! fingerprint to see which parts changed and carries every other file over untouched.
//!
//! An origin is data from outside, so it is bounded and checked: size and file count are capped,
//! names must stay inside the document, and the digest of the content is stored with it.

use std::sync::Arc;

use flare_core::SymbolId;
use thiserror::Error;

/// The most source bytes a document keeps, in all: 256 MiB. A source beyond this is not kept and
/// the import says so, so no export of it claims to be lossless.
pub const MAX_ORIGIN_BYTES: usize = 256 * 1024 * 1024;
/// The most files (and empty folders) an unpacked XFL source may have.
pub const MAX_ORIGIN_FILES: usize = 50_000;
/// The longest name of an origin or of a file in it, in bytes.
pub const MAX_ORIGIN_NAME_BYTES: usize = 1024;

/// What the source was.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OriginKind {
    /// A zipped `.fla`: the bytes of the whole file.
    Fla,
    /// An unpacked XFL folder: every file and empty folder in it.
    Xfl,
    /// A Flash movie: the bytes of the whole file.
    Swf,
}

impl OriginKind {
    /// The name the project file uses for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Fla => "fla",
            Self::Xfl => "xfl",
            Self::Swf => "swf",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "fla" => Self::Fla,
            "xfl" => Self::Xfl,
            "swf" => Self::Swf,
            _ => return None,
        })
    }
}

/// One file of an unpacked source. A name ending in `/` is an empty folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginFile {
    pub name: String,
    pub bytes: Vec<u8>,
}

/// The content of the source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginData {
    /// A single file.
    Bytes(Vec<u8>),
    /// A folder's files, in the order they were read.
    Files(Vec<OriginFile>),
}

/// What one library symbol looked like when it was imported.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolBaseline {
    pub id: SymbolId,
    /// Its name as imported, to tell a renamed symbol from one that only changed inside.
    pub name: String,
    /// A digest of the symbol and everything hung off it, taken by the project container.
    pub digest: [u8; 32],
    /// The file it came from, like `LIBRARY/Symbol 1.xml`; empty for a source with no files of
    /// its own for symbols (a movie).
    pub file: String,
}

/// A fingerprint of the document the import produced, in parts, so a later edit can be told from
/// no edit, and an edit's reach from the rest.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Baseline {
    /// Everything outside the symbols' own timelines: settings, scenes, sounds, the list of
    /// symbols by name and kind, and the source data hung off all of those.
    pub main: [u8; 32],
    pub symbols: Vec<SymbolBaseline>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OriginError {
    #[error("the source is {0} bytes; at most {MAX_ORIGIN_BYTES} are kept")]
    TooLarge(usize),
    #[error("the source has more than {MAX_ORIGIN_FILES} files")]
    TooManyFiles,
    #[error("a {0:?} source must be {1}")]
    WrongData(OriginKind, &'static str),
    #[error("the source has a file name that is not safe: {0:?}")]
    UnsafeName(String),
    #[error("the source has two files named {0:?}")]
    DuplicateName(String),
    #[error("the source's digest does not match its content")]
    DigestMismatch,
}

/// Whether `name` stays inside the document: relative, `/`-separated, no `..`, `.` or empty
/// parts. A trailing `/` (an empty folder) is allowed.
pub fn is_safe_entry_name(name: &str) -> bool {
    let bare = name.strip_suffix('/').unwrap_or(name);
    name.len() <= MAX_ORIGIN_NAME_BYTES
        && !bare.is_empty()
        && !bare.starts_with('/')
        && !name.contains('\\')
        && !name.contains('\0')
        && bare
            .split('/')
            .all(|p| !p.is_empty() && p != "." && p != "..")
}

fn digest_of(data: &OriginData) -> [u8; 32] {
    match data {
        OriginData::Bytes(bytes) => *blake3::hash(bytes).as_bytes(),
        OriginData::Files(files) => {
            let mut hasher = blake3::Hasher::new();
            for file in files {
                hasher.update(&(file.name.len() as u64).to_le_bytes());
                hasher.update(file.name.as_bytes());
                hasher.update(&(file.bytes.len() as u64).to_le_bytes());
                hasher.update(&file.bytes);
            }
            *hasher.finalize().as_bytes()
        }
    }
}

/// The checks every origin passes, before anything is hashed.
fn validate(kind: OriginKind, name: &str, data: &OriginData) -> Result<(), OriginError> {
    if name.len() > MAX_ORIGIN_NAME_BYTES || name.contains(['/', '\\', '\0']) {
        return Err(OriginError::UnsafeName(name.to_owned()));
    }
    let total = match (data, kind) {
        (OriginData::Bytes(bytes), OriginKind::Fla | OriginKind::Swf) => bytes.len(),
        (OriginData::Files(files), OriginKind::Xfl) => {
            if files.len() > MAX_ORIGIN_FILES {
                return Err(OriginError::TooManyFiles);
            }
            let mut seen = std::collections::HashSet::new();
            let mut total = 0_usize;
            for file in files {
                if !is_safe_entry_name(&file.name) {
                    return Err(OriginError::UnsafeName(file.name.clone()));
                }
                if !seen.insert(file.name.as_str()) {
                    return Err(OriginError::DuplicateName(file.name.clone()));
                }
                total = total.saturating_add(file.bytes.len());
            }
            total
        }
        (_, OriginKind::Xfl) => return Err(OriginError::WrongData(kind, "a list of files")),
        (_, _) => return Err(OriginError::WrongData(kind, "the bytes of one file")),
    };
    if total > MAX_ORIGIN_BYTES {
        return Err(OriginError::TooLarge(total));
    }
    Ok(())
}

/// The source a document was imported from.
#[derive(Debug, Clone)]
pub struct Origin {
    kind: OriginKind,
    name: String,
    data: OriginData,
    digest: [u8; 32],
    baseline: Baseline,
}

/// Two origins are the same when they hold the same content (by digest) and baseline; this keeps
/// comparing two documents from reading hundreds of megabytes.
impl PartialEq for Origin {
    fn eq(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.name == other.name
            && self.digest == other.digest
            && self.baseline == other.baseline
    }
}

impl Eq for Origin {}

impl Origin {
    /// Checks `data` and takes its digest. `name` is the file name without its extension (the
    /// `.xfl` stub of an XFL folder is named by it).
    pub fn new(
        kind: OriginKind,
        name: impl Into<String>,
        data: OriginData,
        baseline: Baseline,
    ) -> Result<Self, OriginError> {
        let name = name.into();
        validate(kind, &name, &data)?;
        let digest = digest_of(&data);
        Ok(Self {
            kind,
            name,
            data,
            digest,
            baseline,
        })
    }

    /// Like [`Origin::new`] for content read back from a file, which must match the `digest` it
    /// was stored with.
    pub fn from_stored(
        kind: OriginKind,
        name: impl Into<String>,
        data: OriginData,
        digest: [u8; 32],
        baseline: Baseline,
    ) -> Result<Self, OriginError> {
        let name = name.into();
        validate(kind, &name, &data)?;
        if digest_of(&data) != digest {
            return Err(OriginError::DigestMismatch);
        }
        Ok(Self {
            kind,
            name,
            data,
            digest,
            baseline,
        })
    }

    pub fn kind(&self) -> OriginKind {
        self.kind
    }

    /// The source's file name without its extension.
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn data(&self) -> &OriginData {
        &self.data
    }

    /// BLAKE3 of the content (of the framed files, for a folder).
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }

    pub fn baseline(&self) -> &Baseline {
        &self.baseline
    }

    /// How many source bytes this holds.
    pub fn len(&self) -> usize {
        match &self.data {
            OriginData::Bytes(bytes) => bytes.len(),
            OriginData::Files(files) => files.iter().map(|f| f.bytes.len()).sum(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// A shared origin: documents are cloned for every undo step, the source is not.
pub type SharedOrigin = Arc<Origin>;
