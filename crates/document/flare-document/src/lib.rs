//! Canonical authoring model: scenes, symbols, timelines, layers and frame spans.
//!
//! Values are immutable once built; constructors validate. Cross-entity invariants
//! (unique IDs, dangling references, symbol cycles) are checked by [`Document::new`].
//! Editing goes through transactions in a later crate, not through public fields.
#![forbid(unsafe_code)]

mod attachment;
mod shape;
mod sound;
mod stage;
mod timeline;
mod validate;

pub use attachment::{
    Anchor, Attachment, AttachmentError, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS, Placement,
};
pub use shape::{
    Gradient, GradientError, MAX_SEGMENTS, MAX_STROKE_WIDTH, PathSegment, Shape, ShapeError, Stroke,
};
pub use sound::{
    MAX_CUES_PER_LAYER, MAX_SOUND_BYTES, MAX_SOUNDS, MAX_TOTAL_SOUND_BYTES, Sound, SoundCue,
    SoundError,
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

    /// The same symbol under another name.
    #[must_use]
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
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

    /// The timeline for editing. A document built from the edited scenes is validated as a
    /// whole, so this does not check anything itself.
    pub fn timeline_mut(&mut self) -> &mut Timeline {
        &mut self.timeline
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    id: DocumentId,
    frame_rate: FrameRate,
    scenes: Vec<Scene>,
    symbols: Vec<Symbol>,
    stage: Stage,
    attachments: Vec<Attachment>,
    sounds: Vec<Sound>,
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
            attachments: Vec::new(),
            sounds: Vec::new(),
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

    pub fn attachments(&self) -> &[Attachment] {
        &self.attachments
    }

    /// Attaches source data, validating every anchor and the size limits. Replaces any
    /// attachments already present.
    pub fn with_attachments(
        mut self,
        attachments: Vec<Attachment>,
    ) -> Result<Self, Vec<Diagnostic>> {
        self.attachments = attachments;
        let diagnostics = validate::validate_attachments(&self);
        if diagnostics.is_empty() {
            Ok(self)
        } else {
            Err(diagnostics)
        }
    }

    /// Carries attachments over to a rebuilt document, dropping those whose anchor no
    /// longer exists. This is how deleting a layer removes what hung off it.
    #[must_use]
    pub fn with_attachments_retained(mut self, attachments: &[Attachment]) -> Self {
        if attachments.is_empty() {
            // Nothing to keep, so skip the walk that finds the live anchors.
            return self;
        }
        let live = validate::anchors(&self);
        self.attachments = attachments
            .iter()
            .filter(|a| live.contains(&a.anchor()))
            .cloned()
            .collect();
        self
    }

    /// Attaches the sound library, validating limits and that every cue names a sound in it.
    /// Replaces any sounds already present.
    pub fn with_sounds(mut self, sounds: Vec<Sound>) -> Result<Self, Vec<Diagnostic>> {
        self.sounds = sounds;
        let diagnostics = validate::validate_sounds(&self);
        if diagnostics.is_empty() {
            Ok(self)
        } else {
            Err(diagnostics)
        }
    }

    pub fn sounds(&self) -> &[Sound] {
        &self.sounds
    }

    pub fn sound(&self, id: flare_core::SoundId) -> Option<&Sound> {
        self.sounds.iter().find(|s| s.id() == id)
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
mod tests;
