//! Writing back what a document was imported from.
//!
//! Flare turns a `.fla` or an XFL folder into a document, which cannot hold everything in the
//! file. The import keeps the file itself ([`flare_document::Origin`]) with a fingerprint of the
//! document it made, and an export uses both:
//!
//! - The document is still what the import made (its fingerprint is the baseline, so an edit that
//!   was undone counts): the original is written back byte for byte. A `.fla` becomes the same
//!   file, a folder the same files.
//! - The document was edited: Flare rebuilds `DOMDocument.xml` and the library items it must, and
//!   every part of the original it did not change is written as it was: the files that Flare never
//!   models (bitmaps, sounds, publish settings, metadata), the main document when only a symbol
//!   changed, and each symbol that was not touched. What cannot be carried is reported.
//!
//! This module makes the plan; the writers in `writer` put it on disk.

pub(crate) mod entries;

use std::collections::{HashMap, HashSet};

use flare_core::SymbolId;
use flare_document::{Document, OriginKind, SharedOrigin};

use crate::writer::{Diagnostic, ExportError, Output, Severity, build_xfl};
use entries::entries_of;

/// Whether an export is the original, and if not, how it differs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lossless {
    /// The output is the file the project was imported from, byte for byte.
    pub exact: bool,
    /// Why, in a sentence.
    pub reason: String,
}

impl Lossless {
    pub(crate) fn exact(reason: impl Into<String>) -> Self {
        Self {
            exact: true,
            reason: reason.into(),
        }
    }

    pub(crate) fn rebuilt(reason: impl Into<String>) -> Self {
        Self {
            exact: false,
            reason: reason.into(),
        }
    }
}

/// The name of the library entry for a symbol's file, which the XFL spec puts under `LIBRARY/`.
const LIBRARY: &str = "LIBRARY/";
const MAIN: &str = "DOMDocument.xml";

fn info(location: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Info,
        location: location.into(),
        message: message.into(),
    }
}

fn warning(location: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic {
        severity: Severity::Warning,
        location: location.into(),
        message: message.into(),
    }
}

/// The Flash movie `document` was imported from, when the document has not been changed since.
pub fn unchanged_movie(document: &Document) -> Option<SharedOrigin> {
    let origin = document.origin()?;
    if origin.kind() != OriginKind::Swf {
        return None;
    }
    let print = flare_storage::fingerprint(document).ok()?;
    print.matches(origin.baseline()).then(|| origin.clone())
}

/// [`crate::export_xfl`].
pub(crate) fn export(document: &Document) -> Result<Output, ExportError> {
    let Some(origin) = document.origin() else {
        return build_xfl(document, &HashMap::new());
    };
    if origin.kind() == OriginKind::Swf {
        let mut output = build_xfl(document, &HashMap::new())?;
        output.lossless = Lossless::rebuilt(
            "the project was imported from a Flash movie (.swf), which is not an XFL or FLA file; the library and timelines are rebuilt from what Flare understood of it",
        );
        return Ok(output);
    }
    let print = match flare_storage::fingerprint(document) {
        Ok(print) => print,
        Err(error) => {
            let mut output = build_xfl(document, &HashMap::new())?;
            output.lossless = Lossless::rebuilt(format!(
                "the project could not be compared with the file it was imported from ({error})"
            ));
            return Ok(output);
        }
    };
    let baseline = origin.baseline();
    if print.matches(baseline) {
        return Ok(exact(origin));
    }
    edited(document, origin, &print)
}

/// The output that is the original itself.
fn exact(origin: &SharedOrigin) -> Output {
    let what = match origin.kind() {
        OriginKind::Fla => "the .fla",
        _ => "the XFL folder",
    };
    Output {
        name: origin.name().to_owned(),
        files: Vec::new(),
        kept: Vec::new(),
        order: Vec::new(),
        exact: Some(origin.clone()),
        from_folder: origin.kind() == OriginKind::Xfl,
        lossless: Lossless::exact(format!(
            "the project is unchanged since it was imported, so {what} is written back as it was read"
        )),
        diagnostics: vec![info(
            "source",
            format!("unchanged since import: {what} is written back byte for byte"),
        )],
    }
}

