use std::collections::{HashMap, HashSet};

use flare_core::{InstanceId, LayerId, SceneId, ShapeId, SymbolId};
use thiserror::Error;

use crate::{Document, Timeline};

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

    let (mut layers, mut instances, mut shapes) = (HashSet::new(), HashSet::new(), HashSet::new());
    for timeline in timelines(document) {
        for layer in timeline.layers() {
            if !layers.insert(layer.id()) {
                out.push(Diagnostic::DuplicateLayer(layer.id()));
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
