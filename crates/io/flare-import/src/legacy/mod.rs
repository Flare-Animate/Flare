//! Legacy binary `.fla` files (Flash MX 2004 to CS4): a Compound File Binary container with a
//! `Contents` stream (document settings and the library table) and one stream per scene, symbol
//! and media item.
//!
//! There is no official specification. What is read here was worked out from the format notes at
//! <https://github.com/eddiemoore/fla-decoder> and then checked against the genuine
//! legacy files in the vendored Ruffle test suite (format versions 0x129, 0x138, 0x13f and 0x147;
//! all small test movies, none a real project, none made by CS3 or CS4 as far as is known).
//!
//! Read: the container, stream names, stage properties, library table and recoverable layer
//! names and visibility/lock state.
//!
//! Not read: drawings, keyframe contents, text, tweens, sounds and bitmaps themselves, scripts,
//! fonts, library folders, and the names of scenes. Imported scenes and symbols retain their
//! recovered layer stacks, but no artwork or animation is fabricated.

mod cfb;
mod contents;
#[cfg(test)]
mod import_tests;
mod layers;
mod layout;
#[cfg(test)]
mod layout_tests;

use thiserror::Error;

use crate::xfl::{ImportError, Imported, Severity, frame_rate};
use flare_core::{DocumentId, Rgba8, SceneId, SymbolId};
use flare_document::{Document, Layer, MAX_STAGE_SIDE, Scene, Stage, Symbol, SymbolKind, Timeline};

pub use cfb::{CfbError, StreamInfo};
pub use contents::{DocumentProps, ItemKind, LibraryItem, PropsError};
pub use layers::{LayerError, LayerKind, LegacyLayer};
pub use layout::{Era, LegacyInfo};

/// The first eight bytes of every compound file.
pub const CFB_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// The most a legacy file may be, and so the most a caller should read for one.
pub const MAX_LEGACY_FILE_BYTES: u64 = cfb::MAX_FILE_BYTES;

/// The most `Contents` bytes read.
const MAX_CONTENTS_BYTES: u64 = 64 * 1024 * 1024;
/// Total bytes read from page and symbol streams to recover layer metadata.
const MAX_LAYER_STREAM_BYTES: u64 = 128 * 1024 * 1024;
const MAX_LAYER_STREAMS: usize = 4096;

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
    /// Recoverable layer lists by source stream name.
    pub layers: Vec<(String, Vec<LegacyLayer>)>,
    /// Streams skipped because their size or count exceeded the import budget, or they were malformed.
    pub layer_warnings: Vec<(String, String)>,
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
    let mut layers = Vec::new();
    let mut layer_warnings = Vec::new();
    let mut layer_bytes = 0_u64;
    for stream in container.streams().iter().filter(|stream| {
        matches!(
            layout::classify(&stream.name).map(|(kind, _)| kind),
            Some(layout::StreamKind::Page | layout::StreamKind::Symbol)
        )
    }) {
        if layers.len() + layer_warnings.len() >= MAX_LAYER_STREAMS {
            layer_warnings.push((
                stream.name.clone(),
                format!("layer metadata skipped after the {MAX_LAYER_STREAMS} stream limit"),
            ));
            break;
        }
        let Some(total) = layer_bytes.checked_add(stream.size) else {
            layer_warnings.push((stream.name.clone(), "layer stream size overflow".to_owned()));
            continue;
        };
        if stream.size > cfb::MAX_STREAM_BYTES || total > MAX_LAYER_STREAM_BYTES {
            layer_warnings.push((
                stream.name.clone(),
                format!("layer metadata skipped at the {MAX_LAYER_STREAM_BYTES} byte budget"),
            ));
            continue;
        }
        layer_bytes = total;
        match container.read(&stream.name) {
            Ok(bytes) => match layers::read(&bytes) {
                Ok(found) => layers.push((stream.name.clone(), found)),
                Err(error) => layer_warnings.push((stream.name.clone(), error.to_string())),
            },
            Err(error) => layer_warnings.push((stream.name.clone(), error.to_string())),
        }
    }
    Ok(Legacy {
        info,
        properties,
        library,
        layers,
        layer_warnings,
    })
}

/// What the file is, from its stream names and the version at the start of `Contents`.
pub fn inspect(bytes: &[u8]) -> Result<LegacyInfo, LegacyError> {
    read(bytes).map(|legacy| legacy.info)
}

/// Imports a legacy `.fla`'s stage, frame rate, background, library and recoverable layer stacks.
pub fn import_legacy_bytes(bytes: &[u8]) -> Result<Imported, ImportError> {
    let legacy = read(bytes).map_err(|e| ImportError::NotXfl(e.to_string()))?;
    let Legacy {
        info,
        properties,
        library,
        layers,
        layer_warnings,
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
    for (stream, warning) in layer_warnings {
        importer.note(Severity::Warning, &stream, warning);
    }
    let timeline_for = |stream: &str| -> Result<Timeline, ImportError> {
        let items: Vec<Layer> = layers
            .iter()
            .find(|(name, _)| name == stream)
            .map(|(_, layers)| layers.as_slice())
            .unwrap_or_default()
            .iter()
            .map(|source| {
                let layer = Layer::new(flare_core::LayerId::new(), source.name.clone(), Vec::new())
                    .map_err(|error| {
                        ImportError::NotXfl(format!(
                            "invalid layer in legacy stream {stream:?}: {error}"
                        ))
                    })?;
                Ok(match source.kind {
                    LayerKind::Normal => layer,
                    LayerKind::Guide => layer.with_guide(true),
                    LayerKind::Folder => layer.with_folder(true),
                }
                .with_locked(source.locked)
                .with_visible(source.visible))
            })
            .collect::<Result<_, ImportError>>()?;
        Ok(Timeline::new(items))
    };
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
            Symbol::new(
                SymbolId::new(),
                item.name.clone(),
                timeline_for(&item.stream)?,
            )
            .with_kind(kind),
        );
    }
    let mut page_layers: Vec<_> = layers
        .iter()
        .filter(|(stream, _)| {
            matches!(
                layout::classify(stream).map(|(kind, _)| kind),
                Some(layout::StreamKind::Page)
            )
        })
        .collect();
    page_layers.sort_by_key(|(stream, _)| {
        stream
            .split(' ')
            .nth(1)
            .and_then(|number| number.parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });
    let mut scenes: Vec<_> = page_layers
        .iter()
        .map(|(stream, _)| {
            let scene_number = stream
                .split(' ')
                .nth(1)
                .and_then(|number| number.parse::<u32>().ok())
                .unwrap_or(1);
            Ok(Scene::new(
                SceneId::new(),
                format!("Scene {scene_number}"),
                timeline_for(stream)?,
            ))
        })
        .collect::<Result<_, ImportError>>()?;
    if scenes.is_empty() {
        scenes.push(Scene::new(SceneId::new(), "Scene 1", Timeline::default()));
    }
    if scenes.len() < info.pages {
        importer.note(
            Severity::Warning,
            "import",
            format!(
                "layer metadata was recovered for {} of {} scene streams",
                scenes.len(),
                info.pages
            ),
        );
    }
    importer.note(
        Severity::Warning,
        "import",
        "this is a legacy binary FLA: layer names and basic state were recovered, but artwork, keyframe contents, \
         text, media and scripts are not read yet",
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
    let document = Document::new(DocumentId::new(), rate, scenes, symbols)
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
