//! Legacy binary `.fla` files (Flash MX 2004 to CS4): a Compound File Binary container with a
//! `Contents` stream (document settings and the library table) and one stream per scene, symbol
//! and media item.
//!
//! There is no published specification. What is read here was worked out from the community
//! notes at <https://github.com/Kreijstal/fla-decoder> and then checked against the genuine
//! legacy files in the vendored Ruffle test suite (format versions 0x129, 0x138, 0x13f and 0x147;
//! all small test movies, none a real project, none made by CS3 or CS4 as far as is known).
//!
//! Read: the container, the stream names (and from them the style of the file and how many
//! scenes, symbols and media it has), the stage size, frame rate and background colour, and the
//! library table (name and type of each symbol, name and class of each media item).
//!
//! Not read: drawings, timelines, layers, text, tweens, sounds and bitmaps themselves, scripts,
//! fonts, library folders, and the names of scenes. [`import_legacy_bytes`] therefore makes a
//! skeleton: the stage, one empty scene and empty symbols of the right types.

mod cfb;
mod contents;
mod layout;
#[cfg(test)]
mod layout_tests;

use thiserror::Error;

use crate::xfl::{ImportError, Imported, Severity, frame_rate};
use flare_core::{DocumentId, Rgba8, SceneId, SymbolId};
use flare_document::{Document, MAX_STAGE_SIDE, Scene, Stage, Symbol, SymbolKind, Timeline};

pub use cfb::{CfbError, StreamInfo};
pub use contents::{DocumentProps, ItemKind, LibraryItem, PropsError};
pub use layout::{Era, LegacyInfo};

/// The first eight bytes of every compound file.
pub const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The most a legacy file may be, and so the most a caller should read for one.
pub const MAX_LEGACY_FILE_BYTES: u64 = cfb::MAX_FILE_BYTES;

/// The most `Contents` bytes read.
const MAX_CONTENTS_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LegacyError {
    #[error("this looks like a legacy binary FLA but cannot be read: {0}")]
    Container(#[from] CfbError),
    #[error("a compound file with no Contents stream is not a Flash document")]
    NoContents,
    #[error("{info}, but its Contents stream is larger than the {MAX_CONTENTS_BYTES} byte limit")]
    ContentsTooLarge { info: LegacyInfo },
}

/// A legacy `.fla` as far as it can be read.
#[derive(Debug, Clone, PartialEq)]
pub struct Legacy {
    pub info: LegacyInfo,
    /// The document settings, or why they could not be read.
    pub properties: Result<DocumentProps, PropsError>,
    pub library: Vec<LibraryItem>,
}

/// Reads what can be read of a legacy `.fla` held in memory.
pub fn read(bytes: &[u8]) -> Result<Legacy, LegacyError> {
    let container = cfb::Container::open(bytes)?;
    let names: Vec<&str> = container
        .streams()
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let Some(declared) = container.streams().iter().find(|s| s.name == "Contents") else {
        return Err(LegacyError::NoContents);
    };
    let mut info = LegacyInfo::from_names(names.iter().copied(), None);
    if declared.size > MAX_CONTENTS_BYTES {
        return Err(LegacyError::ContentsTooLarge { info });
    }
    let contents = container.read("Contents")?;
    info.format_version = contents
        .get(..2)
        .and_then(|b| <[u8; 2]>::try_from(b).ok())
        .map(u16::from_le_bytes);
    let properties = contents::properties(&contents, MAX_STAGE_SIDE);
    let library = contents::library(&contents, &names);
    Ok(Legacy {
        info,
        properties,
        library,
    })
}

/// What the file is, from its stream names and the version at the start of `Contents`.
pub fn inspect(bytes: &[u8]) -> Result<LegacyInfo, LegacyError> {
    read(bytes).map(|legacy| legacy.info)
}

/// Imports a legacy `.fla` as a skeleton: stage, frame rate, background, one empty scene and the
/// library's symbols as empty symbols of the right types.
pub fn import_legacy_bytes(bytes: &[u8]) -> Result<Imported, ImportError> {
    let legacy = read(bytes).map_err(|e| ImportError::NotXfl(e.to_string()))?;
    let Legacy {
        info,
        properties,
        library,
    } = legacy;
    let props = properties.map_err(|why| {
        let why = match why {
            PropsError::NotFound => "its document settings were not found".to_owned(),
            PropsError::OutOfRange(what) => format!("{what} is out of range"),
        };
        ImportError::NotXfl(format!(
            "{info}; {why}, and drawings, timelines and media are not readable yet"
        ))
    })?;

    let mut importer = crate::xfl::Importer::new();
    let mut symbols = Vec::new();
    for item in &library {
        let place = format!("{} \"{}\"", item.kind.label(), item.name);
        let kind = match item.kind {
            ItemKind::Graphic => SymbolKind::Graphic,
            ItemKind::MovieClip => SymbolKind::MovieClip,
            ItemKind::Button => SymbolKind::Button,
            ItemKind::Unknown => {
                importer.note(
                    Severity::Warning,
                    &item.stream,
                    "the type of this library item could not be read; it is imported as a graphic",
                );
                SymbolKind::Graphic
            }
            ItemKind::Bitmap | ItemKind::Sound | ItemKind::Video | ItemKind::OtherMedia => {
                importer.note(Severity::Unsupported, &place, "media is not read yet");
                continue;
            }
        };
        if symbols.len() >= crate::xfl::MAX_SYMBOLS {
            return Err(ImportError::TooManySymbols);
        }
        symbols.push(
            Symbol::new(SymbolId::new(), item.name.clone(), Timeline::default()).with_kind(kind),
        );
    }
    if info.pages > 1 {
        importer.note(
            Severity::Warning,
            "import",
            format!(
                "the file has {} scenes; only one empty scene was made",
                info.pages
            ),
        );
    }
    importer.note(
        Severity::Warning,
        "import",
        "this is a legacy binary FLA: only the stage, frame rate, background and the library's \
         symbol names and types were read; drawings, timelines, text, media and scripts are not read yet",
    );

    let rate = frame_rate(&format!("{}", props.frame_rate)).map_err(|message| {
        ImportError::BadAttribute {
            what: "frame rate".into(),
            message,
        }
    })?;
    let [r, g, b] = props.background;
    let stage = Stage::new(props.width, props.height, Rgba8::new(r, g, b, 255)).map_err(|e| {
        ImportError::BadAttribute {
            what: "stage".into(),
            message: e.to_string(),
        }
    })?;
    let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::default());
    let document = Document::new(DocumentId::new(), rate, vec![scene], symbols)
        .map_err(ImportError::Invalid)?
        .with_stage(stage);
    if importer.suppressed > 0 {
        let message = format!(
            "{} further diagnostics were suppressed",
            importer.suppressed
        );
        importer.diagnostics.push(crate::xfl::Diagnostic {
            severity: Severity::Info,
            location: "import".into(),
            message,
        });
    }
    Ok(Imported {
        document,
        diagnostics: importer.diagnostics,
    })
}
