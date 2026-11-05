use std::num::NonZeroU32;
use std::sync::Arc;

use flare_core::{InstanceId, LayerId, Rgba8, SymbolId, Transform2D};

use crate::{
    Armature, BitmapInstance, Camera, MAX_CUES_PER_LAYER, MAX_ELEMENTS, Shape, SoundCue,
    SoundError, SpanInterp,
};
use thiserror::Error;

mod layer;

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

    /// A mode from what a person or an agent typed: `Play Once`, `play-once` and `play_once` are
    /// one name. The command line and the MCP server both parse with this.
    pub fn parse(text: &str) -> Option<Self> {
        Self::from_name(&crate::normalize_name(text))
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
    /// A raster image from the document's bitmap library.
    Bitmap(BitmapInstance),
    /// Where the camera looks; only on a camera layer.
    Camera(Camera),
    /// Bones and the instances they carry, in one pose: a keyframe of an armature layer, which
    /// holds nothing else.
    Armature(Armature),
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
    pub(crate) start: u32,
    pub(crate) duration: NonZeroU32,
    pub(crate) content: Content,
    /// More elements drawn above `content` in the same frames; see `span_elements`.
    pub(crate) extras: Vec<Content>,
    pub(crate) tween: Tween,
    /// Easing of the tween, from `-MAX_EASE` to `MAX_EASE`; 0 is even speed.
    pub(crate) ease: i8,
    /// An interpolation family and handles chosen with flare-tween; when set it replaces `ease`.
    pub(crate) interp: Option<SpanInterp>,
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
            interp: None,
        }
    }

    /// The same span tweening: its first element, the others of a keyframe of several are held.
    pub fn with_tween(mut self, tween: Tween) -> Self {
        self.tween = tween;
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
        self.with_tween(other.tween)
            .with_ease(other.ease)
            .with_interp(other.interp)
    }

    /// The same span with its interpolation set (`None` goes back to the plain `ease`).
    #[must_use]
    pub fn with_interp(mut self, interp: Option<SpanInterp>) -> Self {
        self.interp = interp;
        self
    }

    pub fn interp(&self) -> Option<SpanInterp> {
        self.interp
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
            .flat_map(FrameSpan::elements)
            .filter_map(|content| match content {
                Content::Shape(shape) => Some(shape),
                Content::Empty
                | Content::Instance(_)
                | Content::Bitmap(_)
                | Content::Camera(_)
                | Content::Armature(_) => None,
            })
    }

    /// Every symbol this timeline places an instance of (with repeats).
    pub fn referenced_symbols(&self) -> impl Iterator<Item = SymbolId> + '_ {
        self.instances().map(Instance::symbol)
    }

    /// Every instance, those an armature carries included. An armature's instances are in
    /// each of its keyframes, so they repeat from one pose to the next.
    pub(crate) fn instances(&self) -> impl Iterator<Item = &Instance> {
        self.layers
            .iter()
            .flat_map(|layer| layer.spans())
            .flat_map(FrameSpan::elements)
            .flat_map(Content::instances)
    }
}
