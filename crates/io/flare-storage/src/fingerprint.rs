//! A fingerprint of a document, in parts, taken over the same fields the project file stores.
//!
//! Two documents with the same fingerprint save to the same records, so an exporter that only
//! reads what the project file keeps sees no difference between them. The fingerprint is how an
//! import is told apart from an edit of it: it is taken right after the import and kept with the
//! source, and a document whose fingerprint is that one again (an edit that was undone, a project
//! saved and opened) is unchanged. It ignores the file the document came from.
//!
//! It is split so an edit can be located: the symbols' own timelines (with the source data hung
//! off them) one by one, and everything else together.

use std::collections::HashMap;

use flare_core::{LayerId, SymbolId};
use flare_document::{Anchor, Baseline, Document, SymbolBaseline};

use crate::StorageError;
use crate::codec::to_cbor;
use crate::wire;

/// The parts of a document's fingerprint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fingerprint {
    /// Settings, scenes, sounds, the symbols' names and kinds, and the source data hung off all
    /// of these but the symbols' timelines.
    pub main: [u8; 32],
    /// Each symbol with everything in it, in library order.
    pub symbols: Vec<(SymbolId, [u8; 32])>,
}

fn framed(hasher: &mut blake3::Hasher, tag: u8, bytes: &[u8]) {
    hasher.update(&[tag]);
    hasher.update(&(bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

/// The fingerprint of `document`.
pub fn fingerprint(document: &Document) -> Result<Fingerprint, StorageError> {
    // Which symbol each layer belongs to; a layer of a scene is not in the map.
    let mut owner: HashMap<LayerId, SymbolId> = HashMap::new();
    for symbol in document.symbols() {
        for layer in symbol.timeline().layers() {
            owner.insert(layer.id(), symbol.id());
        }
    }
    let owner_of = |anchor: Anchor| match anchor {
        Anchor::Symbol(id) => Some(id),
        Anchor::Layer(layer) => owner.get(&layer).copied(),
        Anchor::Document | Anchor::Scene(_) => None,
    };

    let mut main = blake3::Hasher::new();
    framed(
        &mut main,
        b's',
        &serde_json::to_vec(&wire::Settings::of(document))?,
    );
    for scene in document.scenes() {
        framed(&mut main, b'i', scene.id().as_uuid().as_bytes());
        framed(&mut main, b'c', &to_cbor(&wire::SceneWire::of(scene))?);
    }
    for sound in document.sounds() {
        framed(&mut main, b'i', sound.id().as_uuid().as_bytes());
        framed(&mut main, b'u', &wire::SoundMeta::frame(sound)?);
    }
    for bitmap in document.bitmaps() {
        framed(&mut main, b'i', bitmap.id().as_uuid().as_bytes());
        framed(&mut main, b'h', bitmap.content_hash());
    }
    for symbol in document.symbols() {
        framed(&mut main, b'i', symbol.id().as_uuid().as_bytes());
        framed(&mut main, b'n', symbol.name().as_bytes());
        framed(&mut main, b'k', symbol.kind().name().as_bytes());
    }

    let mut symbol_hashers: HashMap<SymbolId, blake3::Hasher> = HashMap::new();
    for symbol in document.symbols() {
        let mut hasher = blake3::Hasher::new();
        framed(&mut hasher, b'c', &to_cbor(&wire::SymbolWire::of(symbol))?);
        symbol_hashers.insert(symbol.id(), hasher);
    }
    for attachment in document.attachments() {
        let raw = to_cbor(&wire::AttachmentWire::of(attachment))?;
        match owner_of(attachment.anchor()).and_then(|id| symbol_hashers.get_mut(&id)) {
            Some(hasher) => framed(hasher, b'a', &raw),
            None => framed(&mut main, b'a', &raw),
        }
    }
    Ok(Fingerprint {
        main: *main.finalize().as_bytes(),
        symbols: document
            .symbols()
            .iter()
            .filter_map(|symbol| {
                symbol_hashers
                    .get(&symbol.id())
                    .map(|h| (symbol.id(), *h.finalize().as_bytes()))
            })
            .collect(),
    })
}

/// The baseline for a document just imported, with the file each symbol came from.
pub fn baseline(
    document: &Document,
    mut file_of: impl FnMut(SymbolId) -> Option<String>,
) -> Result<Baseline, StorageError> {
    let print = fingerprint(document)?;
    Ok(Baseline {
        main: print.main,
        symbols: print
            .symbols
            .into_iter()
            .map(|(id, digest)| SymbolBaseline {
                id,
                name: document
                    .symbol(id)
                    .map(|s| s.name().to_owned())
                    .unwrap_or_default(),
                digest,
                file: file_of(id).unwrap_or_default(),
            })
            .collect(),
    })
}

impl Fingerprint {
    /// Whether this is the fingerprint `baseline` was taken from: the whole document is as the
    /// import made it. Adding, removing or reordering a symbol is a change.
    pub fn matches(&self, baseline: &Baseline) -> bool {
        self.main == baseline.main
            && self.symbols.len() == baseline.symbols.len()
            && self
                .symbols
                .iter()
                .zip(&baseline.symbols)
                .all(|((id, digest), base)| *id == base.id && *digest == base.digest)
    }

    /// Whether the symbol `id` is as it was imported.
    pub fn symbol_unchanged(&self, baseline: &Baseline, id: SymbolId) -> bool {
        let now = self.symbols.iter().find(|(s, _)| *s == id).map(|(_, d)| d);
        let then = baseline
            .symbols
            .iter()
            .find(|s| s.id == id)
            .map(|s| &s.digest);
        now.is_some() && now == then
    }
}
