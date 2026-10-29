use std::num::NonZeroU32;
use std::sync::Arc;

use flare_core::{InstanceId, LayerId, SymbolId, Transform2D};

use crate::Shape;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    id: InstanceId,
    symbol: SymbolId,
    transform: Transform2D,
}

impl Instance {
    pub fn new(id: InstanceId, symbol: SymbolId, transform: Transform2D) -> Self {
        Self {
            id,
            symbol,
            transform,
        }
    }

    pub fn id(&self) -> InstanceId {
        self.id
    }

    pub fn symbol(&self) -> SymbolId {
        self.symbol
    }

    pub fn transform(&self) -> Transform2D {
        self.transform
    }
}

/// What a span holds. An explicitly empty keyframe is not the same as no span at all.
#[derive(Debug, Clone, PartialEq)]
pub enum Content {
    Empty,
    Instance(Instance),
    Shape(Shape),
}

/// How a span's transform moves toward the next span on the same layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tween {
    /// Held for the whole span.
    #[default]
    None,
    /// Linear motion tween to the next adjacent span's instance transform.
    Linear,
}

/// A held exposure: `content` stays for `duration` frames starting at `start`.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSpan {
    start: u32,
    duration: NonZeroU32,
    content: Content,
    tween: Tween,
}

impl FrameSpan {
    pub fn new(start: u32, duration: NonZeroU32, content: Content) -> Self {
        Self {
            start,
            duration,
            content,
            tween: Tween::None,
        }
    }

    pub fn with_tween(mut self, tween: Tween) -> Self {
        self.tween = tween;
        self
    }

    pub fn tween(&self) -> Tween {
        self.tween
    }

    pub fn start(&self) -> u32 {
        self.start
    }

    pub fn duration(&self) -> NonZeroU32 {
        self.duration
    }

    /// One past the last frame, widened so it cannot overflow.
    pub fn end(&self) -> u64 {
        u64::from(self.start) + u64::from(self.duration.get())
    }

    pub fn content(&self) -> &Content {
        &self.content
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TimelineError {
    #[error("frame spans in a layer overlap at frame {0}")]
    OverlappingSpans(u32),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    id: LayerId,
    name: Arc<str>,
    /// Shared, so copying a layer copies neither its name nor its spans.
    spans: Arc<[FrameSpan]>,
    visible: bool,
    locked: bool,
}

impl Layer {
    /// Spans are sorted by start frame; overlapping spans are rejected.
    pub fn new(
        id: LayerId,
        name: impl Into<String>,
        mut spans: Vec<FrameSpan>,
    ) -> Result<Self, TimelineError> {
        spans.sort_by_key(FrameSpan::start);
        for pair in spans.windows(2) {
            if pair[0].end() > u64::from(pair[1].start()) {
                return Err(TimelineError::OverlappingSpans(pair[1].start()));
            }
        }
        Ok(Self {
            id,
            name: name.into().into(),
            spans: spans.into(),
            visible: true,
            locked: false,
        })
    }

    /// A copy with different spans and everything else (name, visibility, lock) kept.
    pub fn with_spans(&self, spans: Vec<FrameSpan>) -> Result<Self, TimelineError> {
        let mut layer = Self::new(self.id, &*self.name, spans)?;
        layer.visible = self.visible;
        layer.locked = self.locked;
        Ok(layer)
    }

    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into().into();
        self
    }

    /// Hidden layers are skipped by evaluation, so they neither render nor export.
    #[must_use]
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.visible = visible;
        self
    }

    /// Locked layers cannot be selected or edited through the stage.
    #[must_use]
    pub fn with_locked(mut self, locked: bool) -> Self {
        self.locked = locked;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_locked(&self) -> bool {
        self.locked
    }

    pub fn id(&self) -> LayerId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn spans(&self) -> &[FrameSpan] {
        &self.spans
    }

    /// The span starting exactly at `frame`, if any.
    pub fn span_at_start(&self, frame: u64) -> Option<&FrameSpan> {
        u32::try_from(frame)
            .ok()
            .and_then(|f| self.span_at(f))
            .filter(|s| u64::from(s.start()) == frame)
    }

    /// The span exposing `frame`, if any. Spans are sorted and non-overlapping.
    pub fn span_at(&self, frame: u32) -> Option<&FrameSpan> {
        let index = self
            .spans
            .partition_point(|span| span.end() <= u64::from(frame));
        self.spans.get(index).filter(|span| span.start() <= frame)
    }
}

/// Layers ordered top to bottom.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Timeline {
    /// Shared, so copying a timeline (every undo snapshot does) copies no layers.
    layers: Arc<Vec<Layer>>,
}

impl Timeline {
    pub fn new(layers: Vec<Layer>) -> Self {
        Self {
            layers: Arc::new(layers),
        }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// The layers for editing. The list is copied first if another timeline shares it, so an
    /// edit never shows through a snapshot; layers themselves stay shared until replaced.
    pub fn layers_mut(&mut self) -> &mut Vec<Layer> {
        Arc::make_mut(&mut self.layers)
    }

    /// Number of frames up to the end of the last span on any layer.
    pub fn frame_count(&self) -> u64 {
        self.layers
            .iter()
            .filter_map(|layer| layer.spans().last())
            .map(FrameSpan::end)
            .max()
            .unwrap_or(0)
    }

    pub(crate) fn shapes(&self) -> impl Iterator<Item = &Shape> {
        self.layers
            .iter()
            .flat_map(|layer| layer.spans())
            .filter_map(|span| match span.content() {
                Content::Shape(shape) => Some(shape),
                Content::Empty | Content::Instance(_) => None,
            })
    }

    /// Every symbol this timeline places an instance of (with repeats).
    pub fn referenced_symbols(&self) -> impl Iterator<Item = SymbolId> + '_ {
        self.instances().map(Instance::symbol)
    }

    pub(crate) fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.layers
            .iter()
            .flat_map(|layer| layer.spans())
            .filter_map(|span| match span.content() {
                Content::Instance(instance) => Some(instance),
                Content::Empty | Content::Shape(_) => None,
            })
    }
}
