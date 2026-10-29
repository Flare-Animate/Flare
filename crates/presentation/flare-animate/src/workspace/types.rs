//! The small types the workspace is made of.

use egui::{Key, Pos2, Rect};
use flare_core::{LayerId, SymbolId};
use flare_history::EditorSession;
use flare_runtime::{Rendered, Runtime};
use flare_tools::handles::Handle;
use glamx::{DAffine2, DVec2};
use std::sync::Arc;

/// How many frames before and after the current one onion skinning can show.
pub const MAX_ONION_FRAMES: u32 = 8;

pub(super) const ONION_OPACITY: f32 = 0.4;

/// Onion-skin settings: faint copies of neighbouring frames drawn beneath the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnionSkin {
    pub enabled: bool,
    pub before: u32,
    pub after: u32,
}

impl OnionSkin {
    pub(super) const OFF: Self = Self {
        enabled: false,
        before: 0,
        after: 0,
    };
}

/// The frames to show as ghosts around `frame` in a scene of `frames` frames, as
/// (frame, opacity) in drawing order: farthest first, so nearer frames sit on top and are
/// more opaque (opacity falls off with distance). Frames outside the scene are skipped.
pub fn onion_ghosts(frame: u32, frames: u32, before: u32, after: u32) -> Vec<(u32, f32)> {
    let (before, after) = (before.min(MAX_ONION_FRAMES), after.min(MAX_ONION_FRAMES));
    let opacity = |distance: u32| ONION_OPACITY / distance as f32;
    let mut out: Vec<(u32, f32, u32)> = Vec::new();
    for distance in 1..=before {
        if let Some(f) = frame.checked_sub(distance) {
            out.push((f, opacity(distance), distance));
        }
    }
    for distance in 1..=after {
        if let Some(f) = frame.checked_add(distance).filter(|f| *f < frames) {
            out.push((f, opacity(distance), distance));
        }
    }
    out.sort_by_key(|(_, _, distance)| std::cmp::Reverse(*distance));
    out.into_iter().map(|(f, o, _)| (f, o)).collect()
}

/// A file dialog the shell should show on the workspace's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogRequest {
    Open,
    SaveAs,
    ImportSound,
    ExportImage,
    PublishSwf,
}

/// A request for the shell to move or zoom its window, from the integrated title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRequest {
    Drag,
    ToggleMaximize,
}

pub(super) const MAX_LOG_LINES: usize = 200;

/// The tabs of the Properties panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum PropsTab {
    Tool,
    Object,
    Frame,
    Doc,
}

/// The numbers on the Transform panel: scale in percent, rotation and skew in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct TransformEdit {
    pub(super) scale_x: f32,
    pub(super) scale_y: f32,
    pub(super) rotate: f32,
    pub(super) skew_x: f32,
    pub(super) skew_y: f32,
}

impl Default for TransformEdit {
    fn default() -> Self {
        Self {
            scale_x: 100.0,
            scale_y: 100.0,
            rotate: 0.0,
            skew_x: 0.0,
            skew_y: 0.0,
        }
    }
}

impl TransformEdit {
    /// The matrix these numbers describe, taken about `center`; `None` for the identity or a
    /// scale that would collapse the art.
    pub(super) fn matrix(self, center: Pos2) -> Option<DAffine2> {
        if self == Self::default() || self.scale_x.abs() < 0.01 || self.scale_y.abs() < 0.01 {
            return None;
        }
        let c = DVec2::new(f64::from(center.x), f64::from(center.y));
        let linear = DAffine2::from_angle(f64::from(self.rotate.to_radians()))
            * DAffine2::from_mat2_translation(
                glamx::DMat2::from_cols(
                    DVec2::new(1.0, f64::from(self.skew_y.to_radians().tan())),
                    DVec2::new(f64::from(self.skew_x.to_radians().tan()), 1.0),
                ),
                DVec2::ZERO,
            )
            * DAffine2::from_scale(DVec2::new(
                f64::from(self.scale_x / 100.0),
                f64::from(self.scale_y / 100.0),
            ));
        Some(DAffine2::from_translation(c) * linear * DAffine2::from_translation(-c))
    }
}

/// Whether a tool puts points where the pointer is, so they should snap to the grid.
pub(super) fn tool_places_points(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Rectangle | Tool::Ellipse | Tool::Line | Tool::Pen | Tool::PolyStar | Tool::Text
    )
}

