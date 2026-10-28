//! Dependency transfer for the in-process frame/layer clipboard. A copied bundle
//! owns its source assets, so opening another document cannot strand references.
use super::*;
use ac_core::{Document, ElementKind, Layer};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone)]
pub(super) struct Source {
    session: u64,
    doc: Document,
}

impl Studio {
    pub(super) fn clipboard_source(&self) -> Source {
        Source {
            session: self.session,
            doc: self.editor.doc.with_floats_committed(),
        }
    }

    pub(super) fn prepare_clipboard(
        &self,
        source: &Source,
        payload: &[Layer],
    ) -> Option<(Document, Vec<Layer>)> {
        let same = source.session == self.session;
        let src = if same { &self.editor.doc } else { &source.doc };
        let mut transfer = Transfer {
            src,
            doc: self.editor.doc.clone(),
            same,
            target: self.editor.editing,
            symbols: BTreeMap::new(),
            bitmaps: BTreeMap::new(),
            visiting: BTreeSet::new(),
        };
        let mut payload = payload.to_vec();
        transfer.layers(&mut payload)?;
        // Deduplication may reuse destination symbols. Recheck the final graph,
        // including their descendants, against the timeline receiving this paste.
        let mut validate = Transfer {
            src: &transfer.doc,
            doc: transfer.doc.clone(),
            same: true,
            target: self.editor.editing,
            symbols: BTreeMap::new(),
            bitmaps: BTreeMap::new(),
            visiting: BTreeSet::new(),
        };
        validate.layers(&mut payload)?;
        if transfer.doc.library.len() > 256 || transfer.doc.bitmaps.len() > 256 {
            return None;
        }
        // Preserve the file loader's round-trip bound, including transferred assets.
        if serde_json::to_vec(&transfer.doc).ok()?.len() + serde_json::to_vec(&payload).ok()?.len()
            > 16 * 1024 * 1024
        {
            return None;
        }
        Some((transfer.doc, payload))
    }
}

struct Transfer<'a> {
    src: &'a Document,
    doc: Document,
    same: bool,
    target: Option<u32>,
    symbols: BTreeMap<u32, u32>,
    bitmaps: BTreeMap<u32, u32>,
    visiting: BTreeSet<u32>,
}
impl Transfer<'_> {
    fn layers(&mut self, layers: &mut [Layer]) -> Option<()> {
        for l in layers {
            for k in l.keyframes.values_mut() {
                self.elements(&mut k.elements)?;
                if let Some(boundary) = &mut k.tween_boundary {
                    self.elements(&mut boundary.elements)?;
                }
            }
        }
        Some(())
    }
    fn elements(&mut self, elements: &mut [Element]) -> Option<()> {
        for e in elements {
            match &mut e.kind {
                ElementKind::Instance(i) => i.symbol = self.symbol(i.symbol)?,
                ElementKind::Bitmap(b) => b.bitmap = self.bitmap(b.bitmap)?,
                ElementKind::Shape(s) => {
                    if let Some(b) = &mut s.bitmap_fill {
                        b.bitmap = self.bitmap(b.bitmap)?;
                    }
                }
                ElementKind::Group(children) => self.elements(children)?,
                _ => {}
            }
        }
        Some(())
    }
    fn bitmap(&mut self, id: u32) -> Option<u32> {
        if let Some(&id) = self.bitmaps.get(&id) {
            return Some(id);
        }
        let asset = self.src.bitmap(id)?;
        if self.same {
            return Some(id);
        }
        let mut copy = asset.clone();
        // Repeated paste reuses identical destination assets, never an ID collision.
        let equal = self.doc.bitmaps.iter().find(|b| {
            b.name == copy.name
                && b.width == copy.width
                && b.height == copy.height
                && b.smoothing == copy.smoothing
                && b.png == copy.png
        });
        let new = if let Some(b) = equal {
            b.id
        } else {
            copy.id = self
                .doc
                .bitmaps
                .iter()
                .map(|b| b.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)?;
            let new = copy.id;
            self.doc.bitmaps.push(copy);
            new
        };
        self.bitmaps.insert(id, new);
        Some(new)
    }
    fn symbol(&mut self, id: u32) -> Option<u32> {
        if self.visiting.contains(&id) || (self.same && self.target == Some(id)) {
            return None;
        }
        if let Some(&new) = self.symbols.get(&id) {
            return Some(new);
        }
        let mut symbol = self.src.symbol(id)?.clone();
        self.visiting.insert(id);
        self.layers(&mut symbol.timeline.layers)?;
        self.visiting.remove(&id);
        let new = if self.same {
            id
        } else if let Some(s) = self.doc.library.iter().find(|s| {
            s.name == symbol.name
                && s.kind == symbol.kind
                && s.timeline == symbol.timeline
                && Some(s.id) != self.target
        }) {
            s.id
        } else {
            symbol.id = self
                .doc
                .library
                .iter()
                .map(|s| s.id)
                .max()
                .unwrap_or(0)
                .max(self.doc.next_symbol_id.saturating_sub(1))
                .checked_add(1)?;
            self.doc.next_symbol_id = symbol.id.checked_add(1)?;
            let new = symbol.id;
            self.doc.library.push(symbol);
            new
        };
        self.symbols.insert(id, new);
        Some(new)
    }
}
