use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_document::{
    Content, Document, FrameSpan, Layer, MAX_STAGE_SIDE, Scene, Shape, Stage,
    Stroke as ShapeStroke, Timeline, Tween,
};
use flare_edit::{Command, EditorSession, LayerChange, frames};
use glamx::{DAffine2, DVec2};

use crate::handles::{self, Handle};
use crate::layout::{self, LayoutError, Pane};
use crate::ops::{self, SpanRef};
use crate::pen::{PenPath, Press};
use crate::timeline_view;
use crate::view::{self, StageView};
use crate::{MENU_LAYOUT, MenuAction, MenuEntry};

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
}

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
    /// How many frames have been rendered (as opposed to taken from the cache).
    frames_rendered: u64,
    drag: Option<Drag>,
    /// Set by Escape so the release that follows a cancelled drag commits nothing.
    drag_cancelled: bool,
    tool: Tool,
    selected: Option<SpanRef>,
    /// How close, in stage units, a press must be to a scale handle to grab it.
    handle_radius: f32,
    /// Shift is held: corner scaling keeps the proportions.
    uniform_scale: bool,
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
    status: Option<String>,
    dialog: Option<DialogRequest>,
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
            frames_rendered: 0,
            drag: None,
            drag_cancelled: false,
            tool: Tool::Rectangle,
            selected: None,
            handle_radius: 6.0,
            uniform_scale: false,
            clipboard: Vec::new(),
            timeline_zoom: 1.0,
            also_selected: Vec::new(),
            stage_view: StageView::default(),
            stage_panel: Rect::NOTHING,
            span_drag: None,
            selected_symbol: None,
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
            status: None,
            dialog: None,
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

    /// Returns (and clears) a dialog the shell should show.
    pub fn take_dialog(&mut self) -> Option<DialogRequest> {
        self.dialog.take()
    }

    fn report(&mut self, message: impl Into<String>) {
        self.status = Some(message.into());
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
        fresh.stroke_enabled = self.stroke_enabled;
        fresh.stroke_color = self.stroke_color;
        fresh.stroke_width = self.stroke_width;
        fresh.native_menu = self.native_menu;
        fresh.recovery_dir = self.recovery_dir.take();
        fresh.autosave_interval = self.autosave_interval;
        fresh.layout = std::mem::replace(
            &mut self.layout,
            egui_tiles::Tree::empty(egui::Id::unique("taken")),
        );
        fresh.onion = self.onion;
        *self = fresh;
    }

    /// Opens a project, refusing to replace unsaved work.
    pub fn open(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before opening another project");
            return;
        }
        match flare_storage::load(path) {
            Ok(document) => {
                // The old document is clean, so any recovery file for it is obsolete.
                self.discard_recovery();
                self.install_document(document, Some(path.to_owned()));
            }
            Err(error) => self.report(format!("Cannot open {}: {error}", path.display())),
        }
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
        match flare_storage::load(path) {
            Ok(document) => {
                self.install_document(document, None);
                self.session.mark_unsaved();
                // The file stays until the work is saved or discarded, so a second crash
                // before saving still leaves it recoverable.
                self.recovery_written = true;
                self.report("Recovered unsaved work");
            }
            Err(error) => self.report(format!("Cannot recover {}: {error}", path.display())),
        }
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
        self.last_autosave = Some(now);
        match flare_storage::save_recovery(self.session.document(), &dir) {
            Ok(_) => self.recovery_written = true,
            Err(error) => self.report(format!("Autosave failed: {error}")),
        }
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
        match flare_storage::save(self.session.document(), path) {
            Ok(()) => {
                self.session.mark_saved();
                self.discard_recovery();
                self.path = Some(path.to_owned());
                self.report(format!("Saved {}", path.display()));
            }
            Err(error) => self.report(format!("Cannot save: {error}")),
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
            Tool::Line => (b - a).length() < 1.0,
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
            Tool::Rectangle => (Shape::rect(id, x0, y0, w, h, fill), "Rectangle"),
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
            Tool::Select | Tool::Pen => return,
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
        let Some((scratch, _)) = flare_edit::symbol_workspace(self.document(), symbol) else {
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
            MenuAction::Undo => self.active().can_undo(),
            MenuAction::Redo => self.active().can_redo(),
            MenuAction::Paste => !self.clipboard.is_empty(),
            MenuAction::Delete
            | MenuAction::Duplicate
            | MenuAction::Copy
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
            MenuAction::ToggleLayerVisible | MenuAction::ToggleLayerLocked => layer.is_some(),
            MenuAction::MoveLayerUp => position.is_some_and(|p| p > 0),
            MenuAction::MoveLayerDown => position.is_some_and(|p| p + 1 < layer_count),
            MenuAction::DeleteLayer => layer.is_some() && layer_count > 1,
            MenuAction::ResetLayout => true,
            MenuAction::ToggleOnionSkin => true,
            MenuAction::TestMovie => true,
        }
    }

    /// The menu item's current label (some toggle between two meanings).
    pub fn label(&self, action: MenuAction) -> String {
        let layer = self.active_layer();
        let text = match action {
            MenuAction::Open => "Open...",
            MenuAction::Save => "Save",
            MenuAction::SaveAs => "Save As...",
            MenuAction::Quit => "Quit Flare",
            MenuAction::Undo => "Undo",
            MenuAction::Redo => "Redo",
            MenuAction::Delete => "Delete",
            MenuAction::Duplicate => "Duplicate",
            MenuAction::Copy => "Copy",
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
            MenuAction::Save => self.save(),
            MenuAction::SaveAs => self.dialog = Some(DialogRequest::SaveAs),
            MenuAction::Quit => self.quit_requested = true,
            MenuAction::Undo => self.undo(),
            MenuAction::Redo => self.redo(),
            MenuAction::Delete => self.delete_selection(),
            MenuAction::Duplicate => self.duplicate_selection(),
            MenuAction::Copy => self.copy_selection(),
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
            MenuAction::MoveLayerUp => self.move_active_layer(-1),
            MenuAction::MoveLayerDown => self.move_active_layer(1),
            MenuAction::DeleteLayer => self.delete_active_layer(),
            MenuAction::ResetLayout => self.reset_layout(),
            MenuAction::ToggleOnionSkin => self.set_onion_skin(!self.onion.enabled),
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
        self.layer_command("delete the layer", |scene, layer| Command::RemoveLayer {
            scene,
            layer,
        });
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
        if let Some(scene) = self.scene_id() {
            let editable = |ws: &Self, span: SpanRef| {
                ops::find_span(ws.document(), scene, span).is_some()
                    && ws
                        .layer(span.layer)
                        .is_some_and(|l| l.is_visible() && !l.is_locked())
            };
            let mut all: Vec<SpanRef> = self.selection_all();
            all.retain(|span| editable(self, *span));
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
            Tool::Rectangle | Tool::Ellipse | Tool::Line => Some(DragMode::Draw),
            // The pen builds a path point by point; see `pen_press`.
            Tool::Pen => None,
            Tool::Select => {
                let grabbed = if additive {
                    None
                } else {
                    self.selection_bounds()
                        .and_then(|b| handles::pick(b, point, self.handle_radius).map(|h| (h, b)))
                };
                if let Some((handle, bounds)) = grabbed {
                    self.drag = Some(Drag {
                        bounds,
                        mode: DragMode::Scale(handle),
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
                DragMode::Move => self.move_selection(drag.current - drag.start),
                DragMode::Scale(handle) => {
                    self.scale_selection(handle, drag.bounds, drag.current);
                }
            }
        }
    }

    /// Escape, tool switches and focus loss cancel a gesture without committing it.
    fn cancel_drag(&mut self) {
        self.span_drag = None;
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
        let scene = self.scene_id()?;
        self.selection_all()
            .into_iter()
            .filter_map(|span| ops::span_bounds(self.document(), scene, span))
            .reduce(|a, b| a.union(b))
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
        if !commands.is_empty() {
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
        self.frame = (u64::from(self.frame) + whole as u64).rem_euclid(u64::from(frames)) as u32;
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

    fn ensure_stage_texture(&mut self, ctx: &egui::Context) {
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
        let Some(scene) = self.document().scenes().first().map(|s| s.id()) else {
            self.texture = None;
            return;
        };
        let stage = self.document().stage();
        let document = self.document();
        let ghost_frames = if onion.enabled {
            onion_ghosts(self.frame, self.scene_frames(), onion.before, onion.after)
        } else {
            Vec::new()
        };
        let (context, revision) = (key.0, key.1);

        enum Outcome {
            Cached(usize),
            Rendered(u64, egui::ColorImage),
            Failed(String),
        }
        let outcome = match flare_eval::evaluate_frame(document, scene, self.frame) {
            Err(e) => Outcome::Failed(e.to_string()),
            Ok(current) => {
                // A ghost that fails to evaluate is simply left out; the current frame matters.
                let evaluated: Vec<(flare_eval::Frame<'_>, f32)> = ghost_frames
                    .iter()
                    .filter_map(|(f, opacity)| {
                        flare_eval::evaluate_frame(document, scene, *f)
                            .ok()
                            .map(|e| (e, *opacity))
                    })
                    .collect();
                let signature = {
                    use std::hash::{Hash, Hasher};
                    let mut hasher = std::collections::hash_map::DefaultHasher::new();
                    current.signature().hash(&mut hasher);
                    for (ghost, opacity) in &evaluated {
                        ghost.signature().hash(&mut hasher);
                        opacity.to_bits().hash(&mut hasher);
                    }
                    hasher.finish()
                };
                let hit = self.frame_cache.iter().position(|c| {
                    c.context == context
                        && c.revision == revision
                        && c.signature == signature
                        && c.view == view
                });
                match hit {
                    Some(index) => Outcome::Cached(index),
                    None => {
                        let ghosts: Vec<(&flare_eval::Frame<'_>, f32)> =
                            evaluated.iter().map(|(e, o)| (e, *o)).collect();
                        let viewport = flare_render::Viewport {
                            x: f64::from(view.region.min.x),
                            y: f64::from(view.region.min.y),
                            width: f64::from(view.region.width()),
                            height: f64::from(view.region.height()),
                        };
                        match flare_render::render_onion_viewport(
                            &ghosts,
                            &current,
                            &viewport,
                            view.size[0],
                            view.size[1],
                            stage.background(),
                        ) {
                            Ok(image) => {
                                let size = [image.width() as usize, image.height() as usize];
                                Outcome::Rendered(
                                    signature,
                                    egui::ColorImage::from_rgba_unmultiplied(size, image.rgba()),
                                )
                            }
                            Err(e) => Outcome::Failed(e.to_string()),
                        }
                    }
                }
            }
        };
        let handle = match outcome {
            Outcome::Cached(index) => {
                // Most recently used goes last.
                let entry = self.frame_cache.remove(index);
                let handle = entry.handle.clone();
                self.frame_cache.push(entry);
                handle
            }
            Outcome::Rendered(signature, color) => {
                self.frames_rendered += 1;
                let handle = ctx.load_texture("stage", color, egui::TextureOptions::LINEAR);
                // Pictures of other document states can never be shown again.
                self.frame_cache
                    .retain(|c| c.context == context && c.revision == revision);
                self.frame_cache.push(CachedFrame {
                    context,
                    revision,
                    signature,
                    view,
                    handle: handle.clone(),
                });
                self.trim_frame_cache();
                handle
            }
            Outcome::Failed(message) => {
                self.report(format!("Cannot render frame: {message}"));
                return;
            }
        };
        self.texture = Some(StageTexture {
            handle,
            view,
            key,
            onion,
        });
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
        let now = ui.input(|i| i.time);
        self.advance(now);
        self.autosave(now);
        self.handle_shortcuts(&ctx);
        self.validate_selection();
        self.ensure_stage_texture(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
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
            if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::D))
                && !ctx.egui_wants_keyboard_input()
            {
                self.duplicate_selection();
            }
        }
        let typing = ctx.egui_wants_keyboard_input();
        // Cmd/Ctrl+C and V arrive as Copy and Paste events. They are not menu accelerators, so
        // text fields keep them; when no field wants the keyboard they act on the selection.
        let (copy, paste) = ctx.input(|i| {
            (
                i.events.iter().any(|e| matches!(e, egui::Event::Copy)),
                i.events.iter().any(|e| matches!(e, egui::Event::Paste(_))),
            )
        });
        if !typing {
            if copy {
                self.copy_selection();
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
        let clicked = ui
            .add_enabled(enabled, egui::Button::new(self.label(action)))
            .clicked();
        if clicked {
            self.perform(action);
            ui.close();
        }
    }

    /// The in-window menu bar, plus the breadcrumb, title and status. With a native menu
    /// bar present only the latter are drawn.
    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            if !self.native_menu {
                for (title, entries) in MENU_LAYOUT {
                    ui.menu_button(*title, |ui| {
                        for entry in *entries {
                            match entry {
                                MenuEntry::Action(action) => self.action_item(ui, *action),
                                MenuEntry::Separator => {
                                    ui.separator();
                                }
                            }
                        }
                    });
                }
                ui.separator();
            }
            let crumbs = self.breadcrumb();
            if crumbs.len() > 1 {
                for (depth, name) in crumbs.iter().enumerate() {
                    if depth > 0 {
                        ui.label(">");
                    }
                    if ui.link(name).clicked() {
                        self.exit_to_depth(depth);
                        break;
                    }
                }
                ui.separator();
            }
            ui.label(self.title());
            if let Some(status) = &self.status {
                ui.separator();
                ui.label(status);
            }
        });
    }

    fn tools(&mut self, ui: &mut egui::Ui) {
        ui.heading("Tools");
        let mut tool = self.tool;
        ui.selectable_value(&mut tool, Tool::Select, "Select / Move");
        ui.selectable_value(&mut tool, Tool::Rectangle, "Rectangle");
        ui.selectable_value(&mut tool, Tool::Ellipse, "Ellipse");
        ui.selectable_value(&mut tool, Tool::Line, "Line");
        ui.selectable_value(&mut tool, Tool::Pen, "Pen");
        self.set_tool(tool);
        ui.weak("Drag on the stage. Esc cancels. Delete removes, F8 converts to a symbol.");
        ui.weak("Pen: click for corners, drag for curves, click the first point to close, Enter or double-click to finish.");
        ui.separator();
        ui.label("Fill");
        ui.color_edit_button_srgba(&mut self.fill);
        ui.checkbox(&mut self.gradient_enabled, "Gradient");
        ui.add_enabled_ui(self.gradient_enabled, |ui| {
            ui.horizontal(|ui| {
                ui.color_edit_button_srgba(&mut self.gradient_end);
                ui.checkbox(&mut self.gradient_vertical, "Vertical");
            });
        });
        ui.checkbox(&mut self.stroke_enabled, "Outline");
        ui.add_enabled_ui(self.stroke_enabled, |ui| {
            ui.color_edit_button_srgba(&mut self.stroke_color);
            ui.add(
                egui::DragValue::new(&mut self.stroke_width)
                    .range(0.5..=200.0)
                    .suffix(" px"),
            );
        });
        ui.separator();
        self.stage_settings(ui);
    }

    /// Stage size and background. Size edits apply on release or Enter so that scrubbing a
    /// value is a single undo step.
    fn stage_settings(&mut self, ui: &mut egui::Ui) {
        ui.label("Stage");
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

    fn library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Library");
        let entries: Vec<(SymbolId, String)> = self
            .document()
            .symbols()
            .iter()
            .map(|s| (s.id(), s.name().to_owned()))
            .collect();
        if entries.is_empty() {
            ui.weak("No symbols");
        }
        for (id, name) in entries {
            let chosen = self.selected_symbol == Some(id);
            if ui.selectable_label(chosen, name).clicked() {
                self.selected_symbol = Some(id);
            }
        }
        ui.separator();
        if ui
            .add_enabled(
                self.selected_symbol.is_some(),
                egui::Button::new("Place on stage"),
            )
            .clicked()
        {
            self.place_selected_symbol();
        }
    }

    fn stage(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
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
        let to_doc = |p: Pos2| screen_to_stage(p, image_rect, extent);
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
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
        if let Some(scene) = self.scene_id() {
            for selected in self.selection_all() {
                if let Some(bounds) = ops::span_bounds(self.document(), scene, selected) {
                    let shown = bounds.translate(moving);
                    let rect = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
                    painter.rect_stroke(rect, 0.0, outline, StrokeKind::Outside);
                }
            }
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
        ui.horizontal(|ui| {
            let label = if self.playing { "Pause" } else { "Play" };
            if ui.button(label).clicked() {
                self.playing = !self.playing;
            }
            let last = self.scene_frames() - 1;
            let mut frame = self.frame;
            ui.add(
                egui::DragValue::new(&mut frame)
                    .range(0..=last)
                    .prefix("Frame "),
            );
            if frame != self.frame {
                self.set_frame(frame);
            }
            let rate = self.document().frame_rate();
            ui.weak(format!("{}/{} fps", rate.numerator(), rate.denominator()));
            ui.separator();
            let mut enabled = self.onion.enabled;
            ui.checkbox(&mut enabled, "Onion skin");
            self.set_onion_skin(enabled);
            ui.add_enabled_ui(enabled, |ui| {
                let (mut before, mut after) = (self.onion.before, self.onion.after);
                ui.add(
                    egui::DragValue::new(&mut before)
                        .range(0..=MAX_ONION_FRAMES)
                        .prefix("before "),
                );
                ui.add(
                    egui::DragValue::new(&mut after)
                        .range(0..=MAX_ONION_FRAMES)
                        .prefix("after "),
                );
                self.set_onion_range(before, after);
            });
            ui.separator();
            if ui
                .small_button("\u{2212}")
                .on_hover_text("Zoom timeline out")
                .clicked()
            {
                self.zoom_timeline(1.0 / timeline_view::STEP);
            }
            if ui
                .small_button("+")
                .on_hover_text("Zoom timeline in")
                .clicked()
            {
                self.zoom_timeline(timeline_view::STEP);
            }
            if ui
                .small_button("Fit")
                .on_hover_text("Fit the scene")
                .clicked()
            {
                self.timeline_zoom = timeline_view::MIN_ZOOM;
            }
            ui.separator();
            // Renaming needs text input, which a menu item cannot provide.
            let edit = ui.add(
                egui::TextEdit::singleline(&mut self.layer_name_edit)
                    .hint_text("Rename active layer")
                    .desired_width(150.0),
            );
            let submitted = edit.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            if (ui.button("Rename").clicked() || submitted) && !self.layer_name_edit.is_empty() {
                let name = std::mem::take(&mut self.layer_name_edit);
                self.rename_active_layer(&name);
            }
        });
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
            let label_w = 110.0;
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
                if p.x >= track.min.x {
                    let f = ((p.x - track.min.x) / per_frame).floor().max(0.0) as u32;
                    self.set_frame(f);
                }
                // Clicking a row (below the ruler) makes that layer the active one.
                let index = ((p.y - rect.min.y) / row).floor() as i64 - 1;
                let picked = usize::try_from(index).ok().and_then(|i| {
                    let layers = self.document().scenes().first()?.timeline().layers();
                    layers.get(i).map(Layer::id)
                });
                if let Some(id) = picked {
                    // The two small toggles at the right edge of the label column.
                    let eye = p.x >= track.min.x - 40.0 && p.x < track.min.x - 20.0;
                    let lock = p.x >= track.min.x - 20.0 && p.x < track.min.x;
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

            // Ruler ticks roughly every 40px.
            let step = ((40.0 / per_frame).ceil() as u32).max(1);
            let ink = ui.visuals().text_color();
            for f in (0..frames).step_by(step as usize) {
                let x = track.min.x + f as f32 * per_frame;
                painter.text(
                    Pos2::new(x + 2.0, rect.min.y),
                    Align2::LEFT_TOP,
                    f.to_string(),
                    FontId::monospace(10.0),
                    ink,
                );
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
                    let dim = ink.gamma_multiply(0.35);
                    painter.text(
                        Pos2::new(rect.min.x + 4.0, y + 3.0),
                        Align2::LEFT_TOP,
                        layer.name(),
                        FontId::proportional(12.0),
                        if layer.is_visible() { ink } else { dim },
                    );
                    let flag = |x: f32, on: bool, glyph: &str| {
                        painter.text(
                            Pos2::new(x, y + 3.0),
                            Align2::LEFT_TOP,
                            glyph,
                            FontId::monospace(12.0),
                            if on { ink } else { dim },
                        );
                    };
                    flag(track.min.x - 36.0, layer.is_visible(), "V");
                    flag(track.min.x - 16.0, layer.is_locked(), "L");
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
                [Pos2::new(head, rect.min.y), Pos2::new(head, rect.max.y)],
                Stroke::new(2.0, Color32::from_rgb(220, 60, 60)),
            );
        });
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
            Pane::Library => self.workspace.library(ui),
            Pane::Tools => {
                egui::ScrollArea::vertical().show(ui, |ui| self.workspace.tools(ui));
            }
        }
        egui_tiles::UiResponse::None
    }

    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.title().into()
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use egui::{Event, RawInput};

    fn document() -> Document {
        let layer = Layer::new(LayerId::new(), "Layer 1", vec![]).expect("layer");
        let scene = Scene::new(SceneId::new(), "Scene 1", Timeline::new(vec![layer]));
        let rate = FrameRate::new(24, 1).expect("rate");
        Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("document")
    }

    fn workspace() -> Workspace {
        Workspace::new(document(), None)
    }

    fn layer_count(ws: &Workspace) -> usize {
        ws.document().scenes()[0].timeline().layers().len()
    }

    fn input(time: f64, events: Vec<Event>) -> RawInput {
        RawInput {
            time: Some(time),
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0))),
            events,
            ..RawInput::default()
        }
    }

    /// Runs one UI pass. A real renderer consumes the texture updates; tests discard them.
    fn pass(ws: &mut Workspace, ctx: &egui::Context, input: RawInput) {
        let mut output = ctx.run_ui(input, |ui| ws.show(ui));
        output.textures_delta.clear();
    }

    fn run(ws: &mut Workspace, ctx: &egui::Context, time: f64) {
        pass(ws, ctx, input(time, vec![]));
    }

    fn key(key: Key, modifiers: Modifiers) -> Event {
        Event::Key {
            key,
            physical_key: Some(key),
            pressed: true,
            repeat: false,
            modifiers,
        }
    }

    fn rect(ws: &mut Workspace) {
        ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(110.0, 60.0));
    }

    #[test]
    fn renders_the_stage_and_refreshes_when_content_or_frame_changes() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0);
        assert_eq!(ws.texture.as_ref().map(|t| t.key), Some((0, 0, 0)));
        rect(&mut ws);
        run(&mut ws, &ctx, 0.1);
        let after_edit = ws.texture.as_ref().expect("texture").key;
        assert_eq!(after_edit.2, 0);
        assert!(after_edit.1 > 0);
        ws.set_frame(5);
        run(&mut ws, &ctx, 0.2);
        assert_eq!(ws.texture.as_ref().map(|t| t.key.2), Some(5));
    }

    #[test]
    fn untitled_documents_start_clean_with_one_layer() {
        let ws = Workspace::untitled().expect("untitled");
        assert_eq!(layer_count(&ws), 1);
        assert!(!ws.is_dirty());
        assert_eq!(ws.title(), "Untitled - Flare");
    }

    #[test]
    fn a_rectangle_is_one_undo_step_and_tracks_dirty_state() {
        let mut ws = workspace();
        assert!(!ws.is_dirty());
        rect(&mut ws);
        assert_eq!(layer_count(&ws), 2, "layer and span are added together");
        assert!(ws.is_dirty());
        assert!(ws.title().contains('*'));
        ws.undo();
        assert_eq!(layer_count(&ws), 1);
        assert!(!ws.is_dirty());
        ws.redo();
        assert_eq!(layer_count(&ws), 2);
    }

    #[test]
    fn rectangles_get_a_stroke_only_when_outline_is_enabled() {
        let mut ws = workspace();
        rect(&mut ws);
        ws.stroke_enabled = true;
        ws.stroke_color = Color32::from_rgb(1, 2, 3);
        ws.stroke_width = 6.0;
        ws.commit_rectangle(Pos2::new(200.0, 200.0), Pos2::new(260.0, 240.0));
        let stroke_of = |layer: usize| {
            let timeline = ws.document().scenes()[0].timeline();
            match timeline.layers()[layer].spans()[0].content() {
                Content::Shape(shape) => shape.stroke(),
                _ => panic!("shape expected"),
            }
        };
        assert_eq!(stroke_of(1), None);
        let stroke = stroke_of(2).expect("outlined rectangle");
        assert_eq!(
            (stroke.width(), stroke.color()),
            (6.0, Rgba8::opaque(1, 2, 3))
        );
    }

    #[test]
    fn clicks_and_degenerate_drags_create_nothing() {
        let mut ws = workspace();
        ws.commit_rectangle(Pos2::new(5.0, 5.0), Pos2::new(5.0, 5.0));
        ws.commit_rectangle(Pos2::new(5.0, 5.0), Pos2::new(5.5, 90.0));
        assert_eq!(layer_count(&ws), 1);
        assert!(!ws.is_dirty());
    }

    #[test]
    fn keyboard_shortcuts_undo_and_redo() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0);
        rect(&mut ws);
        let command = Modifiers::COMMAND;
        pass(&mut ws, &ctx, input(0.1, vec![key(Key::Z, command)]));
        assert_eq!(layer_count(&ws), 1);
        let both = Modifiers::COMMAND | Modifiers::SHIFT;
        pass(&mut ws, &ctx, input(0.2, vec![key(Key::Z, both)]));
        assert_eq!(layer_count(&ws), 2);
    }

    #[test]
    fn escape_cancels_a_drag_without_committing() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0);
        ws.drag = Some(Drag {
            bounds: Rect::NOTHING,
            mode: DragMode::Draw,
            start: Pos2::new(1.0, 1.0),
            current: Pos2::new(50.0, 50.0),
        });
        pass(
            &mut ws,
            &ctx,
            input(0.1, vec![key(Key::Escape, Modifiers::NONE)]),
        );
        assert!(ws.drag.is_none());
        assert!(ws.drag_cancelled);
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn playback_advances_at_the_document_frame_rate_and_loops() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        rect(&mut ws); // makes the scene 24 frames long
        run(&mut ws, &ctx, 0.0);
        ws.playing = true;
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.5);
        assert_eq!(ws.frame(), 12);
        run(&mut ws, &ctx, 1.0);
        assert_eq!(ws.frame(), 0, "24 fps over 24 frames wraps around");
        ws.playing = false;
        run(&mut ws, &ctx, 5.0);
        assert_eq!(ws.frame(), 0);
    }

    #[test]
    fn frames_are_clamped_to_the_scene() {
        let mut ws = workspace();
        ws.set_frame(10_000);
        assert_eq!(ws.frame(), 0, "an empty scene has a single frame");
        rect(&mut ws);
        ws.set_frame(10_000);
        assert_eq!(ws.frame(), 23);
    }

    #[test]
    fn save_and_open_round_trip_and_unsaved_work_is_protected() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        let mut ws = workspace();
        ws.save();
        assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
        rect(&mut ws);
        ws.save_as(&path);
        assert!(!ws.is_dirty());
        assert_eq!(ws.path(), Some(path.as_path()));

        rect(&mut ws);
        ws.open(&path);
        assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
        assert_eq!(layer_count(&ws), 3, "dirty work was not replaced");

        ws.save();
        let mut other = workspace();
        other.open(&path);
        assert_eq!(layer_count(&other), 3);
        assert_eq!(other.path(), Some(path.as_path()));
    }

    #[test]
    fn failed_open_reports_and_keeps_the_current_document() {
        let dir = tempfile::tempdir().expect("dir");
        let mut ws = workspace();
        ws.open(&dir.path().join("missing.flare"));
        assert!(ws.status().is_some_and(|s| s.contains("Cannot open")));
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn screen_positions_map_to_clamped_stage_coordinates() {
        let rect = Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(400.0, 225.0)); // half scale
        assert_eq!(
            screen_to_stage(Pos2::new(100.0, 50.0), rect, Vec2::new(800.0, 450.0)),
            Pos2::ZERO
        );
        assert_eq!(
            screen_to_stage(Pos2::new(300.0, 162.5), rect, Vec2::new(800.0, 450.0)),
            Pos2::new(400.0, 225.0)
        );
        assert_eq!(
            screen_to_stage(Pos2::new(-999.0, 9999.0), rect, Vec2::new(800.0, 450.0)),
            Pos2::new(0.0, 450.0)
        );
    }

    fn spans_of(ws: &Workspace, layer: usize) -> usize {
        ws.document().scenes()[0].timeline().layers()[layer]
            .spans()
            .len()
    }

    /// Select the rectangle drawn by `rect()` (it covers 10..110 x 10..60).
    fn draw_and_select(ws: &mut Workspace) {
        rect(ws);
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.end_drag(); // a plain press and release: selects, moves nothing
    }

    fn shape_bounds(ws: &Workspace) -> Rect {
        let scene = ws.scene_id().expect("scene");
        let selected = ws.selected.expect("selected");
        ops::span_bounds(ws.document(), scene, selected).expect("bounds")
    }

    #[test]
    fn select_tool_selects_the_shape_under_the_pointer_and_clears_on_empty_space() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        assert!(ws.has_selection());
        ws.begin_drag(Pos2::new(700.0, 400.0));
        assert!(!ws.has_selection());
        assert!(ws.drag.is_none(), "nothing to move");
    }

    #[test]
    fn dragging_a_selection_moves_it_as_one_undo_step() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        let before = ws.document().clone();
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.update_drag(Pos2::new(80.0, 20.0));
        assert_eq!(
            ws.document(),
            &before,
            "the document is untouched mid-gesture"
        );
        ws.end_drag();
        assert_eq!(shape_bounds(&ws).min, Pos2::new(40.0, 0.0));
        ws.undo();
        assert_eq!(ws.document(), &before);
        assert!(ws.has_selection(), "the span still exists after undo");
    }

    #[test]
    fn cancelled_and_switched_gestures_commit_nothing() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        let before = ws.document().clone();
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.update_drag(Pos2::new(300.0, 300.0));
        ws.cancel_drag();
        ws.end_drag();
        assert_eq!(ws.document(), &before);

        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.update_drag(Pos2::new(300.0, 300.0));
        ws.set_tool(Tool::Rectangle);
        ws.end_drag();
        assert_eq!(ws.document(), &before);
    }

    #[test]
    fn delete_removes_the_selection_and_undo_brings_it_back() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        ws.delete_selection();
        assert_eq!(spans_of(&ws, 1), 0);
        assert!(!ws.has_selection());
        ws.undo();
        assert_eq!(spans_of(&ws, 1), 1);
    }

    #[test]
    fn convert_to_symbol_then_place_instances_from_the_library() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        assert!(ws.can_convert_selection());
        ws.convert_selection_to_symbol();
        assert_eq!(ws.document().symbols().len(), 1);
        assert!(ws.selected_symbol.is_some());
        assert!(
            ws.has_selection(),
            "the instance span takes the shape's place"
        );
        assert!(!ws.can_convert_selection(), "an instance is not a shape");

        let layers = layer_count(&ws);
        ws.place_selected_symbol();
        assert_eq!(layer_count(&ws), layers + 1);
        ws.undo();
        assert_eq!(layer_count(&ws), layers);
        ws.undo();
        assert_eq!(ws.document().symbols().len(), 0);
        assert!(
            ws.selected_symbol.is_none(),
            "stale library selection is dropped"
        );
    }

    #[test]
    fn placing_without_a_library_selection_explains_itself() {
        let mut ws = workspace();
        ws.place_selected_symbol();
        assert!(ws.status().is_some_and(|s| s.contains("library")));
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn delete_and_f8_shortcuts_act_on_the_selection() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        draw_and_select(&mut ws);
        run(&mut ws, &ctx, 0.0);
        pass(
            &mut ws,
            &ctx,
            input(0.1, vec![key(Key::F8, Modifiers::NONE)]),
        );
        assert_eq!(ws.document().symbols().len(), 1);
        pass(
            &mut ws,
            &ctx,
            input(0.2, vec![key(Key::Delete, Modifiers::NONE)]),
        );
        assert_eq!(spans_of(&ws, 1), 0);
    }

    #[test]
    fn stage_edits_are_undoable_and_invalid_sizes_are_refused() {
        let mut ws = workspace();
        let original = ws.document().stage();
        ws.set_stage_size(400, 200);
        assert_eq!(
            (
                ws.document().stage().width(),
                ws.document().stage().height()
            ),
            (400, 200)
        );
        assert!(ws.is_dirty());
        ws.set_stage_background(Rgba8::opaque(1, 2, 3));
        assert_eq!(ws.document().stage().background(), Rgba8::opaque(1, 2, 3));
        assert_eq!(ws.document().stage().width(), 400, "size is kept");
        ws.undo();
        ws.undo();
        assert_eq!(ws.document().stage(), original);

        ws.set_stage_size(0, 100);
        assert!(ws.status().is_some_and(|s| s.contains("stage sides")));
        assert_eq!(ws.document().stage(), original);
        let before = ws.document().clone();
        ws.set_stage_background(original.background());
        assert_eq!(
            ws.document(),
            &before,
            "an unchanged background is not an edit"
        );
    }

    #[test]
    fn placement_and_rendering_follow_the_document_stage() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.set_stage_size(400, 200);
        draw_and_select(&mut ws);
        ws.convert_selection_to_symbol();
        ws.place_selected_symbol();
        let scene = ws.scene_id().expect("scene");
        let layer = ws.document().scenes()[0]
            .timeline()
            .layers()
            .last()
            .expect("layer")
            .id();
        let placed =
            ops::span_bounds(ws.document(), scene, SpanRef { layer, start: 0 }).expect("bounds");
        assert_eq!(placed.center(), Pos2::new(200.0, 100.0));
        run(&mut ws, &ctx, 0.0);
        assert!(ws.texture.is_some());
        assert_eq!(ws.stage_extent(), Vec2::new(400.0, 200.0));
    }

    fn active_spans(ws: &Workspace) -> Vec<(u32, u32)> {
        ws.active_layer()
            .expect("layer")
            .spans()
            .iter()
            .map(|s| (s.start(), s.duration().get()))
            .collect()
    }

    #[test]
    fn drawing_makes_the_new_layer_active_and_frame_ops_act_on_it() {
        let mut ws = workspace();
        rect(&mut ws);
        assert_eq!(
            ws.active_layer_id(),
            Some(ws.document().scenes()[0].timeline().layers()[1].id())
        );
        assert_eq!(active_spans(&ws), vec![(0, 24)]);
        ws.set_frame(10);
        ws.frame_edit(FrameOp::InsertKeyframe);
        assert_eq!(active_spans(&ws), vec![(0, 10), (10, 14)]);
        ws.frame_edit(FrameOp::InsertBlankKeyframe); // already a keyframe at 10
        assert!(
            ws.status()
                .is_some_and(|s| s.contains("Cannot insert a blank keyframe"))
        );
        ws.set_frame(3);
        ws.frame_edit(FrameOp::InsertFrame);
        assert_eq!(active_spans(&ws), vec![(0, 11), (11, 14)]);
        ws.frame_edit(FrameOp::RemoveFrame);
        assert_eq!(active_spans(&ws), vec![(0, 10), (10, 14)]);
        ws.set_frame(10);
        ws.frame_edit(FrameOp::ClearKeyframe);
        assert_eq!(active_spans(&ws), vec![(0, 24)]);
        ws.undo();
        assert_eq!(
            active_spans(&ws),
            vec![(0, 10), (10, 14)],
            "each operation is one undo step"
        );
    }

    #[test]
    fn selecting_a_layer_redirects_frame_operations() {
        let mut ws = workspace();
        rect(&mut ws);
        let first = ws.document().scenes()[0].timeline().layers()[0].id();
        ws.select_layer(first);
        ws.frame_edit(FrameOp::InsertFrame);
        assert!(
            ws.status().is_some(),
            "the first layer is empty, so there is nothing to extend"
        );
        assert_eq!(layer_count(&ws), 2);
    }

    #[test]
    fn function_keys_drive_frame_operations() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        rect(&mut ws);
        run(&mut ws, &ctx, 0.0);
        ws.set_frame(6);
        pass(
            &mut ws,
            &ctx,
            input(0.1, vec![key(Key::F6, Modifiers::NONE)]),
        );
        assert_eq!(active_spans(&ws), vec![(0, 6), (6, 18)]);
        pass(
            &mut ws,
            &ctx,
            input(0.2, vec![key(Key::F5, Modifiers::NONE)]),
        );
        assert_eq!(active_spans(&ws), vec![(0, 6), (6, 19)]);
        pass(
            &mut ws,
            &ctx,
            input(0.3, vec![key(Key::F5, Modifiers::SHIFT)]),
        );
        assert_eq!(active_spans(&ws), vec![(0, 6), (6, 18)]);
        pass(
            &mut ws,
            &ctx,
            input(0.4, vec![key(Key::F6, Modifiers::SHIFT)]),
        );
        assert_eq!(active_spans(&ws), vec![(0, 24)]);
        ws.set_frame(8);
        pass(
            &mut ws,
            &ctx,
            input(0.5, vec![key(Key::F7, Modifiers::NONE)]),
        );
        assert_eq!(active_spans(&ws), vec![(0, 8), (8, 16)]);
    }

    #[test]
    fn ellipse_and_line_tools_commit_shapes_on_their_own_layers() {
        let mut ws = workspace();
        ws.set_tool(Tool::Ellipse);
        ws.begin_drag(Pos2::new(100.0, 100.0));
        ws.update_drag(Pos2::new(160.0, 140.0));
        ws.end_drag();
        ws.set_tool(Tool::Line);
        ws.begin_drag(Pos2::new(10.0, 10.0));
        ws.update_drag(Pos2::new(90.0, 40.0));
        ws.end_drag();
        let layers = ws.document().scenes()[0].timeline().layers();
        assert_eq!(layers.len(), 3);
        let shape_of = |layer: usize| match layers[layer].spans()[0].content() {
            Content::Shape(shape) => shape.clone(),
            _ => panic!("shape expected"),
        };
        assert_eq!(layers[1].name(), "Ellipse");
        let ellipse = shape_of(1);
        let bounds = ops::shape_bounds(&ellipse).expect("bounds");
        assert_eq!(bounds.min, Pos2::new(100.0, 100.0));
        assert_eq!(bounds.max, Pos2::new(160.0, 140.0));
        assert_eq!(layers[2].name(), "Line");
        let line = shape_of(2);
        assert_eq!(line.path().len(), 2);
        assert!(
            line.stroke().is_some(),
            "a line is its stroke, even with Outline off"
        );
        assert!(!ws.stroke_enabled);
    }

    #[test]
    fn zero_length_lines_and_flat_ellipses_create_nothing() {
        let mut ws = workspace();
        ws.set_tool(Tool::Line);
        ws.begin_drag(Pos2::new(5.0, 5.0));
        ws.end_drag();
        ws.set_tool(Tool::Ellipse);
        ws.begin_drag(Pos2::new(5.0, 5.0));
        ws.update_drag(Pos2::new(50.0, 5.4));
        ws.end_drag();
        assert_eq!(layer_count(&ws), 1);
        assert!(!ws.is_dirty());
    }

    #[test]
    fn motion_tweens_toggle_on_a_selected_instance_and_interpolate() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        assert_eq!(ws.selection_tween(), None, "a shape cannot tween");
        ws.set_selection_tween(Tween::Linear);
        assert!(
            ws.status()
                .is_some_and(|s| s.contains("Select an instance"))
        );
        ws.convert_selection_to_symbol();
        assert_eq!(ws.selection_tween(), Some(Tween::None));
        // Two keyframes with different positions, tweened between them.
        ws.set_frame(10);
        ws.frame_edit(FrameOp::InsertKeyframe);
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.update_drag(Pos2::new(150.0, 30.0)); // moves the second keyframe 100 right
        ws.end_drag();
        ws.set_frame(0);
        ws.select_layer(ws.active_layer_id().expect("layer"));
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.end_drag();
        ws.set_selection_tween(Tween::Linear);
        assert_eq!(ws.selection_tween(), Some(Tween::Linear));
        let scene = ws.scene_id().expect("scene");
        let at_frame = |ws: &Workspace, f: u32| {
            let frame = flare_eval::evaluate_frame(ws.document(), scene, f).expect("eval");
            frame.shapes[0].transform.translation.x
        };
        assert_eq!(at_frame(&ws, 0), 0.0);
        assert_eq!(at_frame(&ws, 5), 50.0, "halfway through the tween");
        assert_eq!(at_frame(&ws, 10), 100.0);
    }

    fn active_layer(ws: &Workspace) -> Layer {
        ws.active_layer().expect("layer").clone()
    }

    #[test]
    fn layer_operations_rename_hide_lock_move_and_delete() {
        let mut ws = workspace();
        rect(&mut ws); // layers: [Layer 1 (empty), Rectangle]
        ws.rename_active_layer("  Hero ");
        assert_eq!(active_layer(&ws).name(), "Hero");
        ws.rename_active_layer("   ");
        assert!(ws.status().is_some_and(|s| s.contains("needs a name")));
        assert_eq!(active_layer(&ws).name(), "Hero");

        let id = ws.active_layer_id().expect("id");
        ws.toggle_layer_visible(id);
        assert!(!active_layer(&ws).is_visible());
        ws.toggle_layer_visible(id);
        assert!(active_layer(&ws).is_visible());

        ws.move_active_layer(-1);
        assert_eq!(ws.document().scenes()[0].timeline().layers()[0].id(), id);
        ws.move_active_layer(-1);
        assert!(ws.status().is_some_and(|s| s.contains("edge")));
        ws.delete_active_layer();
        assert_eq!(layer_count(&ws), 1);
        ws.delete_active_layer();
        assert!(
            ws.status()
                .is_some_and(|s| s.contains("at least one layer"))
        );
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn locked_layers_are_not_selectable_and_refuse_frame_edits() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        let id = ws.active_layer_id().expect("id");
        ws.toggle_layer_locked(id);
        assert!(
            !ws.has_selection(),
            "locking drops a selection on that layer"
        );
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(50.0, 30.0));
        assert!(!ws.has_selection(), "clicks go through a locked layer");
        ws.frame_edit(FrameOp::InsertFrame);
        assert!(ws.status().is_some_and(|s| s.contains("locked")));
        ws.toggle_layer_locked(id);
        ws.begin_drag(Pos2::new(50.0, 30.0));
        assert!(ws.has_selection());
    }

    #[test]
    fn clicks_select_through_a_locked_layer_to_the_one_below() {
        let mut ws = workspace();
        ws.stroke_enabled = false;
        ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0)); // lower layer
        ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0)); // new layer, on top? (below in order)
        let layers: Vec<LayerId> = ws.document().scenes()[0]
            .timeline()
            .layers()
            .iter()
            .map(Layer::id)
            .collect();
        // Layers are top-to-bottom; the last one is drawn first, so layers[1] paints over layers[2].
        ws.toggle_layer_locked(layers[1]);
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(50.0, 50.0));
        assert_eq!(ws.selected.map(|s| s.layer), Some(layers[2]));
    }

    #[test]
    fn hidden_layers_do_not_render_or_hit() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        let id = ws.active_layer_id().expect("id");
        ws.toggle_layer_visible(id);
        assert!(ws.hit(Pos2::new(50.0, 30.0)).is_none());
        let scene = ws.scene_id().expect("scene");
        let frame = flare_eval::evaluate_frame(ws.document(), scene, 0).expect("eval");
        assert!(frame.shapes.is_empty());
    }

    /// A workspace with symbol "Symbol 1" (a converted rectangle) placed in the scene, with
    /// the instance selected.
    fn with_symbol() -> (Workspace, SymbolId) {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        ws.convert_selection_to_symbol();
        let symbol = ws.selected_symbol.expect("symbol");
        (ws, symbol)
    }

    fn symbol_layers(ws: &Workspace, symbol: SymbolId) -> usize {
        ws.session
            .document()
            .symbol(symbol)
            .expect("symbol")
            .timeline()
            .layers()
            .len()
    }

    #[test]
    fn editing_a_symbol_in_place_applies_to_the_parent_as_one_undo_step() {
        let (mut ws, symbol) = with_symbol();
        let before = ws.document().clone();
        ws.edit_selected_symbol();
        assert!(ws.in_symbol());
        assert_eq!(
            ws.breadcrumb(),
            vec!["Scene 1".to_owned(), "Symbol 1".to_owned()]
        );
        assert_eq!(ws.document().scenes().len(), 1);
        assert_eq!(ws.frame(), 0);

        ws.set_tool(Tool::Rectangle);
        ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
        assert!(ws.is_dirty());
        assert_eq!(
            symbol_layers(&ws, symbol),
            1,
            "the parent is untouched until you leave"
        );

        assert!(ws.exit_symbol());
        assert!(!ws.in_symbol());
        assert_eq!(symbol_layers(&ws, symbol), 2);
        ws.undo();
        assert_eq!(ws.document(), &before, "leaving was a single undo step");
    }

    #[test]
    fn undo_inside_a_symbol_is_local_to_that_symbol() {
        let (mut ws, symbol) = with_symbol();
        ws.edit_selected_symbol();
        ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
        assert_eq!(layer_count(&ws), 2);
        ws.undo();
        assert_eq!(layer_count(&ws), 1);
        ws.undo(); // nothing left to undo inside; the scene's own history is not touched
        assert_eq!(layer_count(&ws), 1);
        assert!(ws.exit_symbol());
        assert_eq!(
            symbol_layers(&ws, symbol),
            1,
            "no net change leaves no edit behind"
        );
        assert_eq!(ws.document().symbols().len(), 1);
    }

    #[test]
    fn symbols_created_inside_a_symbol_join_the_library_when_you_leave() {
        let (mut ws, outer) = with_symbol();
        ws.edit_selected_symbol();
        // Inside: select the shape and convert it, creating a nested symbol.
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.end_drag();
        assert!(ws.can_convert_selection());
        ws.convert_selection_to_symbol();
        let inner = ws.selected_symbol.expect("inner symbol");
        assert_ne!(inner, outer);
        assert!(ws.exit_symbol());
        assert!(
            ws.document().symbol(inner).is_some(),
            "the new symbol reached the library"
        );
        assert_eq!(ws.document().symbols().len(), 2);
        // The outer symbol now contains an instance of the new one.
        let outer_symbol = ws.document().symbol(outer).expect("outer");
        assert!(
            outer_symbol
                .timeline()
                .referenced_symbols()
                .any(|s| s == inner)
        );
    }

    #[test]
    fn nested_symbols_can_be_entered_and_left_by_breadcrumb_depth() {
        let (mut ws, _) = with_symbol();
        ws.edit_selected_symbol();
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(50.0, 30.0));
        ws.end_drag();
        ws.convert_selection_to_symbol();
        ws.edit_selected_symbol();
        assert_eq!(ws.breadcrumb().len(), 3);
        assert!(ws.exit_to_depth(0));
        assert!(!ws.in_symbol());
        assert_eq!(ws.document().symbols().len(), 2);
    }

    #[test]
    fn a_symbol_cannot_contain_itself_or_its_own_ancestors() {
        let (mut ws, outer) = with_symbol();
        ws.edit_selected_symbol();
        ws.selected_symbol = Some(outer);
        ws.place_selected_symbol();
        assert!(
            ws.status()
                .is_some_and(|s| s.contains("cannot contain itself"))
        );
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn a_rejected_exit_keeps_the_edit_open_with_the_reason() {
        use flare_document::{Instance, Symbol};
        // B contains A. Editing A and placing B inside it would make a loop.
        let a = SymbolId::new();
        let b = SymbolId::new();
        let instance = |symbol| {
            let i = Instance::new(
                flare_core::InstanceId::new(),
                symbol,
                flare_core::Transform2D::IDENTITY,
            );
            Content::Instance(i)
        };
        let one = |content| {
            let span = FrameSpan::new(0, NonZeroU32::MIN, content);
            Timeline::new(vec![
                Layer::new(LayerId::new(), "L", vec![span]).expect("layer"),
            ])
        };
        let base = document();
        let doc = Document::new(
            base.id(),
            base.frame_rate(),
            base.scenes().to_vec(),
            vec![
                Symbol::new(a, "A", Timeline::default()),
                Symbol::new(b, "B", one(instance(a))),
            ],
        )
        .expect("doc");
        let mut ws = Workspace::new(doc, None);
        assert!(ws.enter_symbol(a));
        ws.selected_symbol = Some(b);
        ws.place_selected_symbol(); // allowed inside the scratch: B is not A
        assert_eq!(layer_count(&ws), 1, "a layer with B was added");
        assert!(!ws.exit_symbol(), "A containing B containing A is a loop");
        assert!(ws.in_symbol(), "still editing A");
        assert!(ws.status().is_some_and(|s| s.contains("Cannot leave")));
        ws.undo();
        assert!(ws.exit_symbol(), "after undoing the loop, leaving works");
    }

    #[test]
    fn saving_inside_a_symbol_applies_pending_edits_first() {
        let dir = tempfile::tempdir().expect("dir");
        let path = dir.path().join("a.flare");
        let (mut ws, symbol) = with_symbol();
        ws.edit_selected_symbol();
        ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
        ws.save_as(&path);
        assert!(!ws.in_symbol(), "saving left the symbol");
        assert!(!ws.is_dirty());
        let mut reopened = workspace();
        reopened.open(&path);
        assert_eq!(symbol_layers(&reopened, symbol), 2);
    }

    #[test]
    fn entering_and_leaving_refresh_the_stage_and_missing_symbols_are_reported() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0);
        let before = ws.texture.as_ref().expect("texture").key;
        assert!(!ws.enter_symbol(SymbolId::new()));
        assert!(ws.status().is_some_and(|s| s.contains("no longer exists")));

        let (mut ws, _) = with_symbol();
        run(&mut ws, &ctx, 0.1);
        let scene_key = ws.texture.as_ref().expect("texture").key;
        ws.edit_selected_symbol();
        run(&mut ws, &ctx, 0.2);
        let symbol_key = ws.texture.as_ref().expect("texture").key;
        assert_ne!(
            scene_key.0, symbol_key.0,
            "contexts never share a cache key"
        );
        assert!(before.0 == 0);
    }

    #[test]
    fn menu_actions_report_state_and_perform_through_the_workspace() {
        let mut ws = workspace();
        assert!(!ws.is_enabled(MenuAction::Undo));
        assert!(!ws.is_enabled(MenuAction::Delete));
        assert!(!ws.is_enabled(MenuAction::ExitSymbol));
        assert!(
            !ws.is_enabled(MenuAction::DeleteLayer),
            "the last layer stays"
        );
        assert!(ws.is_enabled(MenuAction::Save) && ws.is_enabled(MenuAction::Quit));

        ws.perform(MenuAction::Undo); // disabled: must be a no-op, not an error
        assert!(!ws.is_dirty());

        ws.perform(MenuAction::ToolEllipse);
        assert_eq!(ws.tool(), Tool::Ellipse);
        rect(&mut ws);
        assert!(ws.is_enabled(MenuAction::Undo));
        assert!(ws.is_enabled(MenuAction::DeleteLayer));
        assert!(ws.is_enabled(MenuAction::MoveLayerUp));
        assert!(
            !ws.is_enabled(MenuAction::MoveLayerDown),
            "already the bottom layer"
        );
        ws.perform(MenuAction::Undo);
        assert_eq!(layer_count(&ws), 1);
        ws.perform(MenuAction::Redo);
        assert_eq!(layer_count(&ws), 2);

        ws.perform(MenuAction::PlayPause);
        assert!(ws.is_playing());
        assert_eq!(ws.label(MenuAction::PlayPause), "Pause");
        ws.perform(MenuAction::PlayPause);
        assert_eq!(ws.label(MenuAction::PlayPause), "Play");
    }

    #[test]
    fn toggling_actions_relabel_themselves() {
        let mut ws = workspace();
        rect(&mut ws);
        assert_eq!(ws.label(MenuAction::ToggleLayerVisible), "Hide Layer");
        ws.perform(MenuAction::ToggleLayerVisible);
        assert_eq!(ws.label(MenuAction::ToggleLayerVisible), "Show Layer");
        assert_eq!(ws.label(MenuAction::ToggleLayerLocked), "Lock Layer");
        ws.perform(MenuAction::ToggleLayerLocked);
        assert_eq!(ws.label(MenuAction::ToggleLayerLocked), "Unlock Layer");
        assert!(
            !ws.is_enabled(MenuAction::InsertFrame),
            "a locked layer refuses frame edits"
        );
    }

    #[test]
    fn tween_and_symbol_actions_follow_the_selection() {
        let (mut ws, _) = with_symbol();
        assert!(ws.is_enabled(MenuAction::EditSymbol));
        assert!(
            !ws.is_enabled(MenuAction::ConvertToSymbol),
            "an instance is not a shape"
        );
        assert_eq!(
            ws.label(MenuAction::ToggleMotionTween),
            "Create Motion Tween"
        );
        ws.perform(MenuAction::ToggleMotionTween);
        assert_eq!(
            ws.label(MenuAction::ToggleMotionTween),
            "Remove Motion Tween"
        );
        ws.perform(MenuAction::EditSymbol);
        assert!(ws.in_symbol());
        assert!(ws.is_enabled(MenuAction::ExitSymbol));
        ws.perform(MenuAction::ExitSymbol);
        assert!(!ws.in_symbol());
    }

    #[test]
    fn dialogs_and_quit_are_requests_for_the_shell() {
        let mut ws = workspace();
        ws.perform(MenuAction::Open);
        assert_eq!(ws.take_dialog(), Some(DialogRequest::Open));
        ws.perform(MenuAction::SaveAs);
        assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
        assert!(!ws.take_quit_request());
        ws.perform(MenuAction::Quit);
        assert!(ws.take_quit_request());
        assert!(!ws.take_quit_request(), "a request is delivered once");
    }

    #[test]
    fn a_native_menu_owns_its_shortcuts_so_they_are_not_applied_twice() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        rect(&mut ws);
        ws.set_native_menu(true);
        run(&mut ws, &ctx, 0.0);
        let command = Modifiers::COMMAND;
        pass(&mut ws, &ctx, input(0.1, vec![key(Key::Z, command)]));
        assert_eq!(layer_count(&ws), 2, "undo was left to the native menu");
        pass(
            &mut ws,
            &ctx,
            input(0.2, vec![key(Key::F6, Modifiers::NONE)]),
        );
        assert_eq!(active_spans(&ws), vec![(0, 24)], "so was insert keyframe");
        ws.set_native_menu(false);
        pass(&mut ws, &ctx, input(0.3, vec![key(Key::Z, command)]));
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn autosave_writes_a_recovery_file_only_when_dirty_and_the_interval_has_passed() {
        let dir = tempfile::tempdir().expect("dir");
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.enable_recovery(dir.path().to_owned());
        ws.set_autosave_interval(10.0);
        let id = ws.document().id();
        let file = flare_storage::recovery_path(dir.path(), id);

        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 60.0);
        assert!(!file.exists(), "a clean document is never autosaved");

        rect(&mut ws);
        run(&mut ws, &ctx, 61.0);
        assert!(
            !file.exists(),
            "the interval counts from the last clean moment"
        );
        run(&mut ws, &ctx, 71.5);
        assert!(file.exists());
        assert_eq!(flare_storage::load(&file).expect("load"), *ws.document());

        let before = std::fs::metadata(&file)
            .expect("meta")
            .modified()
            .expect("time");
        run(&mut ws, &ctx, 72.0);
        assert_eq!(
            std::fs::metadata(&file)
                .expect("meta")
                .modified()
                .expect("time"),
            before
        );
    }

    #[test]
    fn saving_or_undoing_back_to_clean_removes_the_recovery_file() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("p.flare");
        let store = dir.path().join("recovery");
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.enable_recovery(store.clone());
        ws.set_autosave_interval(1.0);
        let file = flare_storage::recovery_path(&store, ws.document().id());

        run(&mut ws, &ctx, 0.0);
        rect(&mut ws);
        run(&mut ws, &ctx, 5.0);
        assert!(file.exists());
        ws.undo();
        run(&mut ws, &ctx, 6.0);
        assert!(!file.exists(), "undone back to the clean state");

        rect(&mut ws);
        run(&mut ws, &ctx, 20.0);
        assert!(file.exists());
        ws.save_as(&project);
        assert!(
            !file.exists(),
            "saving to the project supersedes the recovery file"
        );
    }

    #[test]
    fn recovered_work_is_unsaved_and_keeps_its_file_until_saved() {
        let dir = tempfile::tempdir().expect("dir");
        let store = dir.path().join("recovery");
        let mut crashed = workspace();
        rect(&mut crashed);
        let saved = flare_storage::save_recovery(crashed.document(), &store).expect("recovery");

        let mut ws = workspace();
        ws.enable_recovery(store.clone());
        ws.recover(&saved);
        assert_eq!(layer_count(&ws), 2);
        assert!(
            ws.is_dirty(),
            "recovered work has never been saved to a project"
        );
        assert!(ws.path().is_none());
        assert!(saved.exists(), "kept in case of another crash");
        ws.discard_recovery();
        assert!(!saved.exists());
    }

    #[test]
    fn recovery_refuses_to_replace_unsaved_work_and_reports_bad_files() {
        let dir = tempfile::tempdir().expect("dir");
        let mut ws = workspace();
        rect(&mut ws);
        ws.recover(&dir.path().join("nope.flare"));
        assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
        assert_eq!(layer_count(&ws), 2);
        ws.undo();
        ws.recover(&dir.path().join("nope.flare"));
        assert!(ws.status().is_some_and(|s| s.contains("Cannot recover")));
    }

    #[test]
    fn opening_a_project_keeps_the_native_menu_tool_and_recovery_settings() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("p.flare");
        let mut source = workspace();
        source.save_as(&project);

        let mut ws = workspace();
        ws.set_native_menu(true);
        ws.set_tool(Tool::Ellipse);
        ws.fill = Color32::from_rgb(1, 2, 3);
        ws.stroke_enabled = true;
        ws.enable_recovery(dir.path().join("recovery"));
        ws.open(&project);
        assert_eq!(ws.path(), Some(project.as_path()));
        assert!(
            ws.native_menu,
            "opening must not turn the in-window menu back on"
        );
        assert_eq!(ws.tool(), Tool::Ellipse);
        assert_eq!(ws.fill, Color32::from_rgb(1, 2, 3));
        assert!(ws.stroke_enabled);
        assert!(ws.recovery_dir.is_some());
    }

    #[test]
    fn the_dock_survives_drawing_and_every_panel_stays_present() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        for t in 0..5 {
            run(&mut ws, &ctx, f64::from(t));
        }
        let json = ws.layout_json().expect("serialize");
        assert!(
            layout::from_json(&json).is_ok(),
            "all four panels are still in the layout"
        );
        assert!(
            ws.texture.is_some(),
            "the stage panel was drawn inside the dock"
        );
    }

    #[test]
    fn a_saved_layout_restores_and_an_unusable_one_is_refused_and_changes_nothing() {
        let mut ws = workspace();
        let saved = ws.layout_json().expect("serialize");
        ws.reset_layout();
        ws.restore_layout(&saved).expect("restore");

        let before = ws.layout_json().expect("serialize");
        for bad in ["not json", "{}", &saved.replace("\"Tools\"", "\"Stage\"")] {
            assert!(ws.restore_layout(bad).is_err(), "{bad}");
        }
        let after = ws.layout_json().expect("serialize");
        let value = |t: &str| serde_json::from_str::<serde_json::Value>(t).expect("json");
        assert_eq!(
            value(&before),
            value(&after),
            "a refused layout leaves the current one alone"
        );
    }

    #[test]
    fn resetting_the_layout_never_touches_the_document() {
        let mut ws = workspace();
        rect(&mut ws);
        let document = ws.document().clone();
        let dirty = ws.is_dirty();
        ws.perform(MenuAction::ResetLayout);
        assert_eq!(ws.document(), &document);
        assert_eq!(ws.is_dirty(), dirty);
        assert!(ws.is_enabled(MenuAction::ResetLayout));
    }

    #[test]
    fn opening_a_project_keeps_the_users_panel_arrangement() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("p.flare");
        let mut source = workspace();
        source.save_as(&project);
        let mut ws = workspace();
        // A real arrangement: every panel in one tab group, nothing like the default.
        let mut tiles = egui_tiles::Tiles::default();
        let panes: Vec<_> = [Pane::Timeline, Pane::Library, Pane::Stage, Pane::Tools]
            .into_iter()
            .map(|pane| tiles.insert_pane(pane))
            .collect();
        let root = tiles.insert_tab_tile(panes);
        let custom_tree = egui_tiles::Tree::new(egui::Id::unique("custom"), root, tiles);
        let custom = layout::to_json(&custom_tree).expect("serialize");
        ws.restore_layout(&custom)
            .expect("a tab group of all four panels is valid");

        let value = |t: &str| serde_json::from_str::<serde_json::Value>(t).expect("json");
        let default = layout::to_json(&layout::default_tree()).expect("serialize");
        assert_ne!(
            value(&custom),
            value(&default),
            "the test needs a non-default layout"
        );

        ws.open(&project);
        assert_eq!(value(&ws.layout_json().expect("serialize")), value(&custom));
        ws.reset_layout();
        assert_eq!(
            value(&ws.layout_json().expect("serialize")),
            value(&default)
        );
    }

    #[test]
    fn ghosts_are_chosen_nearest_last_with_falling_opacity_and_stay_inside_the_scene() {
        // Frame 5 of 10, two before and two after: farthest first so nearer ones draw on top.
        let ghosts = onion_ghosts(5, 10, 2, 2);
        let frames: Vec<u32> = ghosts.iter().map(|(f, _)| *f).collect();
        assert_eq!(frames, vec![3, 7, 4, 6]);
        assert!(ghosts[0].1 < ghosts[2].1, "farther ghosts are fainter");
        assert_eq!(ghosts[2].1, ghosts[3].1, "equal distance, equal opacity");
        // Edges of the scene are skipped, never wrapped or negative.
        assert_eq!(onion_ghosts(0, 10, 3, 0), vec![]);
        assert_eq!(onion_ghosts(9, 10, 0, 3), vec![]);
        assert_eq!(
            onion_ghosts(1, 3, 3, 3)
                .iter()
                .map(|(f, _)| *f)
                .collect::<Vec<_>>(),
            vec![0, 2]
        );
        // The range is capped, and zero means none.
        assert_eq!(
            onion_ghosts(100, 1000, 50, 0).len(),
            MAX_ONION_FRAMES as usize
        );
        assert!(onion_ghosts(5, 10, 0, 0).is_empty());
        assert!(
            onion_ghosts(u32::MAX, u32::MAX, 0, 2).is_empty(),
            "no overflow at the top"
        );
    }

    #[test]
    fn onion_skin_re_renders_when_toggled_and_is_off_while_playing() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0);
        assert_eq!(ws.texture.as_ref().map(|t| t.onion.enabled), Some(false));
        let before = ws.texture.as_ref().expect("texture").key;

        ws.perform(MenuAction::ToggleOnionSkin);
        assert!(ws.onion_skin().enabled);
        assert_eq!(ws.label(MenuAction::ToggleOnionSkin), "Hide Onion Skin");
        run(&mut ws, &ctx, 0.1);
        assert_eq!(ws.texture.as_ref().map(|t| t.onion.enabled), Some(true));
        assert_eq!(
            ws.texture.as_ref().expect("texture").key,
            before,
            "same content, new settings"
        );

        rect(&mut ws);
        ws.playing = true;
        run(&mut ws, &ctx, 0.2);
        assert_eq!(
            ws.texture.as_ref().map(|t| t.onion.enabled),
            Some(false),
            "off while playing"
        );
        ws.playing = false;
        run(&mut ws, &ctx, 0.3);
        assert_eq!(
            ws.texture.as_ref().map(|t| t.onion.enabled),
            Some(true),
            "back when stopped"
        );
    }

    #[test]
    fn onion_range_is_clamped_and_survives_opening_a_project() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("p.flare");
        let mut source = workspace();
        source.save_as(&project);
        let mut ws = workspace();
        ws.set_onion_range(99, 3);
        ws.set_onion_skin(true);
        assert_eq!(
            (ws.onion_skin().before, ws.onion_skin().after),
            (MAX_ONION_FRAMES, 3)
        );
        ws.open(&project);
        assert_eq!(
            ws.onion_skin(),
            OnionSkin {
                enabled: true,
                before: MAX_ONION_FRAMES,
                after: 3
            }
        );
    }

    #[test]
    fn test_movie_is_a_request_for_the_shell_and_includes_pending_symbol_edits() {
        let (mut ws, symbol) = with_symbol();
        assert!(!ws.take_test_movie_request());
        ws.perform(MenuAction::TestMovie);
        assert!(ws.take_test_movie_request());
        assert!(!ws.take_test_movie_request(), "delivered once");

        ws.edit_selected_symbol();
        ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
        assert!(ws.in_symbol());
        assert_eq!(
            ws.main_document()
                .symbol(symbol)
                .expect("symbol")
                .timeline()
                .layers()
                .len(),
            1
        );
        ws.perform(MenuAction::TestMovie);
        assert!(
            !ws.in_symbol(),
            "the symbol edit was applied before playing"
        );
        assert_eq!(
            ws.main_document()
                .symbol(symbol)
                .expect("symbol")
                .timeline()
                .layers()
                .len(),
            2
        );
        assert!(ws.take_test_movie_request());
        assert_eq!(ws.label(MenuAction::TestMovie), "Test Movie");
    }

    #[test]
    fn a_line_can_be_selected_by_clicking_its_stroke_and_moved() {
        let mut ws = workspace();
        ws.set_tool(Tool::Line);
        ws.begin_drag(Pos2::new(100.0, 100.0));
        ws.update_drag(Pos2::new(300.0, 100.0));
        ws.end_drag();
        ws.set_tool(Tool::Select);
        ws.begin_drag(Pos2::new(200.0, 101.0)); // within half the 4px stroke
        assert!(
            ws.has_selection(),
            "a line has no fill, so only its stroke can be clicked"
        );
        ws.update_drag(Pos2::new(200.0, 151.0));
        ws.end_drag();
        let bounds = ops::span_bounds(
            ws.document(),
            ws.scene_id().expect("scene"),
            ws.selected.expect("selected"),
        )
        .expect("bounds");
        assert!(
            (bounds.center().y - 150.0).abs() < 1.0,
            "moved 50 down: {bounds:?}"
        );
        ws.begin_drag(Pos2::new(200.0, 400.0));
        assert!(!ws.has_selection(), "empty space still deselects");
    }

    fn pen_triangle(ws: &mut Workspace) {
        ws.set_tool(Tool::Pen);
        for at in [(10.0, 10.0), (110.0, 10.0), (110.0, 90.0)] {
            ws.pen_press(Pos2::new(at.0, at.1));
            ws.pen_release();
        }
    }

    fn layer_names(ws: &Workspace) -> Vec<String> {
        ws.document().scenes()[0]
            .timeline()
            .layers()
            .iter()
            .map(|l| l.name().to_string())
            .collect()
    }

    #[test]
    fn finishing_a_pen_path_is_one_undo_step_on_a_new_layer() {
        let mut ws = workspace();
        pen_triangle(&mut ws);
        assert!(ws.pen_is_active());
        assert_eq!(layer_count(&ws), 1, "nothing is committed while drawing");
        ws.pen_finish();
        assert!(!ws.pen_is_active());
        assert_eq!(layer_count(&ws), 2);
        assert_eq!(layer_names(&ws)[1], "Path");
        ws.undo();
        assert_eq!(layer_count(&ws), 1, "one step removes the whole path");
    }

    #[test]
    fn an_open_pen_path_is_stroked_even_with_outline_off() {
        let mut ws = workspace();
        ws.stroke_enabled = false;
        pen_triangle(&mut ws);
        ws.pen_finish();
        let layers = ws.document().scenes()[0].timeline().layers();
        let Some(Content::Shape(shape)) = layers[1].spans().first().map(|s| s.content().clone())
        else {
            panic!("a shape expected")
        };
        assert!(
            shape.stroke().is_some(),
            "an open path with no stroke would be invisible"
        );
    }

    #[test]
    fn a_closed_pen_path_strokes_only_when_outline_is_on() {
        for outline in [false, true] {
            let mut ws = workspace();
            ws.stroke_enabled = outline;
            pen_triangle(&mut ws);
            ws.pen_press(Pos2::new(11.0, 11.0));
            assert!(
                !ws.pen_is_active(),
                "clicking the first point closes and commits"
            );
            let layers = ws.document().scenes()[0].timeline().layers();
            let Some(Content::Shape(shape)) =
                layers[1].spans().first().map(|s| s.content().clone())
            else {
                panic!("a shape expected")
            };
            assert_eq!(shape.stroke().is_some(), outline);
        }
    }

    #[test]
    fn escape_and_switching_tools_cancel_the_pen_path() {
        let mut ws = workspace();
        pen_triangle(&mut ws);
        ws.pen_cancel();
        assert!(!ws.pen_is_active());
        pen_triangle(&mut ws);
        ws.set_tool(Tool::Select);
        assert!(!ws.pen_is_active());
        assert_eq!(layer_count(&ws), 1, "cancelling commits nothing");
    }

    #[test]
    fn finishing_with_one_point_reports_and_keeps_nothing() {
        let mut ws = workspace();
        ws.set_tool(Tool::Pen);
        ws.pen_press(Pos2::new(5.0, 5.0));
        ws.pen_release();
        ws.pen_finish();
        assert_eq!(ws.status(), Some("A path needs at least two points"));
        assert_eq!(layer_count(&ws), 1);
    }

    #[test]
    fn the_pen_menu_action_selects_the_tool() {
        let mut ws = workspace();
        ws.perform(MenuAction::ToolPen);
        assert_eq!(ws.tool(), Tool::Pen);
    }

    /// Two rectangles side by side on separate layers; returns the workspace.
    fn two_rectangles() -> Workspace {
        let mut ws = workspace();
        for (a, b) in [((10.0, 10.0), (50.0, 50.0)), ((100.0, 10.0), (140.0, 50.0))] {
            ws.set_tool(Tool::Rectangle);
            ws.begin_drag(Pos2::new(a.0, a.1));
            ws.update_drag(Pos2::new(b.0, b.1));
            ws.end_drag();
        }
        ws.set_tool(Tool::Select);
        ws
    }

    fn rect_x(ws: &Workspace, layer: usize) -> f32 {
        let layers = ws.document().scenes()[0].timeline().layers();
        let Content::Shape(shape) = layers[layer].spans()[0].content() else {
            panic!("a shape expected")
        };
        ops::shape_bounds(shape).expect("bounds").min.x
    }

    #[test]
    fn shift_click_adds_and_removes_spans_from_the_selection() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        assert_eq!(ws.selection_all().len(), 2);
        ws.select_at(Pos2::new(30.0, 30.0), true);
        assert_eq!(ws.selection_all().len(), 1, "shift-click again removes it");
        ws.select_at(Pos2::new(30.0, 30.0), false);
        assert_eq!(
            ws.selection_all().len(),
            1,
            "a plain click replaces the selection"
        );
        ws.select_at(Pos2::new(300.0, 300.0), true);
        assert_eq!(ws.selection_all().len(), 1, "shift on empty stage keeps it");
        ws.select_at(Pos2::new(300.0, 300.0), false);
        assert!(ws.selection_all().is_empty());
    }

    #[test]
    fn dragging_a_selected_span_moves_the_whole_selection_in_one_undo_step() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        ws.begin_drag(Pos2::new(30.0, 30.0));
        ws.update_drag(Pos2::new(40.0, 30.0));
        ws.end_drag();
        assert_eq!((rect_x(&ws, 1), rect_x(&ws, 2)), (20.0, 110.0));
        assert_eq!(ws.selection_all().len(), 2, "the group stays selected");
        ws.undo();
        assert_eq!((rect_x(&ws, 1), rect_x(&ws, 2)), (10.0, 100.0));
    }

    #[test]
    fn delete_removes_every_selected_span_at_once() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        ws.delete_selection();
        assert!(ws.selection_all().is_empty());
        let layers = ws.document().scenes()[0].timeline().layers();
        assert!(layers[1].spans().is_empty() && layers[2].spans().is_empty());
        ws.undo();
        let layers = ws.document().scenes()[0].timeline().layers();
        assert!(!layers[1].spans().is_empty() && !layers[2].spans().is_empty());
    }

    #[test]
    fn locking_a_layer_drops_only_its_span_from_the_selection() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        let locked = ws.document().scenes()[0].timeline().layers()[1].id();
        ws.toggle_layer_locked(locked);
        assert_eq!(ws.selection_all().len(), 1);
        assert_eq!(
            ws.selection_all()[0].layer,
            ws.document().scenes()[0].timeline().layers()[2].id()
        );
    }

    #[test]
    fn zoom_menu_actions_zoom_the_stage_and_fit_resets_it() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0); // lays out the stage panel
        assert!(!ws.is_enabled(MenuAction::ZoomFit), "already fitted");
        ws.perform(MenuAction::ZoomIn);
        assert!(!ws.stage_view.is_fit());
        assert!(ws.is_enabled(MenuAction::ZoomFit));
        let zoomed = ws.stage_view;
        ws.perform(MenuAction::ZoomOut);
        assert_ne!(ws.stage_view, zoomed);
        ws.perform(MenuAction::ZoomIn);
        ws.perform(MenuAction::ZoomIn);
        ws.perform(MenuAction::ZoomFit);
        assert!(ws.stage_view.is_fit());
        assert!(!ws.is_dirty(), "the view is not part of the document");
    }

    #[test]
    fn duplicate_copies_the_selection_independently_on_a_new_layer() {
        let mut ws = workspace();
        draw_and_select(&mut ws);
        let original = ws.selected.expect("selected");
        ws.perform(MenuAction::Duplicate);
        assert_eq!(layer_count(&ws), 3);
        let copy = ws.selected.expect("the copy is selected");
        assert_ne!(copy.layer, original.layer);
        let scene = ws.scene_id().expect("scene");
        let bounds = |at| ops::span_bounds(ws.document(), scene, at).expect("bounds");
        assert_eq!(bounds(copy).min, bounds(original).min + Vec2::splat(12.0));
        let id_of = |at: SpanRef| match ops::find_span(ws.document(), scene, at)
            .expect("span")
            .content()
        {
            Content::Shape(shape) => shape.id(),
            _ => panic!("a shape expected"),
        };
        assert_ne!(
            id_of(copy),
            id_of(original),
            "the copy has its own identity"
        );
        // Moving the copy leaves the original alone.
        let before = bounds(original);
        let _ = bounds;
        ws.move_selection(Vec2::new(40.0, 0.0));
        let scene = ws.scene_id().expect("scene");
        let after = ops::span_bounds(ws.document(), scene, original).expect("bounds");
        assert_eq!(after, before);
        ws.undo();
        ws.undo();
        assert_eq!(layer_count(&ws), 2, "duplicating is one undo step");
    }

    #[test]
    fn duplicate_without_a_selection_does_nothing() {
        let mut ws = workspace();
        assert!(!ws.is_enabled(MenuAction::Duplicate));
        ws.perform(MenuAction::Duplicate);
        assert_eq!(layer_count(&ws), 1);
        assert!(!ws.is_dirty());
    }

    #[test]
    fn duplicate_copies_every_selected_span_and_selects_the_copies() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        let originals = ws.selection_all();
        ws.perform(MenuAction::Duplicate);
        assert_eq!(layer_count(&ws), 5);
        let copies = ws.selection_all();
        assert_eq!(copies.len(), 2);
        assert!(copies.iter().all(|c| !originals.contains(c)));
        ws.undo();
        assert_eq!(layer_count(&ws), 3, "one undo step for the whole group");
    }

    fn selected_shape(ws: &Workspace) -> Shape {
        let scene = ws.scene_id().expect("scene");
        match ops::find_span(ws.document(), scene, ws.selected.expect("selected"))
            .expect("span")
            .content()
        {
            Content::Shape(shape) => shape.clone(),
            _ => panic!("a shape expected"),
        }
    }

    fn endpoints(shape: &Shape) -> Vec<(f64, f64)> {
        shape
            .path()
            .iter()
            .filter_map(|s| match s {
                flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
                    Some((p.as_vec().x, p.as_vec().y))
                }
                _ => None,
            })
            .collect()
    }

    /// A selected line from (10, 10) to (50, 30), which is not symmetric under flips.
    fn line_selected() -> Workspace {
        let mut ws = workspace();
        ws.set_tool(Tool::Line);
        ws.begin_drag(Pos2::new(10.0, 10.0));
        ws.update_drag(Pos2::new(50.0, 30.0));
        ws.end_drag();
        ws.set_tool(Tool::Select);
        // Strokes are not hit-testable on this branch, so select the new layer's span directly.
        let layer = ws.current_layer.expect("the line's layer");
        ws.selected = Some(SpanRef { layer, start: 0 });
        ws
    }

    #[test]
    fn flips_mirror_about_the_center_of_the_selection() {
        let mut ws = line_selected();
        ws.perform(MenuAction::FlipHorizontal);
        assert_eq!(
            endpoints(&selected_shape(&ws)),
            vec![(50.0, 10.0), (10.0, 30.0)]
        );
        ws.perform(MenuAction::FlipVertical);
        assert_eq!(
            endpoints(&selected_shape(&ws)),
            vec![(50.0, 30.0), (10.0, 10.0)]
        );
        assert!(
            selected_shape(&ws).stroke().is_some(),
            "the stroke survives"
        );
    }

    #[test]
    fn rotation_turns_clockwise_on_screen_and_four_turns_come_home() {
        let mut ws = line_selected();
        ws.perform(MenuAction::RotateClockwise);
        let turned = endpoints(&selected_shape(&ws));
        // (10,10) is up-left of the center (30,20); clockwise 90 degrees puts it at (40,0).
        assert!(
            (turned[0].0 - 40.0).abs() < 1e-9 && turned[0].1.abs() < 1e-9,
            "{turned:?}"
        );
        ws.perform(MenuAction::RotateCounterclockwise);
        let back = endpoints(&selected_shape(&ws));
        assert!(
            (back[0].0 - 10.0).abs() < 1e-9 && (back[0].1 - 10.0).abs() < 1e-9,
            "{back:?}"
        );
        for _ in 0..4 {
            ws.perform(MenuAction::RotateClockwise);
        }
        let home = endpoints(&selected_shape(&ws));
        assert!(
            (home[1].0 - 50.0).abs() < 1e-9 && (home[1].1 - 30.0).abs() < 1e-9,
            "{home:?}"
        );
    }

    #[test]
    fn scaling_grows_and_shrinks_about_the_center_and_each_is_one_undo_step() {
        let mut ws = line_selected();
        // The path itself scales; the stroke width does not, so measure the path.
        let length = |ws: &Workspace| {
            let p = endpoints(&selected_shape(ws));
            ((p[1].0 - p[0].0).powi(2) + (p[1].1 - p[0].1).powi(2)).sqrt()
        };
        let middle = |ws: &Workspace| {
            let p = endpoints(&selected_shape(ws));
            ((p[0].0 + p[1].0) / 2.0, (p[0].1 + p[1].1) / 2.0)
        };
        let before = length(&ws);
        ws.perform(MenuAction::ScaleUp);
        let grown = length(&ws);
        assert!((grown - before * 1.25).abs() < 1e-9);
        assert_eq!(middle(&ws), (30.0, 20.0), "scaling is about the center");
        ws.perform(MenuAction::ScaleDown);
        assert!((length(&ws) - before).abs() < 1e-9);
        ws.undo();
        assert!((length(&ws) - grown).abs() < 1e-9);
    }

    #[test]
    fn transforms_need_a_selection_and_an_instance_keeps_its_symbol() {
        let mut ws = workspace();
        assert!(!ws.is_enabled(MenuAction::FlipHorizontal));
        draw_and_select(&mut ws);
        ws.convert_selection_to_symbol();
        let scene = ws.scene_id().expect("scene");
        let at = ws.selected.expect("instance selected");
        let symbol = match ops::find_span(ws.document(), scene, at)
            .expect("span")
            .content()
        {
            Content::Instance(i) => i.symbol(),
            _ => panic!("an instance expected"),
        };
        let before = shape_bounds(&ws);
        ws.perform(MenuAction::RotateClockwise);
        let after = shape_bounds(&ws);
        assert!((after.width() - before.height()).abs() < 1e-3);
        assert!((after.center() - before.center()).length() < 1e-3);
        match ops::find_span(ws.document(), scene, at)
            .expect("span")
            .content()
        {
            Content::Instance(i) => assert_eq!(i.symbol(), symbol),
            _ => panic!("still an instance"),
        }
    }

    #[test]
    fn degenerate_matrices_are_refused() {
        let mut ws = line_selected();
        let before = ws.document().clone();
        ws.transform_selection(DAffine2::from_scale(DVec2::ZERO));
        ws.transform_selection(DAffine2::from_scale(DVec2::new(f64::NAN, 1.0)));
        assert_eq!(ws.document(), &before);
    }

    #[test]
    fn transforming_a_group_pivots_on_the_center_of_the_whole_selection() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        // The rectangles span x 10..140; flipping the group swaps them end for end.
        ws.perform(MenuAction::FlipHorizontal);
        assert_eq!((rect_x(&ws, 1), rect_x(&ws, 2)), (100.0, 10.0));
        ws.undo();
        assert_eq!(
            (rect_x(&ws, 1), rect_x(&ws, 2)),
            (10.0, 100.0),
            "one undo step"
        );
    }

    /// One layer holding spans at 0..3 and 6..8, for timeline drags.
    fn spans_workspace() -> (Workspace, LayerId) {
        let id = LayerId::new();
        let span = |start: u32, len: u32| {
            FrameSpan::new(
                start,
                NonZeroU32::new(len).expect("nonzero"),
                Content::Empty,
            )
        };
        let layer = Layer::new(id, "L", vec![span(0, 3), span(6, 2)]).expect("layer");
        let scene = Scene::new(SceneId::new(), "S", Timeline::new(vec![layer]));
        let rate = FrameRate::new(24, 1).expect("rate");
        let doc = Document::new(DocumentId::new(), rate, vec![scene], vec![]).expect("doc");
        (Workspace::new(doc, None), id)
    }

    #[test]
    fn dragging_a_span_body_moves_it_and_the_edge_resizes_it() {
        let (mut ws, id) = spans_workspace();
        assert!(ws.begin_span_drag(id, 1.5, 0.3));
        ws.update_span_drag(4);
        assert_eq!(
            ws.span_drag_preview(),
            Some((id, 3, 3)),
            "grabbed one frame in"
        );
        ws.end_span_drag();
        assert_eq!(active_spans_of(&ws, id), vec![(3, 3), (6, 2)]);
        ws.undo();
        assert_eq!(
            active_spans_of(&ws, id),
            vec![(0, 3), (6, 2)],
            "one undo step"
        );

        assert!(ws.begin_span_drag(id, 2.9, 0.3), "within the edge");
        ws.update_span_drag(5);
        assert_eq!(ws.span_drag_preview(), Some((id, 0, 6)));
        ws.end_span_drag();
        assert_eq!(active_spans_of(&ws, id), vec![(0, 6), (6, 2)]);
    }

    fn active_spans_of(ws: &Workspace, id: LayerId) -> Vec<(u32, u32)> {
        ws.layer(id)
            .expect("layer")
            .spans()
            .iter()
            .map(|s| (s.start(), s.duration().get()))
            .collect()
    }

    #[test]
    fn a_span_drag_that_would_overlap_is_refused_with_a_message() {
        let (mut ws, id) = spans_workspace();
        assert!(ws.begin_span_drag(id, 0.5, 0.3));
        ws.update_span_drag(5); // 5..8 meets the span at 6
        ws.end_span_drag();
        assert_eq!(active_spans_of(&ws, id), vec![(0, 3), (6, 2)]);
        assert!(ws.status().is_some_and(|m| m.contains("overlap")));
        assert!(!ws.is_dirty());
    }

    #[test]
    fn gaps_locked_layers_and_escape_do_not_drag_spans() {
        let (mut ws, id) = spans_workspace();
        assert!(!ws.begin_span_drag(id, 4.5, 0.3), "a gap scrubs instead");
        assert!(!ws.begin_span_drag(id, -1.0, 0.3));
        assert!(!ws.begin_span_drag(id, f32::NAN, 0.3));
        assert!(ws.begin_span_drag(id, 0.5, 0.3));
        ws.update_span_drag(2);
        ws.cancel_drag();
        ws.end_span_drag();
        assert_eq!(
            active_spans_of(&ws, id),
            vec![(0, 3), (6, 2)],
            "escape cancels"
        );
        ws.toggle_layer_locked(id);
        assert!(!ws.begin_span_drag(id, 0.5, 0.3), "locked layers refuse");
    }

    #[test]
    fn timeline_zoom_changes_the_view_not_the_document_and_still_draws() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        run(&mut ws, &ctx, 0.0);
        ws.zoom_timeline(timeline_view::STEP);
        assert!(ws.timeline_zoom > 1.0);
        run(&mut ws, &ctx, 0.1);
        ws.zoom_timeline(1000.0);
        assert_eq!(ws.timeline_zoom, timeline_view::MAX_ZOOM);
        run(&mut ws, &ctx, 0.2);
        ws.zoom_timeline(0.0001);
        assert_eq!(ws.timeline_zoom, timeline_view::MIN_ZOOM);
        assert!(!ws.is_dirty());
    }

    #[test]
    fn gradient_fills_apply_to_new_shapes_across_their_bounds_and_survive_moves() {
        let mut ws = workspace();
        ws.gradient_enabled = true;
        ws.gradient_end = Color32::from_rgb(0, 0, 255);
        ws.fill = Color32::from_rgb(255, 0, 0);
        ws.commit_rectangle(Pos2::new(10.0, 20.0), Pos2::new(110.0, 60.0));
        let layer = ws.current_layer.expect("layer");
        ws.selected = Some(SpanRef { layer, start: 0 });
        let g = selected_shape(&ws).gradient().expect("a gradient");
        assert_eq!((g.from().as_vec().x, g.from().as_vec().y), (10.0, 20.0));
        assert_eq!(
            (g.to().as_vec().x, g.to().as_vec().y),
            (110.0, 20.0),
            "horizontal"
        );
        assert_eq!(g.start(), Rgba8::opaque(255, 0, 0));
        assert_eq!(g.end(), Rgba8::opaque(0, 0, 255));

        ws.move_selection(Vec2::new(5.0, 7.0));
        let g = selected_shape(&ws)
            .gradient()
            .expect("the move keeps the gradient");
        assert_eq!((g.from().as_vec().x, g.from().as_vec().y), (15.0, 27.0));
        assert_eq!(
            (g.to().as_vec().x, g.to().as_vec().y),
            (115.0, 27.0),
            "and moves it too"
        );

        ws.gradient_vertical = true;
        ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(50.0, 80.0));
        let layer = ws.current_layer.expect("layer");
        ws.selected = Some(SpanRef { layer, start: 0 });
        let g = selected_shape(&ws).gradient().expect("a gradient");
        assert_eq!(
            (g.to().as_vec().x, g.to().as_vec().y),
            (0.0, 80.0),
            "vertical"
        );
    }

    #[test]
    fn gradients_survive_duplicating_and_the_setting_survives_opening_a_project() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("p.flare");
        let mut ws = workspace();
        ws.gradient_enabled = true;
        ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
        let layer = ws.current_layer.expect("layer");
        ws.selected = Some(SpanRef { layer, start: 0 });
        ws.save_as(&project);
        ws.open(&project);
        assert!(ws.gradient_enabled, "opening keeps the tool settings");
        let layer = ws.document().scenes()[0].timeline().layers()[1].id();
        ws.selected = Some(SpanRef { layer, start: 0 });
        assert!(
            selected_shape(&ws).gradient().is_some(),
            "saved and reloaded"
        );
    }

    #[test]
    fn copy_then_paste_places_independent_copies_in_place_at_the_current_frame() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        assert!(!ws.is_enabled(MenuAction::Paste), "nothing copied yet");
        ws.perform(MenuAction::Copy);
        assert!(ws.is_enabled(MenuAction::Paste));
        ws.set_frame(5);
        ws.perform(MenuAction::Paste);
        assert_eq!(layer_count(&ws), 5);
        let pasted = ws.selection_all();
        assert_eq!(pasted.len(), 2);
        assert!(pasted.iter().all(|s| s.start == 5), "at the current frame");
        let scene = ws.scene_id().expect("scene");
        let ids: Vec<ShapeId> = ws.document().scenes()[0]
            .timeline()
            .layers()
            .iter()
            .flat_map(|l| l.spans().iter())
            .filter_map(|s| match s.content() {
                Content::Shape(shape) => Some(shape.id()),
                _ => None,
            })
            .collect();
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(ids.len(), unique.len(), "every shape has its own identity");
        let bounds = ops::span_bounds(ws.document(), scene, pasted[0]).expect("bounds");
        assert_eq!(bounds.min.x, 10.0, "pasted in place, not offset");
        // The clipboard survives the original being deleted, and one undo removes the paste.
        ws.undo();
        assert_eq!(layer_count(&ws), 3);
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.delete_selection();
        ws.perform(MenuAction::Paste);
        assert_eq!(layer_count(&ws), 5);
    }

    #[test]
    fn copy_and_paste_events_act_on_the_selection_unless_a_field_wants_the_keys() {
        let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
        ws.select_at(Pos2::new(30.0, 30.0), false);
        pass(&mut ws, &ctx, input(0.0, vec![Event::Copy]));
        assert!(ws.is_enabled(MenuAction::Paste));
        pass(&mut ws, &ctx, input(0.1, vec![Event::Paste(String::new())]));
        assert_eq!(layer_count(&ws), 4);
    }

    #[test]
    fn pasting_an_instance_into_a_document_without_its_symbol_is_refused() {
        let mut source = workspace();
        draw_and_select(&mut source);
        source.convert_selection_to_symbol();
        source.perform(MenuAction::Copy);
        assert!(source.is_enabled(MenuAction::Paste));
        let items = source.clipboard.clone();
        let mut other = workspace();
        other.clipboard = items;
        other.perform(MenuAction::Paste);
        assert_eq!(layer_count(&other), 1);
        assert!(other.status().is_some_and(|m| m.contains("symbol")));
        assert!(!other.is_dirty());
        // Back in the document that has the symbol it works.
        source.perform(MenuAction::Paste);
        assert_eq!(layer_count(&source), 3);
    }

    #[test]
    fn copying_nothing_or_an_empty_span_keeps_the_clipboard() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.perform(MenuAction::Copy);
        let kept = ws.clipboard.clone();
        ws.select_at(Pos2::new(300.0, 300.0), false);
        ws.copy_selection();
        assert_eq!(ws.clipboard, kept);
    }

    #[test]
    fn dragging_a_corner_handle_scales_the_selection_about_the_opposite_corner() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.begin_drag(Pos2::new(50.0, 50.0)); // the bottom-right handle
        assert!(matches!(
            ws.drag.map(|d| d.mode),
            Some(DragMode::Scale(Handle::BottomRight))
        ));
        ws.update_drag(Pos2::new(90.0, 90.0));
        ws.end_drag();
        let b = shape_bounds(&ws);
        assert_eq!(
            (b.min, b.max),
            (Pos2::new(10.0, 10.0), Pos2::new(90.0, 90.0))
        );
        assert_eq!(rect_x(&ws, 2), 100.0, "the other rectangle is untouched");
        ws.undo();
        assert_eq!(
            shape_bounds(&ws).max,
            Pos2::new(50.0, 50.0),
            "one undo step"
        );
    }

    #[test]
    fn handles_of_a_group_scale_every_member_about_the_group_anchor() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.select_at(Pos2::new(120.0, 30.0), true);
        // The group spans x 10..140; the right handle sits at (140, 30).
        ws.begin_drag(Pos2::new(140.0, 30.0));
        ws.update_drag(Pos2::new(270.0, 30.0));
        ws.end_drag();
        // Doubling the width about x = 10: the second rectangle (100..140) lands at 190..270.
        assert_eq!(rect_x(&ws, 1), 10.0);
        assert_eq!(rect_x(&ws, 2), 190.0);
    }

    #[test]
    fn shift_keeps_proportions_and_pressing_off_a_handle_still_moves() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.uniform_scale = true;
        ws.begin_drag(Pos2::new(50.0, 50.0));
        ws.update_drag(Pos2::new(90.0, 60.0)); // x doubles, y grows less
        ws.end_drag();
        let b = shape_bounds(&ws);
        assert_eq!((b.width(), b.height()), (80.0, 80.0));

        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.begin_drag(Pos2::new(30.0, 30.0)); // inside, away from every handle
        assert_eq!(ws.drag.map(|d| d.mode), Some(DragMode::Move));
    }

    #[test]
    fn a_cancelled_scale_changes_nothing_and_shift_press_never_grabs_a_handle() {
        let mut ws = two_rectangles();
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.begin_drag(Pos2::new(50.0, 50.0));
        ws.update_drag(Pos2::new(90.0, 90.0));
        ws.cancel_drag();
        ws.end_drag();
        assert_eq!(shape_bounds(&ws).max, Pos2::new(50.0, 50.0));
        ws.begin_drag_with(Pos2::new(50.0, 50.0), true);
        assert!(ws.drag.is_none(), "shift only edits the selection");
    }

    #[test]
    fn dragging_to_the_very_last_frame_does_not_overflow() {
        let (mut ws, id) = spans_workspace();
        assert!(ws.begin_span_drag(id, 2.9, 0.3), "the right edge");
        ws.update_span_drag(u32::MAX);
        let (_, start, duration) = ws.span_drag_preview().expect("preview");
        assert_eq!((start, u64::from(duration)), (0, u64::from(u32::MAX)));
        ws.end_span_drag();
        assert_eq!(
            active_spans_of(&ws, id),
            vec![(0, 3), (6, 2)],
            "it would overlap, so refused"
        );
    }

    #[test]
    fn only_rows_in_view_are_painted() {
        let clip = |a: f32, b: f32| Rect::from_min_max(Pos2::new(0.0, a), Pos2::new(100.0, b));
        // Header row 0..22; layer 0 spans 22..44, layer 1 44..66, and so on.
        assert_eq!(
            visible_rows(0.0, 22.0, clip(0.0, 44.0)),
            (0, 1),
            "only layer 0 is below the header"
        );
        assert_eq!(visible_rows(0.0, 22.0, clip(0.0, 45.0)), (0, 2));
        assert_eq!(visible_rows(0.0, 22.0, clip(100.0, 140.0)), (3, 3));
        // The track starts below the top of the scroll content.
        assert_eq!(visible_rows(50.0, 22.0, clip(0.0, 80.0)), (0, 1));
        for bad in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
            assert_eq!(visible_rows(0.0, bad, clip(0.0, 80.0)).1, 0, "row {bad}");
        }
        assert_eq!(visible_rows(f32::NAN, 22.0, clip(0.0, 80.0)), (0, 0));
        assert_eq!(visible_rows(0.0, 22.0, clip(f32::NAN, f32::NAN)), (0, 0));
    }

    #[test]
    fn the_texture_covers_what_is_on_screen_at_screen_resolution() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.set_stage_size(2000, 1000);
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.1); // the second pass knows the panel
        let fitted = ws.texture.as_ref().expect("texture").view;
        assert_eq!(
            fitted.region.size(),
            Vec2::new(2000.0, 1000.0),
            "all of it is visible"
        );
        assert!(
            fitted.size[0] < 1400 && fitted.size[1] < 800,
            "the texture is about the size of the window, not of the 2000x1000 stage: {:?}",
            fitted.size
        );

        // Zoomed in 8x about the center, only an eighth of the stage is visible, and the texture
        // is no bigger than before: the pixels go into what can be seen.
        for _ in 0..3 {
            ws.perform(MenuAction::ZoomIn);
            ws.perform(MenuAction::ZoomIn);
            ws.perform(MenuAction::ZoomIn);
        }
        run(&mut ws, &ctx, 0.2);
        run(&mut ws, &ctx, 0.3);
        let zoomed = ws.texture.as_ref().expect("texture").view;
        assert!(
            zoomed.region.width() < fitted.region.width() / 4.0,
            "{zoomed:?}"
        );
        assert!(
            zoomed.size[0] <= 1400 && zoomed.size[1] <= 800,
            "{:?}",
            zoomed.size
        );
        let density = |v: &TextureView| v.size[0] as f32 / v.region.width();
        assert!(
            density(&zoomed) > density(&fitted) * 4.0,
            "and it is sharper"
        );
    }

    #[test]
    fn a_stage_too_big_to_render_whole_still_shows_the_visible_part() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.set_stage_size(16_000, 16_000);
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.1);
        let texture = ws.texture.as_ref().expect("a texture, not a render error");
        assert!(
            texture.view.size[0] <= texture_side_limit(&ctx)
                && texture.view.size[1] <= texture_side_limit(&ctx)
        );
        assert!(
            ws.status().is_none_or(|m| !m.contains("Cannot render")),
            "{:?}",
            ws.status()
        );
    }

    #[test]
    fn panning_changes_the_texture_view_but_not_the_document() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.set_stage_size(2000, 1000);
        run(&mut ws, &ctx, 0.0); // lays out the panel, which zooming aims at
        for _ in 0..4 {
            ws.perform(MenuAction::ZoomIn);
        }
        run(&mut ws, &ctx, 0.1);
        run(&mut ws, &ctx, 0.15);
        let before = ws.texture.as_ref().expect("texture").view;
        ws.stage_view.pan_by(Vec2::new(-150.0, 0.0));
        run(&mut ws, &ctx, 0.2);
        let after = ws.texture.as_ref().expect("texture").view;
        assert!(
            after.region.min.x > before.region.min.x,
            "{before:?} then {after:?}"
        );
        assert_eq!(after.size, before.size);
    }

    /// A shape held for 24 frames, plus a second layer that appears only at frame 12.
    fn held_then_changing() -> Workspace {
        let mut ws = workspace();
        ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(60.0, 60.0));
        let layer = ws.current_layer.expect("layer");
        ws.set_frame(12);
        ws.current_layer = Some(layer);
        ws.frame_edit(FrameOp::InsertKeyframe);
        ws.set_frame(0);
        ws
    }

    #[test]
    fn frames_that_look_the_same_are_rendered_once() {
        let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.01); // the panel is known now, so the view settles
        let settled = ws.frames_rendered;
        // Frames 1..12 show exactly the picture of frame 0: no further rendering.
        for frame in 1..12 {
            ws.set_frame(frame);
            run(&mut ws, &ctx, 0.1 + f64::from(frame) * 0.01);
        }
        assert_eq!(
            ws.frames_rendered, settled,
            "a held exposure is rendered once"
        );
        assert_eq!(
            ws.texture.as_ref().map(|t| t.key.2),
            Some(11),
            "but the right frame is current"
        );
    }

    #[test]
    fn a_looping_animation_costs_nothing_after_the_first_pass() {
        let (ctx, mut ws) = (egui::Context::default(), workspace());
        ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
        ws.set_tool(Tool::Select);
        let layer = ws.current_layer.expect("layer");
        for frame in [8, 16] {
            ws.set_frame(frame);
            ws.current_layer = Some(layer);
            ws.frame_edit(FrameOp::InsertKeyframe);
        }
        ws.set_frame(0);
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.01);
        let first = ws.frames_rendered;
        for frame in 0..24 {
            ws.set_frame(frame);
            run(&mut ws, &ctx, 0.2 + f64::from(frame) * 0.01);
        }
        let after_one_pass = ws.frames_rendered;
        for frame in 0..24 {
            ws.set_frame(frame);
            run(&mut ws, &ctx, 1.0 + f64::from(frame) * 0.01);
        }
        assert_eq!(
            ws.frames_rendered, after_one_pass,
            "the second loop renders nothing"
        );
        assert!(after_one_pass >= first);
    }

    #[test]
    fn editing_invalidates_cached_pictures_and_the_cache_stays_within_its_limits() {
        let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
        run(&mut ws, &ctx, 0.0);
        run(&mut ws, &ctx, 0.01);
        let before = ws.frames_rendered;
        ws.select_at(Pos2::new(30.0, 30.0), false);
        ws.move_selection(Vec2::new(20.0, 0.0));
        run(&mut ws, &ctx, 0.1);
        assert!(
            ws.frames_rendered > before,
            "the edit shows up, not a stale picture"
        );
        assert!(
            ws.frame_cache
                .iter()
                .all(|c| c.revision == ws.active().revision()),
            "pictures of older document states are dropped"
        );
        // The limits hold however many different pictures come through.
        for _ in 0..(FRAME_CACHE_ENTRIES + 10) {
            ws.frame_cache.push(CachedFrame {
                context: ws.context_serial,
                revision: ws.active().revision(),
                signature: ws.frame_cache.len() as u64 + 1_000,
                view: ws.texture.as_ref().expect("texture").view,
                handle: ws.texture.as_ref().expect("texture").handle.clone(),
            });
        }
        ws.trim_frame_cache();
        assert!(ws.frame_cache.len() <= FRAME_CACHE_ENTRIES);
        assert!(!ws.frame_cache.is_empty());
    }
}
