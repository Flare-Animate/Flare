//! Maps an XFL document (DOMDocument.xml plus LIBRARY/ items) onto a Flare document.
//!
//! The importer is deliberately explicit about its subset. Anything it cannot represent is
//! reported as a [`Diagnostic`] rather than silently dropped or guessed, and there is no
//! round-trip preservation of foreign data yet: what is not imported is lost on save.
//!
//! Imported: stage size, frame rate and background; scenes; normal layers with name,
//! visibility and lock; keyframes and their durations; symbol instances with their matrix;
//! solid-color fills and solid strokes of drawn shapes (lines and quadratic curves);
//! the library as Graphic symbols; classic motion tweens between instances.
//! Reported and skipped: gradients, bitmaps, text, video, sound, scripts, filters, color
//! effects, masks, guides, folders, primitives, shape tweens, 3D, and symbol runtime
//! behaviour (movie clips and buttons become Graphic symbols).

use crate::xml::{Node, XmlError};
use flare_core::{FrameRate, LayerId, Rgba8, SymbolId};
use flare_document::{Anchor, Attachment, Document, Placement};
use std::collections::HashMap;
use std::io::{self, Read};
use thiserror::Error;

mod elements;
mod entry;
mod shapes;
mod timeline;

pub use entry::import_xfl;
#[allow(unused_imports)]
use entry::*;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;

const MAX_SYMBOLS: usize = 10_000;

const MAX_DIAGNOSTICS: usize = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Imported faithfully but worth knowing about (for example elements split over layers).
    Info,
    /// Imported with a change in meaning.
    Warning,
    /// Not imported.
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

#[derive(Debug)]
pub struct Imported {
    pub document: Document,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Error)]
pub enum ImportError {
    #[error("not an XFL document: {0}")]
    NotXfl(String),
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("{file}: {error}")]
    Xml { file: String, error: XmlError },
    #[error("{0} is larger than the {MAX_FILE_BYTES} byte limit")]
    TooLarge(String),
    #[error("the document has no timelines")]
    NoTimelines,
    #[error("the library has more than {MAX_SYMBOLS} items")]
    TooManySymbols,
    #[error("invalid {what}: {message}")]
    BadAttribute { what: String, message: String },
    #[error("the imported document is invalid: {0:?}")]
    Invalid(Vec<flare_document::Diagnostic>),
}

/// Where XFL files are read from. Names use `/` and are relative to the document root,
/// like `DOMDocument.xml` or `LIBRARY/Symbol 1.xml`.
pub trait FileSource {
    /// `Ok(None)` means the file does not exist.
    fn read(&self, name: &str) -> io::Result<Option<Vec<u8>>>;
}

/// Reads at most `MAX_FILE_BYTES + 1` bytes so an oversized file is detected without being
/// loaded in full.
pub(crate) fn read_capped(reader: &mut impl io::Read, name: &str) -> Result<Vec<u8>, ImportError> {
    let mut bytes = Vec::new();
    reader.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(ImportError::TooLarge(name.to_owned()));
    }
    Ok(bytes)
}

// ---- small attribute helpers ----------------------------------------------------------

fn color_from(text: &str, alpha: Option<&str>) -> Option<Rgba8> {
    let hex = text.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    let alpha = match alpha {
        None => 255,
        Some(a) => {
            let a: f64 = a.parse().ok().filter(|v: &f64| v.is_finite())?;
            (a.clamp(0.0, 1.0) * 255.0).round() as u8
        }
    };
    Some(Rgba8::new(byte(0)?, byte(2)?, byte(4)?, alpha))
}

fn bool_attr(node: &Node, key: &str, default: bool) -> bool {
    node.attr(key)
        .map_or(default, |v| v.eq_ignore_ascii_case("true"))
}

/// A frame rate as an exact rational: NTSC rates stay exact, others become n/1000.
pub fn frame_rate(text: &str) -> Result<FrameRate, String> {
    let rate: f64 = text
        .parse()
        .map_err(|_| format!("frame rate {text:?} is not a number"))?;
    if !rate.is_finite() || rate <= 0.0 || rate > 1000.0 {
        return Err(format!("frame rate {text:?} is out of range"));
    }
    for (num, den) in [(24_000_u32, 1001_u32), (30_000, 1001), (60_000, 1001)] {
        if (rate - f64::from(num) / f64::from(den)).abs() < 0.005 {
            return FrameRate::new(num, den).map_err(|e| e.to_string());
        }
    }
    FrameRate::new((rate * 1000.0).round() as u32, 1000).map_err(|e| e.to_string())
}

fn stage_side(node: &Node, key: &str, default: u32) -> Result<u32, ImportError> {
    let Some(text) = node.attr(key) else {
        return Ok(default);
    };
    let value: f64 = text.parse().map_err(|_| ImportError::BadAttribute {
        what: key.to_owned(),
        message: format!("{text:?} is not a number"),
    })?;
    if !value.is_finite() || value < 1.0 || value > f64::from(flare_document::MAX_STAGE_SIDE) {
        return Err(ImportError::BadAttribute {
            what: key.to_owned(),
            message: format!("{text:?} is outside the supported stage size"),
        });
    }
    Ok(value.round() as u32)
}

// ---- the importer ---------------------------------------------------------------------

/// Where an element being imported sits, so what cannot be imported can be kept and later
/// written back in the same place.
#[derive(Clone, Copy)]
struct Spot {
    layer: LayerId,
    frame: u32,
}

struct Importer {
    symbols: HashMap<String, SymbolId>,
    diagnostics: Vec<Diagnostic>,
    suppressed: usize,
    /// Source text that has no Flare equivalent, kept for export.
    kept: Vec<Attachment>,
}

impl Importer {
    /// Keeps `xml` as source data anchored to `anchor`. Too-large items are reported, not kept.
    fn keep(&mut self, anchor: Anchor, placement: Placement, xml: String, place: &str) {
        match Attachment::new("xfl", anchor, placement, xml) {
            Ok(attachment) => self.kept.push(attachment),
            Err(error) => self.note(
                Severity::Unsupported,
                place,
                format!("could not be kept as source data ({error})"),
            ),
        }
    }

    fn note(&mut self, severity: Severity, location: &str, message: impl Into<String>) {
        let diagnostic = Diagnostic {
            severity,
            location: location.to_owned(),
            message: message.into(),
        };
        if self.diagnostics.contains(&diagnostic) {
            return;
        }
        if self.diagnostics.len() >= MAX_DIAGNOSTICS {
            self.suppressed += 1;
        } else {
            self.diagnostics.push(diagnostic);
        }
    }
}
