use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId, SymbolId};
use flare_document::{
    Content, Document, FrameSpan, Layer, MAX_STAGE_SIDE, Scene, Shape, Stage,
    Stroke as ShapeStroke, Timeline,
};
use flare_edit::{Command, EditorSession, frames};
use glamx::DVec2;

use crate::ops::{self, SpanRef};

/// A file dialog the shell should show on the workspace's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogRequest {
    Open,
    SaveAs,
}

struct StageTexture {
    handle: egui::TextureHandle,
    /// (session revision, frame) the texture was rendered for.
    key: (u64, u32),
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

pub struct Workspace {
    session: EditorSession,
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
    selected_symbol: Option<SymbolId>,
    current_layer: Option<LayerId>,
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
            selected_symbol: None,
            current_layer: None,
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

    pub fn document(&self) -> &Document {
        self.session.document()
    }

    pub fn frame(&self) -> u32 {
        self.frame
    }

    pub fn is_playing(&self) -> bool {
        self.playing
    }

    pub fn is_dirty(&self) -> bool {
        self.session.is_dirty()
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
    pub fn open(&mut self, path: &Path) {
        if self.is_dirty() {
            self.report("Save or undo your changes before opening another project");
            return;
        }
        match flare_storage::load(path) {
            Ok(document) => {
                *self = Self::new(document, Some(path.to_owned()));
            }
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
        if self.session.undo() {
            self.set_frame(self.frame);
            self.validate_selection();
        }
    }

    pub fn redo(&mut self) {
        if self.session.redo() {
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
        match self.session.apply(batch) {
            Ok(()) => {
                self.status = None;
                self.current_layer = Some(layer_id);
            }
            Err(error) => self.report(format!("Cannot draw: {error}")),
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

    /// Runs a frame operation on the active layer at the current frame.
    pub fn frame_edit(&mut self, op: FrameOp) {
        let (Some(scene), Some(layer)) = (self.scene_id(), self.active_layer()) else {
            return;
        };
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
        if let (Some(selected), Some(scene)) = (self.selected, self.scene_id())
            && ops::find_span(self.document(), scene, selected).is_none()
        {
            self.selected = None;
        }
        if let Some(symbol) = self.selected_symbol
            && self.document().symbol(symbol).is_none()
        {
            self.selected_symbol = None;
        }
    }

    /// The scene-level span of the topmost shape under `point` at the current frame.
    fn hit(&self, point: Pos2) -> Option<SpanRef> {
        let scene = self.scene_id()?;
        let frame = flare_eval::evaluate_frame(self.document(), scene, self.frame).ok()?;
        let at = DVec2::new(f64::from(point.x), f64::from(point.y));
        let drawn = frame.shapes.get(flare_render::hit_test(&frame, at)?)?;
        ops::locate_hit(self.document(), scene, self.frame, drawn)
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
        match self.session.apply(commands) {
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
        let key = (self.session.revision(), self.frame);
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
        if redo {
            self.redo();
        } else if undo {
            self.undo();
        }
        if save {
            self.save();
        }
        let typing = ctx.egui_wants_keyboard_input();
        if !typing {
            if toggle {
                self.playing = !self.playing;
            }
            if delete {
                self.delete_selection();
            }
            if convert {
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
        if !typing {
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

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open...").clicked() {
                    self.dialog = Some(DialogRequest::Open);
                    ui.close();
                }
                if ui.button("Save").clicked() {
                    self.save();
                    ui.close();
                }
                if ui.button("Save As...").clicked() {
                    self.dialog = Some(DialogRequest::SaveAs);
                    ui.close();
                }
            });
            ui.menu_button("Edit", |ui| {
                if ui
                    .add_enabled(self.session.can_undo(), egui::Button::new("Undo"))
                    .clicked()
                {
                    self.undo();
                    ui.close();
                }
                if ui
                    .add_enabled(self.session.can_redo(), egui::Button::new("Redo"))
                    .clicked()
                {
                    self.redo();
                    ui.close();
                }
                ui.separator();
                if ui
                    .add_enabled(self.has_selection(), egui::Button::new("Delete"))
                    .clicked()
                {
                    self.delete_selection();
                    ui.close();
                }
                if ui
                    .add_enabled(
                        self.can_convert_selection(),
                        egui::Button::new("Convert to Symbol (F8)"),
                    )
                    .clicked()
                {
                    self.convert_selection_to_symbol();
                    ui.close();
                }
            });
            ui.menu_button("Timeline", |ui| {
                let items = [
                    (FrameOp::InsertFrame, "Insert Frame (F5)"),
                    (FrameOp::RemoveFrame, "Remove Frame (Shift+F5)"),
                    (FrameOp::InsertKeyframe, "Insert Keyframe (F6)"),
                    (FrameOp::InsertBlankKeyframe, "Insert Blank Keyframe (F7)"),
                    (FrameOp::ClearKeyframe, "Clear Keyframe (Shift+F6)"),
                ];
                for (op, label) in items {
                    if ui.button(label).clicked() {
                        self.frame_edit(op);
                        ui.close();
                    }
                }
            });
            ui.separator();
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
        let scale = (response.rect.width() / extent.x).min(response.rect.height() / extent.y);
        let image_rect = Rect::from_center_size(response.rect.center(), extent * scale.max(0.01));
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

        let to_doc = |p: Pos2| screen_to_stage(p, image_rect, extent);
        let to_screen = |p: Pos2| image_rect.min + p.to_vec2() * scale;
        if response.drag_started()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.begin_drag(to_doc(p));
        }
        if response.dragged()
            && let Some(p) = response.interact_pointer_pos()
        {
            self.update_drag(to_doc(p));
        }
        if response.drag_stopped() {
            self.end_drag();
        }
        // A plain click with the select tool also selects (or clears the selection).
        if response.clicked()
            && self.tool == Tool::Select
            && let Some(p) = response.interact_pointer_pos()
        {
            self.selected = self.hit(to_doc(p));
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
                self.select_layer(id);
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
                painter.text(
                    Pos2::new(rect.min.x + 4.0, y + 3.0),
                    Align2::LEFT_TOP,
                    layer.name(),
                    FontId::proportional(12.0),
                    ink,
                );
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
        assert_eq!(ws.texture.as_ref().map(|t| t.key), Some((0, 0)));
        rect(&mut ws);
        run(&mut ws, &ctx, 0.1);
        let after_edit = ws.texture.as_ref().expect("texture").key;
        assert_eq!(after_edit.1, 0);
        assert!(after_edit.0 > 0);
        ws.set_frame(5);
        run(&mut ws, &ctx, 0.2);
        assert_eq!(ws.texture.as_ref().map(|t| t.key.1), Some(5));
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
}
