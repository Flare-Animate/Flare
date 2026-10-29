use std::collections::{HashMap, HashSet};
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use egui::{Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_commands::{Command, LayerChange};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_document::{
    Content, Document, FrameSpan, Layer, MAX_STAGE_SIDE, Scene, Shape, Stage,
    Stroke as ShapeStroke, Timeline, Tween,
};
use flare_editor::frames;
use flare_history::EditorSession;
use glamx::{DAffine2, DVec2};

use crate::layout::{self, LayoutError, Pane};
use crate::timeline_view;
use crate::{MENU_LAYOUT, MenuAction, MenuEntry};
use flare_runtime::{AudioJob, Done, IoJob, Mode, RenderJob, Rendered, Runtime, TextJob};
use flare_tools::handles::{self, Handle};
use flare_tools::ops::{self, SpanRef};
use flare_tools::pen::{PenPath, Press};
use flare_ui::view::{self, StageView};

/// How many frames before and after the current one onion skinning can show.
pub const MAX_ONION_FRAMES: u32 = 8;

const ONION_OPACITY: f32 = 0.4;

/// Onion-skin settings: faint copies of neighbouring frames drawn beneath the current one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OnionSkin {
    pub enabled: bool,
    pub before: u32,
    pub after: u32,
}

impl OnionSkin {
    const OFF: Self = Self {
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
}

/// A request for the shell to move or zoom its window, from the integrated title bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowRequest {
    Drag,
    ToggleMaximize,
}

const MAX_LOG_LINES: usize = 200;

/// The tabs of the Properties panel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PropsTab {
    Tool,
    Object,
    Frame,
    Doc,
}

