//! Canonical authoring model: scenes, symbols, timelines, layers and frame spans.
//!
//! Values are immutable once built; constructors validate. Cross-entity invariants
//! (unique IDs, dangling references, symbol cycles) are checked by [`Document::new`].
//! Editing goes through transactions in a later crate, not through public fields.
#![forbid(unsafe_code)]

mod shape;
mod stage;
mod timeline;
mod validate;

pub use shape::{
    Gradient, GradientError, MAX_SEGMENTS, MAX_STROKE_WIDTH, PathSegment, Shape, ShapeError, Stroke,
};
pub use stage::{MAX_STAGE_SIDE, Stage, StageError};
pub use timeline::{Content, FrameSpan, Instance, Layer, Timeline, TimelineError, Tween};
pub use validate::Diagnostic;

use flare_core::{DocumentId, FrameRate, SceneId, SymbolId};

/// Graphic symbols only for now; Movie Clip and Button are preserved data later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKind {
    Graphic,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    id: SymbolId,
    name: String,
    kind: SymbolKind,
    timeline: Timeline,
}

impl Symbol {
    pub fn new(id: SymbolId, name: impl Into<String>, timeline: Timeline) -> Self {
        Self {
            id,
            name: name.into(),
            kind: SymbolKind::Graphic,
            timeline,
        }
    }

    pub fn id(&self) -> SymbolId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    id: SceneId,
    name: String,
    timeline: Timeline,
}

impl Scene {
    pub fn new(id: SceneId, name: impl Into<String>, timeline: Timeline) -> Self {
        Self {
            id,
            name: name.into(),
            timeline,
        }
    }

    pub fn id(&self) -> SceneId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn timeline(&self) -> &Timeline {
        &self.timeline
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    id: DocumentId,
    frame_rate: FrameRate,
    scenes: Vec<Scene>,
    symbols: Vec<Symbol>,
    stage: Stage,
}

impl Document {
    /// Builds a document, or returns every invariant violation found.
    pub fn new(
        id: DocumentId,
        frame_rate: FrameRate,
        scenes: Vec<Scene>,
        symbols: Vec<Symbol>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let document = Self {
            id,
            frame_rate,
            scenes,
            symbols,
            stage: Stage::default(),
        };
        let diagnostics = validate::validate(&document);
        if diagnostics.is_empty() {
            Ok(document)
        } else {
            Err(diagnostics)
        }
    }

    /// Replaces the stage. Stage values validate themselves, so this cannot fail.
    #[must_use]
    pub fn with_stage(mut self, stage: Stage) -> Self {
        self.stage = stage;
        self
    }

    pub fn stage(&self) -> Stage {
        self.stage
    }

    pub fn id(&self) -> DocumentId {
        self.id
    }

    pub fn frame_rate(&self) -> FrameRate {
        self.frame_rate
    }

    pub fn scenes(&self) -> &[Scene] {
        &self.scenes
    }

    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }

