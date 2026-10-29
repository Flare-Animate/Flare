use std::collections::{HashMap, HashSet};

use flare_core::{InstanceId, LayerId, SceneId, ShapeId, SoundId, SymbolId};
use thiserror::Error;

use crate::{Anchor, Document, MAX_ATTACHMENTS, MAX_SOUNDS, MAX_TOTAL_SOUND_BYTES, Timeline};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Diagnostic {
    #[error("duplicate scene id {0:?}")]
    DuplicateScene(SceneId),
    #[error("duplicate symbol id {0:?}")]
    DuplicateSymbol(SymbolId),
    #[error("duplicate layer id {0:?}")]
    DuplicateLayer(LayerId),
    #[error("duplicate instance id {0:?}")]
    DuplicateInstance(InstanceId),
    #[error("duplicate shape id {0:?}")]
    DuplicateShape(ShapeId),
    #[error("instance {instance:?} references missing symbol {symbol:?}")]
    DanglingSymbol {
        instance: InstanceId,
        symbol: SymbolId,
    },
    #[error("symbol {0:?} contains itself, directly or indirectly")]
    SymbolCycle(SymbolId),
    #[error("an attachment is anchored to {0:?}, which does not exist")]
    DanglingAttachment(Anchor),
    #[error("the document has more than {MAX_ATTACHMENTS} attachments")]
    TooManyAttachments,
    #[error("duplicate sound id {0:?}")]
    DuplicateSound(SoundId),
    #[error("the document has more than {MAX_SOUNDS} sounds")]
    TooManySounds,
    #[error("the sounds together are larger than {MAX_TOTAL_SOUND_BYTES} bytes")]
    SoundsTooLarge,
    #[error("layer {layer:?} is in {parent:?}, which is not a folder above it")]
    BadFolderParent { layer: LayerId, parent: LayerId },
    #[error("folder {0:?} has frames; a folder only groups other layers")]
    FolderWithContent(LayerId),
    #[error("layer {0:?} is a motion guide and also a mask or a folder")]
    GuideConflict(LayerId),
    #[error("a cue on layer {layer:?} plays sound {sound:?}, which is not in the document")]
    DanglingSound { layer: LayerId, sound: SoundId },
}

/// Every entity an attachment can hang off.
pub(crate) fn anchors(document: &Document) -> HashSet<Anchor> {
    let mut set = HashSet::from([Anchor::Document]);
    set.extend(document.scenes.iter().map(|s| Anchor::Scene(s.id)));
    set.extend(document.symbols.iter().map(|s| Anchor::Symbol(s.id)));
    for timeline in timelines(document) {
        set.extend(timeline.layers().iter().map(|l| Anchor::Layer(l.id())));
    }
    set
}

pub(crate) fn validate_attachments(document: &Document) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if document.attachments.len() > MAX_ATTACHMENTS {
        out.push(Diagnostic::TooManyAttachments);
    }
    let live = anchors(document);
    for attachment in &document.attachments {
        if !live.contains(&attachment.anchor()) {
            out.push(Diagnostic::DanglingAttachment(attachment.anchor()));
        }
    }
    out
}

pub(crate) fn validate_sounds(document: &Document) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if document.sounds.len() > MAX_SOUNDS {
        out.push(Diagnostic::TooManySounds);
    }
    let total: usize = document.sounds.iter().map(|s| s.data().len()).sum();
    if total > MAX_TOTAL_SOUND_BYTES {
        out.push(Diagnostic::SoundsTooLarge);
    }
    let mut ids = HashSet::with_capacity(document.sounds.len());
    for sound in &document.sounds {
        if !ids.insert(sound.id()) {
            out.push(Diagnostic::DuplicateSound(sound.id()));
        }
    }
    for timeline in timelines(document) {
        for layer in timeline.layers() {
            for cue in layer.cues() {
                if !ids.contains(&cue.sound) {
                    out.push(Diagnostic::DanglingSound {
                        layer: layer.id(),
                        sound: cue.sound,
                    });
                }
            }
        }
    }
    out
}

fn timelines(document: &Document) -> impl Iterator<Item = &Timeline> {
    document
        .scenes
        .iter()
        .map(|scene| &scene.timeline)
        .chain(document.symbols.iter().map(|symbol| &symbol.timeline))
}