/// The numbers on the Transform panel: scale in percent, rotation and skew in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
struct TransformEdit {
    scale_x: f32,
    scale_y: f32,
    rotate: f32,
    skew_x: f32,
    skew_y: f32,
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
    fn matrix(self, center: Pos2) -> Option<DAffine2> {
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
fn tool_places_points(tool: Tool) -> bool {
    matches!(
        tool,
        Tool::Rectangle | Tool::Ellipse | Tool::Line | Tool::Pen | Tool::PolyStar | Tool::Text
    )
}

/// `point` rounded to the nearest multiple of `grid` on each axis.
fn snap_point(point: Pos2, grid: f32) -> Pos2 {
    let grid = grid.max(f32::EPSILON);
    Pos2::new(
        (point.x / grid).round() * grid,
        (point.y / grid).round() * grid,
    )
}

/// The runtime workers start inline; under test text jobs also know a bundled font by name, so
/// they do not depend on what a machine has installed.
fn new_runtime() -> Runtime {
    #[allow(unused_mut)]
    let mut runtime = Runtime::inline();
    #[cfg(test)]
    runtime.add_font(epaint_default_fonts::HACK_REGULAR.to_vec());
    runtime
}

/// A drawn frame as a texture image.
fn into_color_image(rendered: Rendered) -> egui::ColorImage {
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
const TOP_BAR_HEIGHT: f32 = 38.0;
const INTEGRATED_TOP_BAR_HEIGHT: f32 = 28.0;

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
const DECODED_AUDIO_SAMPLES: usize = 16_000_000;

/// Largest side, in pixels, of the stage texture. The texture covers only what is visible, so
/// this is reached only on a very large window or display.
const MAX_TEXTURE_SIDE: u32 = 4096;

/// What part of the stage a texture shows and how many pixels it has.
#[derive(Debug, Clone, Copy, PartialEq)]
struct TextureView {
    /// The visible part of the stage, in stage units.
    region: Rect,
    size: [u32; 2],
}

/// Text being typed on the stage.
struct PendingText {
    /// Where the top-left corner of the text will be, in stage units.
    at: Pos2,
    buffer: String,
    /// The editor has been shown at least once, so it has been given the keyboard.
    focused: bool,
}

/// Selection bounds with the (context, revision, selection hash) they were computed for.
type SelectionRects = ((u64, u64, u64), Vec<Rect>);

mod panels;

/// Which color the color panel edits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ColorTarget {
    Fill,
    Stroke,
}

/// The letter that picks each tool, as in other animation tools.
const TOOL_KEYS: &[(Key, Tool)] = &[
    (Key::V, Tool::Select),
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
struct FrameRange {
    layer: LayerId,
    from: u32,
    to: u32,
}

/// A frame handed to the render worker and not yet back.
struct PendingRender {
    id: u64,
    key: (u64, u64, u32),
    onion: OnionSkin,
    view: TextureView,
    signature: u64,
}

/// How many bytes of rendered frames are kept for reuse, and at most how many frames.
const FRAME_CACHE_BYTES: usize = 96 * 1024 * 1024;
const FRAME_CACHE_ENTRIES: usize = 48;

/// A rendered frame kept so that showing the same picture again costs no rendering: a drawing
/// held for many frames, or an animation that loops.
struct CachedFrame {
    /// The document state the picture is of; a different one can never be reused.
    context: u64,
    revision: u64,
    /// What the frame (and its onion-skin ghosts) draws; see `Frame::signature`.
    signature: u64,
    view: TextureView,
    handle: egui::TextureHandle,
}

impl CachedFrame {
    fn bytes(&self) -> usize {
        self.view.size[0] as usize * self.view.size[1] as usize * 4
    }
}

struct StageTexture {
    handle: egui::TextureHandle,
    /// Which part of the stage the pixels show, so the image is drawn where it belongs.
    view: TextureView,
    /// The onion-skin settings the texture was rendered with.
    onion: OnionSkin,
    /// (context, session revision, frame) the texture was rendered for.
    key: (u64, u64, u32),
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
    fn label(self) -> &'static str {
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
enum DragMode {
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
struct Drag {
    /// The selection's bounds when the gesture began (used by scaling).
    bounds: Rect,
    mode: DragMode,
    start: Pos2,
    current: Pos2,
}

/// What a timeline drag grabbed on a span.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpanGrab {
    /// The body: `offset` frames in from the span's first frame.
    Move { offset: u32 },
    /// The right edge.
    Resize,
}

/// A span being dragged in the timeline; the document changes when it is released.
#[derive(Clone, Copy, Debug)]
struct SpanDrag {
    layer: LayerId,
    start: u32,
    grab: SpanGrab,
    /// The frame under the pointer now.
    frame: u32,
}

/// One level of in-place symbol editing: a scratch document whose scene is the symbol's
/// timeline, with its own undo history. Leaving applies it to the parent as one edit.
struct Context {
    symbol: SymbolId,
    name: String,
    session: EditorSession,
}

pub struct Workspace {
    session: EditorSession,
    /// Symbols being edited in place, outermost first. Empty means editing the scene.
    contexts: Vec<Context>,
    /// Bumped whenever the editing context changes, so cached renders never mix contexts.
    context_serial: u64,
    path: Option<PathBuf>,
    frame: u32,
    playing: bool,
    /// Fractional frames accumulated while playing.
    play_remainder: f64,
    last_time: Option<f64>,
    texture: Option<StageTexture>,
    /// Recently shown frames, least recently used first.
    frame_cache: Vec<CachedFrame>,
    /// Drawing and file work that runs off the UI thread.
    background: Runtime,
    /// The frame being drawn, if any. A result for anything else is out of date and dropped.
    pending_render: Option<PendingRender>,
    /// Finished drawings waiting for the UI to turn them into textures.
    render_inbox: Vec<(u64, Result<egui::ColorImage, String>)>,
    /// The open or recover request in flight, if any.
    opening: Option<u64>,
    /// The autosave in flight, if any. Another is not started until it finishes.
    autosaving: Option<u64>,
    /// How many frames have been rendered (as opposed to taken from the cache).
    frames_rendered: u64,
    drag: Option<Drag>,
    /// Set by Escape so the release that follows a cancelled drag commits nothing.
    drag_cancelled: bool,
    tool: Tool,
    selected: Option<SpanRef>,
    /// Which color the color panel edits.
    color_target: ColorTarget,
    /// Align panel option (not wired up yet).
    align_to_stage: bool,
    /// Scale (percent), rotation and skew (degrees) typed into the Transform panel.
    transform_edit: TransformEdit,
    /// The hex field in the Color panel while it is being typed in.
    hex_buffer: Option<String>,
    /// Where the pointer is over the stage, in stage units.
    pointer_stage: Option<Pos2>,
    /// The theme and icon loaders have been installed on the egui context.
    chrome_ready: bool,
    /// Sounds decoded and ready to play, by sound.
    decoded: HashMap<flare_core::SoundId, Arc<flare_audio::Audio>>,
    /// Sounds being decoded, and sounds that could not be, so neither is asked for again.
    decoding: HashSet<flare_core::SoundId>,
    decode_failed: HashSet<flare_core::SoundId>,
    /// What the app should do with the sound card, since it last asked.
    audio_events: Vec<AudioEvent>,
    /// The sound import in flight, if any.
    importing_sound: Option<u64>,
    /// Whether the timeline was playing, and at which frame, when sound was last brought up
    /// to date.
    audio_synced: Option<(bool, u32)>,
    /// Font family list and size for the text tool.
    text_family: String,
    text_size: f32,
    /// Corner radius of new rectangles, in stage units.
    rect_radius: f32,
    poly_sides: u32,
    poly_star: bool,
    poly_inner: f32,
    /// The loop being drawn with the lasso, in stage coordinates.
    lasso: Vec<Pos2>,
    /// The points of the pencil or brush stroke being drawn, in stage coordinates.
    freehand: Vec<Pos2>,
    brush_size: f32,
    /// How far a smoothed line may stray from the drag, in screen pixels.
    smoothing: f32,
    /// The text being typed, if the user has clicked the stage with the text tool.
    text_edit: Option<PendingText>,
    /// Text handed to the font worker and not yet back: (job number, where to put it).
    text_pending: Option<(u64, Pos2)>,
    /// Identity of the text editor popup; fixed so its focus survives from frame to frame.
    text_editor_id: egui::Id,
    /// The frames selected in the timeline (Shift-click), if any.
    frame_range: Option<FrameRange>,
    /// What Copy Frames remembered, for Paste Frames.
    frame_clipboard: Option<frames::FrameClip>,
    /// The document state the selection was last checked against, so an unchanged document is
    /// not rechecked every frame.
    selection_checked: Option<(u64, u64)>,
    /// Bounds of the selected spans, for the document state and selection they were made for.
    selection_rects_cache: std::cell::RefCell<Option<SelectionRects>>,
    /// How close, in stage units, a press must be to a scale handle to grab it.
    handle_radius: f32,
    /// Shift is held: corner scaling keeps the proportions.
    uniform_scale: bool,
    /// Alt is held: an edge handle skews instead of scaling.
    skew_modifier: bool,
    /// What Copy remembered, for Paste.
    clipboard: Vec<ops::ClipItem>,
    /// Timeline zoom: 1.0 fits the whole scene in the panel.
    timeline_zoom: f32,
    /// Spans selected along with `selected` (Shift-click). Move and Delete act on all of them.
    also_selected: Vec<SpanRef>,
    stage_view: StageView,
    /// The stage panel's last rectangle, so menu zoom can aim at its center.
    stage_panel: Rect,
    span_drag: Option<SpanDrag>,
    selected_symbol: Option<SymbolId>,
    library_search: String,
    /// The symbol being renamed in the library and the text so far.
    renaming: Option<(SymbolId, String)>,
    current_layer: Option<LayerId>,
    layer_name_edit: String,
    /// A native menu bar owns the menu and its shortcuts, so the in-window ones step aside.
    native_menu: bool,
    quit_requested: bool,
    /// Where autosave writes recovery files, if enabled.
    recovery_dir: Option<PathBuf>,
    autosave_interval: f64,
    last_autosave: Option<f64>,
    /// A recovery file exists for the current document and has not been discarded.
    recovery_written: bool,
    /// The dockable panel arrangement.
    layout: egui_tiles::Tree<Pane>,
    onion: OnionSkin,
    test_movie_requested: bool,
    /// The pen tool's path while it is being drawn.
    pen: PenPath,
    fill: Color32,
    /// New rectangles and ellipses fade from `fill` to `gradient_end` across their bounds.
    gradient_enabled: bool,
    gradient_end: Color32,
    gradient_vertical: bool,
    stroke_enabled: bool,
    stroke_color: Color32,
    stroke_width: f32,
    /// Stage size being edited in the tools panel, applied when the edit finishes.
    stage_edit: Option<(u32, u32)>,
    fps_edit: Option<f64>,
    show_rulers: bool,
    /// Playback wraps to the first frame; off, it stops on the last.
    loop_playback: bool,
    muted: bool,
    show_grid: bool,
    /// Points drawn and moves are rounded to the grid.
    snap_to_grid: bool,
    /// Grid spacing in stage units.
    grid_size: f32,
    /// Everything `report` has said, oldest first, for the Output panel.
    log: Vec<String>,
    props_tab: PropsTab,
    /// Position and size typed into the Object tab: x, y, width, height.
    object_edit: Option<[f32; 4]>,
    status: Option<String>,
    dialog: Option<DialogRequest>,
    window_request: Option<WindowRequest>,
    titlebar_inset: f32,
    top_bar_id: egui::Id,
}

impl Workspace {
    pub fn new(document: Document, path: Option<PathBuf>) -> Self {
        Self {
            session: EditorSession::new(document),
            contexts: Vec::new(),
            context_serial: 0,
            path,
            frame: 0,
            playing: false,
            play_remainder: 0.0,
            last_time: None,
            texture: None,
            frame_cache: Vec::new(),
            background: new_runtime(),
            pending_render: None,
            render_inbox: Vec::new(),
            opening: None,
            autosaving: None,
            frames_rendered: 0,
            drag: None,
            drag_cancelled: false,
            tool: Tool::Rectangle,
            selected: None,
            color_target: ColorTarget::Fill,
            align_to_stage: false,
            transform_edit: TransformEdit::default(),
            hex_buffer: None,
            pointer_stage: None,
            chrome_ready: false,
            decoded: HashMap::new(),
            decoding: HashSet::new(),
            decode_failed: HashSet::new(),
            audio_events: Vec::new(),
            importing_sound: None,
            audio_synced: None,
            text_family: "sans-serif".into(),
            rect_radius: 0.0,
            poly_sides: 5,
            poly_star: false,
            poly_inner: 0.5,
            lasso: Vec::new(),
            freehand: Vec::new(),
            brush_size: 12.0,
            smoothing: 2.0,
            text_size: 48.0,
            text_edit: None,
            text_pending: None,
            text_editor_id: egui::Id::unique("flare_text_editor"),
            frame_range: None,
            frame_clipboard: None,
            selection_checked: None,
            selection_rects_cache: std::cell::RefCell::new(None),
            handle_radius: 6.0,
            uniform_scale: false,
            skew_modifier: false,
            clipboard: Vec::new(),
            timeline_zoom: 1.0,
            also_selected: Vec::new(),
            stage_view: StageView::default(),
            stage_panel: Rect::NOTHING,
            span_drag: None,
            selected_symbol: None,
            library_search: String::new(),
            renaming: None,
            current_layer: None,
            layer_name_edit: String::new(),
            native_menu: false,
            quit_requested: false,
            recovery_dir: None,
            autosave_interval: 30.0,
            last_autosave: None,
            recovery_written: false,
            layout: layout::default_tree(),
            onion: OnionSkin {
                enabled: false,
                before: 1,
                after: 1,
            },
            test_movie_requested: false,
            pen: PenPath::default(),
            fill: Color32::from_rgb(232, 69, 60),
            gradient_enabled: false,
            gradient_end: Color32::from_rgb(60, 90, 220),
            gradient_vertical: false,
            stroke_enabled: false,
            stroke_color: Color32::BLACK,
            stroke_width: 4.0,
            stage_edit: None,
            fps_edit: None,
            show_rulers: true,
            loop_playback: true,
            muted: false,
            show_grid: false,
            snap_to_grid: false,
            grid_size: 20.0,
            log: Vec::new(),
            props_tab: PropsTab::Tool,
            object_edit: None,
            status: None,
            dialog: None,
            window_request: None,
            titlebar_inset: 0.0,
            top_bar_id: egui::Id::unique("top-bar"),
        }
    }

    /// A new untitled document: one scene with one empty layer at 24 fps.
    pub fn untitled() -> Result<Self, String> {
        let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).map_err(|e| e.to_string())?;
        let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
        let rate = FrameRate::new(24, 1).map_err(|e| e.to_string())?;
        let document = Document::new(DocumentId::new(), rate, vec![scene], vec![])
            .map_err(|diagnostics| format!("{diagnostics:?}"))?;
        Ok(Self::new(document, None))
    }

    /// Zooms the stage about the center of its panel (menu commands).
    fn zoom_stage(&mut self, factor: f32) {
        let extent = self.stage_extent();
        self.stage_view
            .zoom_center(factor, self.stage_panel, extent);
    }

    fn stage_extent(&self) -> Vec2 {
        let stage = self.document().stage();
        Vec2::new(stage.width() as f32, stage.height() as f32)
    }

    /// Changes the stage size as one undo step, keeping the background.
    pub fn set_stage_size(&mut self, width: u32, height: u32) {
        let current = self.document().stage();
        match Stage::new(width, height, current.background()) {
            Ok(stage) => self.run_batch(vec![Command::SetStage(stage)], "resize the stage"),
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Changes the stage background as one undo step, keeping the size.
    pub fn set_stage_background(&mut self, background: Rgba8) {
        let current = self.document().stage();
        if current.background() == background {
            return;
        }
        match Stage::new(current.width(), current.height(), background) {
            Ok(stage) => self.run_batch(vec![Command::SetStage(stage)], "change the background"),
            Err(error) => self.report(error.to_string()),
        }
    }

    /// Sets the document's frame rate (0.01 to 240 fps) as one undo step.
    pub fn set_frame_rate(&mut self, fps: f64) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the frame rate");
            return;
        }
        if !fps.is_finite() || !(0.01..=240.0).contains(&fps) {
            self.report("The frame rate must be between 0.01 and 240 fps");
            return;
        }
        let milli = (fps * 1000.0).round() as u32;
        match flare_core::FrameRate::new(milli, 1000) {
            Ok(rate) if rate != self.document().frame_rate() => {
                self.run_batch(vec![Command::SetFrameRate(rate)], "change the frame rate");
            }
            Ok(_) => {}
            Err(error) => self.report(error.to_string()),
        }
    }

    /// The session edits go to: the innermost symbol being edited, else the document's.
    fn active(&self) -> &EditorSession {
        self.contexts.last().map_or(&self.session, |c| &c.session)
    }

    fn active_mut(&mut self) -> &mut EditorSession {
        match self.contexts.last_mut() {
            Some(context) => &mut context.session,
            None => &mut self.session,
        }
    }

    /// The document being edited right now (a symbol's scratch document in symbol context).
    pub fn document(&self) -> &Document {
        self.active().document()
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty() || self.contexts.iter().any(|c| c.session.is_dirty())
    }

    pub fn status(&self) -> Option<&str> {
        self.status.as_deref()
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Window title with an unsaved-changes marker.
    pub fn title(&self) -> String {
        let name = self.path.as_deref().and_then(Path::file_name).map_or_else(
            || "Untitled".to_owned(),
            |n| n.to_string_lossy().into_owned(),
        );
        let dirty = if self.is_dirty() { "*" } else { "" };
        format!("{name}{dirty} - Flare")
    }

    /// Width at the left of the top bar the window's own buttons cover; non-zero when the shell
    /// draws no title bar of its own.
    pub fn set_titlebar_inset(&mut self, inset: f32) {
        self.titlebar_inset = inset;
    }

    /// Returns (and clears) a window move or zoom the top bar asked for.
    pub fn take_window_request(&mut self) -> Option<WindowRequest> {
        self.window_request.take()
    }

    /// Returns (and clears) a dialog the shell should show.
    pub fn take_dialog(&mut self) -> Option<DialogRequest> {
        self.dialog.take()
    }

    fn report(&mut self, message: impl Into<String>) {
        let message = message.into();
        if self.log.last() != Some(&message) {
            self.log.push(message.clone());
            if self.log.len() > MAX_LOG_LINES {
                self.log.remove(0);
            }
        }
        self.status = Some(message);
    }

    fn scene_frames(&self) -> u32 {
        let frames = self
            .document()
            .scenes()
            .first()
            .map_or(0, |scene| scene.timeline().frame_count());
        u32::try_from(frames).unwrap_or(u32::MAX).max(1)
    }

    pub fn set_frame(&mut self, frame: u32) {
        self.frame = frame.min(self.scene_frames() - 1);
    }

    // ---- file operations -------------------------------------------------------------

    /// Replaces the document being edited, keeping the user's tool and environment settings
    /// (tool, colors, native menu, recovery, panel layout, onion skin) rather than resetting them with it.
    fn install_document(&mut self, document: Document, path: Option<PathBuf>) {
        let mut fresh = Self::new(document, path);
        fresh.tool = self.tool;
        fresh.fill = self.fill;
        fresh.gradient_enabled = self.gradient_enabled;
        fresh.gradient_end = self.gradient_end;
        fresh.gradient_vertical = self.gradient_vertical;
        fresh.text_family = std::mem::take(&mut self.text_family);
        fresh.text_size = self.text_size;
        fresh.stroke_enabled = self.stroke_enabled;
        fresh.stroke_color = self.stroke_color;
        fresh.stroke_width = self.stroke_width;
        fresh.native_menu = self.native_menu;
        fresh.titlebar_inset = self.titlebar_inset;
        fresh.recovery_dir = self.recovery_dir.take();
        fresh.autosave_interval = self.autosave_interval;
        fresh.layout = std::mem::replace(
            &mut self.layout,
            egui_tiles::Tree::empty(egui::Id::unique("taken")),
        );
        fresh.onion = self.onion;
        // Whatever was playing belonged to the old project.
        fresh.audio_events = std::mem::take(&mut self.audio_events);
        fresh.audio_events.push(AudioEvent::Stop);
        // The workers belong to the app, not to a document; results of the old document's jobs
        // find nothing waiting for them and are dropped.
        fresh.background = std::mem::replace(&mut self.background, new_runtime());
        *self = fresh;
    }

    /// Opens a project, refusing to replace unsaved work. The file is read off the UI thread.
    pub fn open(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before opening another project");
            return;
        }
        self.start_open(path, false);
    }

    fn start_open(&mut self, path: &Path, recovery: bool) {
        let id = self.background.next_id();
        self.opening = Some(id);
        self.background.submit_io(IoJob::Open {
            id,
            path: path.to_owned(),
            recovery,
        });
        if self.background.mode() == Mode::Threads {
            self.status = Some(format!("Opening {}...", path.display()));
        }
        self.settle_inline();
    }

    // ---- crash recovery --------------------------------------------------------------

    /// Turns on autosave into `dir`. Recovery files are separate from the project file.
    pub fn enable_recovery(&mut self, dir: PathBuf) {
        self.recovery_dir = Some(dir);
    }

    pub fn set_autosave_interval(&mut self, seconds: f64) {
        self.autosave_interval = seconds.max(1.0);
    }

    /// Loads a recovery file as unsaved work (it counts as dirty until saved to a project).
    pub fn recover(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before recovering other work");
            return;
        }
        self.start_open(path, true);
    }

    /// Deletes the recovery file for the current document, if there is one.
    pub fn discard_recovery(&mut self) {
        self.recovery_written = false;
        if let Some(dir) = &self.recovery_dir
            && let Err(error) = flare_storage::discard_recovery(dir, self.session.document().id())
        {
            self.status = Some(format!("Could not remove the recovery file: {error}"));
        }
    }

    /// Writes a recovery file when there are unsaved changes and the interval has passed.
    /// Only the document itself is saved: an in-progress symbol edit is not applied yet.
    fn autosave(&mut self, now: f64) {
        let Some(dir) = self.recovery_dir.clone() else {
            return;
        };
        if !self.session.is_dirty() {
            // Clean again (saved, or undone back to the saved state): nothing to recover.
            if self.recovery_written {
                self.discard_recovery();
            }
            self.last_autosave = Some(now);
            return;
        }
        if self
            .last_autosave
            .is_some_and(|last| now - last < self.autosave_interval)
        {
            return;
        }
        if self.autosaving.is_some() {
            return; // the last one is still being written
        }
        self.last_autosave = Some(now);
        let id = self.background.next_id();
        self.autosaving = Some(id);
        self.background.submit_io(IoJob::Autosave {
            id,
            dir,
            document: self.session.snapshot(),
        });
        self.settle_inline();
    }

    pub fn save(&mut self) {
        match self.path.clone() {
            Some(path) => self.save_as(&path),
            None => self.dialog = Some(DialogRequest::SaveAs),
        }
    }

    pub fn save_as(&mut self, path: &Path) {
        // Saving writes the whole document, so pending symbol edits are applied first.
        if !self.exit_all_symbols() {
            return;
        }
        // The file is written off the UI thread from a snapshot, so editing carries on while it
        // is written; whatever changes meanwhile simply stays unsaved.
        self.background.submit_io(IoJob::Save {
            path: path.to_owned(),
            document: self.session.snapshot(),
        });
        self.settle_inline();
    }

    // ---- sound ------------------------------------------------------------------------

    /// Imports a sound file: it is read and decoded on a worker, then added to the project with
    /// a cue at the current frame on a new layer, as one undo step.
    pub fn import_sound(&mut self, path: &Path) {
        if self.in_symbol() {
            self.report("Leave symbol editing before importing a sound");
            return;
        }
        let id = self.background.next_id();
        self.importing_sound = Some(id);
        self.background.submit_audio(AudioJob::Import {
            id,
            path: path.to_owned(),
        });
        if self.background.mode() == Mode::Threads {
            self.status = Some(format!("Importing {}...", path.display()));
        }
        self.settle_inline();
    }

    fn add_sound(&mut self, path: &Path, bytes: Vec<u8>, audio: Arc<flare_audio::Audio>) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let name = path
            .file_stem()
            .map_or_else(|| "Sound".to_owned(), |n| n.to_string_lossy().into_owned());
        let id = flare_core::SoundId::new();
        let sound = match flare_document::Sound::new(
            id,
            name.clone(),
            bytes,
            audio.sample_rate(),
            audio.channels(),
            audio.frames() as u64,
        ) {
            Ok(sound) => sound,
            Err(error) => return self.report(format!("Cannot import {name}: {error}")),
        };
        let layer_id = LayerId::new();
        let cue = flare_document::SoundCue {
            start: self.frame,
            sound: id,
        };
        let layer = match Layer::new(layer_id, format!("Sound: {name}"), vec![])
            .map_err(|e| e.to_string())
            .and_then(|l| l.with_cues(vec![cue]).map_err(|e| e.to_string()))
        {
            Ok(layer) => layer,
            Err(message) => return self.report(format!("Cannot import {name}: {message}")),
        };
        self.run_batch(
            vec![Command::AddSound(sound), Command::AddLayer { scene, layer }],
            "import the sound",
        );
        if self.status.is_none() {
            self.decoded.insert(id, audio);
            self.current_layer = Some(layer_id);
        }
    }

    /// The sounds that only the cues of `layer` play: what deleting the layer would leave unused.
    fn orphaned_sounds(&self, layer: LayerId) -> Vec<flare_core::SoundId> {
        let document = self.session.document();
        let timelines = document
            .scenes()
            .iter()
            .map(|s| s.timeline())
            .chain(document.symbols().iter().map(|s| s.timeline()));
        let (mut own, mut elsewhere) = (Vec::new(), HashSet::new());
        for timeline in timelines {
            for l in timeline.layers() {
                if l.id() == layer {
                    own.extend(l.cues().iter().map(|c| c.sound));
                } else {
                    elsewhere.extend(l.cues().iter().map(|c| c.sound));
                }
            }
        }
        own.sort_by_key(|id| *id.as_uuid());
        own.dedup();
        own.retain(|id| !elsewhere.contains(id));
        own
    }

    /// Asks for every sound in the project to be decoded (once), and forgets decodings of
    /// sounds that are gone.
    fn ensure_sounds_decoded(&mut self) {
        let present: HashSet<flare_core::SoundId> = self
            .session
            .document()
            .sounds()
            .iter()
            .map(flare_document::Sound::id)
            .collect();
        self.decoded.retain(|id, _| present.contains(id));
        self.decode_failed.retain(|id| present.contains(id));
        let wanted: Vec<(flare_core::SoundId, Arc<[u8]>)> = self
            .session
            .document()
            .sounds()
            .iter()
            .filter(|s| {
                !self.decoded.contains_key(&s.id())
                    && !self.decoding.contains(&s.id())
                    && !self.decode_failed.contains(&s.id())
            })
            .map(|s| (s.id(), s.shared_data()))
            .collect();
        for (sound, data) in wanted {
            self.decoding.insert(sound);
            self.background
                .submit_audio(AudioJob::Decode { sound, data });
        }
        self.settle_inline();
    }

    /// Keeps the sound card in step with the timeline: sounds start when playing starts, again
    /// when the timeline wraps or jumps back, and stop when it pauses.
    fn sync_audio(&mut self) {
        let now = (self.playing, self.frame);
        let before = self.audio_synced.replace(now);
        let rate = self.document().frame_rate();
        let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
        let (was_playing, was_frame) = before.unwrap_or((false, 0));
        if !self.playing {
            if was_playing {
                self.audio_events.push(AudioEvent::Stop);
            }
            return;
        }
        // A jump back (the loop wrapping, or a scrub) or a jump of over a second forward starts
        // the sounds over from where the playhead is; ordinary playback does not.
        let jumped = self.frame < was_frame || f64::from(self.frame - was_frame) > fps;
        if was_playing && !jumped {
            return;
        }
        self.audio_events.push(AudioEvent::Stop);
        let Some(scene) = self.session.document().scenes().first() else {
            return;
        };
        for layer in scene.timeline().layers().iter().filter(|l| l.is_visible()) {
            for cue in layer
                .cues()
                .iter()
                .filter(|c| c.start >= self.frame && !self.muted)
            {
                if let Some(audio) = self.decoded.get(&cue.sound) {
                    self.audio_events.push(AudioEvent::Play {
                        audio: Arc::clone(audio),
                        delay_seconds: f64::from(cue.start - self.frame) / fps,
                    });
                }
            }
        }
    }

    /// What to do with the sound card since the last call.
    pub fn take_audio_events(&mut self) -> Vec<AudioEvent> {
        std::mem::take(&mut self.audio_events)
    }

    // ---- text -------------------------------------------------------------------------

    /// The small editor shown where the user clicked with the text tool. Ctrl/Cmd+Enter or the
    /// Place button hands the text over; Escape or Cancel drops it.
    fn text_editor(&mut self, ctx: &egui::Context, image_rect: Rect, scale: f32) {
        let id = self.text_editor_id;
        let Some(edit) = self.text_edit.as_mut() else {
            return;
        };
        let at = image_rect.min + edit.at.to_vec2() * scale;
        let (mut place, mut cancel) = (false, false);
        egui::Area::new(id)
            .fixed_pos(at)
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    let field = ui.add(
                        egui::TextEdit::multiline(&mut edit.buffer)
                            .desired_rows(2)
                            .desired_width(240.0)
                            .hint_text("Type text"),
                    );
                    if !edit.focused {
                        field.request_focus();
                        edit.focused = true;
                    }
                    ui.horizontal(|ui| {
                        place |= ui.button("Place").clicked();
                        cancel |= ui.button("Cancel").clicked();
                    });
                    ui.input(|i| {
                        place |= i.modifiers.command && i.key_pressed(Key::Enter);
                        cancel |= i.key_pressed(Key::Escape);
                    });
                });
            });
        if place {
            self.place_text();
        } else if cancel {
            self.text_edit = None;
        }
    }

    /// Starts the text being typed becoming shapes. The font work happens off the UI thread; the
    /// shapes appear when it is done.
    pub fn place_text(&mut self) {
        let Some(edit) = self.text_edit.take() else {
            return;
        };
        if edit.buffer.trim().is_empty() {
            return;
        }
        let id = self.background.next_id();
        self.text_pending = Some((id, edit.at));
        self.background.submit_text(TextJob {
            id,
            text: edit.buffer,
            style: flare_text::TextStyle {
                family: self.text_family.clone(),
                size: self.text_size,
            },
        });
        self.settle_inline();
    }

    /// Makes the shape for finished text outlines, with its corner at `at`, on a new layer.
    fn place_outline(&mut self, outline: flare_text::TextOutline, at: Pos2) {
        let shift = |p: &flare_core::DocumentPoint| {
            flare_core::DocumentPoint::new(
                p.as_vec().x + f64::from(at.x),
                p.as_vec().y + f64::from(at.y),
            )
            .ok()
        };
        let moved: Option<Vec<flare_document::PathSegment>> = outline
            .path
            .iter()
            .map(|segment| {
                use flare_document::PathSegment as S;
                Some(match segment {
                    S::MoveTo(p) => S::MoveTo(shift(p)?),
                    S::LineTo(p) => S::LineTo(shift(p)?),
                    S::CubicTo(a, b, c) => S::CubicTo(shift(a)?, shift(b)?, shift(c)?),
                    S::Close => S::Close,
                })
            })
            .collect();
        let fill = {
            let c = self.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let Some(path) = moved else {
            self.report("Cannot place text: it would leave the drawable range");
            return;
        };
        match Shape::new(ShapeId::new(), path, fill) {
            Ok(shape) => self.insert_shape_layer(shape, "Text"),
            Err(error) => self.report(format!("Cannot place text: {error}")),
        }
    }

    // ---- background work -------------------------------------------------------------

    /// Moves drawing and file work onto worker threads. Until this is called it runs on the
    /// calling thread, which keeps headless use and tests deterministic.
    pub fn start_background_threads(&mut self, ctx: &egui::Context) {
        let ctx = ctx.clone();
        self.background
            .start_threads(std::sync::Arc::new(move || ctx.request_repaint()));
    }

    /// Whether a save, open or autosave is still being worked on.
    pub fn has_pending_io(&self) -> bool {
        self.background.io_pending()
    }

    /// Waits for every save, open and autosave started so far and applies their results. For
    /// closing the app only: everything else keeps the UI free of waiting.
    pub fn finish_background(&mut self) {
        for done in self.background.finish_io() {
            self.handle_done(done);
        }
    }

    /// Applies the results of finished work. Never blocks.
    fn drain_background(&mut self) {
        for done in self.background.poll() {
            self.handle_done(done);
        }
    }

    /// With no worker threads a job has already finished by the time it was handed over, so its
    /// result is applied at once, as though the work had been instant.
    fn settle_inline(&mut self) {
        if self.background.mode() == Mode::Inline {
            self.drain_background();
        }
    }

    fn handle_done(&mut self, done: Done) {
        match done {
            Done::Rendered { id, result } => {
                self.render_inbox.push((id, result.map(into_color_image)))
            }
            Done::SoundImported { id, path, result } => {
                if self
                    .importing_sound
                    .take_if(|pending| *pending == id)
                    .is_none()
                {
                    return;
                }
                match result {
                    Ok((bytes, audio)) => self.add_sound(&path, bytes, audio),
                    Err(message) => {
                        self.report(format!("Cannot import {}: {message}", path.display()));
                    }
                }
            }
            Done::SoundDecoded { sound, result } => {
                self.decoding.remove(&sound);
                match result {
                    Ok(audio) => {
                        let held: usize = self.decoded.values().map(|a| a.frames()).sum();
                        if held + audio.frames() > DECODED_AUDIO_SAMPLES / 2 {
                            self.decode_failed.insert(sound);
                            self.report("Too much sound is loaded to play every sound at once");
                        } else {
                            self.decoded.insert(sound, audio);
                        }
                    }
                    Err(message) => {
                        self.decode_failed.insert(sound);
                        self.report(format!("Cannot play a sound: {message}"));
                    }
                }
            }
            Done::Text { id, result } => {
                let Some((_, at)) = self.text_pending.take_if(|(pending, _)| *pending == id) else {
                    return; // asked for again since
                };
                match result {
                    Ok(outline) => self.place_outline(outline, at),
                    Err(message) => self.report(format!("Cannot place text: {message}")),
                }
            }
            Done::Saved {
                path,
                document,
                result,
            } => match result {
                Ok(()) => {
                    self.session.mark_snapshot_saved(&document);
                    // Once saved, the recovery file for the work is obsolete.
                    if !self.session.is_dirty() {
                        self.discard_recovery();
                    }
                    self.path = Some(path.clone());
                    self.report(format!("Saved {}", path.display()));
                }
                Err(error) => self.report(format!("Cannot save: {error}")),
            },
            Done::Autosaved { id, result } => {
                if self.autosaving == Some(id) {
                    self.autosaving = None;
                }
                match result {
                    Ok(_) => self.recovery_written = true,
                    Err(error) => self.report(format!("Autosave failed: {error}")),
                }
            }
            Done::Opened {
                id,
                path,
                recovery,
                result,
            } => {
                if self.opening != Some(id) {
                    return; // superseded by a later request
                }
                self.opening = None;
                match result {
                    Err(error) => {
                        let what = if recovery { "recover" } else { "open" };
                        self.report(format!("Cannot {what} {}: {error}", path.display()));
                    }
                    Ok(_) if self.is_dirty() => {
                        self.report("Your project changed while the file was opening, so it was not replaced");
                    }
                    Ok(document) if recovery => {
                        self.install_document(document, None);
                        self.session.mark_unsaved();
                        // The file stays until the work is saved or discarded, so a second
                        // crash before saving still leaves it recoverable.
                        self.recovery_written = true;
                        self.report("Recovered unsaved work");
                    }
                    Ok(document) => {
                        // The old document is clean, so any recovery file for it is obsolete.
                        self.discard_recovery();
                        self.install_document(document, Some(path));
                    }
                }
            }
        }
    }

    // ---- editing ---------------------------------------------------------------------

    pub fn undo(&mut self) {
        if self.active_mut().undo() {
            self.set_frame(self.frame);
            self.validate_selection();
        }
    }

    pub fn redo(&mut self) {
        if self.active_mut().redo() {
            self.set_frame(self.frame);
            self.validate_selection();
        }
    }

    /// Commits a rectangle as one undo step: a new layer plus its shape span.
    pub fn commit_rectangle(&mut self, a: Pos2, b: Pos2) {
        self.commit_shape(Tool::Rectangle, a, b);
    }

    /// Commits the dragged shape for a drawing tool as one undo step: a new layer plus
    /// its shape span. Lines need a stroke, so they use the stroke settings even when
    /// the Outline checkbox is off.
    fn commit_shape(&mut self, tool: Tool, a: Pos2, b: Pos2) {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        let (w, h) = (f64::from(x1 - x0), f64::from(y1 - y0));
        let too_small = match tool {
            Tool::Line | Tool::PolyStar => (b - a).length() < 1.0,
            _ => w < 1.0 || h < 1.0,
        };
        if too_small {
            return; // a click, not a drag
        }
        if self.document().scenes().is_empty() {
            self.report("This document has no scene");
            return;
        }
        let fill = {
            let c = self.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let stroke = {
            let c = self.stroke_color.to_array();
            ShapeStroke::new(
                f64::from(self.stroke_width),
                Rgba8::new(c[0], c[1], c[2], c[3]),
            )
        };
        let id = ShapeId::new();
        let (x0, y0) = (f64::from(x0), f64::from(y0));
        let (shape, name) = match tool {
            Tool::Rectangle => (
                Shape::rounded_rect(id, x0, y0, w, h, f64::from(self.rect_radius), fill),
                "Rectangle",
            ),
            Tool::PolyStar => {
                let radius = f64::from((b - a).length());
                // The first corner points where the drag went.
                let rotation = f64::from((b.x - a.x).atan2(a.y - b.y));
                let inner = self.poly_star.then_some(f64::from(self.poly_inner));
                let center = (f64::from(a.x), f64::from(a.y));
                let name = if self.poly_star { "Star" } else { "Polygon" };
                (
                    Shape::polygon(id, center, radius, self.poly_sides, rotation, inner, fill),
                    name,
                )
            }
            Tool::Ellipse => (
                Shape::ellipse(id, x0 + w / 2.0, y0 + h / 2.0, w / 2.0, h / 2.0, fill),
                "Ellipse",
            ),
            Tool::Line => {
                let from = (f64::from(a.x), f64::from(a.y));
                let to = (f64::from(b.x), f64::from(b.y));
                match &stroke {
                    Ok(stroke) => (Shape::line(id, from, to, *stroke), "Line"),
                    Err(error) => return self.report(error.to_string()),
                }
            }
            _ => return,
        };
        let Some(mut shape) = shape else {
            self.report("Shape coordinates must be finite");
            return;
        };
        if self.gradient_enabled && tool != Tool::Line {
            let end = {
                let c = self.gradient_end.to_array();
                Rgba8::new(c[0], c[1], c[2], c[3])
            };
            let (from, to) = if self.gradient_vertical {
                ((x0, y0), (x0, y0 + h))
            } else {
                ((x0, y0), (x0 + w, y0))
            };
            let point = |(x, y): (f64, f64)| flare_core::DocumentPoint::new(x, y).ok();
            let gradient = point(from)
                .zip(point(to))
                .and_then(|(from, to)| flare_document::Gradient::linear(from, to, fill, end).ok());
            match gradient {
                Some(gradient) => shape = shape.with_gradient(gradient),
                None => return self.report("The gradient needs a shape with some size"),
            }
        }
        if self.stroke_enabled && tool != Tool::Line {
            match stroke {
                Ok(stroke) => shape = shape.with_stroke(stroke),
                Err(error) => return self.report(error.to_string()),
            }
        }
        self.insert_shape_layer(shape, name);
    }

    /// Adds `shape` on a new layer spanning the scene (at least 24 frames), as one undo step,
    /// and makes that layer active.
    fn insert_shape_layer(&mut self, shape: Shape, name: &str) {
        let Some(scene) = self.document().scenes().first() else {
            self.report("This document has no scene");
            return;
        };
        let scene_id = scene.id();
        let duration = u32::try_from(scene.timeline().frame_count())
            .unwrap_or(u32::MAX)
            .max(24);
        let span = FrameSpan::new(
            0,
            NonZeroU32::new(duration).unwrap_or(NonZeroU32::MIN),
            Content::Shape(shape),
        );
        let layer_id = LayerId::new();
        let layer = match Layer::new(layer_id, name, vec![]) {
            Ok(layer) => layer,
            Err(error) => return self.report(error.to_string()),
        };
        let batch = [
            Command::AddLayer {
                scene: scene_id,
                layer,
            },
            Command::InsertSpan {
                scene: scene_id,
                layer: layer_id,
                span,
            },
        ];
        match self.active_mut().apply(batch) {
            Ok(()) => {
                self.status = None;
                self.current_layer = Some(layer_id);
            }
            Err(error) => self.report(format!("Cannot draw: {error}")),
        }
    }

    // ---- pen tool --------------------------------------------------------------------

    pub fn pen_is_active(&self) -> bool {
        self.pen.is_active()
    }

    /// A press with the pen: adds a point (which a following drag turns into a handle), or
    /// closes the path when it lands on the first point.
    pub fn pen_press(&mut self, at: Pos2) {
        if self.tool != Tool::Pen {
            return;
        }
        if self.pen.press(at) == Press::Close {
            self.commit_pen(true);
        }
    }

    pub fn pen_drag(&mut self, to: Pos2) {
        if self.tool == Tool::Pen {
            self.pen.drag(to);
        }
    }

    pub fn pen_release(&mut self) {
        self.pen.release();
    }

    /// Enter or double-click: ends an open path.
    pub fn pen_finish(&mut self) {
        if self.tool != Tool::Pen || !self.pen.is_active() {
            return;
        }
        self.pen.trim_duplicates();
        self.commit_pen(false);
    }

    /// Backspace: removes the last point.
    pub fn pen_remove_last(&mut self) {
        self.pen.remove_last();
    }

    /// Escape: discards the path without touching the document.
    pub fn pen_cancel(&mut self) {
        self.pen.clear();
    }

    /// Turns the pen path into a shape on a new layer, as one undo step. A closed path is
    /// filled (and outlined when Outline is on); an open path has no area, so it is drawn with
    /// the stroke settings, like a line.
    fn commit_pen(&mut self, closed: bool) {
        let Some(path) = self.pen.segments(closed) else {
            self.report(if closed {
                "A closed path needs at least three points"
            } else {
                "A path needs at least two points"
            });
            return;
        };
        let stroke = {
            let c = self.stroke_color.to_array();
            ShapeStroke::new(
                f64::from(self.stroke_width),
                Rgba8::new(c[0], c[1], c[2], c[3]),
            )
        };
        let fill = {
            let c = self.fill.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let fill = if closed { fill } else { Rgba8::TRANSPARENT };
        let shape = match Shape::new(ShapeId::new(), path, fill) {
            Ok(shape) => shape,
            Err(error) => return self.report(error.to_string()),
        };
        let shape = if !closed || self.stroke_enabled {
            match stroke {
                Ok(stroke) => shape.with_stroke(stroke),
                Err(error) => return self.report(error.to_string()),
            }
        } else {
            shape
        };
        self.insert_shape_layer(shape, "Path");
        if self.status.is_none() {
            self.pen.clear();
        }
    }

    // ---- in-place symbol editing -----------------------------------------------------

    pub fn in_symbol(&self) -> bool {
        !self.contexts.is_empty()
    }

    /// Names from the scene down to the symbol being edited, for a breadcrumb.
    pub fn breadcrumb(&self) -> Vec<String> {
        let scene = self
            .session
            .document()
            .scenes()
            .first()
            .map(|s| s.name().to_owned());
        scene
            .into_iter()
            .chain(self.contexts.iter().map(|c| c.name.clone()))
            .collect()
    }

    fn reset_view(&mut self) {
        self.cancel_drag();
        self.selected = None;
        self.also_selected.clear();
        self.current_layer = None;
        self.frame = 0;
        self.texture = None;
        self.frame_cache.clear();
        self.context_serial += 1;
    }

    /// Starts editing a symbol's own timeline in place. Returns whether it started.
    pub fn enter_symbol(&mut self, symbol: SymbolId) -> bool {
        let Some((scratch, _)) = flare_editor::symbol_workspace(self.document(), symbol) else {
            self.report("That symbol no longer exists");
            return false;
        };
        let name = self
            .document()
            .symbol(symbol)
            .map_or_else(String::new, |s| s.name().to_owned());
        self.contexts.push(Context {
            symbol,
            name,
            session: EditorSession::new(scratch),
        });
        self.reset_view();
        self.status = None;
        true
    }

    /// The symbol behind the selected instance, if an instance is selected.
    fn selected_instance_symbol(&self) -> Option<SymbolId> {
        let (selected, scene) = (self.selected?, self.scene_id()?);
        match ops::find_span(self.document(), scene, selected)?.content() {
            Content::Instance(instance) => Some(instance.symbol()),
            _ => None,
        }
    }

    pub fn can_edit_selected_symbol(&self) -> bool {
        self.selected_instance_symbol().is_some()
    }

    pub fn edit_selected_symbol(&mut self) {
        match self.selected_instance_symbol() {
            Some(symbol) => {
                self.enter_symbol(symbol);
            }
            None => self.report("Select a symbol instance to edit it"),
        }
    }

    /// Leaves the innermost symbol, applying its edits to the parent as one undo step.
    /// Symbols created inside it (Convert to Symbol) join the library in the same step.
    /// If the parent rejects the result (for example a symbol that contains itself) the
    /// edit stays open and the reason is shown.
    pub fn exit_symbol(&mut self) -> bool {
        let Some(context) = self.contexts.pop() else {
            return false;
        };
        let inner = context.session.document();
        let timeline = inner
            .scenes()
            .first()
            .map(|s| s.timeline().clone())
            .unwrap_or_default();
        let parent = self.active().document();
        let new_symbols: Vec<flare_document::Symbol> = inner
            .symbols()
            .iter()
            .filter(|s| parent.symbol(s.id()).is_none())
            .cloned()
            .collect();
        let changed = parent
            .symbol(context.symbol)
            .is_none_or(|s| s.timeline() != &timeline);
        if changed || !new_symbols.is_empty() {
            let merge = Command::MergeSymbolEdit {
                symbol: context.symbol,
                timeline,
                new_symbols,
            };
            if let Err(error) = self.active_mut().apply([merge]) {
                self.contexts.push(context);
                self.report(format!("Cannot leave the symbol: {error}"));
                return false;
            }
        }
        self.reset_view();
        self.status = None;
        true
    }

    /// Leaves symbols until only `depth` remain (0 = back to the scene).
    pub fn exit_to_depth(&mut self, depth: usize) -> bool {
        while self.contexts.len() > depth {
            if !self.exit_symbol() {
                return false;
            }
        }
        true
    }

    fn exit_all_symbols(&mut self) -> bool {
        self.exit_to_depth(0)
    }

    // ---- dock layout -----------------------------------------------------------------

    /// The current panel arrangement as JSON, for saving between sessions.
    pub fn layout_json(&self) -> Result<String, serde_json::Error> {
        layout::to_json(&self.layout)
    }

    /// Restores a saved arrangement. An unusable one is refused and the current layout is
    /// kept, so a stale or damaged file can never leave the workspace without a panel.
    pub fn restore_layout(&mut self, json: &str) -> Result<(), LayoutError> {
        self.layout = layout::from_json(json)?;
        Ok(())
    }

    /// Back to the default arrangement. Never touches the document.
    pub fn reset_layout(&mut self) {
        self.layout = layout::default_tree();
    }

    // ---- menu actions ----------------------------------------------------------------

    /// Tells the workspace a native menu bar is present, so it does not draw its own menu
    /// or handle the menu's shortcuts a second time.
    pub fn set_native_menu(&mut self, native: bool) {
        self.native_menu = native;
    }

    /// Returns (and clears) a request to quit the application.
    pub fn take_quit_request(&mut self) -> bool {
        std::mem::take(&mut self.quit_requested)
    }

    /// What an image export needs, shared not copied: the saved document and the frame on show.
    pub fn image_export(&self) -> (Document, u32) {
        (self.main_document().clone(), self.frame)
    }

    /// Returns (and clears) a request to play the document in an external player.
    pub fn take_test_movie_request(&mut self) -> bool {
        std::mem::take(&mut self.test_movie_requested)
    }

    /// The document as saved, which is what a test movie plays: unlike `document()`, it is
    /// never a symbol's scratch document.
    pub fn main_document(&self) -> &Document {
        self.session.document()
    }

    /// Whether `action` can run right now (menu items are greyed out when it cannot).
    pub fn is_enabled(&self, action: MenuAction) -> bool {
        let layer = self.active_layer();
        let editable_layer = layer.is_some_and(|l| !l.is_locked());
        let layer_count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        let position = layer.and_then(|l| {
            self.document()
                .scenes()
                .first()?
                .timeline()
                .layers()
                .iter()
                .position(|c| c.id() == l.id())
        });
        match action {
            MenuAction::Open | MenuAction::Save | MenuAction::SaveAs | MenuAction::Quit => true,
            MenuAction::ImportSound => !self.in_symbol(),
            MenuAction::ExportImage => !self.document().scenes().is_empty(),
            MenuAction::Undo => self.active().can_undo(),
            MenuAction::Redo => self.active().can_redo(),
            MenuAction::Paste => !self.clipboard.is_empty(),
            MenuAction::SelectAll => self.scene_id().is_some_and(|scene| {
                !ops::spans_at_frame(self.document(), scene, self.frame).is_empty()
            }),
            MenuAction::Rewind | MenuAction::StepBackward => self.frame > 0,
            MenuAction::StepForward | MenuAction::ActualSize => true,
            MenuAction::CopyFrames => editable_layer || self.frame_range.is_some(),
            MenuAction::PasteFrames => {
                self.frame_clipboard.is_some() && layer.is_some_and(|l| !l.is_locked())
            }
            MenuAction::Delete
            | MenuAction::Duplicate
            | MenuAction::Copy
            | MenuAction::Cut
            | MenuAction::DeselectAll
            | MenuAction::FlipHorizontal
            | MenuAction::FlipVertical
            | MenuAction::RotateClockwise
            | MenuAction::RotateCounterclockwise
            | MenuAction::ScaleUp
            | MenuAction::ScaleDown => self.has_selection(),
            MenuAction::ConvertToSymbol => self.can_convert_selection(),
            MenuAction::EditSymbol => self.can_edit_selected_symbol(),
            MenuAction::ExitSymbol => self.in_symbol(),
            MenuAction::ToolSelect
            | MenuAction::ToolRectangle
            | MenuAction::ToolEllipse
            | MenuAction::ToolLine
            | MenuAction::ToolPen
            | MenuAction::ToolText
            | MenuAction::ToolHand
            | MenuAction::ToolZoom
            | MenuAction::ToolEyedropper
            | MenuAction::ToolBucket
            | MenuAction::ToolInkBottle
            | MenuAction::ToolEraser
            | MenuAction::ToolPolyStar
            | MenuAction::ToolLasso
            | MenuAction::ToolPencil
            | MenuAction::ToolBrush
            | MenuAction::PlayPause
            | MenuAction::ZoomIn
            | MenuAction::ZoomOut => true,
            MenuAction::ZoomFit => !self.stage_view.is_fit(),
            MenuAction::InsertFrame
            | MenuAction::RemoveFrame
            | MenuAction::InsertKeyframe
            | MenuAction::InsertBlankKeyframe
            | MenuAction::ClearKeyframe => editable_layer,
            MenuAction::ToggleMotionTween => self.selection_tween().is_some(),
            MenuAction::ToggleLayerVisible
            | MenuAction::ToggleLayerLocked
            | MenuAction::ToggleLayerMask => layer.is_some(),
            MenuAction::MoveLayerUp => position.is_some_and(|p| p > 0),
            MenuAction::MoveLayerDown => position.is_some_and(|p| p + 1 < layer_count),
            MenuAction::DeleteLayer => layer.is_some() && layer_count > 1,
            MenuAction::ResetLayout => true,
            MenuAction::ToggleOnionSkin
            | MenuAction::ToggleRulers
            | MenuAction::ToggleGrid
            | MenuAction::ToggleSnap
            | MenuAction::ToggleLoop
            | MenuAction::ToggleMute => true,
            MenuAction::TestMovie => true,
        }
    }

    /// The menu item's current label (some toggle between two meanings).
    pub fn label(&self, action: MenuAction) -> String {
        let layer = self.active_layer();
        let text = match action {
            MenuAction::Open => "Open...",
            MenuAction::ImportSound => "Import Sound...",
            MenuAction::ExportImage => "Export Image...",
            MenuAction::Save => "Save",
            MenuAction::SaveAs => "Save As...",
            MenuAction::Quit => "Quit Flare",
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Delete => "Delete",
            MenuAction::Duplicate => "Duplicate",
            MenuAction::Copy => "Copy",
            MenuAction::Cut => "Cut",
            MenuAction::SelectAll => "Select All",
            MenuAction::DeselectAll => "Deselect All",
            MenuAction::Rewind => "Rewind",
            MenuAction::StepForward => "Step Forward",
            MenuAction::StepBackward => "Step Backward",
            MenuAction::ActualSize => "Actual Size",
            MenuAction::CopyFrames => "Copy Frames",
            MenuAction::PasteFrames => "Paste Frames",
            MenuAction::Paste => "Paste",
            MenuAction::FlipHorizontal => "Flip Horizontal",
            MenuAction::FlipVertical => "Flip Vertical",
            MenuAction::RotateClockwise => "Rotate 90\u{b0} Clockwise",
            MenuAction::RotateCounterclockwise => "Rotate 90\u{b0} Counterclockwise",
            MenuAction::ScaleUp => "Scale Up",
            MenuAction::ScaleDown => "Scale Down",
            MenuAction::ConvertToSymbol => "Convert to Symbol",
            MenuAction::EditSymbol => "Edit Symbol",
            MenuAction::ExitSymbol => "Exit Symbol",
            MenuAction::ToolSelect => "Select / Move",
            MenuAction::ToolRectangle => "Rectangle",
            MenuAction::ToolEllipse => "Ellipse",
            MenuAction::ToolLine => "Line",
            MenuAction::ToolPen => "Pen",
            MenuAction::ToolText => "Text",
            MenuAction::ToolHand => "Hand",
            MenuAction::ToolZoom => "Zoom",
            MenuAction::ToolEyedropper => "Eyedropper",
            MenuAction::ToolBucket => "Paint Bucket",
            MenuAction::ToolInkBottle => "Ink Bottle",
            MenuAction::ToolEraser => "Eraser",
            MenuAction::ToolPolyStar => "PolyStar",
            MenuAction::ToolLasso => "Lasso",
            MenuAction::ToolPencil => "Pencil",
            MenuAction::ToolBrush => "Brush",
            MenuAction::ZoomIn => "Zoom In",
            MenuAction::ZoomOut => "Zoom Out",
            MenuAction::ZoomFit => "Fit Stage in Window",
            MenuAction::InsertFrame => "Insert Frame",
            MenuAction::RemoveFrame => "Remove Frame",
            MenuAction::InsertKeyframe => "Insert Keyframe",
            MenuAction::InsertBlankKeyframe => "Insert Blank Keyframe",
            MenuAction::ClearKeyframe => "Clear Keyframe",
            MenuAction::ToggleMotionTween => {
                if self.selection_tween() == Some(Tween::Linear) {
                    "Remove Motion Tween"
                } else {
                    "Create Motion Tween"
                }
            }
            MenuAction::PlayPause => {
                if self.playing {
                    "Pause"
                } else {
                    "Play"
                }
            }
            MenuAction::ToggleLayerVisible => {
                if layer.is_none_or(Layer::is_visible) {
                    "Hide Layer"
                } else {
                    "Show Layer"
                }
            }
            MenuAction::ToggleLayerLocked => {
                if layer.is_some_and(Layer::is_locked) {
                    "Unlock Layer"
                } else {
                    "Lock Layer"
                }
            }
            MenuAction::ToggleLayerMask => {
                if layer.is_some_and(Layer::is_mask) {
                    "Remove Mask"
                } else {
                    "Mask Layer Below"
                }
            }
            MenuAction::MoveLayerUp => "Move Layer Up",
            MenuAction::MoveLayerDown => "Move Layer Down",
            MenuAction::DeleteLayer => "Delete Layer",
            MenuAction::ResetLayout => "Reset Workspace Layout",
            MenuAction::ToggleOnionSkin => {
                if self.onion.enabled {
                    "Hide Onion Skin"
                } else {
                    "Show Onion Skin"
                }
            }
            MenuAction::ToggleRulers => {
                if self.show_rulers {
                    "Hide Rulers"
                } else {
                    "Show Rulers"
                }
            }
            MenuAction::ToggleGrid => {
                if self.show_grid {
                    "Hide Grid"
                } else {
                    "Show Grid"
                }
            }
            MenuAction::ToggleSnap => {
                if self.snap_to_grid {
                    "Snapping: On"
                } else {
                    "Snapping: Off"
                }
            }
            MenuAction::ToggleLoop => {
                if self.loop_playback {
                    "Loop Playback: On"
                } else {
                    "Loop Playback: Off"
                }
            }
            MenuAction::ToggleMute => {
                if self.muted {
                    "Unmute Sounds"
                } else {
                    "Mute Sounds"
                }
            }
            MenuAction::TestMovie => "Test Movie",
        };
        text.to_owned()
    }

    /// Runs `action`. Disabled actions do nothing, so menus need not re-check.
    pub fn perform(&mut self, action: MenuAction) {
        if !self.is_enabled(action) {
            return;
        }
        let layer = self.active_layer_id();
        match action {
            MenuAction::Open => self.dialog = Some(DialogRequest::Open),
            MenuAction::ImportSound => self.dialog = Some(DialogRequest::ImportSound),
            MenuAction::ExportImage => self.dialog = Some(DialogRequest::ExportImage),
            MenuAction::Save => self.save(),
            MenuAction::SaveAs => self.dialog = Some(DialogRequest::SaveAs),
            MenuAction::Quit => self.quit_requested = true,
            MenuAction::Undo => self.undo(),
            MenuAction::Redo => self.redo(),
            MenuAction::Delete => self.delete_selection(),
            MenuAction::Duplicate => self.duplicate_selection(),
            MenuAction::Copy => self.copy_selection(),
            MenuAction::Cut => self.cut_selection(),
            MenuAction::SelectAll => self.select_all(),
            MenuAction::DeselectAll => self.clear_selection(),
            MenuAction::Rewind => self.set_frame(0),
            MenuAction::StepForward => self.set_frame(self.frame.saturating_add(1)),
            MenuAction::StepBackward => self.set_frame(self.frame.saturating_sub(1)),
            MenuAction::ActualSize => self.set_stage_percent(100.0),
            MenuAction::CopyFrames => self.copy_frames(),
            MenuAction::PasteFrames => self.paste_frames(),
            MenuAction::Paste => self.paste(),
            MenuAction::FlipHorizontal => {
                self.transform_selection(DAffine2::from_scale(DVec2::new(-1.0, 1.0)))
            }
            MenuAction::FlipVertical => {
                self.transform_selection(DAffine2::from_scale(DVec2::new(1.0, -1.0)))
            }
            MenuAction::RotateClockwise => {
                self.transform_selection(DAffine2::from_angle(std::f64::consts::FRAC_PI_2));
            }
            MenuAction::RotateCounterclockwise => {
                self.transform_selection(DAffine2::from_angle(-std::f64::consts::FRAC_PI_2));
            }
            MenuAction::ScaleUp => {
                self.transform_selection(DAffine2::from_scale(DVec2::splat(SCALE_STEP)))
            }
            MenuAction::ScaleDown => {
                self.transform_selection(DAffine2::from_scale(DVec2::splat(1.0 / SCALE_STEP)))
            }
            MenuAction::ConvertToSymbol => self.convert_selection_to_symbol(),
            MenuAction::EditSymbol => self.edit_selected_symbol(),
            MenuAction::ExitSymbol => {
                self.exit_symbol();
            }
            MenuAction::ToolSelect => self.set_tool(Tool::Select),
            MenuAction::ToolRectangle => self.set_tool(Tool::Rectangle),
            MenuAction::ToolEllipse => self.set_tool(Tool::Ellipse),
            MenuAction::ToolLine => self.set_tool(Tool::Line),
            MenuAction::ToolPen => self.set_tool(Tool::Pen),
            MenuAction::ToolText => self.set_tool(Tool::Text),
            MenuAction::ToolHand => self.set_tool(Tool::Hand),
            MenuAction::ToolZoom => self.set_tool(Tool::Zoom),
            MenuAction::ToolEyedropper => self.set_tool(Tool::Eyedropper),
            MenuAction::ToolBucket => self.set_tool(Tool::Bucket),
            MenuAction::ToolInkBottle => self.set_tool(Tool::InkBottle),
            MenuAction::ToolEraser => self.set_tool(Tool::Eraser),
            MenuAction::ToolPolyStar => self.set_tool(Tool::PolyStar),
            MenuAction::ToolLasso => self.set_tool(Tool::Lasso),
            MenuAction::ToolPencil => self.set_tool(Tool::Pencil),
            MenuAction::ToolBrush => self.set_tool(Tool::Brush),
            MenuAction::InsertFrame => self.frame_edit(FrameOp::InsertFrame),
            MenuAction::RemoveFrame => self.frame_edit(FrameOp::RemoveFrame),
            MenuAction::InsertKeyframe => self.frame_edit(FrameOp::InsertKeyframe),
            MenuAction::InsertBlankKeyframe => self.frame_edit(FrameOp::InsertBlankKeyframe),
            MenuAction::ClearKeyframe => self.frame_edit(FrameOp::ClearKeyframe),
            MenuAction::ToggleMotionTween => {
                let next = if self.selection_tween() == Some(Tween::Linear) {
                    Tween::None
                } else {
                    Tween::Linear
                };
                self.set_selection_tween(next);
            }
            MenuAction::PlayPause => self.playing = !self.playing,
            MenuAction::ZoomIn => self.zoom_stage(view::STEP),
            MenuAction::ZoomOut => self.zoom_stage(1.0 / view::STEP),
            MenuAction::ZoomFit => self.stage_view.reset(),
            MenuAction::ToggleLayerVisible => {
                if let Some(id) = layer {
                    self.toggle_layer_visible(id);
                }
            }
            MenuAction::ToggleLayerLocked => {
                if let Some(id) = layer {
                    self.toggle_layer_locked(id);
                }
            }
            MenuAction::ToggleLayerMask => {
                if let Some(id) = layer {
                    self.toggle_layer_mask(id);
                }
            }
            MenuAction::MoveLayerUp => self.move_active_layer(-1),
            MenuAction::MoveLayerDown => self.move_active_layer(1),
            MenuAction::DeleteLayer => self.delete_active_layer(),
            MenuAction::ResetLayout => self.reset_layout(),
            MenuAction::ToggleOnionSkin => self.set_onion_skin(!self.onion.enabled),
            MenuAction::ToggleRulers => self.show_rulers = !self.show_rulers,
            MenuAction::ToggleGrid => self.show_grid = !self.show_grid,
            MenuAction::ToggleSnap => self.snap_to_grid = !self.snap_to_grid,
            MenuAction::ToggleLoop => self.loop_playback = !self.loop_playback,
            MenuAction::ToggleMute => {
                self.muted = !self.muted;
                // Sounds already started stop now; unmuting waits for the next start.
                self.audio_events.push(AudioEvent::Stop);
            }
            MenuAction::TestMovie => {
                // Pending symbol edits belong in the movie, so apply them first, like Save.
                if self.exit_all_symbols() {
                    self.test_movie_requested = true;
                }
            }
        }
    }

    // ---- layers and frames -----------------------------------------------------------

    /// The layer frame operations act on: the chosen one, else the first.
    fn active_layer(&self) -> Option<&Layer> {
        let layers = self.document().scenes().first()?.timeline().layers();
        self.current_layer
            .and_then(|id| layers.iter().find(|layer| layer.id() == id))
            .or_else(|| layers.first())
    }

    pub fn active_layer_id(&self) -> Option<LayerId> {
        self.active_layer().map(Layer::id)
    }

    pub fn select_layer(&mut self, layer: LayerId) {
        self.current_layer = Some(layer);
    }

    fn layer_command(&mut self, what: &str, make: impl FnOnce(SceneId, LayerId) -> Command) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        self.run_batch(vec![make(scene, layer)], what);
        self.validate_selection();
    }

    pub fn toggle_layer_visible(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_visible) else {
            return;
        };
        let change = LayerChange::Visible(!current);
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change,
            }],
            "change visibility",
        );
    }

    pub fn toggle_layer_locked(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_locked) else {
            return;
        };
        let change = LayerChange::Locked(!current);
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change,
            }],
            "change the lock",
        );
        self.validate_selection();
    }

    /// Turns the layer into a mask for the layer below it, or back into a normal layer.
    pub fn toggle_layer_mask(&mut self, layer: LayerId) {
        let Some(scene) = self.scene_id() else { return };
        let Some(current) = self.layer(layer).map(Layer::is_mask) else {
            return;
        };
        self.run_batch(
            vec![Command::UpdateLayer {
                scene,
                layer,
                change: LayerChange::Mask(!current),
            }],
            "change the mask",
        );
        self.validate_selection();
    }

    fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.document()
            .scenes()
            .first()?
            .timeline()
            .layers()
            .iter()
            .find(|l| l.id() == id)
    }

    pub fn rename_active_layer(&mut self, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            self.report("A layer needs a name");
            return;
        }
        let name = name.to_owned();
        self.layer_command("rename the layer", |scene, layer| Command::UpdateLayer {
            scene,
            layer,
            change: LayerChange::Name(name),
        });
    }

    pub fn delete_active_layer(&mut self) {
        let count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        if count <= 1 {
            self.report("A scene needs at least one layer");
            return;
        }
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer_id()) else {
            return;
        };
        // Sounds only this layer played go with it, so deleting it leaves nothing behind.
        let mut commands = vec![Command::RemoveLayer { scene, layer }];
        commands.extend(
            self.orphaned_sounds(layer)
                .into_iter()
                .map(Command::RemoveSound),
        );
        self.run_batch(commands, "delete the layer");
        self.validate_selection();
        self.current_layer = None;
    }

    /// Moves the active layer up (`-1`) or down (`+1`) in the stacking order.
    pub fn move_active_layer(&mut self, direction: i32) {
        let Some(id) = self.active_layer_id() else {
            return;
        };
        let Some(layers) = self
            .document()
            .scenes()
            .first()
            .map(|s| s.timeline().layers())
        else {
            return;
        };
        let Some(from) = layers.iter().position(|l| l.id() == id) else {
            return;
        };
        let to = if direction < 0 {
            from.checked_sub(1)
        } else {
            Some(from + 1)
        };
        match to {
            Some(index) if index < layers.len() => {
                self.layer_command("move the layer", |scene, layer| Command::MoveLayer {
                    scene,
                    layer,
                    index,
                });
            }
            _ => self.report("The layer is already at the edge"),
        }
    }

    /// Runs a frame operation on the active layer at the current frame.
    pub fn frame_edit(&mut self, op: FrameOp) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer()) else {
            return;
        };
        if layer.is_locked() {
            self.report(format!("Cannot {}: the layer is locked", op.label()));
            return;
        }
        let commands = match op {
            FrameOp::InsertFrame => frames::insert_frame(scene, layer, self.frame),
            FrameOp::RemoveFrame => frames::remove_frame(scene, layer, self.frame),
            FrameOp::InsertKeyframe => frames::insert_keyframe(scene, layer, self.frame),
            FrameOp::InsertBlankKeyframe => frames::insert_blank_keyframe(scene, layer, self.frame),
            FrameOp::ClearKeyframe => frames::clear_keyframe(scene, layer, self.frame),
        };
        match commands {
            Some(commands) => {
                self.run_batch(commands, op.label());
                self.set_frame(self.frame);
                self.validate_selection();
            }
            None => self.report(format!(
                "Cannot {} at frame {} of this layer",
                op.label(),
                self.frame
            )),
        }
    }

    // ---- dragging spans in the timeline -----------------------------------------------

    /// Starts dragging the span of `layer` under the fractional frame position `at`: its right
    /// edge (within `edge` frames) resizes, the rest moves. False when there is no span there
    /// or the layer is locked, in which case the drag should scrub instead.
    fn begin_span_drag(&mut self, layer: LayerId, at: f32, edge: f32) -> bool {
        if !at.is_finite() || at < 0.0 {
            return false;
        }
        let frame = at.floor() as u32;
        let Some(found) = self.layer(layer) else {
            return false;
        };
        if found.is_locked() {
            return false;
        }
        let Some(span) = found
            .spans()
            .iter()
            .find(|s| s.start() <= frame && u64::from(frame) < s.end())
        else {
            return false;
        };
        let grab = if span.end() as f32 - at <= edge {
            SpanGrab::Resize
        } else {
            SpanGrab::Move {
                offset: frame - span.start(),
            }
        };
        self.span_drag = Some(SpanDrag {
            layer,
            start: span.start(),
            grab,
            frame,
        });
        true
    }

    fn update_span_drag(&mut self, frame: u32) {
        if let Some(drag) = &mut self.span_drag {
            drag.frame = frame;
        }
    }

    /// Where the dragged span would land: (layer, start, duration).
    fn span_drag_preview(&self) -> Option<(LayerId, u32, u32)> {
        let drag = self.span_drag?;
        let span = self
            .layer(drag.layer)?
            .spans()
            .iter()
            .find(|s| s.start() == drag.start)?;
        Some(match drag.grab {
            SpanGrab::Move { offset } => (
                drag.layer,
                drag.frame.saturating_sub(offset),
                span.duration().get(),
            ),
            SpanGrab::Resize => (
                drag.layer,
                drag.start,
                drag.frame
                    .saturating_add(1)
                    .saturating_sub(drag.start)
                    .max(1),
            ),
        })
    }

    fn end_span_drag(&mut self) {
        let Some((layer, start, duration)) = self.span_drag_preview() else {
            self.span_drag = None;
            return;
        };
        let Some(drag) = self.span_drag.take() else {
            return;
        };
        let (Some(scene), Some(found)) = (self.scene_id(), self.layer(layer)) else {
            return;
        };
        let (commands, what) = match drag.grab {
            SpanGrab::Move { .. } => (frames::move_span(scene, found, drag.start, start), "move"),
            SpanGrab::Resize => (
                frames::resize_span(scene, found, drag.start, duration),
                "resize",
            ),
        };
        let unchanged = match drag.grab {
            SpanGrab::Move { .. } => start == drag.start,
            SpanGrab::Resize => found
                .spans()
                .iter()
                .find(|s| s.start() == drag.start)
                .is_some_and(|s| s.duration().get() == duration),
        };
        match commands {
            Some(commands) => {
                self.run_batch(commands, what);
                self.set_frame(self.frame);
                self.validate_selection();
            }
            None if !unchanged => self.report(format!(
                "Cannot {what} the span there: it would overlap another one"
            )),
            None => {}
        }
    }

    // ---- tools and selection ---------------------------------------------------------

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.cancel_drag();
            self.pen.clear();
            self.text_edit = None;
            self.tool = tool;
        }
    }

    pub fn has_selection(&self) -> bool {
        self.selected.is_some()
    }

    fn scene_id(&self) -> Option<SceneId> {
        self.document().scenes().first().map(|scene| scene.id())
    }

    /// Drops a selection whose span no longer exists (after undo, delete or open).
    fn validate_selection(&mut self) {
        self.selection_checked = Some((self.context_serial, self.active().revision()));
        if self
            .frame_range
            .is_some_and(|r| self.layer(r.layer).is_none())
        {
            self.frame_range = None;
        }
        if let Some(scene) = self.scene_id() {
            // One pass over the layers answers for every selected span.
            let editable: std::collections::HashSet<(LayerId, u32)> = self
                .document()
                .scenes()
                .iter()
                .find(|s| s.id() == scene)
                .into_iter()
                .flat_map(|s| s.timeline().layers())
                .filter(|l| l.is_visible() && !l.is_locked())
                .flat_map(|l| l.spans().iter().map(|span| (l.id(), span.start())))
                .collect();
            let mut all: Vec<SpanRef> = self.selection_all();
            all.retain(|span| editable.contains(&(span.layer, span.start)));
            self.selected = all.first().copied();
            self.also_selected = all.into_iter().skip(1).collect();
        } else {
            self.selected = None;
            self.also_selected.clear();
        }
        if let Some(symbol) = self.selected_symbol
            && self.document().symbol(symbol).is_none()
        {
            self.selected_symbol = None;
        }
    }

    /// The scene-level span of the topmost shape under `point` at the current frame,
    /// looking through locked layers.
    fn hit(&self, point: Pos2) -> Option<SpanRef> {
        let scene = self.scene_id()?;
        let frame = flare_eval::evaluate_frame(self.document(), scene, self.frame).ok()?;
        let at = DVec2::new(f64::from(point.x), f64::from(point.y));
        let timeline = self.document().scenes().first()?.timeline();
        flare_render::hit_test_all(&frame, at)
            .into_iter()
            .filter_map(|index| {
                ops::locate_hit(self.document(), scene, self.frame, &frame.shapes[index])
            })
            .find(|span| {
                timeline
                    .layers()
                    .iter()
                    .find(|l| l.id() == span.layer)
                    .is_some_and(|l| !l.is_locked())
            })
    }

    /// Every selected span, the primary one first.
    fn selection_all(&self) -> Vec<SpanRef> {
        self.selected
            .into_iter()
            .chain(self.also_selected.iter().copied())
            .collect()
    }

    /// Selects what is at `point`. With `additive` (Shift) the span is toggled in or out of
    /// the selection; without it the selection becomes just that span, or empty.
    fn select_at(&mut self, point: Pos2, additive: bool) {
        let hit = self.hit(point);
        if !additive {
            self.selected = hit;
            self.also_selected.clear();
            return;
        }
        let Some(hit) = hit else { return };
        let mut all = self.selection_all();
        match all.iter().position(|s| *s == hit) {
            Some(i) => {
                all.remove(i);
            }
            None => all.push(hit),
        }
        self.selected = all.first().copied();
        self.also_selected = all.into_iter().skip(1).collect();
    }

    #[cfg(test)]
    fn begin_drag(&mut self, point: Pos2) {
        self.begin_drag_with(point, false);
    }

    fn begin_drag_with(&mut self, point: Pos2, additive: bool) {
        self.drag_cancelled = false;
        let mode = match self.tool {
            Tool::Rectangle | Tool::Ellipse | Tool::Line | Tool::PolyStar => Some(DragMode::Draw),
            // The pen builds a path point by point; see `pen_press`.
            Tool::Pen | Tool::Text => None,
            Tool::Hand
            | Tool::Zoom
            | Tool::Eyedropper
            | Tool::Bucket
            | Tool::InkBottle
            | Tool::Eraser
            | Tool::Lasso
            | Tool::Pencil
            | Tool::Brush => None,
            Tool::Select => {
                let grabbed = if additive {
                    None
                } else {
                    self.selection_bounds()
                        .and_then(|b| handles::pick(b, point, self.handle_radius).map(|h| (h, b)))
                };
                if let Some((handle, bounds)) = grabbed {
                    // Alt turns an edge handle from scaling into skewing.
                    let mode = if self.skew_modifier && handle.is_edge() {
                        DragMode::Skew(handle)
                    } else {
                        DragMode::Scale(handle)
                    };
                    self.drag = Some(Drag {
                        bounds,
                        mode,
                        start: point,
                        current: point,
                    });
                    return;
                }
                if !additive
                    && let Some(bounds) = self.selection_bounds()
                    && handles::in_rotate_zone(bounds, point, self.handle_radius)
                {
                    self.drag = Some(Drag {
                        bounds,
                        mode: DragMode::Rotate,
                        start: point,
                        current: point,
                    });
                    return;
                }
                // Pressing on something already selected drags the whole selection.
                let on_selection = !additive
                    && self
                        .hit(point)
                        .is_some_and(|hit| self.selection_all().contains(&hit));
                if !on_selection {
                    self.select_at(point, additive);
                }
                // Shift only edits the selection; it never starts a move.
                (!additive && self.selected.is_some()).then_some(DragMode::Move)
            }
        };
        self.drag = mode.map(|mode| Drag {
            bounds: Rect::NOTHING,
            mode,
            start: point,
            current: point,
        });
    }

    /// A click with one of the tools that act on a single shape: it changes that shape's paint,
    /// takes its color, or deletes it, as one undo step.
    fn click_with_tool(&mut self, point: Pos2, shift: bool) {
        if !matches!(
            self.tool,
            Tool::Eyedropper | Tool::Bucket | Tool::InkBottle | Tool::Eraser
        ) {
            return;
        }
        let (Some(scene), Some(at)) = (self.scene_id(), self.hit(point)) else {
            return;
        };
        let to_rgba = |c: Color32| {
            let c = c.to_array();
            Rgba8::new(c[0], c[1], c[2], c[3])
        };
        let commands = match self.tool {
            Tool::Eraser => Some(ops::delete(scene, at)),
            Tool::Bucket => {
                ops::restyle(self.document(), scene, at, Some(to_rgba(self.fill)), None)
            }
            Tool::InkBottle => {
                match ShapeStroke::new(f64::from(self.stroke_width), to_rgba(self.stroke_color)) {
                    Ok(stroke) => {
                        ops::restyle(self.document(), scene, at, None, Some(Some(stroke)))
                    }
                    Err(error) => return self.report(error.to_string()),
                }
            }
            _ => {
                let Some(Content::Shape(shape)) =
                    ops::find_span(self.document(), scene, at).map(|s| s.content().clone())
                else {
                    return;
                };
                let from = |c: Rgba8| Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
                match (shift, shape.stroke()) {
                    (true, Some(stroke)) => self.stroke_color = from(stroke.color()),
                    (true, None) => {}
                    (false, _) => self.fill = from(shape.flat_fill()),
                }
                None
            }
        };
        if let Some(commands) = commands {
            self.run_batch(commands, "paint");
        }
    }

    /// Commits the pencil or brush stroke as one undo step: a smoothed open path stroked with
    /// round caps. The brush paints with the fill color at the brush size; the pencil uses the
    /// stroke color and width.
    fn finish_freehand(&mut self, scale: f32) {
        let points = std::mem::take(&mut self.freehand);
        let tolerance = self.smoothing / scale.max(f32::EPSILON);
        let points = flare_tools::freehand::simplify(&points, tolerance);
        let Some(path) = flare_tools::freehand::smooth_path(&points) else {
            return; // a click, not a drag
        };
        let (color, width, name) = match self.tool {
            Tool::Brush => (self.fill, self.brush_size, "Brush stroke"),
            _ => (self.stroke_color, self.stroke_width, "Pencil line"),
        };
        let c = color.to_array();
        let stroke = ShapeStroke::new(f64::from(width), Rgba8::new(c[0], c[1], c[2], c[3]));
        match stroke.map_err(|e| e.to_string()).and_then(|stroke| {
            Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
                .map(|shape| shape.with_stroke(stroke))
                .map_err(|e| e.to_string())
        }) {
            Ok(shape) => self.insert_shape_layer(shape, name),
            Err(error) => self.report(error),
        }
    }

    /// Selects what the finished lasso loop encloses; with `additive` it adds to the selection.
    fn finish_lasso(&mut self, additive: bool) {
        let polygon = std::mem::take(&mut self.lasso);
        let Some(scene) = self.scene_id() else { return };
        let found = ops::spans_in_polygon(self.document(), scene, self.frame, &polygon);
        let mut all = if additive {
            self.selection_all()
        } else {
            Vec::new()
        };
        for span in found {
            if !all.contains(&span) {
                all.push(span);
            }
        }
        self.selected = all.first().copied();
        self.also_selected = all.into_iter().skip(1).collect();
    }

    fn update_drag(&mut self, point: Pos2) {
        if let Some(drag) = &mut self.drag {
            drag.current = point;
        }
    }

    fn end_drag(&mut self) {
        let cancelled = std::mem::take(&mut self.drag_cancelled);
        if let Some(drag) = self.drag.take()
            && !cancelled
        {
            match drag.mode {
                DragMode::Draw => self.commit_shape(self.tool, drag.start, drag.current),
                DragMode::Move => {
                    let mut delta = drag.current - drag.start;
                    if self.snap_to_grid {
                        delta = snap_point(delta.to_pos2(), self.grid_size).to_vec2();
                    }
                    self.move_selection(delta);
                }
                DragMode::Scale(handle) => {
                    self.scale_selection(handle, drag.bounds, drag.current);
                }
                DragMode::Rotate | DragMode::Skew(_) => {
                    if let Some(matrix) = self.drag_matrix(&drag) {
                        self.apply_to_selection(matrix);
                    }
                }
            }
        }
    }

    /// Escape, tool switches and focus loss cancel a gesture without committing it.
    fn cancel_drag(&mut self) {
        self.span_drag = None;
        self.lasso.clear();
        self.freehand.clear();
        if self.drag.take().is_some() {
            self.drag_cancelled = true;
        }
    }

    fn run_batch(&mut self, commands: Vec<Command>, what: &str) {
        match self.active_mut().apply(commands) {
            Ok(()) => self.status = None,
            Err(error) => self.report(format!("Cannot {what}: {error}")),
        }
    }

    fn move_selection(&mut self, delta: Vec2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
        let commands: Vec<Command> = self
            .selection_all()
            .into_iter()
            .filter_map(|span| ops::translate(self.document(), scene, span, delta))
            .flatten()
            .collect();
        if !commands.is_empty() {
            self.run_batch(commands, "move");
        }
    }

    /// Moves the selection so its shapes line up on one another's extent, or on the stage when
    /// "Align to stage" is on, as one undo step.
    pub fn align_selection(&mut self, how: ops::Align) {
        let Some((scene, spans, rects)) = self.selection_with_bounds() else {
            return;
        };
        let target = if self.align_to_stage {
            Rect::from_min_size(Pos2::ZERO, self.stage_extent())
        } else {
            rects
                .iter()
                .copied()
                .reduce(|a, b| a.union(b))
                .unwrap_or(Rect::NOTHING)
        };
        let deltas = ops::align_deltas(&rects, how, target);
        self.move_each(scene, &spans, &deltas);
    }

    /// Spaces three or more selected shapes evenly along an axis.
    pub fn distribute_selection(&mut self, horizontal: bool) {
        let Some((scene, spans, rects)) = self.selection_with_bounds() else {
            return;
        };
        if spans.len() < 3 {
            self.report("Distributing needs at least three shapes selected");
            return;
        }
        let deltas = ops::distribute_deltas(&rects, horizontal);
        self.move_each(scene, &spans, &deltas);
    }

    /// Selected spans that have bounds, with those bounds, in selection order.
    fn selection_with_bounds(&self) -> Option<(SceneId, Vec<SpanRef>, Vec<Rect>)> {
        let scene = self.scene_id()?;
        let (spans, rects): (Vec<_>, Vec<_>) = self
            .selection_all()
            .into_iter()
            .filter_map(|at| Some((at, ops::span_bounds(self.document(), scene, at)?)))
            .unzip();
        (!spans.is_empty()).then_some((scene, spans, rects))
    }

    fn move_each(&mut self, scene: SceneId, spans: &[SpanRef], deltas: &[Vec2]) {
        let commands: Vec<Command> = spans
            .iter()
            .zip(deltas)
            .filter_map(|(at, d)| {
                ops::translate(
                    self.document(),
                    scene,
                    *at,
                    DVec2::new(f64::from(d.x), f64::from(d.y)),
                )
            })
            .flatten()
            .collect();
        if !commands.is_empty() {
            self.run_batch(commands, "align");
        }
    }

    /// Moves the selected shapes' layers to the top (`front`) or the bottom of the stack,
    /// keeping their order among themselves, as one undo step.
    pub fn arrange_selection(&mut self, front: bool) {
        let Some(scene) = self.scene_id() else { return };
        let Some(timeline) = self.document().scenes().first().map(|s| s.timeline()) else {
            return;
        };
        let count = timeline.layers().len();
        let mut positions: Vec<(usize, LayerId)> = self
            .selection_all()
            .iter()
            .filter_map(|at| {
                let p = timeline.layers().iter().position(|l| l.id() == at.layer)?;
                Some((p, at.layer))
            })
            .collect();
        positions.sort();
        positions.dedup();
        if front {
            positions.reverse();
        }
        let commands: Vec<Command> = positions
            .into_iter()
            .map(|(_, layer)| Command::MoveLayer {
                scene,
                layer,
                index: if front { 0 } else { count.saturating_sub(1) },
            })
            .collect();
        if !commands.is_empty() {
            self.run_batch(commands, "arrange");
        }
    }

    /// Applies the Transform panel's numbers to the selection about the center of its bounds
    /// and resets them, as one undo step.
    pub fn apply_transform_edit(&mut self) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        if let Some(matrix) = self.transform_edit.matrix(bounds.center()) {
            self.apply_to_selection(matrix);
        }
        self.transform_edit = TransformEdit::default();
    }

    /// Moves and resizes the selection so its bounds become `target`, as one undo step.
    pub fn set_selection_bounds(&mut self, target: Rect) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        if bounds.width() < 1e-3
            || bounds.height() < 1e-3
            || target.width() < 1.0
            || target.height() < 1.0
        {
            return;
        }
        let scale = DVec2::new(
            f64::from(target.width() / bounds.width()),
            f64::from(target.height() / bounds.height()),
        );
        let (from, to) = (
            DVec2::new(f64::from(bounds.min.x), f64::from(bounds.min.y)),
            DVec2::new(f64::from(target.min.x), f64::from(target.min.y)),
        );
        let matrix = DAffine2::from_translation(to)
            * DAffine2::from_scale(scale)
            * DAffine2::from_translation(-from);
        if matrix != DAffine2::IDENTITY {
            self.apply_to_selection(matrix);
        }
    }

    /// Swaps the fill and stroke colors.
    pub fn swap_colors(&mut self) {
        std::mem::swap(&mut self.fill, &mut self.stroke_color);
    }

    /// Black stroke, white fill.
    pub fn default_colors(&mut self) {
        self.stroke_color = Color32::BLACK;
        self.fill = Color32::WHITE;
    }

    /// Applies `matrix` to every selected span about the center of the selection's bounds, as
    /// one undo step. The y axis points down, so a positive angle turns clockwise on screen.
    pub fn transform_selection(&mut self, matrix: DAffine2) {
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        let center = DVec2::new(f64::from(bounds.center().x), f64::from(bounds.center().y));
        let about =
            DAffine2::from_translation(center) * matrix * DAffine2::from_translation(-center);
        self.apply_to_selection(about);
    }

    /// The union of the bounds of every selected span.
    fn selection_bounds(&self) -> Option<Rect> {
        self.selection_rects().into_iter().reduce(|a, b| a.union(b))
    }

    /// The bounds of every selected span. Kept until the document or the selection changes, so
    /// painting a big selection each frame costs nothing after the first.
    fn selection_rects(&self) -> Vec<Rect> {
        use std::hash::{Hash, Hasher};
        let spans = self.selection_all();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        for span in &spans {
            span.layer.hash(&mut hasher);
            span.start.hash(&mut hasher);
        }
        let stamp = (
            self.context_serial,
            self.active().revision(),
            hasher.finish(),
        );
        if let Some((cached, rects)) = &*self.selection_rects_cache.borrow()
            && *cached == stamp
        {
            return rects.clone();
        }
        let rects = self
            .scene_id()
            .map(|scene| ops::spans_bounds(self.document(), scene, &spans))
            .unwrap_or_default();
        *self.selection_rects_cache.borrow_mut() = Some((stamp, rects.clone()));
        rects
    }

    /// The transform a rotate or skew drag stands for right now, if it stands for one.
    fn drag_matrix(&self, drag: &Drag) -> Option<DAffine2> {
        match drag.mode {
            DragMode::Rotate => {
                let center = drag.bounds.center();
                let angle = handles::rotation_between(
                    center,
                    drag.start,
                    drag.current,
                    self.uniform_scale,
                )?;
                (angle != 0.0).then(|| handles::rotation_matrix(center, angle))
            }
            DragMode::Skew(handle) => {
                let k = handles::skew_for(drag.bounds, handle, drag.current)?;
                (k != 0.0).then(|| handles::skew_matrix(drag.bounds, handle, k))
            }
            DragMode::Draw | DragMode::Move | DragMode::Scale(_) => None,
        }
    }

    /// Scales the selection so `handle` of `bounds` moves to `pointer`, as one undo step.
    fn scale_selection(&mut self, handle: Handle, bounds: Rect, pointer: Pos2) {
        if let Some(scale) = handles::scale_for(bounds, handle, pointer, self.uniform_scale) {
            if scale == (1.0, 1.0) {
                return;
            }
            self.apply_to_selection(handles::matrix(bounds, handle, scale));
        }
    }

    /// Maps every selected span through `about` (stage coordinates), as one undo step.
    fn apply_to_selection(&mut self, about: DAffine2) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let spans = self.selection_all();
        let commands: Vec<Command> = spans
            .into_iter()
            .filter_map(|span| ops::transform(self.document(), scene, span, about))
            .flatten()
            .collect();
        if commands.is_empty() {
            if about != DAffine2::IDENTITY {
                self.report(
                    "Nothing could be transformed (an instance in a motion tween cannot be skewed or mirrored)",
                );
            }
        } else {
            self.run_batch(commands, "transform");
        }
    }

    /// Copies the selection onto a new layer, nudged so it can be seen, and selects the copy.
    pub fn duplicate_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let offset = DVec2::new(DUPLICATE_OFFSET, DUPLICATE_OFFSET);
        let mut commands = Vec::new();
        let mut copies = Vec::new();
        for span in self.selection_all() {
            if let Some((batch, copy)) = ops::duplicate(self.document(), scene, span, offset) {
                commands.extend(batch);
                copies.push(copy);
            }
        }
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "duplicate");
        if self.status.is_none() {
            self.selected = copies.first().copied();
            self.also_selected = copies.iter().skip(1).copied().collect();
            self.current_layer = self.selected.map(|s| s.layer);
        }
    }

    /// Selects frames `a..=b` (either order) of `layer` for copying.
    pub fn select_frames(&mut self, layer: LayerId, a: u32, b: u32) {
        if self.layer(layer).is_some() {
            self.frame_range = Some(FrameRange {
                layer,
                from: a.min(b),
                to: a.max(b),
            });
        }
    }

    /// Remembers the selected frames (or, with none selected, the current frame) of the active
    /// layer for Paste Frames.
    pub fn copy_frames(&mut self) {
        let (layer, from, to) = match self.frame_range {
            Some(r) => (r.layer, r.from, r.to),
            None => match self.active_layer_id() {
                Some(id) => (id, self.frame, self.frame),
                None => return,
            },
        };
        match self
            .layer(layer)
            .and_then(|l| frames::copy_frames(l, from, to))
        {
            Some(clip) => self.frame_clipboard = Some(clip),
            None => self.report("There is nothing to copy in those frames"),
        }
    }

    /// Pastes the remembered frames onto the active layer starting at the current frame,
    /// replacing what is there, and selects what was pasted.
    pub fn paste_frames(&mut self) {
        let (Some(scene), Some(layer), Some(clip)) = (
            self.scene_id(),
            self.active_layer(),
            self.frame_clipboard.clone(),
        ) else {
            return;
        };
        if layer.is_locked() {
            self.report("Cannot paste frames: the layer is locked");
            return;
        }
        let id = layer.id();
        match frames::paste_frames(scene, layer, self.frame, &clip) {
            Some(commands) => {
                self.run_batch(commands, "paste frames");
                if self.status.is_none() {
                    self.select_frames(
                        id,
                        self.frame,
                        self.frame.saturating_add(clip.length() - 1),
                    );
                    self.set_frame(self.frame);
                    self.validate_selection();
                }
            }
            None => self.report("Cannot paste frames there: they would run past the last frame"),
        }
    }

    /// Remembers every selected span for Paste.
    pub fn copy_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let items: Vec<ops::ClipItem> = self
            .selection_all()
            .into_iter()
            .filter_map(|span| ops::copy_span(self.document(), scene, span))
            .collect();
        if !items.is_empty() {
            self.clipboard = items;
        }
    }

    /// Copy, then delete: one undo step restores what was cut.
    pub fn cut_selection(&mut self) {
        if !self.has_selection() {
            return;
        }
        self.copy_selection();
        self.delete_selection();
    }

    /// Selects everything on unlocked layers at the current frame.
    pub fn select_all(&mut self) {
        let Some(scene) = self.scene_id() else { return };
        let all = ops::spans_at_frame(self.document(), scene, self.frame);
        self.selected = all.first().copied();
        self.also_selected = all.into_iter().skip(1).collect();
    }

    pub fn clear_selection(&mut self) {
        self.selected = None;
        self.also_selected.clear();
    }

    /// Pastes the remembered spans in place at the current frame, each on its own new layer,
    /// and selects them.
    pub fn paste(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let mut commands = Vec::new();
        let mut pasted = Vec::new();
        for item in &self.clipboard {
            match ops::paste(self.document(), scene, item, self.frame) {
                Some((batch, at)) => {
                    commands.extend(batch);
                    pasted.push(at);
                }
                None => {
                    self.report("Cannot paste: its symbol is not in this document");
                    return;
                }
            }
        }
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "paste");
        if self.status.is_none() {
            self.selected = pasted.first().copied();
            self.also_selected = pasted.iter().skip(1).copied().collect();
            self.current_layer = self.selected.map(|s| s.layer);
        }
    }

    pub fn delete_selection(&mut self) {
        let Some(scene) = self.scene_id() else {
            return;
        };
        let commands: Vec<Command> = self
            .selection_all()
            .into_iter()
            .flat_map(|span| ops::delete(scene, span))
            .collect();
        if commands.is_empty() {
            return;
        }
        self.run_batch(commands, "delete");
        self.validate_selection();
    }

    /// The selected span's tween, when it is an instance (the only thing that can tween).
    pub fn selection_tween(&self) -> Option<Tween> {
        let (selected, scene) = (self.selected?, self.scene_id()?);
        let span = ops::find_span(self.document(), scene, selected)?;
        matches!(span.content(), Content::Instance(_)).then(|| span.tween())
    }

    /// Turns the motion tween on the selected instance span on or off.
    pub fn set_selection_tween(&mut self, tween: Tween) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        match ops::set_tween(self.document(), scene, selected, tween) {
            Some(commands) => self.run_batch(commands, "change the tween"),
            None => self.report("Select an instance to change its motion tween"),
        }
    }

    pub fn can_convert_selection(&self) -> bool {
        match (self.selected, self.scene_id()) {
            (Some(selected), Some(scene)) => {
                ops::convert_to_symbol(self.document(), scene, selected).is_some()
            }
            _ => false,
        }
    }

    /// Convert to Symbol: the selected shape becomes a library symbol with one instance.
    pub fn convert_selection_to_symbol(&mut self) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        let Some((commands, symbol)) = ops::convert_to_symbol(self.document(), scene, selected)
        else {
            self.report("Select a shape to convert to a symbol");
            return;
        };
        self.run_batch(commands, "convert to symbol");
        if self.status.is_none() {
            self.selected_symbol = Some(symbol);
        }
    }

    /// Adds an instance of the library-selected symbol, centered on the stage.
    /// Adds an empty symbol to the library and selects it. Library edits belong to the main
    /// document, so they wait until symbol editing is left.
    pub fn new_symbol(&mut self) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the library");
            return;
        }
        let taken: std::collections::HashSet<String> = self
            .document()
            .symbols()
            .iter()
            .map(|s| s.name().to_owned())
            .collect();
        let name = (1..)
            .map(|n| format!("Symbol {n}"))
            .find(|name| !taken.contains(name))
            .unwrap_or_default();
        let id = SymbolId::new();
        let symbol = flare_document::Symbol::new(id, name, flare_document::Timeline::default());
        self.run_batch(vec![Command::AddSymbol(symbol)], "add the symbol");
        if self.status.is_none() {
            self.selected_symbol = Some(id);
        }
    }

    /// Renames a library symbol; a blank name is ignored.
    pub fn rename_symbol(&mut self, id: SymbolId, name: &str) {
        let name = name.trim();
        if name.is_empty() || self.in_symbol() {
            return;
        }
        if self.document().symbol(id).is_some_and(|s| s.name() == name) {
            return;
        }
        let command = Command::RenameSymbol {
            symbol: id,
            name: name.to_owned(),
        };
        self.run_batch(vec![command], "rename the symbol");
    }

    /// Deletes a library symbol; the edit is refused while an instance still uses it.
    pub fn delete_symbol(&mut self, id: SymbolId) {
        if self.in_symbol() {
            self.report("Leave symbol editing to change the library");
            return;
        }
        self.run_batch(vec![Command::RemoveSymbol(id)], "delete the symbol");
        if self.selected_symbol == Some(id) {
            self.selected_symbol = None;
        }
    }

    pub fn place_selected_symbol(&mut self) {
        let (Some(symbol), Some(scene)) = (self.selected_symbol, self.scene_id()) else {
            self.report("Select a symbol in the library first");
            return;
        };
        if self.contexts.iter().any(|c| c.symbol == symbol) {
            self.report("A symbol cannot contain itself");
            return;
        }
        let stage = self.document().stage();
        let center = DVec2::new(
            f64::from(stage.width()) / 2.0,
            f64::from(stage.height()) / 2.0,
        );
        if let Some(commands) = ops::place_instance(self.document(), scene, symbol, center) {
            self.run_batch(commands, "place instance");
        }
    }

    // ---- playback --------------------------------------------------------------------

    fn advance(&mut self, now: f64) {
        let last = self.last_time.replace(now);
        if !self.playing {
            return;
        }
        let Some(last) = last else { return };
        let rate = self.document().frame_rate();
        let fps = f64::from(rate.numerator()) / f64::from(rate.denominator());
        self.play_remainder += (now - last).max(0.0) * fps;
        let whole = self.play_remainder.floor();
        self.play_remainder -= whole;
        let frames = self.scene_frames();
        // `whole` is small (one UI frame's worth), so the cast cannot overflow.
        let next = u64::from(self.frame) + whole as u64;
        if !self.loop_playback && next >= u64::from(frames) {
            self.frame = frames.saturating_sub(1);
            self.playing = false;
            return;
        }
        self.frame = next.rem_euclid(u64::from(frames)) as u32;
    }

    // ---- rendering -------------------------------------------------------------------

    /// Onion skinning is off while playing, where it would only cost time.
    fn effective_onion(&self) -> OnionSkin {
        if self.playing {
            OnionSkin::OFF
        } else {
            self.onion
        }
    }

    pub fn onion_skin(&self) -> OnionSkin {
        self.onion
    }

    pub fn set_onion_skin(&mut self, enabled: bool) {
        self.onion.enabled = enabled;
    }

    pub fn set_onion_range(&mut self, before: u32, after: u32) {
        self.onion.before = before.min(MAX_ONION_FRAMES);
        self.onion.after = after.min(MAX_ONION_FRAMES);
    }

    /// The part of the stage to render and at what size: what is on screen, at the resolution it
    /// is shown, as of the last layout. Before any layout it is the whole stage.
    fn desired_texture_view(&self, pixels_per_point: f32, max_side: u32) -> Option<TextureView> {
        let extent = self.stage_extent();
        let (region, density) = match self.stage_view.visible_region(self.stage_panel, extent) {
            Some(region) => {
                let scale = self.stage_view.scale(self.stage_panel, extent);
                (region, (scale * pixels_per_point).max(f32::EPSILON))
            }
            None if self.stage_panel == Rect::NOTHING => {
                (Rect::from_min_size(Pos2::ZERO, extent), 1.0)
            }
            None => return None,
        };
        let side = |length: f32| ((length * density).ceil() as u32).clamp(1, max_side.max(1));
        Some(TextureView {
            region,
            size: [side(region.width()), side(region.height())],
        })
    }

    /// Keeps the stage texture matching what should be on screen. A picture that is already
    /// cached is shown at once; otherwise the frame is drawn on the render worker and the
    /// previous picture stays up (drawn where it belongs) until the new one arrives.
    fn ensure_stage_texture(&mut self, ctx: &egui::Context) {
        self.install_rendered(ctx);
        let key = (self.context_serial, self.active().revision(), self.frame);
        let onion = self.effective_onion();
        let Some(view) = self.desired_texture_view(ctx.pixels_per_point(), texture_side_limit(ctx))
        else {
            // Nothing of the stage is on screen: keep the last texture, there is nothing to draw.
            return;
        };
        if self
            .texture
            .as_ref()
            .is_some_and(|t| t.key == key && t.onion == onion && t.view == view)
        {
            return;
        }
        if self
            .pending_render
            .as_ref()
            .is_some_and(|p| p.key == key && p.onion == onion && p.view == view)
        {
            return; // already being drawn
        }
        let Some(scene) = self.document().scenes().first().map(|s| s.id()) else {
            self.texture = None;
            return;
        };
        let ghost_frames = if onion.enabled {
            onion_ghosts(self.frame, self.scene_frames(), onion.before, onion.after)
        } else {
            Vec::new()
        };
        let (context, revision) = (key.0, key.1);
        // What the frame draws, to see whether the same picture is already around. Evaluating a
        // frame is cheap next to drawing it.
        let signature = {
            use std::hash::{Hash, Hasher};
            let document = self.document();
            let current = match flare_eval::evaluate_frame(document, scene, self.frame) {
                Ok(frame) => frame,
                Err(e) => {
                    self.report(format!("Cannot render frame: {e}"));
                    return;
                }
            };
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            current.signature().hash(&mut hasher);
            for (frame, opacity) in &ghost_frames {
                if let Ok(ghost) = flare_eval::evaluate_frame(document, scene, *frame) {
                    ghost.signature().hash(&mut hasher);
                    opacity.to_bits().hash(&mut hasher);
                }
            }
            hasher.finish()
        };
        let hit = self.frame_cache.iter().position(|c| {
            c.context == context
                && c.revision == revision
                && c.signature == signature
                && c.view == view
        });
        if let Some(index) = hit {
            // Most recently used goes last.
            let entry = self.frame_cache.remove(index);
            let handle = entry.handle.clone();
            self.frame_cache.push(entry);
            self.pending_render = None;
            self.texture = Some(StageTexture {
                handle,
                view,
                key,
                onion,
            });
            return;
        }
        let id = self.background.next_id();
        self.pending_render = Some(PendingRender {
            id,
            key,
            onion,
            view,
            signature,
        });
        let stage = self.document().stage();
        self.background.submit_render(RenderJob {
            id,
            document: self.active().snapshot(),
            scene,
            frame: self.frame,
            ghosts: ghost_frames,
            viewport: flare_render::Viewport {
                x: f64::from(view.region.min.x),
                y: f64::from(view.region.min.y),
                width: f64::from(view.region.width()),
                height: f64::from(view.region.height()),
            },
            size: view.size,
            background: stage.background(),
        });
        // Inline, the picture is ready already; with workers it arrives on a later pass.
        self.settle_inline();
        self.install_rendered(ctx);
    }

    /// Turns finished drawings into textures. Only the frame asked for most recently is kept:
    /// anything else was overtaken while it was being drawn.
    fn install_rendered(&mut self, ctx: &egui::Context) {
        for (id, result) in std::mem::take(&mut self.render_inbox) {
            let Some(pending) = self.pending_render.take_if(|p| p.id == id) else {
                continue;
            };
            let color = match result {
                Ok(color) => color,
                Err(message) => {
                    self.report(format!("Cannot render frame: {message}"));
                    continue;
                }
            };
            self.frames_rendered += 1;
            let handle = ctx.load_texture("stage", color, egui::TextureOptions::LINEAR);
            let (context, revision) = (pending.key.0, pending.key.1);
            // Pictures of other document states can never be shown again.
            self.frame_cache
                .retain(|c| c.context == context && c.revision == revision);
            self.frame_cache.push(CachedFrame {
                context,
                revision,
                signature: pending.signature,
                view: pending.view,
                handle: handle.clone(),
            });
            self.trim_frame_cache();
            self.texture = Some(StageTexture {
                handle,
                view: pending.view,
                key: pending.key,
                onion: pending.onion,
            });
        }
    }

    /// Drops the least recently used frames until the cache fits its limits. The newest frame
    /// is always kept, so a single large frame is still shown.
    fn trim_frame_cache(&mut self) {
        while self.frame_cache.len() > 1
            && (self.frame_cache.len() > FRAME_CACHE_ENTRIES
                || self
                    .frame_cache
                    .iter()
                    .map(CachedFrame::bytes)
                    .sum::<usize>()
                    > FRAME_CACHE_BYTES)
        {
            self.frame_cache.remove(0);
        }
    }

    // ---- UI --------------------------------------------------------------------------

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        if !self.chrome_ready {
            flare_ui::theme::apply(&ctx);
            flare_ui::icons::install(&ctx);
            self.chrome_ready = true;
        }
        let now = ui.input(|i| i.time);
        self.drain_background();
        self.advance(now);
        self.ensure_sounds_decoded();
        self.sync_audio();
        self.autosave(now);
        self.handle_shortcuts(&ctx);
        // Only a changed document can make the selection stale; checking it every frame cost
        // time in proportion to the selection times the layers.
        if self.selection_checked != Some((self.context_serial, self.active().revision())) {
            self.validate_selection();
        }
        self.ensure_stage_texture(&ctx);

        egui::Panel::bottom("status")
            .exact_size(24.0)
            .show(ui, |ui| self.status_bar(ui));
        egui::Panel::top("menu")
            .exact_size(if self.titlebar_inset > 0.0 {
                INTEGRATED_TOP_BAR_HEIGHT
            } else {
                TOP_BAR_HEIGHT
            })
            .frame(egui::Frame::new().fill(flare_ui::theme::BG))
            .show(ui, |ui| self.menu_bar(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            // The dock needs `&mut self` to draw panel contents, so take it out while it runs.
            let mut layout = std::mem::replace(
                &mut self.layout,
                egui_tiles::Tree::empty(egui::Id::unique("taken")),
            );
            layout.ui(&mut Dock { workspace: self }, ui);
            self.layout = layout;
        });

        if self.playing {
            ctx.request_repaint();
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let (redo, undo, save, toggle, left, right, escape, delete, convert) = ctx.input_mut(|i| {
            // Redo must be consumed first: egui lets Cmd+Shift+Z also match plain Cmd+Z.
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::S),
                !i.modifiers.any() && i.key_pressed(Key::Space),
                !i.modifiers.any() && i.key_pressed(Key::ArrowLeft),
                !i.modifiers.any() && i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Escape),
                !i.modifiers.any() && (i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)),
                !i.modifiers.any() && i.key_pressed(Key::F8),
            )
        });
        // With a native menu bar the menu owns these shortcuts; handling them here too
        // would apply them twice.
        if !self.native_menu {
            if redo {
                self.redo();
            } else if undo {
                self.undo();
            }
            if save {
                self.save();
            }
            let frames_chord = Modifiers::COMMAND | Modifiers::ALT;
            if ctx.input_mut(|i| i.consume_key(frames_chord, Key::C)) {
                self.copy_frames();
            }
            if ctx.input_mut(|i| i.consume_key(frames_chord, Key::V)) {
                self.paste_frames();
            }
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D))
                && !ctx.egui_wants_keyboard_input()
            {
                self.duplicate_selection();
            }
        }
        let typing = ctx.egui_wants_keyboard_input();
        // Letters pick tools, but only when nothing is being typed and no modifier is held, so
        // they never fight a text field or a shortcut.
        if !typing {
            let picked = TOOL_KEYS
                .iter()
                .find(|(key, _)| key_press(ctx, *key).is_some_and(|m| !m.any()))
                .map(|(_, tool)| *tool);
            if let Some(tool) = picked {
                self.set_tool(tool);
            }
        }
        // Cmd/Ctrl+C and V arrive as Copy and Paste events. They are not menu accelerators, so
        // text fields keep them; when no field wants the keyboard they act on the selection.
        let (copy, cut, paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Cut)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        if !typing {
            if copy {
                self.copy_selection();
            }
            if cut {
                self.cut_selection();
            }
            if paste {
                self.paste();
            }
            if toggle {
                self.playing = !self.playing;
            }
            if delete && self.pen.is_active() {
                self.pen_remove_last();
            } else if delete {
                self.delete_selection();
            }
            if enter_pressed(ctx) {
                self.pen_finish();
            }
            if convert && !self.native_menu {
                self.convert_selection_to_symbol();
            }
            if left {
                self.set_frame(self.frame.saturating_sub(1));
            }
            if right {
                self.set_frame(self.frame.saturating_add(1));
            }
        }
        if escape {
            self.cancel_drag();
            self.pen_cancel();
        }
        if !typing && !self.native_menu {
            // Modifiers come from the key event itself, which is what the platform reported
            // for that press.
            let f5 = key_press(ctx, Key::F5);
            let f6 = key_press(ctx, Key::F6);
            let f7 = key_press(ctx, Key::F7);
            if let Some(m) = f5 {
                self.frame_edit(if m.shift {
                    FrameOp::RemoveFrame
                } else {
                    FrameOp::InsertFrame
                });
            }
            if let Some(m) = f6 {
                self.frame_edit(if m.shift {
                    FrameOp::ClearKeyframe
                } else {
                    FrameOp::InsertKeyframe
                });
            }
            if f7.is_some() {
                self.frame_edit(FrameOp::InsertBlankKeyframe);
            }
        }
    }

    /// One menu item for `action`: greyed out when it cannot run, runs it when clicked.
    fn action_item(&mut self, ui: &mut egui::Ui, action: MenuAction) {
        let enabled = self.is_enabled(action);
        let tint = if enabled {
            flare_ui::theme::TEXT
        } else {
            flare_ui::theme::TEXT_DISABLED
        };
        let label = self.label(action);
        let mut button = match action.icon() {
            Some(icon) => {
                egui::Button::image_and_text(flare_ui::icons::image(icon, 14.0, tint), label)
            }
            None => egui::Button::new(label),
        };
        // The shortcut, or for a tool the key that picks it, right-aligned in the item.
        if let Some(text) = action
            .shortcut_text()
            .or_else(|| action.tool_key().map(str::to_owned))
        {
            button = button.shortcut_text(text);
        }
        let clicked = ui.add_enabled(enabled, button).clicked();
        if clicked {
            self.perform(action);
            ui.close();
        }
    }

    /// The top bar: room for the window buttons, the app mark, the in-window menus (when there
    /// is no native menu bar), the document name, and quick actions. Empty space drags the
    /// window.
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        let bar = ui.max_rect();
        let drag = ui.interact(bar, self.top_bar_id, egui::Sense::click_and_drag());
        if drag.double_clicked() {
            self.window_request = Some(WindowRequest::ToggleMaximize);
        } else if drag.drag_started() {
            self.window_request = Some(WindowRequest::Drag);
        }
        ui.painter().hline(
            bar.x_range(),
            bar.bottom() - 0.5,
            egui::Stroke::new(1.0, flare_ui::theme::BORDER),
        );
        ui.horizontal_centered(|ui| {
            ui.add_space(self.titlebar_inset.max(10.0));
            ui.add(flare_ui::icons::image("home", 16.0, flare_ui::theme::TEXT));
            ui.label(
                egui::RichText::new("Flare")
                    .strong()
                    .color(flare_ui::theme::TEXT),
            );
            ui.add_space(6.0);
            egui::MenuBar::new().ui(ui, |ui| {
                if self.native_menu {
                    return;
                }
                for (title, entries) in MENU_LAYOUT {
                    ui.menu_button(*title, |ui| {
                        for entry in *entries {
                            match entry {
                                MenuEntry::Action(action) => self.action_item(ui, *action),
                                MenuEntry::Separator => {
                                    ui.separator();
                                }
                                MenuEntry::Mock(label) => {
                                    ui.add_enabled(false, egui::Button::new(*label))
                                        .on_disabled_hover_text(flare_ui::chrome::NOT_YET);
                                }
                            }
                        }
                    });
                }
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(10.0);
                let can_redo = self.active().can_redo();
                if flare_ui::chrome::icon_button(ui, "redo", 16.0, false, can_redo, "Redo")
                    .clicked()
                {
                    self.redo();
                }
                let can_undo = self.active().can_undo();
                if flare_ui::chrome::icon_button(ui, "undo", 16.0, false, can_undo, "Undo")
                    .clicked()
                {
                    self.undo();
                }
                let (icon, hint) = if self.is_playing() {
                    ("transport-pause", "Pause")
                } else {
                    ("transport-play", "Play")
                };
                if flare_ui::chrome::icon_button(ui, icon, 16.0, false, true, hint).clicked() {
                    self.perform(MenuAction::PlayPause);
                }
                // The name sits in the middle of what is left, whatever the window width.
                let title = egui::RichText::new(self.title()).color(flare_ui::theme::TEXT_DIM);
                let rect = ui.available_rect_before_wrap();
                ui.painter().text(
                    egui::pos2(bar.center().x, rect.center().y),
                    egui::Align2::CENTER_CENTER,
                    title.text(),
                    egui::FontId::proportional(13.0),
                    flare_ui::theme::TEXT_DIM,
                );
            });
        });
    }

    /// Stage size and background. Size edits apply on release or Enter so that scrubbing a
    /// value is a single undo step.
    fn stage_settings(&mut self, ui: &mut egui::Ui) {
        let stage = self.document().stage();
        let (mut width, mut height) = self.stage_edit.unwrap_or((stage.width(), stage.height()));
        let wr = ui.add(
            egui::DragValue::new(&mut width)
                .range(1..=MAX_STAGE_SIDE)
                .prefix("W "),
        );
        let hr = ui.add(
            egui::DragValue::new(&mut height)
                .range(1..=MAX_STAGE_SIDE)
                .prefix("H "),
        );
        if wr.changed() || hr.changed() {
            self.stage_edit = Some((width, height));
        }
        let done = wr.drag_stopped() || wr.lost_focus() || hr.drag_stopped() || hr.lost_focus();
        if done
            && let Some((w, h)) = self.stage_edit.take()
            && (w, h) != (stage.width(), stage.height())
        {
            self.set_stage_size(w, h);
        }
        let bg = stage.background().to_array();
        let mut color = Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]);
        if ui.color_edit_button_srgba(&mut color).changed() {
            let c = color.to_srgba_unmultiplied();
            self.set_stage_background(Rgba8::new(c[0], c[1], c[2], c[3]));
        }
    }

    fn stage(&mut self, ui: &mut egui::Ui) {
        self.document_tabs(ui);
        self.stage_bar(ui);
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        painter.rect_filled(response.rect, 0.0, flare_ui::theme::STAGE_SURROUND);
        let extent = self.stage_extent();
        self.stage_panel = response.rect;
        if response.hovered() {
            // Pinch or Ctrl/Cmd+scroll zooms about the cursor; plain scroll and middle-drag pan.
            let (zoom, scroll, cursor) =
                ui.input(|i| (i.zoom_delta(), i.smooth_scroll_delta, i.pointer.hover_pos()));
            if zoom != 1.0 {
                self.stage_view.zoom_about(
                    cursor.unwrap_or(response.rect.center()),
                    zoom,
                    response.rect,
                    extent,
                );
            } else if scroll != Vec2::ZERO {
                self.stage_view.pan_by(scroll);
            }
        }
        if response.dragged_by(egui::PointerButton::Middle) {
            self.stage_view.pan_by(response.drag_delta());
        }
        let scale = self.stage_view.scale(response.rect, extent);
        let image_rect = self.stage_view.image_rect(response.rect, extent);
        if let Some(texture) = &self.texture {
            // Until the texture catches up with a zoom or pan it is drawn where it belongs, never
            // stretched over the stage; the stage color shows through the rest.
            let bg = self.document().stage().background().to_array();
            painter.rect_filled(
                image_rect,
                0.0,
                Color32::from_rgba_unmultiplied(bg[0], bg[1], bg[2], bg[3]),
            );
            let region = texture.view.region;
            let shown = Rect::from_min_max(
                image_rect.min + region.min.to_vec2() * scale,
                image_rect.min + region.max.to_vec2() * scale,
            );
            painter.image(
                texture.handle.id(),
                shown,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            if self.desired_texture_view(ui.ctx().pixels_per_point(), texture_side_limit(ui.ctx()))
                != Some(texture.view)
            {
                ui.ctx().request_repaint();
            }
        } else {
            painter.text(
                image_rect.center(),
                Align2::CENTER_CENTER,
                "Nothing to show",
                FontId::proportional(16.0),
                Color32::GRAY,
            );
        }
        painter.rect_stroke(
            image_rect,
            0.0,
            Stroke::new(1.0, Color32::DARK_GRAY),
            StrokeKind::Outside,
        );

        if self.show_grid {
            self.stage_grid(&painter, response.rect, image_rect, extent, scale);
        }
        if self.show_rulers {
            self.stage_rulers(&painter, response.rect, image_rect, scale);
        }
        self.text_editor(ui.ctx(), image_rect, scale);
        if !self.stage_view.is_fit() {
            painter.text(
                response.rect.right_bottom() - Vec2::new(8.0, 8.0),
                Align2::RIGHT_BOTTOM,
                format!("{:.0}%", scale / fit_scale(response.rect, extent) * 100.0),
                FontId::proportional(12.0),
                Color32::GRAY,
            );
        }
        self.handle_radius = 8.0 / scale.max(f32::EPSILON);
        self.uniform_scale = ui.input(|i| i.modifiers.shift);
        self.skew_modifier = ui.input(|i| i.modifiers.alt);
        // Tools that place points snap them to the grid; tools that find things do not, or a
        // thin shape could not be clicked.
        let snap = self
            .snap_to_grid
            .then_some(self.grid_size)
            .filter(|_| tool_places_points(self.tool));
        let to_doc = |p: Pos2| {
            let at = screen_to_stage(p, image_rect, extent);
            snap.map_or(at, |grid| snap_point(at, grid))
        };
        self.pointer_stage = response.hover_pos().map(to_doc);
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
        if self.tool == Tool::Text
            && response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.text_edit = Some(PendingText {
                at: to_doc(p),
                buffer: String::new(),
                focused: false,
            });
        }
        if self.tool == Tool::Hand && response.dragged_by(egui::PointerButton::Primary) {
            self.stage_view.pan_by(response.drag_delta());
        }
        if matches!(self.tool, Tool::Pencil | Tool::Brush) {
            let primary = egui::PointerButton::Primary;
            if let Some(p) = response.interact_pointer_pos()
                && response.dragged_by(primary)
            {
                let at = to_doc(p);
                if self.freehand.last().is_none_or(|last| *last != at) {
                    self.freehand.push(at);
                }
            }
            if response.drag_stopped_by(primary) {
                self.finish_freehand(scale);
            }
            if self.freehand.len() > 1 {
                let (color, width) = match self.tool {
                    Tool::Brush => (self.fill, self.brush_size),
                    _ => (self.stroke_color, self.stroke_width),
                };
                let points: Vec<Pos2> = self.freehand.iter().map(|p| to_screen(*p)).collect();
                painter.add(egui::Shape::line(points, Stroke::new(width * scale, color)));
            }
        }
        if self.tool == Tool::Lasso {
            let primary = egui::PointerButton::Primary;
            if let Some(p) = response.interact_pointer_pos()
                && response.dragged_by(primary)
            {
                let at = to_doc(p);
                let step = 2.0 / scale.max(f32::EPSILON);
                if self
                    .lasso
                    .last()
                    .is_none_or(|last| (*last - at).length() >= step)
                {
                    self.lasso.push(at);
                }
            }
            if response.drag_stopped_by(primary) {
                self.finish_lasso(ui.input(|i| i.modifiers.shift));
            }
            if self.lasso.len() > 1 {
                let mut points: Vec<Pos2> = self.lasso.iter().map(|p| to_screen(*p)).collect();
                points.push(points[0]);
                painter.add(egui::Shape::line(
                    points,
                    Stroke::new(1.5, flare_ui::theme::ACCENT),
                ));
            }
        }
        if self.tool == Tool::Zoom
            && response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let factor = if ui.input(|i| i.modifiers.alt) {
                1.0 / 1.5
            } else {
                1.5
            };
            self.stage_view.zoom_about(p, factor, response.rect, extent);
        }
        if response.clicked()
            && let Some(p) = response.interact_pointer_pos()
        {
            let shift = ui.input(|i| i.modifiers.shift);
            self.click_with_tool(to_doc(p), shift);
        }
        if self.tool == Tool::Pen {
            // A press that turns into a drag starts where the button went down, not where
            // egui noticed the drag; a plain click adds a corner point.
            let origin = ui.input(|i| i.pointer.press_origin());
            let here = response.interact_pointer_pos();
            if response.drag_started_by(egui::PointerButton::Primary)
                && let Some(p) = origin.or(here)
            {
                self.pen_press(to_doc(p));
            }
            if response.dragged_by(egui::PointerButton::Primary)
                && let Some(p) = here
            {
                self.pen_drag(to_doc(p));
            }
            if response.drag_stopped_by(egui::PointerButton::Primary) {
                self.pen_release();
            }
            if response.double_clicked() {
                self.pen_finish();
            } else if response.clicked()
                && let Some(p) = here
            {
                self.pen_press(to_doc(p));
                self.pen_release();
            }
        }
        let additive = ui.input(|i| i.modifiers.shift);
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.begin_drag_with(to_doc(p), additive);
        }
        if response.dragged_by(primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.update_drag(to_doc(p));
        }
        if response.drag_stopped_by(primary) {
            self.end_drag();
        }
        // A plain click with the select tool also selects (or clears the selection).
        if response.clicked()
            && self.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            self.select_at(to_doc(p), additive);
        }

        // Double-click an instance to edit its symbol; double-click empty stage to go back.
        if response.double_clicked()
            && self.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            self.select_at(to_doc(p), false);
            match self.selected {
                Some(_) => {
                    if let Some(symbol) = self.selected_instance_symbol() {
                        self.enter_symbol(symbol);
                    }
                }
                None => {
                    self.exit_symbol();
                }
            }
        }

        // The pen path being built: the curve so far, its points, and the dragged handles.
        if self.pen.is_active() {
            let ink = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
            let handle = Stroke::new(1.0, Color32::from_rgb(120, 120, 120));
            let points = self.pen.points();
            for pair in points.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let c1 = a.out.unwrap_or(a.anchor);
                let c2 = b.out.map_or(b.anchor, |o| b.anchor + (b.anchor - o));
                let steps = 20;
                let sample = |t: f32| {
                    let u = 1.0 - t;
                    to_screen(Pos2::new(
                        u * u * u * a.anchor.x
                            + 3.0 * u * u * t * c1.x
                            + 3.0 * u * t * t * c2.x
                            + t * t * t * b.anchor.x,
                        u * u * u * a.anchor.y
                            + 3.0 * u * u * t * c1.y
                            + 3.0 * u * t * t * c2.y
                            + t * t * t * b.anchor.y,
                    ))
                };
                let line: Vec<Pos2> = (0..=steps)
                    .map(|i| sample(i as f32 / steps as f32))
                    .collect();
                painter.add(egui::Shape::line(line, ink));
            }
            for point in points {
                if let Some(out) = point.out {
                    let mirror = point.anchor + (point.anchor - out);
                    painter.line_segment([to_screen(mirror), to_screen(out)], handle);
                    painter.circle_filled(to_screen(out), 3.0, Color32::from_rgb(120, 120, 120));
                }
                let at = to_screen(point.anchor);
                painter.rect_filled(
                    Rect::from_center_size(at, Vec2::splat(7.0)),
                    0.0,
                    Color32::WHITE,
                );
                painter.rect_stroke(
                    Rect::from_center_size(at, Vec2::splat(7.0)),
                    0.0,
                    ink,
                    StrokeKind::Outside,
                );
            }
        }

        let outline = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
        let moving = self
            .drag
            .filter(|d| d.mode == DragMode::Move)
            .map_or(Vec2::ZERO, |d| d.current - d.start);
        for bounds in self.selection_rects() {
            let shown = bounds.translate(moving);
            let rect = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
            painter.rect_stroke(rect, 0.0, outline, StrokeKind::Outside);
        }
        if self.tool == Tool::Select
            && let Some(bounds) = self.selection_bounds()
        {
            // The handles follow a move in progress, or show the scaled result of a scale.
            let shown = match self.drag {
                Some(Drag {
                    mode: DragMode::Scale(handle),
                    bounds: grabbed,
                    current,
                    ..
                }) => handles::scale_for(grabbed, handle, current, self.uniform_scale).map_or(
                    grabbed,
                    |scale| {
                        let m = handles::matrix(grabbed, handle, scale);
                        let corner = |p: Pos2| {
                            let q = m.transform_point2(DVec2::new(f64::from(p.x), f64::from(p.y)));
                            Pos2::new(q.x as f32, q.y as f32)
                        };
                        Rect::from_two_pos(corner(grabbed.min), corner(grabbed.max))
                    },
                ),
                _ => bounds.translate(moving),
            };
            // A rotate or skew in progress shows the turned or sheared frame.
            if let Some(drag) = self.drag
                && let Some(m) = self.drag_matrix(&drag)
            {
                let corner = |p: Pos2| {
                    let q = m.transform_point2(DVec2::new(f64::from(p.x), f64::from(p.y)));
                    to_screen(Pos2::new(q.x as f32, q.y as f32))
                };
                let b = drag.bounds;
                let outline_points = vec![
                    corner(b.left_top()),
                    corner(b.right_top()),
                    corner(b.right_bottom()),
                    corner(b.left_bottom()),
                ];
                painter.add(egui::Shape::closed_line(outline_points, outline));
            }
            let frame = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
            painter.rect_stroke(
                frame,
                0.0,
                Stroke::new(1.0, Color32::from_rgb(40, 120, 255)),
                StrokeKind::Outside,
            );
            for handle in Handle::ALL {
                let at = to_screen(handle.position(shown));
                let square = Rect::from_center_size(at, Vec2::splat(7.0));
                painter.rect_filled(square, 0.0, Color32::WHITE);
                painter.rect_stroke(square, 0.0, outline, StrokeKind::Outside);
            }
        }
        if let Some(drag) = self.drag.filter(|d| d.mode == DragMode::Draw) {
            let (from, to) = (to_screen(drag.start), to_screen(drag.current));
            let preview = Rect::from_two_pos(from, to);
            let ink = Stroke::new(1.0, Color32::BLACK);
            match self.tool {
                Tool::Line => {
                    painter.line_segment(
                        [from, to],
                        Stroke::new(self.stroke_width * scale, self.stroke_color),
                    );
                }
                Tool::Ellipse => {
                    let radius = preview.size() / 2.0;
                    painter.add(egui::epaint::EllipseShape::filled(
                        preview.center(),
                        radius,
                        self.fill.gamma_multiply(0.5),
                    ));
                    painter.rect_stroke(preview, 0.0, ink, StrokeKind::Outside);
                }
                _ => {
                    painter.rect_filled(preview, 0.0, self.fill.gamma_multiply(0.5));
                    painter.rect_stroke(preview, 0.0, ink, StrokeKind::Outside);
                }
            }
        }
    }

    fn timeline(&mut self, ui: &mut egui::Ui) {
        self.timeline_toolbar(ui);
        ui.separator();

        let available = ui.available_width();
        if ui.rect_contains_pointer(ui.max_rect()) {
            let zoom = ui.input(|i| i.zoom_delta());
            if zoom != 1.0 {
                self.zoom_timeline(zoom);
            }
        }
        egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
            let frames = self.scene_frames();
            let layer_count = self
                .document()
                .scenes()
                .first()
                .map_or(0, |s| s.timeline().layers().len());
            let row = 22.0;
            let label_w = 150.0;
            let height = row * (layer_count as f32 + 1.0);
            let per_frame = timeline_view::frame_width(
                (available - label_w).max(0.0),
                u64::from(frames),
                self.timeline_zoom,
            );
            let (rect, response) = ui.allocate_exact_size(
                Vec2::new(label_w + per_frame * frames as f32, height),
                Sense::click_and_drag(),
            );
            let painter = ui.painter_at(rect);
            let track = Rect::from_min_max(Pos2::new(rect.min.x + label_w, rect.min.y), rect.max);

            let frame_at = |x: f32| (x - track.min.x) / per_frame;
            let row_layer = |y: f32| {
                let index = ((y - rect.min.y) / row).floor() as i64 - 1;
                usize::try_from(index).ok().and_then(|i| {
                    let layers = self.document().scenes().first()?.timeline().layers();
                    layers.get(i).map(Layer::id)
                })
            };
            if response.drag_started()
                && let Some(p) = ui.input(|i| i.pointer.press_origin())
                && p.x >= track.min.x
                && let Some(id) = row_layer(p.y)
                && self.begin_span_drag(id, frame_at(p.x), 6.0 / per_frame)
            {
                self.select_layer(id);
            }
            if self.span_drag.is_some() {
                if response.dragged()
                    && let Some(p) = response.interact_pointer_pos()
                {
                    self.update_span_drag(frame_at(p.x).floor().max(0.0) as u32);
                }
                if response.drag_stopped() {
                    self.end_span_drag();
                }
            } else if (response.clicked() || response.dragged())
                && let Some(p) = response.interact_pointer_pos()
            {
                let in_track = p.x >= track.min.x;
                let f = ((p.x - track.min.x) / per_frame).floor().max(0.0) as u32;
                let extend = response.clicked() && in_track && ui.input(|i| i.modifiers.shift);
                // Clicking a row (below the ruler) makes that layer the active one.
                let index = ((p.y - rect.min.y) / row).floor() as i64 - 1;
                let picked = usize::try_from(index).ok().and_then(|i| {
                    let layers = self.document().scenes().first()?.timeline().layers();
                    layers.get(i).map(Layer::id)
                });
                if extend && let Some(id) = picked {
                    // Shift-click selects the frames between the playhead (or the range's own
                    // start) and the clicked one, without moving the playhead.
                    let anchor = self
                        .frame_range
                        .filter(|r| r.layer == id)
                        .map_or(self.frame, |r| r.from);
                    self.select_frames(id, anchor, f);
                } else if in_track {
                    self.set_frame(f);
                    if response.clicked() {
                        self.frame_range = None;
                    }
                }
                if let Some(id) = picked {
                    // The two small toggles at the right edge of the label column.
                    let eye = p.x >= track.min.x - 54.0 && p.x < track.min.x - 36.0;
                    let lock = p.x >= track.min.x - 36.0 && p.x < track.min.x - 18.0;
                    if response.clicked() && eye {
                        self.toggle_layer_visible(id);
                    } else if response.clicked() && lock {
                        self.toggle_layer_locked(id);
                    } else {
                        self.select_layer(id);
                    }
                }
            }
            let active = self.active_layer_id();

            // The ruler: a band with a tick on every frame and a number every few.
            let ink = flare_ui::theme::TEXT;
            let band = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + row));
            painter.rect_filled(band, 0.0, flare_ui::theme::PANEL_RAISED);
            painter.hline(
                rect.x_range(),
                band.bottom(),
                Stroke::new(1.0, flare_ui::theme::BORDER),
            );
            let step = ((40.0 / per_frame).ceil() as u32).max(1);
            for f in 0..frames {
                let x = track.min.x + f as f32 * per_frame;
                if x > painter.clip_rect().max.x {
                    break;
                }
                let major = f % step == 0;
                painter.vline(
                    x,
                    (band.bottom() - if major { 7.0 } else { 3.0 })..=band.bottom(),
                    Stroke::new(1.0, flare_ui::theme::TEXT_DISABLED),
                );
                if major {
                    painter.text(
                        Pos2::new(x + 3.0, band.top() + 2.0),
                        Align2::LEFT_TOP,
                        (f + 1).to_string(),
                        FontId::monospace(10.0),
                        flare_ui::theme::TEXT_DIM,
                    );
                }
            }
            if let Some(scene) = self.document().scenes().first() {
                // Only the rows in view are painted; with thousands of layers drawing all of
                // them every frame dominated the cost of an idle frame.
                let (first, count) = visible_rows(rect.min.y, row, painter.clip_rect());
                for (i, layer) in scene
                    .timeline()
                    .layers()
                    .iter()
                    .enumerate()
                    .skip(first)
                    .take(count)
                {
                    let y = rect.min.y + row * (i as f32 + 1.0);
                    if i % 2 == 1 {
                        painter.rect_filled(
                            Rect::from_min_max(
                                Pos2::new(rect.min.x, y),
                                Pos2::new(rect.max.x, y + row),
                            ),
                            0.0,
                            Color32::from_rgba_unmultiplied(255, 255, 255, 6),
                        );
                    }
                    // A line every five frames makes the cells easy to count.
                    for f in (0..frames).step_by(5) {
                        let x = track.min.x + f as f32 * per_frame;
                        if x > painter.clip_rect().max.x {
                            break;
                        }
                        painter.vline(
                            x,
                            y..=y + row,
                            Stroke::new(1.0, Color32::from_rgba_unmultiplied(255, 255, 255, 14)),
                        );
                    }
                    if Some(layer.id()) == active {
                        let band = Rect::from_min_max(
                            Pos2::new(rect.min.x, y),
                            Pos2::new(rect.max.x, y + row),
                        );
                        painter.rect_filled(
                            band,
                            0.0,
                            Color32::from_rgba_unmultiplied(80, 140, 255, 40),
                        );
                    }
                    let dim = flare_ui::theme::TEXT_DISABLED;
                    painter.text(
                        Pos2::new(rect.min.x + 6.0, y + 4.0),
                        Align2::LEFT_TOP,
                        if layer.is_mask() {
                            format!("Mask: {}", layer.name())
                        } else {
                            layer.name().to_owned()
                        },
                        FontId::proportional(12.0),
                        if layer.is_visible() { ink } else { dim },
                    );
                    // Visibility, lock and outline columns, as icons.
                    let icon_at = |x: f32, name: &str, tint: Color32| {
                        flare_ui::icons::image(name, 14.0, tint).paint_at(
                            ui,
                            Rect::from_min_size(Pos2::new(x, y + 4.0), Vec2::splat(14.0)),
                        );
                    };
                    icon_at(
                        track.min.x - 52.0,
                        if layer.is_visible() { "eye" } else { "eye-off" },
                        if layer.is_visible() {
                            flare_ui::theme::TEXT
                        } else {
                            dim
                        },
                    );
                    icon_at(
                        track.min.x - 34.0,
                        if layer.is_locked() {
                            "lock"
                        } else {
                            "lock-open"
                        },
                        if layer.is_locked() {
                            flare_ui::theme::TEXT
                        } else {
                            dim
                        },
                    );
                    icon_at(track.min.x - 16.0, "outline-square", dim);
                    for span in layer.spans() {
                        let x0 = track.min.x + span.start() as f32 * per_frame;
                        let x1 = track.min.x + span.end() as f32 * per_frame;
                        let color = match span.content() {
                            Content::Empty => Color32::from_gray(90),
                            Content::Shape(_) => Color32::from_rgb(70, 130, 180),
                            Content::Instance(_) => Color32::from_rgb(120, 170, 90),
                        };
                        let bar = Rect::from_min_max(
                            Pos2::new(x0, y + 2.0),
                            Pos2::new(x1.max(x0 + 1.0), y + row - 2.0),
                        );
                        let color = if layer.is_visible() {
                            color
                        } else {
                            color.gamma_multiply(0.35)
                        };
                        painter.rect_filled(bar, 2.0, color);
                    }
                    for cue in layer.cues() {
                        let Some(sound) = self.session.document().sound(cue.sound) else {
                            continue;
                        };
                        let fps = f64::from(self.document().frame_rate().numerator())
                            / f64::from(self.document().frame_rate().denominator());
                        let length = (sound.seconds() * fps).ceil().max(1.0) as f32;
                        let x0 = track.min.x + cue.start as f32 * per_frame;
                        let bar = Rect::from_min_max(
                            Pos2::new(x0, y + 2.0),
                            Pos2::new(x0 + length * per_frame, y + row - 2.0),
                        );
                        painter.rect_filled(bar, 2.0, Color32::from_rgb(214, 140, 60));
                        painter.text(
                            bar.left_center() + Vec2::new(3.0, 0.0),
                            Align2::LEFT_CENTER,
                            sound.name(),
                            FontId::proportional(10.0),
                            Color32::BLACK,
                        );
                    }
                    if let Some(range) = self.frame_range.filter(|r| r.layer == layer.id()) {
                        let x0 = track.min.x + range.from as f32 * per_frame;
                        let x1 = track.min.x + (range.to as f32 + 1.0) * per_frame;
                        let marked = Rect::from_min_max(
                            Pos2::new(x0, y + 1.0),
                            Pos2::new(x1, y + row - 1.0),
                        );
                        painter.rect_filled(
                            marked,
                            2.0,
                            Color32::from_rgba_unmultiplied(255, 220, 80, 70),
                        );
                        painter.rect_stroke(
                            marked,
                            2.0,
                            Stroke::new(1.0, Color32::from_rgb(255, 200, 40)),
                            StrokeKind::Inside,
                        );
                    }
                    if let Some((dragged, start, duration)) = self.span_drag_preview()
                        && dragged == layer.id()
                    {
                        let x0 = track.min.x + start as f32 * per_frame;
                        let x1 = x0 + duration as f32 * per_frame;
                        let ghost = Rect::from_min_max(
                            Pos2::new(x0, y + 2.0),
                            Pos2::new(x1, y + row - 2.0),
                        );
                        painter.rect_stroke(
                            ghost,
                            2.0,
                            Stroke::new(2.0, Color32::from_rgb(40, 120, 255)),
                            StrokeKind::Inside,
                        );
                    }
                }
            }
            let head = track.min.x + (self.frame as f32 + 0.5) * per_frame;
            painter.line_segment(
                [
                    Pos2::new(head, rect.min.y + 8.0),
                    Pos2::new(head, rect.max.y),
                ],
                Stroke::new(1.5, Color32::from_rgb(232, 69, 60)),
            );
            flare_ui::icons::image("playhead", 16.0, Color32::from_rgb(232, 69, 60)).paint_at(
                ui,
                Rect::from_center_size(Pos2::new(head, rect.min.y + 8.0), Vec2::splat(16.0)),
            );
        });
    }

    /// Rulers along the top and left edge of the stage view, in stage units.
    /// Grid lines over the stage, skipped when they would be too close to see.
    fn stage_grid(
        &self,
        painter: &egui::Painter,
        panel: Rect,
        image: Rect,
        extent: Vec2,
        scale: f32,
    ) {
        let step = self.grid_size.max(1.0);
        if step * scale < 6.0 {
            return;
        }
        let ink = Stroke::new(1.0, Color32::from_white_alpha(22));
        let clip = image.intersect(panel);
        let mut x = 0.0;
        while x <= extent.x {
            let sx = image.min.x + x * scale;
            if sx >= clip.min.x && sx <= clip.max.x {
                painter.vline(sx, clip.y_range(), ink);
            }
            x += step;
        }
        let mut y = 0.0;
        while y <= extent.y {
            let sy = image.min.y + y * scale;
            if sy >= clip.min.y && sy <= clip.max.y {
                painter.hline(clip.x_range(), sy, ink);
            }
            y += step;
        }
    }

    fn stage_rulers(&self, painter: &egui::Painter, panel: Rect, image: Rect, scale: f32) {
        const THICKNESS: f32 = 16.0;
        let top = Rect::from_min_max(panel.min, Pos2::new(panel.max.x, panel.min.y + THICKNESS));
        let left = Rect::from_min_max(panel.min, Pos2::new(panel.min.x + THICKNESS, panel.max.y));
        let fill = flare_ui::theme::PANEL_RAISED.gamma_multiply(0.92);
        painter.rect_filled(top, 0.0, fill);
        painter.rect_filled(left, 0.0, fill);
        let tick = Stroke::new(1.0, flare_ui::theme::TEXT_DISABLED);
        let font = FontId::monospace(9.0);
        for t in flare_ui::view::ruler_ticks(
            image.min.x,
            scale,
            panel.min.x + THICKNESS,
            panel.max.x,
            56.0,
        ) {
            let height = if t.major { THICKNESS - 4.0 } else { 4.0 };
            painter.vline(t.screen, (top.bottom() - height)..=top.bottom(), tick);
            if t.major {
                painter.text(
                    Pos2::new(t.screen + 2.0, top.top() + 1.0),
                    Align2::LEFT_TOP,
                    format!("{}", t.value.round()),
                    font.clone(),
                    flare_ui::theme::TEXT_DIM,
                );
            }
        }
        for t in flare_ui::view::ruler_ticks(
            image.min.y,
            scale,
            panel.min.y + THICKNESS,
            panel.max.y,
            56.0,
        ) {
            let width = if t.major { THICKNESS - 4.0 } else { 4.0 };
            painter.hline((left.right() - width)..=left.right(), t.screen, tick);
            if t.major {
                painter.text(
                    Pos2::new(left.left() + 1.0, t.screen + 2.0),
                    Align2::LEFT_TOP,
                    format!("{}", t.value.round()),
                    font.clone(),
                    flare_ui::theme::TEXT_DIM,
                );
            }
        }
        painter.rect_filled(
            Rect::from_min_size(panel.min, Vec2::splat(THICKNESS)),
            0.0,
            flare_ui::theme::PANEL_RAISED,
        );
    }

    fn zoom_timeline(&mut self, factor: f32) {
        self.timeline_zoom = timeline_view::zoomed(self.timeline_zoom, factor);
    }
}