    pub fn symbol(&self, id: SymbolId) -> Option<&Symbol> {
        self.symbols.iter().find(|symbol| symbol.id == id)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::num::NonZeroU32;

    use flare_core::{InstanceId, LayerId, Transform2D};

    use super::*;

    fn span(start: u32, duration: u32, content: Content) -> FrameSpan {
        FrameSpan::new(start, NonZeroU32::new(duration).expect("nonzero"), content)
    }

    fn place(symbol: SymbolId) -> Content {
        Content::Instance(Instance::new(
            InstanceId::new(),
            symbol,
            Transform2D::IDENTITY,
        ))
    }

    fn timeline(spans: Vec<FrameSpan>) -> Timeline {
        Timeline::new(vec![
            Layer::new(LayerId::new(), "Layer 1", spans).expect("valid layer"),
        ])
    }

    fn build(scenes: Vec<Scene>, symbols: Vec<Symbol>) -> Result<Document, Vec<Diagnostic>> {
        let rate = FrameRate::new(24, 1).expect("rate");
        Document::new(DocumentId::new(), rate, scenes, symbols)
    }

    #[test]
    fn two_instances_share_one_symbol_definition() {
        let art = SymbolId::new();
        let symbol = Symbol::new(art, "Art", Timeline::default());
        let scene = Scene::new(
            SceneId::new(),
            "Scene 1",
            timeline(vec![span(0, 10, place(art)), span(10, 5, place(art))]),
        );
        let document = build(vec![scene], vec![symbol]).expect("valid");
        assert_eq!(document.symbols().len(), 1);
        assert!(document.symbol(art).is_some());
    }

    #[test]
    fn empty_span_is_distinct_from_absent_span() {
        let layer = Layer::new(LayerId::new(), "L", vec![span(0, 1, Content::Empty)]).expect("ok");
        assert_eq!(layer.spans().len(), 1);
        assert_eq!(layer.spans()[0].content(), &Content::Empty);
    }

    #[test]
    fn overlapping_spans_are_rejected() {
        let spans = vec![span(0, 5, Content::Empty), span(4, 2, Content::Empty)];
        assert_eq!(
            Layer::new(LayerId::new(), "L", spans),
            Err(TimelineError::OverlappingSpans(4))
        );
    }

    #[test]
    fn adjacent_spans_are_allowed_and_sorted() {
        let spans = vec![span(5, 1, Content::Empty), span(0, 5, Content::Empty)];
        let layer = Layer::new(LayerId::new(), "L", spans).expect("adjacent");
        assert_eq!(layer.spans()[0].start(), 0);
    }

    #[test]
    fn layers_default_to_visible_and_unlocked_and_with_spans_keeps_flags() {
        let layer = Layer::new(LayerId::new(), "L", vec![]).expect("layer");
        assert!(layer.is_visible() && !layer.is_locked());
        let flagged = layer
            .with_visible(false)
            .with_locked(true)
            .with_name("Renamed");
        let rebuilt = flagged
            .with_spans(vec![span(0, 2, Content::Empty)])
            .expect("spans");
        assert_eq!(
            (rebuilt.name(), rebuilt.is_visible(), rebuilt.is_locked()),
            ("Renamed", false, true)
        );
        assert_eq!(rebuilt.spans().len(), 1);
        assert!(
            flagged
                .with_spans(vec![span(0, 5, Content::Empty), span(2, 2, Content::Empty)])
                .is_err()
        );
    }

    #[test]
    fn span_lookup_and_frame_count() {
        let spans = vec![span(2, 3, Content::Empty), span(10, 1, Content::Empty)];
        let layer = Layer::new(LayerId::new(), "L", spans).expect("layer");
        assert!(layer.span_at(1).is_none());
        assert_eq!(layer.span_at(2).map(FrameSpan::start), Some(2));
        assert_eq!(layer.span_at(4).map(FrameSpan::start), Some(2));
        assert!(layer.span_at(5).is_none());
        assert_eq!(layer.span_at(10).map(FrameSpan::start), Some(10));
        assert!(layer.span_at(11).is_none());
        assert_eq!(Timeline::new(vec![layer]).frame_count(), 11);
        assert_eq!(Timeline::default().frame_count(), 0);
    }

    #[test]
    fn span_end_cannot_overflow() {
        let tail = span(u32::MAX, u32::MAX, Content::Empty);
        assert_eq!(tail.end(), u64::from(u32::MAX) * 2);
    }

    #[test]
    fn dangling_symbol_reference_is_diagnosed() {
        let missing = SymbolId::new();
        let scene = Scene::new(
            SceneId::new(),
            "S",
            timeline(vec![span(0, 1, place(missing))]),
        );
        let errors = build(vec![scene], vec![]).expect_err("dangling");
        assert!(
            matches!(errors[..], [Diagnostic::DanglingSymbol { symbol, .. }] if symbol == missing)
        );
    }

    #[test]
    fn duplicate_shape_ids_are_diagnosed() {
        use flare_core::{Rgba8, ShapeId};
        let id = ShapeId::new();
        let shape = || Shape::rect(id, 0.0, 0.0, 1.0, 1.0, Rgba8::WHITE).expect("rect");
        let spans = vec![
            span(0, 1, Content::Shape(shape())),
            span(1, 1, Content::Shape(shape())),
        ];
        let scene = Scene::new(SceneId::new(), "S", timeline(spans));
        assert_eq!(
            build(vec![scene], vec![]).expect_err("dup"),
            vec![Diagnostic::DuplicateShape(id)]
        );
    }

    #[test]
    fn duplicate_symbol_ids_are_diagnosed() {
        let id = SymbolId::new();
        let symbols = vec![
            Symbol::new(id, "A", Timeline::default()),
            Symbol::new(id, "B", Timeline::default()),
        ];
        assert_eq!(
            build(vec![], symbols).expect_err("dup"),
            vec![Diagnostic::DuplicateSymbol(id)]
        );
    }

    #[test]
    fn direct_and_indirect_symbol_cycles_are_rejected() {
        let (a, b) = (SymbolId::new(), SymbolId::new());
        let direct = Symbol::new(a, "A", timeline(vec![span(0, 1, place(a))]));
        assert_eq!(
            build(vec![], vec![direct]).expect_err("cycle"),
            vec![Diagnostic::SymbolCycle(a)]
        );

        let sym_a = Symbol::new(a, "A", timeline(vec![span(0, 1, place(b))]));
        let sym_b = Symbol::new(b, "B", timeline(vec![span(0, 1, place(a))]));
        let errors = build(vec![], vec![sym_a, sym_b]).expect_err("cycle");
        assert!(
            errors
                .iter()
                .all(|d| matches!(d, Diagnostic::SymbolCycle(_)))
        );
        assert!(!errors.is_empty());
    }

    #[test]
    fn deep_symbol_chains_do_not_overflow_the_stack() {
        let ids: Vec<SymbolId> = (0..50_000).map(|_| SymbolId::new()).collect();
        let symbols = ids
            .windows(2)
            .map(|pair| Symbol::new(pair[0], "S", timeline(vec![span(0, 1, place(pair[1]))])))
            .chain([Symbol::new(ids[ids.len() - 1], "leaf", Timeline::default())])
            .collect();
        assert!(build(vec![], symbols).is_ok());
    }
}
