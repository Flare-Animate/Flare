//! The layer: its spans, flags and sounds, and the queries that read them. Split from
//! `timeline.rs` for size; the type itself is defined there.

use super::*;

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
            view_opacity: 100,
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
        layer.view_opacity = self.view_opacity;
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

    /// How opaque the stage draws the layer while it is edited, in percent (held at 100). Below
    /// 100 the layer is transparent, as Animate's Layer Properties has it; exports ignore it.
    #[must_use]
    pub fn with_view_opacity(mut self, percent: u8) -> Self {
        self.view_opacity = percent.min(100);
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

    /// How opaque the stage draws the layer, in percent; 100 unless it is transparent.
    pub fn view_opacity(&self) -> u8 {
        self.view_opacity
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