/// Draws each dock panel by calling back into the workspace.
struct Dock<'a> {
    workspace: &'a mut Workspace,
}

impl egui_tiles::Behavior<Pane> for Dock<'_> {
    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        match pane {
            Pane::Stage => self.workspace.stage(ui),
            Pane::Timeline => self.workspace.timeline(ui),
            Pane::Library => self.workspace.library_panel(ui),
            Pane::Tools => self.workspace.tools_panel(ui),
            Pane::Properties => self.workspace.properties_panel(ui),
            Pane::Color => self.workspace.color_panel(ui),
            Pane::Align => self.workspace.align_panel(ui),
            Pane::Transform => self.workspace.transform_panel(ui),
            Pane::History => self.workspace.history_panel(ui),
            Pane::Output => self.workspace.output_panel(ui),
        }
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
    }

    fn tab_bar_height(&self, _style: &egui::Style) -> f32 {
        26.0
    }

    fn gap_width(&self, _style: &egui::Style) -> f32 {
        3.0
    }

    fn tab_bar_color(&self, _visuals: &egui::Visuals) -> egui::Color32 {
        flare_ui::theme::BG
    }

    fn tab_bg_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            flare_ui::theme::PANEL
        } else {
            flare_ui::theme::BG
        }
    }

    fn tab_text_color(
        &self,
        _visuals: &egui::Visuals,
        _tiles: &egui_tiles::Tiles<Pane>,
        _tile_id: egui_tiles::TileId,
        state: &egui_tiles::TabState,
    ) -> egui::Color32 {
        if state.active {
            flare_ui::theme::TEXT
        } else {
            flare_ui::theme::TEXT_DIM
        }
    }
}

