use std::num::NonZeroU32;
use std::sync::Arc;

use flare_core::{InstanceId, LayerId, Rgba8, SymbolId, Transform2D};

use crate::{Camera, MAX_CUES_PER_LAYER, MAX_ELEMENTS, Shape, SoundCue, SoundError};
use thiserror::Error;

/// How a Graphic instance plays the frames of its symbol, as in Animate's Properties: round and
/// round, once and then holding the last frame, or one frame held.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlayMode {
    #[default]
    Loop,
    PlayOnce,
    SingleFrame,
}

impl PlayMode {
    pub const ALL: [Self; 3] = [Self::Loop, Self::PlayOnce, Self::SingleFrame];

    pub fn label(self) -> &'static str {
        match self {
            Self::Loop => "Loop",
            Self::PlayOnce => "Play Once",
            Self::SingleFrame => "Single Frame",
        }
    }

    /// The name XFL and the project file use for it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Loop => "loop",
            Self::PlayOnce => "play once",
            Self::SingleFrame => "single frame",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.name() == name)
    }
}

/// Where a Graphic instance starts in its symbol and how it goes on from there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Playback {
    pub mode: PlayMode,
    /// The symbol's frame (counting from 0) shown when the instance's span begins.
    pub first_frame: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    id: InstanceId,
    symbol: SymbolId,
    transform: Transform2D,
    playback: Playback,
}

impl Instance {
    pub fn new(id: InstanceId, symbol: SymbolId, transform: Transform2D) -> Self {
        Self {
            id,
            symbol,
            transform,
            playback: Playback::default(),
        }
    }

    /// The same instance playing its symbol another way.
    #[must_use]
    pub fn with_playback(mut self, playback: Playback) -> Self {
        self.playback = playback;
        self
    }

    pub fn playback(&self) -> Playback {
        self.playback
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
    /// Where the camera looks; only on a camera layer.
    Camera(Camera),
}

/// How a span's transform moves toward the next span on the same layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tween {
    /// Held for the whole span.
    #[default]
    None,
    /// Linear motion tween to the next adjacent span's instance transform.
    Linear,
    /// The shape changes into the next adjacent span's shape, point by point.
    Shape,
}

/// How far a tween's easing can go either way: Animate's Ease runs from -100 (starts slow) to
/// 100 (ends slow).
pub const MAX_EASE: i8 = 100;

/// A held exposure: `content` stays for `duration` frames starting at `start`.
#[derive(Debug, Clone, PartialEq)]
pub struct FrameSpan {
    start: u32,
    duration: NonZeroU32,
    content: Content,
    /// More elements drawn above `content` in the same frames; see `span_elements`.
    pub(crate) extras: Vec<Content>,
    pub(crate) tween: Tween,
    /// Easing of the tween, from `-MAX_EASE` to `MAX_EASE`; 0 is even speed.
    ease: i8,
}

impl FrameSpan {
    pub fn new(start: u32, duration: NonZeroU32, content: Content) -> Self {
        Self {
            start,
            duration,
            content,
            extras: Vec::new(),
            tween: Tween::None,
            ease: 0,
        }
    }

    /// The same span tweening; a span that holds several elements cannot, and stays held.
    pub fn with_tween(mut self, tween: Tween) -> Self {
        if self.extras.is_empty() {
            self.tween = tween;
        }
        self
    }

    /// The same span with the easing set, kept within `-MAX_EASE..=MAX_EASE`.
    #[must_use]
    pub fn with_ease(mut self, ease: i8) -> Self {
        self.ease = ease.clamp(-MAX_EASE, MAX_EASE);
        self
    }

    /// The same span taking the tween and easing of `other`, for a span rebuilt around new
    /// content or a new time range.
    #[must_use]
    pub fn with_motion_of(self, other: &FrameSpan) -> Self {
        self.with_tween(other.tween).with_ease(other.ease)
    }

