//! Import of Flash movies (`.swf`): the published form of a Flash project, read back into a
//! document so it can be edited.
//!
//! A movie is a flat list of characters (shapes and sprites) and a display list that places
//! them. That maps onto the document as follows. Every shape character becomes a Graphic symbol
//! and every sprite a Movie Clip symbol with a timeline of its own; the main timeline becomes the
//! one scene, with a layer for each depth, highest on top. SWF has no tweens (the file holds the
//! position of each object on each frame), so each frame an object moves on is a keyframe.
//!
//! Imported: stage size, frame rate and background; solid fills and solid strokes of shapes;
//! sprites and their timelines; placements with their matrices; names given by Export Assets.
//! Reported and skipped: gradient and picture fills (a gradient takes its first color), text,
//! buttons, pictures, video, sound, scripts, color effects, filters, blend modes and masks.

use crate::xfl::{
    Diagnostic, ImportError, Imported, Importer, MAX_DIAGNOSTICS, MAX_FILE_BYTES, MAX_SYMBOLS,
    Severity, frame_rate,
};
use flare_core::{DocumentId, Rgba8, SceneId, SymbolId};
use flare_document::{Document, MAX_STAGE_SIDE, Scene, Stage, Symbol};
use std::collections::HashMap;

mod shapes;
mod timeline;

/// The most a movie may expand to once decompressed. A file claims its own size in its header, so
/// the claim is checked before anything is allocated for it.
const MAX_UNCOMPRESSED_BYTES: u32 = 256 * 1024 * 1024;

/// The most keyframes (spans) all the timelines of one movie may hold.
const MAX_SPANS: usize = 500_000;

/// Whether `bytes` starts like a Flash movie: uncompressed, zlib or LZMA.
pub fn looks_like_swf(bytes: &[u8]) -> bool {
    matches!(bytes.get(..3), Some(b"FWS" | b"CWS" | b"ZWS"))
}

struct Movie {
    importer: Importer,
    version: u8,
    /// The symbol made for each character defined so far.
    defined: HashMap<swf::CharacterId, SymbolId>,
    names: HashMap<swf::CharacterId, String>,
    symbols: Vec<Symbol>,
    spans: usize,
}

/// Imports a Flash movie from its bytes.
pub fn import_swf_bytes(bytes: &[u8]) -> Result<Imported, ImportError> {
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge("the movie".into()));
    }
    if !looks_like_swf(bytes) {
        return Err(ImportError::NotSwf(
            "it does not start like a Flash movie".into(),
        ));
    }
    // Bytes 4..8 are the length once decompressed, which the reader would allocate for.
    let claimed = bytes
        .get(4..8)
        .and_then(|b| <[u8; 4]>::try_from(b).ok())
        .map_or(0, u32::from_le_bytes);
    if claimed > MAX_UNCOMPRESSED_BYTES {
        return Err(ImportError::TooLarge("the movie once decompressed".into()));
    }
    let buf = swf::decompress_swf(bytes).map_err(|e| ImportError::NotSwf(e.to_string()))?;
    let movie = swf::parse_swf(&buf).map_err(|e| ImportError::NotSwf(e.to_string()))?;
    let version = movie.header.version();
    let mut importer = Movie {
        importer: Importer::new(),
        version,
        defined: HashMap::new(),
        names: HashMap::new(),
        symbols: Vec::new(),
        spans: 0,
    };
    importer.collect_names(&movie.tags);
    let timeline = importer.timeline(&movie.tags, u32::from(movie.header.num_frames()), "movie")?;

    let fps = movie.header.frame_rate().to_f64();
    let rate = frame_rate(&format!("{fps}")).map_err(|message| ImportError::BadAttribute {
        what: "frame rate".into(),
        message,
    })?;
    let size = movie.header.stage_size();
    let side = |low: swf::Twips, high: swf::Twips| {
        let pixels = (high - low).to_pixels().round();
        if pixels.is_finite() && pixels >= 1.0 && pixels <= f64::from(MAX_STAGE_SIDE) {
            Ok(pixels as u32)
        } else {
            Err(ImportError::BadAttribute {
                what: "stage".into(),
                message: format!("{pixels} pixels is outside the supported stage size"),
            })
        }
    };
    let background = movie
        .header
        .background_color()
        .map_or(Rgba8::WHITE, |c| Rgba8::new(c.r, c.g, c.b, 255));
    let stage = Stage::new(
        side(size.x_min, size.x_max)?,
        side(size.y_min, size.y_max)?,
        background,
    )
    .map_err(|e| ImportError::BadAttribute {
        what: "stage".into(),
        message: e.to_string(),
    })?;

    let scene = Scene::new(SceneId::new(), "Scene 1", timeline);
    let Movie {
        mut importer,
        symbols,
        ..
    } = importer;
    let document = Document::new(DocumentId::new(), rate, vec![scene], symbols)
        .map_err(ImportError::Invalid)?
        .with_stage(stage);
    if importer.suppressed > 0 {
        let message = format!(
            "{} further diagnostics were suppressed",
            importer.suppressed
        );
        importer.diagnostics.push(Diagnostic {
            severity: Severity::Info,
            location: "import".into(),
            message,
        });
    }
    debug_assert!(importer.diagnostics.len() <= MAX_DIAGNOSTICS + 1);
    Ok(Imported {
        document,
        diagnostics: importer.diagnostics,
    })
}

impl Movie {
    fn note(&mut self, severity: Severity, location: &str, message: impl Into<String>) {
        self.importer.note(severity, location, message);
    }

    /// Reads the names the movie gives its characters, ahead of the characters themselves.
    fn collect_names(&mut self, tags: &[swf::Tag<'_>]) {
        for tag in tags {
            if let swf::Tag::ExportAssets(assets) = tag {
                for asset in assets {
                    let name = asset
                        .name
                        .to_string_lossy(swf::SwfStr::encoding_for_version(self.version));
                    if !name.trim().is_empty() {
                        self.names.entry(asset.id).or_insert(name);
                    }
                }
            }
        }
    }

    fn symbol_count_ok(&self) -> Result<(), ImportError> {
        if self.symbols.len() >= MAX_SYMBOLS {
            Err(ImportError::TooManySymbols)
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