/// Whether Enter was pressed this pass.
fn enter_pressed(ctx: &egui::Context) -> bool {
    key_press(ctx, Key::Enter).is_some()
}

/// The layer rows (first index, how many) that intersect `clip`, given the top of the track area,
/// the row height, and one header row above the first layer. Rows partly in view are included.
fn visible_rows(top: f32, row: f32, clip: Rect) -> (usize, usize) {
    if !(row > 0.0 && row.is_finite())
        || !top.is_finite()
        || !clip.min.y.is_finite()
        || !clip.max.y.is_finite()
        || clip.is_negative()
    {
        return (0, 0);
    }
    // Layer `i` occupies y in [top + row * (i + 1), top + row * (i + 2)).
    let first = (((clip.min.y - top) / row).floor() - 1.0).max(0.0);
    let last = (((clip.max.y - top) / row).ceil() - 2.0).max(0.0);
    if !first.is_finite() || !last.is_finite() || last < first {
        return (0, 0);
    }
    (first as usize, (last - first) as usize + 1)
}

/// The largest texture side to ask for: the backend's own limit, and never more than we want.
fn texture_side_limit(ctx: &egui::Context) -> u32 {
    let backend = ctx.input(|i| i.max_texture_side);
    u32::try_from(backend)
        .unwrap_or(u32::MAX)
        .min(MAX_TEXTURE_SIDE)
}