/// `point` rounded to the nearest multiple of `grid` on each axis.
pub(super) fn snap_point(point: Pos2, grid: f32) -> Pos2 {
    let grid = grid.max(f32::EPSILON);
    Pos2::new(
        (point.x / grid).round() * grid,
        (point.y / grid).round() * grid,
    )
}

/// The runtime workers start inline; under test text jobs also know a bundled font by name, so
/// they do not depend on what a machine has installed.
pub(super) fn new_runtime() -> Runtime {
    #[allow(unused_mut)]
    let mut runtime = Runtime::inline();
    #[cfg(test)]
    runtime.add_font(epaint_default_fonts::HACK_REGULAR.to_vec());
    runtime
}

/// A drawn frame as a texture image.
pub(super) fn into_color_image(rendered: Rendered) -> egui::ColorImage {
    let size = rendered.size();
    match rendered {
        Rendered::Premultiplied(image) => {
            egui::ColorImage::from_rgba_premultiplied(size, image.rgba())
        }
        Rendered::Straight(image) => egui::ColorImage::from_rgba_unmultiplied(size, image.rgba()),
    }
}

/// Height of the top bar, and of the integrated one: macOS centers its window buttons 14 px
/// from the top and the shell cannot move them, so the bar they float over is as tall as the
/// system title bar it replaces.
pub(super) const TOP_BAR_HEIGHT: f32 = 38.0;

pub(super) const INTEGRATED_TOP_BAR_HEIGHT: f32 = 28.0;

/// Something for the app to do with the sound card. The workspace decides what plays when; the
/// app owns the device (the workspace has no device code).
#[derive(Clone, Debug)]
pub enum AudioEvent {
    /// Silence everything that is playing.
    Stop,
    /// Start a sound after a delay.
    Play {
        audio: Arc<flare_audio::Audio>,
        delay_seconds: f64,
    },
}

/// Most decoded audio kept for playback, in samples (all channels): about 64 MB of floats.
pub(super) const DECODED_AUDIO_SAMPLES: usize = 16_000_000;

/// Largest side, in pixels, of the stage texture. The texture covers only what is visible, so
/// this is reached only on a very large window or display.
pub(super) const MAX_TEXTURE_SIDE: u32 = 4096;

/// What part of the stage a texture shows and how many pixels it has.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TextureView {
    /// The visible part of the stage, in stage units.
    pub(super) region: Rect,
    pub(super) size: [u32; 2],
}

/// Text being typed on the stage.
pub(super) struct PendingText {
    /// Where the top-left corner of the text will be, in stage units.
    pub(super) at: Pos2,
    pub(super) buffer: String,
    /// The editor has been shown at least once, so it has been given the keyboard.
    pub(super) focused: bool,
}

/// Selection bounds with the (context, revision, selection hash) they were computed for.
pub(super) type SelectionRects = ((u64, u64, u64), Vec<Rect>);

/// Which color the color panel edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ColorTarget {
    Fill,
    Stroke,
    /// The second color of a gradient fill.
    GradientEnd,
}

/// The letter that picks each tool, as in other animation tools.
pub(super) const TOOL_KEYS: &[(Key, Tool)] = &[
    (Key::V, Tool::Select),
    (Key::A, Tool::Subselection),
    (Key::P, Tool::Pen),
    (Key::T, Tool::Text),
    (Key::N, Tool::Line),
    (Key::R, Tool::Rectangle),
    (Key::O, Tool::Ellipse),
    (Key::G, Tool::PolyStar),
    (Key::K, Tool::Bucket),
    (Key::S, Tool::InkBottle),
    (Key::I, Tool::Eyedropper),
    (Key::E, Tool::Eraser),
    (Key::H, Tool::Hand),
    (Key::Z, Tool::Zoom),
    (Key::L, Tool::Lasso),
    (Key::Y, Tool::Pencil),
    (Key::B, Tool::Brush),
];

/// Frames `from..=to` of one layer, selected in the timeline for copying.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct FrameRange {
    pub(super) layer: LayerId,
    pub(super) from: u32,
    pub(super) to: u32,
}

/// A frame handed to the render worker and not yet back.
pub(super) struct PendingRender {
    pub(super) id: u64,
    pub(super) key: (u64, u64, u32),
    pub(super) onion: OnionSkin,
    pub(super) view: TextureView,
    pub(super) signature: u64,
}

