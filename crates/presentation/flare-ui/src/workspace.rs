use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_document::{
    Content, Document, FrameSpan, Layer, MAX_STAGE_SIDE, Scene, Shape, Stage,
    Stroke as ShapeStroke, Timeline, Tween,
};
use flare_edit::{Command, EditorSession, LayerChange, frames};
use glamx::DVec2;

use crate::ops::{self, SpanRef};
use crate::view::{self, StageView};
use crate::{MENU_LAYOUT, MenuAction, MenuEntry};

/// A file dialog the shell should show on the workspace's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogRequest {
    Open,
    SaveAs,
}

struct StageTexture {
    handle: egui::TextureHandle,
    /// (context, session revision, frame) the texture was rendered for.
    key: (u64, u64, u32),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    Select,
    Rectangle,
    Ellipse,
    Line,
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragMode {
    Draw,
    Move,
}

/// Gesture in progress on the stage, in document coordinates. It is tool state only:
/// the document changes when the gesture is released.
#[derive(Clone, Copy)]
struct Drag {
    mode: DragMode,
    start: Pos2,
    current: Pos2,
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
    drag: Option<Drag>,
    /// Set by Escape so the release that follows a cancelled drag commits nothing.
    drag_cancelled: bool,
    tool: Tool,
    selected: Option<SpanRef>,
    stage_view: StageView,
    /// The stage panel's last rectangle, so menu zoom can aim at its center.
    stage_panel: Rect,
    selected_symbol: Option<SymbolId>,
    current_layer: Option<LayerId>,
    layer_name_edit: String,
    /// A native menu bar owns the menu and its shortcuts, so the in-window ones step aside.
    native_menu: bool,
    quit_requested: bool,
    fill: Color32,
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
            drag: None,
            drag_cancelled: false,
            tool: Tool::Rectangle,
            selected: None,
            stage_view: StageView::default(),
            stage_panel: Rect::NOTHING,
            selected_symbol: None,
            current_layer: None,
            layer_name_edit: String::new(),
            native_menu: false,
            quit_requested: false,
            fill: Color32::from_rgb(232, 69, 60),
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

    /// Opens a project, refusing to replace unsaved work.
    /// Replaces the document being edited, keeping the user's tool and environment settings
    /// (tool, colors, native menu) rather than resetting them along with the document.
    fn install_document(&mut self, document: Document, path: Option<PathBuf>) {
        let mut fresh = Self::new(document, path);
        fresh.tool = self.tool;
        fresh.fill = self.fill;
        fresh.stroke_enabled = self.stroke_enabled;
        fresh.stroke_color = self.stroke_color;
        fresh.stroke_width = self.stroke_width;
        fresh.native_menu = self.native_menu;
        *self = fresh;
    }

    pub fn open(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before opening another project");
            return;
        }
        match flare_storage::load(path) {
            Ok(document) => self.install_document(document, Some(path.to_owned())),
            Err(error) => self.report(format!("Cannot open {}: {error}", path.display())),
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
        let Some(scene) = self.document().scenes().first() else {
            self.report("This document has no scene");
            return;
        };
        let scene_id = scene.id();
        let duration = u32::try_from(scene.timeline().frame_count())
            .unwrap_or(u32::MAX)
            .max(24);
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
            Tool::Select => return,
        };
        let Some(mut shape) = shape else {
            self.report("Shape coordinates must be finite");
            return;
        };
        if self.stroke_enabled && tool != Tool::Line {
            match stroke {
                Ok(stroke) => shape = shape.with_stroke(stroke),
                Err(error) => return self.report(error.to_string()),
            }
        }
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
        self.current_layer = None;
        self.frame = 0;
        self.texture = None;
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
            MenuAction::Delete => self.has_selection(),
            MenuAction::ConvertToSymbol => self.can_convert_selection(),
            MenuAction::EditSymbol => self.can_edit_selected_symbol(),
            MenuAction::ExitSymbol => self.in_symbol(),
            MenuAction::ToolSelect
            | MenuAction::ToolRectangle
            | MenuAction::ToolEllipse
            | MenuAction::ToolLine
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
            MenuAction::ConvertToSymbol => "Convert to Symbol",
            MenuAction::EditSymbol => "Edit Symbol",
            MenuAction::ExitSymbol => "Exit Symbol",
            MenuAction::ToolSelect => "Select / Move",
            MenuAction::ToolRectangle => "Rectangle",
            MenuAction::ToolEllipse => "Ellipse",
            MenuAction::ToolLine => "Line",
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
            MenuAction::ConvertToSymbol => self.convert_selection_to_symbol(),
            MenuAction::EditSymbol => self.edit_selected_symbol(),
            MenuAction::ExitSymbol => {
                self.exit_symbol();
            }
            MenuAction::ToolSelect => self.set_tool(Tool::Select),
            MenuAction::ToolRectangle => self.set_tool(Tool::Rectangle),
            MenuAction::ToolEllipse => self.set_tool(Tool::Ellipse),
            MenuAction::ToolLine => self.set_tool(Tool::Line),
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