pub(crate) fn validate(document: &Document) -> Vec<Diagnostic> {
    let mut out = Vec::new();

    let mut scenes = HashSet::new();
    for scene in &document.scenes {
        if !scenes.insert(scene.id) {
            out.push(Diagnostic::DuplicateScene(scene.id));
        }
    }
    let mut symbols = HashSet::new();
    for symbol in &document.symbols {
        if !symbols.insert(symbol.id) {
            out.push(Diagnostic::DuplicateSymbol(symbol.id));
        }
    }

    // Sized up front: the sets hold every layer and shape id in the document, and growing them
    // by doubling rehashed them several times over on every edit.
    let layer_count: usize = timelines(document).map(|t| t.layers().len()).sum();
    let mut layers = HashSet::with_capacity(layer_count);
    let mut shapes = HashSet::with_capacity(layer_count);
    let mut instances = HashSet::new();
    for timeline in timelines(document) {
        for (position, layer) in timeline.layers().iter().enumerate() {
            if !layers.insert(layer.id()) {
                out.push(Diagnostic::DuplicateLayer(layer.id()));
            }
            if layer.is_guide() && (layer.is_mask() || layer.is_folder()) {
                out.push(Diagnostic::GuideConflict(layer.id()));
            }
            if layer.is_folder() && (!layer.spans().is_empty() || layer.is_mask()) {
                out.push(Diagnostic::FolderWithContent(layer.id()));
            }
            // A folder sits above what it holds, so a parent is always an earlier layer and
            // folders cannot contain themselves, directly or around a loop.
            if let Some(parent) = layer.parent() {
                let above = timeline.layers()[..position]
                    .iter()
                    .any(|l| l.id() == parent && l.is_folder());
                if !above {
                    out.push(Diagnostic::BadFolderParent {
                        layer: layer.id(),
                        parent,
                    });
                }
            }
        }
        for shape in timeline.shapes() {
            if !shapes.insert(shape.id()) {
                out.push(Diagnostic::DuplicateShape(shape.id()));
            }
        }
        for instance in timeline.instances() {
            if !instances.insert(instance.id()) {
                out.push(Diagnostic::DuplicateInstance(instance.id()));
            }
            if !symbols.contains(&instance.symbol()) {
                out.push(Diagnostic::DanglingSymbol {
                    instance: instance.id(),
                    symbol: instance.symbol(),
                });
            }
        }
    }

    out.extend(find_cycles(document));
    out
}

/// Depth-first search over symbol containment; reports each symbol that is on a cycle.
fn find_cycles(document: &Document) -> Vec<Diagnostic> {
    let edges: HashMap<SymbolId, Vec<SymbolId>> = document
        .symbols
        .iter()
        .map(|symbol| {
            (
                symbol.id,
                symbol.timeline.instances().map(|i| i.symbol()).collect(),
            )
        })
        .collect();

    #[derive(Clone, Copy, PartialEq)]
    enum Mark {
        Visiting,
        Done,
    }
    let mut marks: HashMap<SymbolId, Mark> = HashMap::new();
    let mut on_cycle = Vec::new();

    // Explicit stack: containment depth comes from untrusted input, so no recursion.
    for start in document.symbols.iter().map(|symbol| symbol.id) {
        if marks.contains_key(&start) {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        marks.insert(start, Mark::Visiting);
        while let Some((node, next)) = stack.pop() {
            let children = edges.get(&node).map(Vec::as_slice).unwrap_or_default();
            let Some(&child) = children.get(next) else {
                marks.insert(node, Mark::Done);
                continue;
            };
            stack.push((node, next + 1));
            match marks.get(&child) {
                Some(Mark::Visiting) => {
                    if !on_cycle.contains(&child) {
                        on_cycle.push(child);
                    }
                }
                Some(Mark::Done) => {}
                None if edges.contains_key(&child) => {
                    marks.insert(child, Mark::Visiting);
                    stack.push((child, 0));
                }
                None => {}
            }
        }
    }
    on_cycle.into_iter().map(Diagnostic::SymbolCycle).collect()
}