    pub fn ease(&self) -> i8 {
        self.ease
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
    #[error("a frame span may hold at most {MAX_ELEMENTS} elements, this one has {0}")]
    TooManyElements(usize),
    #[error("only a shape or a symbol instance can be one of several elements in a frame span")]
    NotAnElement,
    #[error("a frame span that holds several elements cannot tween")]
    ManyElementsTween,
}

/// The colors layers without one of their own are marked with, by position.
pub const OUTLINE_PALETTE: [Rgba8; 8] = [
    Rgba8::opaque(0x4F, 0x80, 0xFF),
    Rgba8::opaque(0xFF, 0x4F, 0x4F),
    Rgba8::opaque(0x4F, 0xFF, 0x4F),
    Rgba8::opaque(0xFF, 0xE0, 0x4F),
    Rgba8::opaque(0xC0, 0x4F, 0xFF),
    Rgba8::opaque(0x4F, 0xE8, 0xE8),
    Rgba8::opaque(0xFF, 0x9A, 0x30),
    Rgba8::opaque(0xFF, 0x4F, 0xC0),
];

#[derive(Debug, Clone, PartialEq)]
pub struct Layer {
    id: LayerId,
    name: Arc<str>,
    /// Shared, so copying a layer copies neither its name nor its spans.
    spans: Arc<[FrameSpan]>,
    visible: bool,
    locked: bool,
    mask: bool,
    /// A folder holds no art of its own: it groups the layers below it that name it as parent.
    folder: bool,
    /// A motion guide is never drawn: its path steers the motion tween of the layer below it.
    guide: bool,
    /// The scene's camera: never drawn, and its spans say where the picture is taken from.
    camera: bool,
    /// The color the layer is drawn in by outline mode and marked with in the timeline.
    outline: Option<Rgba8>,
    /// The folder this layer sits in. It must be a folder layer above this one.
    parent: Option<LayerId>,
    /// Sounds that start at frames of this layer, in order of frame. Shared like the spans.
    cues: Arc<[SoundCue]>,
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
            mask: false,
            folder: false,
            guide: false,
            camera: false,
            outline: None,
            parent: None,
            cues: Arc::from([]),
        })
    }

    /// A copy with different spans and everything else (name, visibility, lock) kept.
    pub fn with_spans(&self, spans: Vec<FrameSpan>) -> Result<Self, TimelineError> {
        let mut layer = Self::new(self.id, &*self.name, spans)?;
        layer.visible = self.visible;
        layer.locked = self.locked;
        layer.mask = self.mask;
        layer.folder = self.folder;
        layer.guide = self.guide;
        layer.camera = self.camera;
        layer.outline = self.outline;
        layer.parent = self.parent;
        layer.cues = Arc::clone(&self.cues);
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

    /// A mask layer is never drawn itself: its shapes are the only place the layer directly
    /// below it shows through.
    #[must_use]
    pub fn with_mask(mut self, mask: bool) -> Self {
        self.mask = mask;
        self
    }

    /// Makes this layer the camera (or a plain layer again).
    #[must_use]
    pub fn with_camera(mut self, camera: bool) -> Self {
        self.camera = camera;
        self
    }

    /// Makes this layer a motion guide (or a plain layer again): not drawn, and the first shape
    /// on it at a frame is the path the layer below follows in a motion tween.
    #[must_use]
    pub fn with_guide(mut self, guide: bool) -> Self {
        self.guide = guide;
        self
    }

    /// Sets the layer's outline color, or clears it so the timeline picks one by position.
    #[must_use]
    pub fn with_outline(mut self, outline: Option<Rgba8>) -> Self {
        self.outline = outline;
        self
    }

    /// Makes this layer a folder (or a plain layer again). Folders hold no spans.
    #[must_use]
    pub fn with_folder(mut self, folder: bool) -> Self {
        self.folder = folder;
        self
    }

    /// Puts this layer in the folder `parent`, or takes it out with `None`. A hidden folder
    /// hides everything in it and a locked one locks it.
    #[must_use]
    pub fn with_parent(mut self, parent: Option<LayerId>) -> Self {
        self.parent = parent;
        self
    }

    /// Sets the sound cues: sorted by frame, with at most one per frame.
    pub fn with_cues(mut self, mut cues: Vec<SoundCue>) -> Result<Self, SoundError> {
        if cues.len() > MAX_CUES_PER_LAYER {
            return Err(SoundError::TooManyCues);
        }
        cues.sort_by_key(|c| c.start);
        cues.dedup_by_key(|c| c.start);
        self.cues = cues.into();
        Ok(self)
    }

    pub fn cues(&self) -> &[SoundCue] {
        &self.cues
    }

    pub fn is_mask(&self) -> bool {
        self.mask
    }

    pub fn outline(&self) -> Option<Rgba8> {
        self.outline
    }

    /// The color to show for this layer, which sits at `index` in its timeline: its own, or one
    /// of the palette by position.
    pub fn outline_or_default(&self, index: usize) -> Rgba8 {
        self.outline
            .unwrap_or(OUTLINE_PALETTE[index % OUTLINE_PALETTE.len()])
    }

    pub fn is_guide(&self) -> bool {
        self.guide
    }

    /// A camera layer is never drawn: it says where the scene is looked at from.
    pub fn is_camera(&self) -> bool {
        self.camera
    }

    pub fn is_folder(&self) -> bool {
        self.folder
    }

    pub fn parent(&self) -> Option<LayerId> {
        self.parent
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
                Content::Empty | Content::Instance(_) | Content::Camera(_) => None,
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
                Content::Empty | Content::Shape(_) | Content::Camera(_) => None,
            })
    }
}
