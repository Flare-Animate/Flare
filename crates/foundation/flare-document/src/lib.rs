//! Canonical authoring model: scenes, symbols, timelines, layers and frame spans.
//!
//! Values are immutable once built; constructors validate. Cross-entity invariants
//! (unique IDs, dangling references, symbol cycles) are checked by [`Document::new`].
//! Editing goes through transactions in a later crate, not through public fields.
#![forbid(unsafe_code)]

mod armature;
mod attachment;
mod camera;
mod layer_queries;
mod origin;
mod shape;
mod sound;
mod span_elements;
mod stage;
mod text;
mod timeline;
mod validate;

pub use armature::{Armature, ArmatureError, Joint, JointPose, MAX_JOINTS, RigEdit};
pub use attachment::{
    Anchor, Attachment, AttachmentError, MAX_ATTACHMENT_BYTES, MAX_ATTACHMENTS, Placement,
};
pub use camera::{Camera, CameraError, MAX_ZOOM, MIN_ZOOM};
pub use origin::{
    Baseline, MAX_ORIGIN_BYTES, MAX_ORIGIN_FILES, MAX_ORIGIN_NAME_BYTES, Origin, OriginData,
    OriginError, OriginFile, OriginKind, SharedOrigin, SymbolBaseline, is_safe_entry_name,
};
pub use shape::{
    Cap, Gradient, GradientError, Join, MAX_SEGMENTS, MAX_STROKE_WIDTH, PathSegment, Shape,
    ShapeError, Stroke,
};
pub use sound::{
    MAX_CUES_PER_LAYER, MAX_SOUND_BYTES, MAX_SOUNDS, MAX_TOTAL_SOUND_BYTES, Sound, SoundCue,
    SoundError,
};
pub use span_elements::MAX_ELEMENTS;
pub use stage::{MAX_STAGE_SIDE, Stage, StageError};
pub use text::{
    MAX_FAMILY_CHARS, MAX_TEXT_CHARS, MAX_TEXT_SIZE, MIN_TEXT_SIZE, TextSource, TextSourceError,
};
pub use timeline::{
    Content, FrameSpan, Instance, Layer, MAX_EASE, OUTLINE_PALETTE, PlayMode, Playback, Timeline,
    TimelineError, Tween,
};
pub use validate::Diagnostic;

use flare_core::{DocumentId, FrameRate, SceneId, SymbolId};

/// What a symbol is, as in Animate. The kind is kept, edited, saved and exchanged through XFL. A
/// Graphic and a Movie Clip play their frames from the one the instance starts on, looping (a
/// movie clip has no clock of its own beyond that, and no scripts); a Button shows only its first
/// frame, the Up state, because the others need a pointer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SymbolKind {
    #[default]
    Graphic,
    MovieClip,
    Button,
}

impl SymbolKind {
    pub const ALL: [Self; 3] = [Self::Graphic, Self::MovieClip, Self::Button];

    pub fn label(self) -> &'static str {
        match self {
            Self::Graphic => "Graphic",
            Self::MovieClip => "Movie Clip",
            Self::Button => "Button",
        }
    }

    /// The name XFL and the project file use for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Graphic => "graphic",
            Self::MovieClip => "movie clip",
            Self::Button => "button",
        }
    }

    /// The kind a name stands for, or `None` for a name that is not one of the three.
    pub fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "graphic" => Self::Graphic,
            "movie clip" => Self::MovieClip,
            "button" => Self::Button,
            _ => return None,
        })
    }
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

    /// The same symbol as another kind.
    #[must_use]
    pub fn with_kind(mut self, kind: SymbolKind) -> Self {
        self.kind = kind;
        self
    }

    /// The same symbol (name and kind kept) with another timeline.
    #[must_use]
    pub fn with_timeline(mut self, timeline: Timeline) -> Self {
        self.timeline = timeline;
        self
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

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
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
    origin: Option<SharedOrigin>,
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
            origin: None,
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

    /// The file this document was imported from, if it was kept.
    pub fn origin(&self) -> Option<&SharedOrigin> {
        self.origin.as_ref()
    }

    /// The same document with the file it came from. Edits carry it through; it is what lets an
    /// export of an unchanged import be the original.
    #[must_use]
    pub fn with_origin(mut self, origin: Option<SharedOrigin>) -> Self {
        self.origin = origin;
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
mod origin_tests;
#[cfg(test)]
mod tests;
