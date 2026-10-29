use std::num::NonZeroU32;
use std::path::{Path, PathBuf};

use egui::{Align2, Color32, FontId, Key, Modifiers, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use flare_core::{DocumentId, FrameRate, LayerId, Rgba8, SceneId, ShapeId};
use flare_document::{Content, Document, FrameSpan, Layer, Scene, Shape, Timeline};
use flare_edit::{Command, EditorSession};

/// Fixed stage size in document units until documents carry their own stage dimensions.
pub const STAGE_SIZE: (u32, u32) = (800, 450);

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

/// Rectangle being dragged on the stage, in document coordinates.
#[derive(Clone, Copy)]
struct Drag {
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
    fill: Color32,
    status: Option<String>,
    dialog: Option<DialogRequest>,
}

fn stage_extent() -> Vec2 {
    Vec2::new(STAGE_SIZE.0 as f32, STAGE_SIZE.1 as f32)
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
            fill: Color32::from_rgb(232, 69, 60),
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
        }
    }

    pub fn redo(&mut self) {
        if self.session.redo() {
            self.set_frame(self.frame);
        }
    }

    /// Commits a rectangle as one undo step: a new layer plus its shape span.
    pub fn commit_rectangle(&mut self, a: Pos2, b: Pos2) {
        let (x0, x1) = (a.x.min(b.x), a.x.max(b.x));
        let (y0, y1) = (a.y.min(b.y), a.y.max(b.y));
        let (w, h) = (f64::from(x1 - x0), f64::from(y1 - y0));
        if w < 1.0 || h < 1.0 {
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
        let color = self.fill.to_array();
        let fill = Rgba8::new(color[0], color[1], color[2], color[3]);
        let Some(shape) = Shape::rect(ShapeId::new(), f64::from(x0), f64::from(y0), w, h, fill)
        else {
            self.report("Rectangle coordinates must be finite");
            return;
        };
        let span = FrameSpan::new(
            0,
            NonZeroU32::new(duration).unwrap_or(NonZeroU32::MIN),
            Content::Shape(shape),
        );
        let layer_id = LayerId::new();
        let layer = match Layer::new(layer_id, "Rectangle", vec![]) {
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
            Ok(()) => self.status = None,
            Err(error) => self.report(format!("Cannot draw: {error}")),
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
        let rendered = flare_eval::evaluate_frame(self.document(), scene, self.frame)
            .map_err(|e| e.to_string())
            .and_then(|frame| {
                flare_render::render(&frame, STAGE_SIZE.0, STAGE_SIZE.1, Rgba8::WHITE)
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
        let (redo, undo, save, toggle, left, right, escape) = ctx.input_mut(|i| {
            // Redo must be consumed first: egui lets Cmd+Shift+Z also match plain Cmd+Z.
            (
                i.consume_key(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::Z),
                i.consume_key(Modifiers::COMMAND, Key::S),
                !i.modifiers.any() && i.key_pressed(Key::Space),
                !i.modifiers.any() && i.key_pressed(Key::ArrowLeft),
                !i.modifiers.any() && i.key_pressed(Key::ArrowRight),
                i.key_pressed(Key::Escape),
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
            if left {
                self.set_frame(self.frame.saturating_sub(1));
            }
            if right {
                self.set_frame(self.frame.saturating_add(1));
            }
        }
        if escape && self.drag.take().is_some() {
            self.drag_cancelled = true;
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
        ui.label("Rectangle: drag on the stage. Esc cancels.");
        ui.separator();
        ui.label("Fill");
        ui.color_edit_button_srgba(&mut self.fill);
    }

    fn library(&mut self, ui: &mut egui::Ui) {
        ui.heading("Library");
        let symbols = self.document().symbols();
        if symbols.is_empty() {
            ui.weak("No symbols");
        }
        for symbol in symbols {
            ui.label(symbol.name());
        }
    }

    fn stage(&mut self, ui: &mut egui::Ui) {
        let available = ui.available_size();
        let (response, painter) = ui.allocate_painter(available, Sense::click_and_drag());
        let extent = stage_extent();
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

        let to_doc = |p: Pos2| screen_to_stage(p, image_rect);
        if response.drag_started() {
            self.drag_cancelled = false;
            if let Some(p) = response.interact_pointer_pos() {
                self.drag = Some(Drag {
                    start: to_doc(p),
                    current: to_doc(p),
                });
            }
        }
        if response.dragged()
            && let (Some(drag), Some(p)) = (&mut self.drag, response.interact_pointer_pos())
        {
            drag.current = to_doc(p);
        }
        if response.drag_stopped() {
            if let Some(drag) = self.drag.take()
                && !self.drag_cancelled
            {
                self.commit_rectangle(drag.start, drag.current);
            }
            self.drag_cancelled = false;
        }
        if let Some(drag) = self.drag {
            let back = |p: Pos2| image_rect.min + p.to_vec2() * scale;
            let preview = Rect::from_two_pos(back(drag.start), back(drag.current));
            painter.rect_filled(preview, 0.0, self.fill.gamma_multiply(0.5));
            painter.rect_stroke(
                preview,
                0.0,
                Stroke::new(1.0, Color32::BLACK),
                StrokeKind::Outside,
            );
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
            && p.x >= track.min.x
        {
            let f = ((p.x - track.min.x) / per_frame).floor().max(0.0) as u32;
            self.set_frame(f);
        }

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

/// Screen position to stage (document) coordinates, clamped to the stage.
fn screen_to_stage(p: Pos2, image_rect: Rect) -> Pos2 {
    let u = (p.x - image_rect.min.x) / image_rect.width().max(f32::EPSILON);
    let v = (p.y - image_rect.min.y) / image_rect.height().max(f32::EPSILON);
    let extent = stage_extent();
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
        assert_eq!(screen_to_stage(Pos2::new(100.0, 50.0), rect), Pos2::ZERO);
        assert_eq!(
            screen_to_stage(Pos2::new(300.0, 162.5), rect),
            Pos2::new(400.0, 225.0)
        );
        assert_eq!(
            screen_to_stage(Pos2::new(-999.0, 9999.0), rect),
            Pos2::new(0.0, 450.0)
        );
    }
}