/// How many bytes of rendered frames are kept for reuse, and at most how many frames.
pub(super) const FRAME_CACHE_BYTES: usize = 96 * 1024 * 1024;

pub(super) const FRAME_CACHE_ENTRIES: usize = 48;

/// A rendered frame kept so that showing the same picture again costs no rendering: a drawing
/// held for many frames, or an animation that loops.
pub(super) struct CachedFrame {
    /// The document state the picture is of; a different one can never be reused.
    pub(super) context: u64,
    pub(super) revision: u64,
    /// What the frame (and its onion-skin ghosts) draws; see `Frame::signature`.
    pub(super) signature: u64,
    pub(super) view: TextureView,
    pub(super) handle: egui::TextureHandle,
}

impl CachedFrame {
    pub(super) fn bytes(&self) -> usize {
        self.view.size[0] as usize * self.view.size[1] as usize * 4
    }
}

pub(super) struct StageTexture {
    pub(super) handle: egui::TextureHandle,
    /// Which part of the stage the pixels show, so the image is drawn where it belongs.
    pub(super) view: TextureView,
    /// The onion-skin settings the texture was rendered with.
    pub(super) onion: OnionSkin,
    /// (context, session revision, frame) the texture was rendered for.
    pub(super) key: (u64, u64, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Ellipse,
    Line,
    Pen,
    /// Click the stage, type, and place: the text becomes ordinary shapes.
    Text,
    /// Drag to pan the stage.
    Hand,
    /// Click to zoom in, Alt-click to zoom out.
    Zoom,
    /// Click a shape to take its fill (Shift: its stroke) as the current color.
    Eyedropper,
    /// Click a shape to give it the current fill.
    Bucket,
    /// Click a shape to give it the current stroke.
    InkBottle,
    /// Click a shape to delete it.
    Eraser,
    /// Drag from the center out: a regular polygon, or a star.
    PolyStar,
    /// Drag a freehand loop around shapes to select them.
    Lasso,
    /// Select a shape, then drag its points to reshape it.
    Subselection,
    /// Drag to draw a smoothed line in the stroke color and width.
    Pencil,
    /// Drag to paint a thick round stroke in the fill color.
    Brush,
}

/// Timeline frame operations on the active layer at the current frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameOp {
    InsertFrame,
    RemoveFrame,
    InsertKeyframe,
    InsertBlankKeyframe,
    ClearKeyframe,
}

impl FrameOp {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::InsertFrame => "insert a frame",
            Self::RemoveFrame => "remove a frame",
            Self::InsertKeyframe => "insert a keyframe",
            Self::InsertBlankKeyframe => "insert a blank keyframe",
            Self::ClearKeyframe => "clear the keyframe",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DragMode {
    Draw,
    Move,
    /// Scaling the selection by one of its handles.
    Scale(Handle),
    /// Turning the selection about the center of its bounds (dragging outside a corner).
    Rotate,
    /// Shearing the selection by an edge handle (dragging it with Alt held).
    Skew(Handle),
}

/// Gesture in progress on the stage, in document coordinates. It is tool state only:
/// the document changes when the gesture is released.
#[derive(Clone, Copy)]
pub(super) struct Drag {
    /// The selection's bounds when the gesture began (used by scaling).
    pub(super) bounds: Rect,
    pub(super) mode: DragMode,
    pub(super) start: Pos2,
    pub(super) current: Pos2,
}

/// What a timeline drag grabbed on a span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SpanGrab {
    /// The body: `offset` frames in from the span's first frame.
    Move { offset: u32 },
    /// The right edge.
    Resize,
}

/// A span being dragged in the timeline; the document changes when it is released.
#[derive(Clone, Copy, Debug)]
pub(super) struct SpanDrag {
    pub(super) layer: LayerId,
    pub(super) start: u32,
    pub(super) grab: SpanGrab,
    /// The frame under the pointer now.
    pub(super) frame: u32,
}

/// One level of in-place symbol editing: a scratch document whose scene is the symbol's
/// timeline, with its own undo history. Leaving applies it to the parent as one edit.
pub(super) struct Context {
    pub(super) symbol: SymbolId,
    pub(super) name: String,
    pub(super) session: EditorSession,
}