/// The export of a document edited since the import, with everything unchanged carried over.
fn edited(
    document: &Document,
    origin: &SharedOrigin,
    print: &flare_storage::Fingerprint,
) -> Result<Output, ExportError> {
    let baseline = origin.baseline();
    // A symbol keeps the library file it came from while its name is the one it was imported
    // with, so a main document that still lists that file stays valid.
    let mut reuse: HashMap<SymbolId, String> = HashMap::new();
    for symbol in document.symbols() {
        if let Some(base) = baseline.symbols.iter().find(|b| b.id == symbol.id())
            && base.name == symbol.name()
            && let Some(bare) = base.file.strip_prefix(LIBRARY)
            && !bare.is_empty()
        {
            reuse.insert(symbol.id(), bare.to_owned());
        }
    }
    let mut output = build_xfl(document, &reuse)?;
    let entries = entries_of(origin);
    let mut originals: HashMap<&str, &[u8]> = HashMap::new();
    for file in &entries.files {
        originals.insert(file.name.as_str(), file.bytes.as_slice());
    }

    let mut kept: Vec<(String, Vec<u8>)> = Vec::new();
    let mut reused: Vec<String> = Vec::new();

    // The main document, as it was, when nothing it holds changed and every library file it lists
    // is written under the name it lists.
    let all_reused = document
        .symbols()
        .iter()
        .all(|s| reuse.contains_key(&s.id()));
    let main_same = print.main == baseline.main && all_reused;
    if main_same && let Some(bytes) = originals.get(MAIN) {
        output.files.retain(|(name, _)| name != MAIN);
        kept.push((MAIN.to_owned(), bytes.to_vec()));
        reused.push(MAIN.to_owned());
    }
    // Each library item that was not touched, as it was.
    for symbol in document.symbols() {
        let Some(bare) = reuse.get(&symbol.id()) else {
            continue;
        };
        let entry = format!("{LIBRARY}{bare}");
        if print.symbol_unchanged(baseline, symbol.id())
            && let Some(bytes) = originals.get(entry.as_str())
        {
            output.files.retain(|(name, _)| *name != entry);
            kept.push((entry.clone(), bytes.to_vec()));
            reused.push(entry);
        }
    }
    // What is left of the generated files is what was rebuilt.
    let rebuilt: Vec<String> = output.files.iter().map(|(n, _)| n.clone()).collect();

    // Everything else in the original: carried over untouched, unless Flare owns it.
    let owned: HashSet<&str> = std::iter::once(MAIN)
        .chain(baseline.symbols.iter().map(|b| b.file.as_str()))
        .collect();
    let written: HashSet<String> = output
        .files
        .iter()
        .map(|(n, _)| n.clone())
        .chain(kept.iter().map(|(n, _)| n.clone()))
        .collect();
    let mut carried = 0_usize;
    let mut dropped: Vec<String> = Vec::new();
    for file in &entries.files {
        if written.contains(&file.name) {
            continue;
        }
        if owned.contains(file.name.as_str()) {
            // A library item of a symbol that was deleted or renamed, or a main document that
            // was rebuilt under the same name (handled above): Flare no longer has it.
            dropped.push(file.name.clone());
            continue;
        }
        carried += 1;
        kept.push((file.name.clone(), file.bytes.clone()));
    }
    output.order = entries.files.iter().map(|f| f.name.clone()).collect();
    output.kept = kept;
    output.from_folder = origin.kind() == OriginKind::Xfl;

    let mut notes = Vec::new();
    if !dropped.is_empty() {
        notes.push(info(
            "source",
            format!(
                "{} original library file(s) belonged to symbols that were deleted or renamed and were not written: {}",
                dropped.len(),
                list(&dropped)
            ),
        ));
    }
    for why in &entries.skipped {
        notes.push(warning(
            "source",
            format!("not carried over from the original: {why}"),
        ));
    }
    notes.push(info(
        "source",
        format!(
            "edited since import: {} file(s) rebuilt ({}), {} written as they were ({}), {} other original file(s) carried over untouched",
            rebuilt.len(),
            list(&rebuilt),
            reused.len(),
            list(&reused),
            carried
        ),
    ));
    if !rebuilt.is_empty() {
        notes.push(warning(
            "source",
            format!(
                "rebuilt from the document, so anything in them that Flare does not model and did not keep as source data is gone: {}",
                list(&rebuilt)
            ),
        ));
    }
    output.diagnostics.extend(notes);
    output.lossless = Lossless::rebuilt(format!(
        "the project was edited since it was imported; {} file(s) are rebuilt ({}) and the other {} are the original's, byte for byte{}",
        rebuilt.len(),
        list(&rebuilt),
        reused.len() + carried,
        if entries.skipped.is_empty() {
            String::new()
        } else {
            format!(
                "; {} original entr{} could not be carried over",
                entries.skipped.len(),
                if entries.skipped.len() == 1 {
                    "y"
                } else {
                    "ies"
                }
            )
        }
    ));
    Ok(output)
}

/// A few names, then a count.
fn list(names: &[String]) -> String {
    const SHOWN: usize = 3;
    let mut text = names
        .iter()
        .take(SHOWN)
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join(", ");
    if names.len() > SHOWN {
        text.push_str(&format!(" and {} more", names.len() - SHOWN));
    }
    if text.is_empty() { "none".into() } else { text }
}
