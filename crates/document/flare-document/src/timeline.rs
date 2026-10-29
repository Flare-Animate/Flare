use std::num::NonZeroU32;

use flare_core::{InstanceId, LayerId, SymbolId, Transform2D};
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
}

/// A held exposure: `content` stays for `duration` frames starting at `start`.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSpan {
    start: u32,
    duration: NonZeroU32,
    content: Content,
}

impl FrameSpan {
    pub fn new(start: u32, duration: NonZeroU32, content: Content) -> Self {
        Self {
            start,
            duration,
            content,
        }
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
    name: String,
    spans: Vec<FrameSpan>,
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
            name: name.into(),
            spans,
        })
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
}

/// Layers ordered top to bottom.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Timeline {
    layers: Vec<Layer>,
}

impl Timeline {
    pub fn new(layers: Vec<Layer>) -> Self {
        Self { layers }
    }

    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    pub(crate) fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.layers
            .iter()
            .flat_map(|layer| layer.spans())
            .filter_map(|span| match span.content() {
                Content::Instance(instance) => Some(instance),
                Content::Empty => None,
            })
    }
}