    // ---- tools and selection ---------------------------------------------------------

    pub fn tool(&self) -> Tool {
        self.tool
    }

    pub fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.cancel_drag();
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
        if let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) {
            let editable = ops::find_span(self.document(), scene, selected).is_some()
                && self
                    .layer(selected.layer)
                    .is_some_and(|l| l.is_visible() && !l.is_locked());
            if !editable {
                self.selected = None;
            }
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

    fn begin_drag(&mut self, point: Pos2) {
        self.drag_cancelled = false;
        let mode = match self.tool {
            Tool::Rectangle | Tool::Ellipse | Tool::Line => Some(DragMode::Draw),
            Tool::Select => {
                self.selected = self.hit(point);
                self.selected.map(|_| DragMode::Move)
            }
        };
        self.drag = mode.map(|mode| Drag {
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
            }
        }
    }

    /// Escape, tool switches and focus loss cancel a gesture without committing it.
    fn cancel_drag(&mut self) {
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
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        let delta = DVec2::new(f64::from(delta.x), f64::from(delta.y));
        if let Some(commands) = ops::translate(self.document(), scene, selected, delta) {
            self.run_batch(commands, "move");
        }
    }

    pub fn delete_selection(&mut self) {
        let (Some(selected), Some(scene)) = (self.selected, self.scene_id()) else {
            return;
        };
        self.run_batch(ops::delete(scene, selected), "delete");
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

    fn ensure_stage_texture(&mut self, ctx: &egui::Context) {
        let key = (self.context_serial, self.active().revision(), self.frame);
        if self.texture.as_ref().is_some_and(|t| t.key == key) {
            return;
        }
        let Some(scene) = self.document().scenes().first().map(|s| s.id()) else {
            self.texture = None;
            return;
        };
        let stage = self.document().stage();
        let rendered = flare_eval::evaluate_frame(self.document(), scene, self.frame)
            .map_err(|e| e.to_string())
            .and_then(|frame| {
                flare_render::render(&frame, stage.width(), stage.height(), stage.background())
                    .map_err(|e| e.to_string())
            });
        match rendered {
            Ok(image) => {
                let size = [image.width() as usize, image.height() as usize];
                let color = egui::ColorImage::from_rgba_unmultiplied(size, image.rgba());
                match &mut self.texture {
                    Some(texture) => {
                        texture.handle.set(color, egui::TextureOptions::LINEAR);
                        texture.key = key;
                    }
                    None => {
                        let handle = ctx.load_texture("stage", color, egui::TextureOptions::LINEAR);
                        self.texture = Some(StageTexture { handle, key });
                    }
                }
            }
            Err(message) => self.report(format!("Cannot render frame: {message}")),
        }
    }

    // ---- UI --------------------------------------------------------------------------

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        self.advance(ui.input(|i| i.time));
        self.handle_shortcuts(&ctx);
        self.validate_selection();
        self.ensure_stage_texture(&ctx);

        egui::Panel::top("menu").show(ui, |ui| self.menu_bar(ui));
        egui::Panel::bottom("timeline")
            .resizable(true)
            .default_size(180.0)
            .show(ui, |ui| self.timeline(ui));
        egui::Panel::left("tools")
            .default_size(150.0)
            .show(ui, |ui| self.tools(ui));
        egui::Panel::right("library")
            .default_size(180.0)
            .show(ui, |ui| self.library(ui));
        egui::CentralPanel::default().show(ui, |ui| self.stage(ui));

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
        }
        let typing = ctx.egui_wants_keyboard_input();
        if !typing {
            if toggle {
                self.playing = !self.playing;
            }
            if delete {
                self.delete_selection();
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
        self.set_tool(tool);
        ui.weak("Drag on the stage. Esc cancels. Delete removes, F8 converts to a symbol.");
        ui.separator();
        ui.label("Fill");
        ui.color_edit_button_srgba(&mut self.fill);
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
            painter.image(
                texture.handle.id(),
                image_rect,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
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
        let to_doc = |p: Pos2| screen_to_stage(p, image_rect, extent);
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
        let primary = egui::PointerButton::Primary;
        if response.drag_started_by(primary)
            && let Some(p) = response.interact_pointer_pos()
        {
            self.begin_drag(to_doc(p));
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
            self.selected = self.hit(to_doc(p));
        }

        // Double-click an instance to edit its symbol; double-click empty stage to go back.
        if response.double_clicked()
            && self.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            self.selected = self.hit(to_doc(p));
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

        let outline = Stroke::new(1.5, Color32::from_rgb(40, 120, 255));
        let moving = self
            .drag
            .filter(|d| d.mode == DragMode::Move)
            .map_or(Vec2::ZERO, |d| d.current - d.start);
        if let (Some(selected), Some(scene)) = (self.selected, self.scene_id())
            && let Some(bounds) = ops::span_bounds(self.document(), scene, selected)
        {
            let shown = bounds.translate(moving);
            let rect = Rect::from_min_max(to_screen(shown.min), to_screen(shown.max));
            painter.rect_stroke(rect, 0.0, outline, StrokeKind::Outside);
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

        let frames = self.scene_frames();
        let layer_count = self
            .document()
            .scenes()
            .first()
            .map_or(0, |s| s.timeline().layers().len());
        let row = 22.0;
        let label_w = 110.0;
        let height = row * (layer_count as f32 + 1.0);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), height),
            Sense::click_and_drag(),
        );
        let painter = ui.painter_at(rect);
        let track = Rect::from_min_max(Pos2::new(rect.min.x + label_w, rect.min.y), rect.max);
        let per_frame = track.width() / frames as f32;

        if (response.clicked() || response.dragged())
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
            for (i, layer) in scene.timeline().layers().iter().enumerate() {
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
            }
        }
        let head = track.min.x + (self.frame as f32 + 0.5) * per_frame;
        painter.line_segment(
            [Pos2::new(head, rect.min.y), Pos2::new(head, rect.max.y)],
            Stroke::new(2.0, Color32::from_rgb(220, 60, 60)),
        );
    }
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

/// Screen position to stage (document) coordinates, clamped to the stage.
/// The scale at which the whole stage just fits `panel`.
fn fit_scale(panel: Rect, extent: Vec2) -> f32 {
    StageView::default().scale(panel, extent)
}

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
            mode: DragMode::Draw,
            start: Pos2::new(1.0, 1.0),
            current: Pos2::new(50.0, 50.0),
        });
        let _ = ctx.run_ui(input(0.1, vec![key(Key::Escape, Modifiers::NONE)]), |ui| {
            ws.show(ui)
        });
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
    fn opening_a_project_keeps_tool_colors_and_the_native_menu_setting() {
        let dir = tempfile::tempdir().expect("dir");
        let project = dir.path().join("p.flare");
        let mut source = workspace();
        source.save_as(&project);

        let mut ws = workspace();
        ws.set_native_menu(true);
        ws.set_tool(Tool::Ellipse);
        ws.fill = Color32::from_rgb(1, 2, 3);
        ws.stroke_enabled = true;
        ws.open(&project);
        assert_eq!(ws.path(), Some(project.as_path()));
        assert!(
            ws.native_menu,
            "opening must not turn the in-window menu back on"
        );
        assert_eq!(ws.tool(), Tool::Ellipse);
        assert_eq!(ws.fill, Color32::from_rgb(1, 2, 3));
        assert!(ws.stroke_enabled);
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
}