/// Modifiers held when `key` was pressed this pass, if it was pressed.
fn key_press(ctx: &egui::Context, key: Key) -> Option<Modifiers> {
    ctx.input(|i| {
        i.events.iter().find_map(|event| match event {
            egui::Event::Key {
                key: k,
                pressed: true,
                modifiers,
                ..
            } if *k == key => Some(*modifiers),
            _ => None,
        })
    })
}

/// How far a duplicate is moved from its original, in stage units.
const DUPLICATE_OFFSET: f64 = 12.0;

/// The scale at which the whole stage just fits `panel`.
fn fit_scale(panel: Rect, extent: Vec2) -> f32 {
    StageView::default().scale(panel, extent)
}

/// Scale Up / Scale Down factor.
const SCALE_STEP: f64 = 1.25;

/// Screen position to stage (document) coordinates, clamped to the stage.
fn screen_to_stage(p: Pos2, image_rect: Rect, extent: Vec2) -> Pos2 {
    let u = (p.x - image_rect.min.x) / image_rect.width().max(f32::EPSILON);
    let v = (p.y - image_rect.min.y) / image_rect.height().max(f32::EPSILON);
    Pos2::new(u.clamp(0.0, 1.0) * extent.x, v.clamp(0.0, 1.0) * extent.y)
}

#[cfg(test)]
mod tests;
