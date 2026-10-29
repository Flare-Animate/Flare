//! Pure frame evaluation: document + scene + frame -> placed instances.
//!
//! No I/O, rendering or UI state. Instances are identified by their instance *path*, so
//! two occurrences of one symbol keep separate clocks and transforms. Graphic symbols
//! are parent-clocked: child frame = frames since the instance span began, looping over
//! the symbol's length. Movie Clip and Button runtime behaviour is out of scope.
#![forbid(unsafe_code)]

use flare_core::{InstanceId, SceneId, SymbolId};
use flare_document::{Content, Document, Timeline};
use glamx::DAffine2;
use thiserror::Error;

const MAX_DEPTH: usize = 64;
const MAX_NODES: usize = 100_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Placed {
    /// Instances from the scene down to this one.
    pub path: Vec<InstanceId>,
    pub symbol: SymbolId,
    /// Composed transform into scene space.
    pub transform: DAffine2,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvalError {
    #[error("no such scene {0:?}")]
    NoSuchScene(SceneId),
    #[error("symbol nesting deeper than {MAX_DEPTH}")]
    TooDeep,
    #[error("more than {MAX_NODES} placed instances")]
    TooManyNodes,
}

/// Placed instances at `frame`, parents before children, bottom layer drawn first.
pub fn evaluate(document: &Document, scene: SceneId, frame: u32) -> Result<Vec<Placed>, EvalError> {
    let scene = document
        .scenes()
        .iter()
        .find(|candidate| candidate.id() == scene)
        .ok_or(EvalError::NoSuchScene(scene))?;
    let mut out = Vec::new();
    walk(
        document,
        scene.timeline(),
        frame,
        DAffine2::IDENTITY,
        &mut Vec::new(),
        &mut out,
    )?;
    Ok(out)
}

fn walk(
    document: &Document,
    timeline: &Timeline,
    frame: u32,
    parent: DAffine2,
    path: &mut Vec<InstanceId>,
    out: &mut Vec<Placed>,
) -> Result<(), EvalError> {
    if path.len() >= MAX_DEPTH {
        return Err(EvalError::TooDeep);
    }
    // Layers are ordered top to bottom, so draw in reverse.
    for layer in timeline.layers().iter().rev() {
        let Some(span) = layer.span_at(frame) else {
            continue;
        };
        let Content::Instance(instance) = span.content() else {
            continue;
        };
        if out.len() >= MAX_NODES {
            return Err(EvalError::TooManyNodes);
        }
        let transform = parent * instance.transform().as_affine();
        path.push(instance.id());
        out.push(Placed {
            path: path.clone(),
            symbol: instance.symbol(),
            transform,
        });
        if let Some(symbol) = document.symbol(instance.symbol()) {
            let length = symbol.timeline().frame_count();
            if length > 0 {
                let elapsed = u64::from(frame - span.start());
                // `elapsed % length` < length <= u32::MAX * 2; the remainder fits u32 only
                // when length fits, so fall back to saturation for absurd lengths.
                let local = u32::try_from(elapsed % length).unwrap_or(u32::MAX);
                walk(document, symbol.timeline(), local, transform, path, out)?;
            }
        }
        path.pop();
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::num::NonZeroU32;

    use flare_core::{DocumentId, FrameRate, LayerId, Transform2D};
    use flare_document::{FrameSpan, Instance, Layer, Scene, Symbol};
    use glamx::DVec2;

    use super::*;

    fn at(x: f64) -> Transform2D {
        Transform2D::new(DAffine2::from_translation(DVec2::new(x, 0.0))).expect("finite")
    }

    fn span(start: u32, duration: u32, symbol: SymbolId, transform: Transform2D) -> FrameSpan {
        let instance = Instance::new(InstanceId::new(), symbol, transform);
        FrameSpan::new(
            start,
            NonZeroU32::new(duration).expect("nonzero"),
            Content::Instance(instance),
        )
    }

    fn timeline(layers: Vec<Vec<FrameSpan>>) -> Timeline {
        Timeline::new(
            layers
                .into_iter()
                .map(|s| Layer::new(LayerId::new(), "L", s).expect("layer"))
                .collect(),
        )
    }

    fn document(scene: Timeline, symbols: Vec<Symbol>) -> (Document, SceneId) {
        let id = SceneId::new();
        let rate = FrameRate::new(24, 1).expect("rate");
        let scenes = vec![Scene::new(id, "S", scene)];
        (
            Document::new(DocumentId::new(), rate, scenes, symbols).expect("document"),
            id,
        )
    }

    #[test]
    fn two_instances_of_one_symbol_keep_separate_paths_and_transforms() {
        let art = SymbolId::new();
        let scene = timeline(vec![
            vec![span(0, 10, art, at(100.0))],
            vec![span(0, 10, art, at(5.0))],
        ]);
        let (doc, id) = document(scene, vec![Symbol::new(art, "Art", Timeline::default())]);
        let placed = evaluate(&doc, id, 3).expect("eval");
        assert_eq!(placed.len(), 2);
        assert_ne!(placed[0].path, placed[1].path);
        // Bottom layer first.
        assert_eq!(placed[0].transform.translation.x, 5.0);
        assert_eq!(placed[1].transform.translation.x, 100.0);
    }

    #[test]
    fn nested_symbols_compose_transforms_and_loop() {
        let (leaf, inner, outer) = (SymbolId::new(), SymbolId::new(), SymbolId::new());
        // Inner is a 4-frame loop: leaf at x=1 for two frames, then x=2 for two.
        let inner_tl = timeline(vec![vec![
            span(0, 2, leaf, at(1.0)),
            span(2, 2, leaf, at(2.0)),
        ]]);
        let outer_tl = timeline(vec![vec![span(0, 8, inner, at(10.0))]]);
        let symbols = vec![
            Symbol::new(leaf, "Leaf", Timeline::default()),
            Symbol::new(inner, "Inner", inner_tl),
            Symbol::new(outer, "Outer", outer_tl),
        ];
        let scene = timeline(vec![vec![span(5, 20, outer, at(100.0))]]);
        let (doc, id) = document(scene, symbols);

        let leaf_x = |frame| {
            let placed = evaluate(&doc, id, frame).expect("eval");
            assert_eq!(placed.len(), 3);
            assert_eq!(placed[2].path.len(), 3);
            placed[2].transform.translation.x
        };
        assert_eq!(leaf_x(5), 111.0); // inner frame 0
        assert_eq!(leaf_x(8), 112.0); // inner frame 3
        assert_eq!(leaf_x(10), 111.0); // inner frame 5 loops to 1
    }

    #[test]
    fn frames_outside_spans_and_empty_spans_place_nothing() {
        let art = SymbolId::new();
        let scene = timeline(vec![vec![
            span(2, 2, art, at(0.0)),
            FrameSpan::new(4, NonZeroU32::new(2).expect("nonzero"), Content::Empty),
        ]]);
        let (doc, id) = document(scene, vec![Symbol::new(art, "Art", Timeline::default())]);
        assert!(evaluate(&doc, id, 0).expect("eval").is_empty());
        assert_eq!(evaluate(&doc, id, 3).expect("eval").len(), 1);
        assert!(evaluate(&doc, id, 4).expect("eval").is_empty());
        assert!(evaluate(&doc, id, 99).expect("eval").is_empty());
    }

    #[test]
    fn unknown_scene_is_an_error() {
        let (doc, _) = document(Timeline::default(), vec![]);
        let ghost = SceneId::new();
        assert_eq!(evaluate(&doc, ghost, 0), Err(EvalError::NoSuchScene(ghost)));
    }

    #[test]
    fn excessive_nesting_is_bounded() {
        let ids: Vec<SymbolId> = (0..=MAX_DEPTH + 1).map(|_| SymbolId::new()).collect();
        let mut symbols: Vec<Symbol> = ids
            .windows(2)
            .map(|pair| {
                Symbol::new(
                    pair[0],
                    "S",
                    timeline(vec![vec![span(0, 1, pair[1], at(0.0))]]),
                )
            })
            .collect();
        symbols.push(Symbol::new(ids[ids.len() - 1], "leaf", Timeline::default()));
        let scene = timeline(vec![vec![span(0, 1, ids[0], at(0.0))]]);
        let (doc, id) = document(scene, symbols);
        assert_eq!(evaluate(&doc, id, 0), Err(EvalError::TooDeep));
    }
}
