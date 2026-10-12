use crate::{
    commands::Command,
    document,
    i18n::{Locale, Localizer},
    theme::{self, Icon},
    widgets,
};
use ac_core::{pt, Color, Editor, Element, Matrix, Path, Shape, SymbolKind, Tween};
use eframe::egui::{self, pos2, vec2, Color32, FontId, Rect, RichText, Sense, Stroke};
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, time::Instant};

#[path = "chrome.rs"]
mod chrome;
#[path = "clipboard.rs"]
mod clipboard;
#[path = "frame_sets.rs"]
mod frame_sets;
#[path = "layer_ops.rs"]
mod layer_ops;
#[cfg(test)]
#[path = "layer_tests.rs"]
mod layer_tests;
#[path = "layer_types.rs"]
mod layer_types;
#[cfg(test)]
#[path = "p1_completion_tests.rs"]
mod p1_completion_tests;
#[cfg(test)]
#[path = "p1_tests.rs"]
mod p1_tests;
#[path = "thumbnails.rs"]
mod thumbnails;
#[path = "window_chrome.rs"]
mod window_chrome;
#[path = "workspace.rs"]
mod workspace;
pub use thumbnails::FrameDisplay;
#[path = "timeline.rs"]
mod timeline;
#[path = "timeline_ops.rs"]
mod timeline_ops;
#[cfg(test)]
#[path = "timeline_tests.rs"]
mod timeline_tests;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Tool {
    #[default]
    Select,
    Rectangle,
    Ellipse,
    Hand,
    Zoom,
}
impl Tool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "tool-select",
            Self::Rectangle => "tool-rectangle",
            Self::Ellipse => "tool-ellipse",
            Self::Hand => "tool-hand",
            Self::Zoom => "tool-zoom",
        }
    }
    fn icon(self) -> Icon {
        match self {
            Self::Select => Icon::Select,
            Self::Rectangle => Icon::Rectangle,
            Self::Ellipse => Icon::Ellipse,
            Self::Hand => Icon::Hand,
            Self::Zoom => Icon::Zoom,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Preferences {
    pub theme: theme::Theme,
    pub locale: Locale,
    pub inspector: bool,
    pub timeline: bool,
    pub rulers: bool,
    pub options_bar: bool,
    pub status_bar: bool,
    pub tools_collapsed: bool,
    pub inspector_left: Option<bool>,
    pub tools_left: Option<bool>,
    pub timeline_top: bool,
    pub frame_width: f32,
    pub span_selection: bool,
    pub frame_display: FrameDisplay,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            theme: theme::Theme::Dark,
            locale: Locale::detect(&sys_locale::get_locale().unwrap_or_default()),
            inspector: true,
            timeline: true,
            rulers: false,
            options_bar: false,
            status_bar: false,
            tools_collapsed: false,
            inspector_left: None,
            tools_left: None,
            timeline_top: false,
            frame_width: 8.0,
            span_selection: false,
            frame_display: FrameDisplay::Cells,
        }
    }
}

#[derive(Clone, Copy)]
enum Pending {
    New,
    Demo,
    Open,
    Quit,
}
struct Drag {
    start: egui::Pos2,
    last: egui::Vec2,
    pan: egui::Vec2,
    tool: Tool,
    moved: bool,
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum ScrollAxis {
    Horizontal,
    Vertical,
}
impl ScrollAxis {
    fn coord(self, p: egui::Pos2) -> f32 {
        match self {
            Self::Horizontal => p.x,
            Self::Vertical => p.y,
        }
    }
    fn set_pan(self, pan: &mut egui::Vec2, value: f32) {
        match self {
            Self::Horizontal => pan.x = value,
            Self::Vertical => pan.y = value,
        }
    }
}
#[derive(Clone, Copy)]
struct ScrollDrag {
    axis: ScrollAxis,
    track_start: f32,
    thumb_len: f32,
    travel: f32,
    max_pan: f32,
    grab_offset: f32,
}

pub struct Studio {
    pub editor: Editor,
    pub prefs: Preferences,
    pub i18n: Localizer,
    pub tool: Tool,
    pub playing: bool,
    path: Option<PathBuf>,
    window_title: String,
    example_name: bool,
    saved_revision: u64,
    session: u64,
    zoom: f32,
    pan: egui::Vec2,
    fit: bool,
    texture: Option<egui::TextureHandle>,
    cache: (u64, u32, Option<u32>),
    drag: Option<Drag>,
    scroll_drag: Option<ScrollDrag>,
    fill: Color32,
    last_time: Instant,
    frame_fraction: f32,
    status: String,
    error: bool,
    preferences_open: bool,
    symbol_open: bool,
    symbol_name: String,
    pending: Option<Pending>,
    allow_close: bool,
    dock_drag: Option<workspace::DockModule>,
    props_tab: bool,
    output_tab: bool,
    new_open: bool,
    new_fields: [f32; 3],
    new_background: Color32,
    about_open: bool,
    command_open: bool,
    command_query: String,
    command_focus: bool,
    library_filter: String,
    doc_fields: [f32; 3],
    transform_fields: [f32; 3],
    layer_name: String,
    form_selection: Vec<ac_core::Selection>,
    form_layer: usize,
    timeline_state: timeline_ops::TimelineState,
    layer_state: layer_ops::LayerState,
}

impl Studio {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let prefs = cc
            .storage
            .and_then(|s| eframe::get_value(s, "flare.studio.preferences.v1"))
            .unwrap_or_default();
        Self::with_preferences(&cc.egui_ctx, prefs)
    }
    pub fn with_preferences(ctx: &egui::Context, prefs: Preferences) -> Self {
        theme::install_fonts(ctx);
        egui_extras::install_image_loaders(ctx);
        theme::apply(ctx, prefs.theme);
        let i18n = Localizer::new(prefs.locale);
        let doc = document::blank(
            &i18n.text("default-scene"),
            &i18n.format("default-layer", &[("number", "1")]),
        );
        let mut studio = Self {
            editor: Editor::new(doc),
            prefs,
            i18n,
            tool: Tool::Select,
            playing: false,
            path: None,
            window_title: String::new(),
            example_name: false,
            saved_revision: 0,
            session: 0,
            zoom: 1.0,
            pan: egui::Vec2::ZERO,
            fit: false,
            texture: None,
            cache: (u64::MAX, 0, None),
            drag: None,
            scroll_drag: None,
            fill: Color32::from_rgb(0, 102, 204),
            last_time: Instant::now(),
            frame_fraction: 0.0,
            status: String::new(),
            error: false,
            preferences_open: false,
            symbol_open: false,
            symbol_name: String::new(),
            pending: None,
            allow_close: false,
            dock_drag: None,
            props_tab: true,
            output_tab: false,
            new_open: false,
            new_fields: [550.0, 400.0, 24.0],
            new_background: Color32::WHITE,
            about_open: false,
            command_open: false,
            command_query: String::new(),
            command_focus: false,
            library_filter: String::new(),
            doc_fields: [960.0, 540.0, 24.0],
            transform_fields: [0.0, 0.0, 1.0],
            layer_name: String::new(),
            form_selection: Vec::new(),
            form_layer: usize::MAX,
            timeline_state: Default::default(),
            layer_state: Default::default(),
        };
        studio.sync_forms();
        studio
    }
    pub fn set_theme(&mut self, ctx: &egui::Context, theme: theme::Theme) {
        self.prefs.theme = theme;
        theme::apply(ctx, theme);
    }
    pub fn dirty(&self) -> bool {
        self.editor.revision != self.saved_revision
    }
    pub fn set_locale(&mut self, locale: Locale) {
        self.prefs.locale = locale;
        self.i18n = Localizer::new(locale);
        // Do not alter document content or command identities when changing language.
        self.status.clear();
    }
    fn reset_layout(&mut self, ctx: &egui::Context) {
        self.prefs.inspector = true;
        self.prefs.timeline = true;
        self.prefs.tools_collapsed = false;
        self.prefs.inspector_left = None;
        self.prefs.tools_left = None;
        self.prefs.timeline_top = false;
        self.dock_drag = None;
        self.fit = true;
        self.pan = egui::Vec2::ZERO;
        ctx.data_mut(|d| {
            for id in [
                "studio.timeline",
                "studio.inspector.ltr",
                "studio.inspector.rtl",
            ] {
                d.remove::<egui::PanelState>(egui::Id::new(id));
            }
        });
    }
    fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| {
                self.i18n.text(if self.example_name {
                    "document-example"
                } else {
                    "document-untitled"
                })
            })
    }
    fn sync_forms(&mut self) {
        self.doc_fields = [
            self.editor.doc.width,
            self.editor.doc.height,
            self.editor.doc.fps,
        ];
        self.form_selection = self.editor.selection.clone();
        self.form_layer = self.editor.layer;
        self.layer_name = self
            .editor
            .timeline()
            .layers
            .get(self.editor.layer)
            .map(|l| l.name.clone())
            .unwrap_or_default();
        self.transform_fields = self
            .editor
            .selected_element()
            .map(|e| [e.matrix.tx, e.matrix.ty, e.alpha])
            .unwrap_or([0.0, 0.0, 1.0]);
    }
    fn fail(&mut self, reason: &str) {
        let reason = if reason.starts_with("error-") {
            self.i18n.text(reason)
        } else {
            reason.to_owned()
        };
        self.status = self.i18n.format("status-error", &[("reason", &reason)]);
        self.error = true;
        self.output_tab = true;
        self.prefs.timeline = true;
    }
    fn replace(&mut self, doc: ac_core::Document, path: Option<PathBuf>, example: bool) {
        self.session = self.session.wrapping_add(1);
        self.reset_timeline_gesture();
        self.timeline_state.focused = false;
        self.timeline_state.form_identity = None;
        self.editor = Editor::new(doc);
        self.saved_revision = 0;
        self.path = path;
        self.example_name = example;
        self.playing = false;
        self.frame_fraction = 0.0;
        self.texture = None;
        self.pan = egui::Vec2::ZERO;
        self.fit = true;
        self.status.clear();
        self.error = false;
        self.sync_forms();
    }
    fn request(&mut self, action: Pending, ctx: &egui::Context) {
        self.playing = false;
        if self.dirty() {
            self.pending = Some(action);
        } else {
            self.perform(action, ctx);
        }
    }
    fn perform(&mut self, action: Pending, ctx: &egui::Context) {
        match action {
            Pending::New => {
                self.new_open = true;
            }
            Pending::Demo => self.replace(document::example(), None, true),
            Pending::Open => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(self.i18n.text("filter-document"), &["json"])
                    .pick_file()
                {
                    match document::open(&path) {
                        Ok(doc) => {
                            self.replace(doc, Some(path), false);
                            self.status = self
                                .i18n
                                .format("status-opened", &[("name", &self.title())]);
                        }
                        Err(e) => self.fail(&e),
                    }
                }
            }
            Pending::Quit => {
                self.allow_close = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }
    fn save_document(&mut self, save_as: bool) -> bool {
        let path = if save_as || self.path.is_none() {
            rfd::FileDialog::new()
                .add_filter(self.i18n.text("filter-document"), &["json"])
                .set_file_name(format!(
                    "{}.acx.json",
                    self.title().trim_end_matches(".acx.json")
                ))
                .save_file()
        } else {
            self.path.clone()
        };
        let Some(path) = path else { return false };
        match document::save(&self.editor.doc, &path) {
            Ok(()) => {
                self.path = Some(path);
                self.saved_revision = self.editor.revision;
                self.error = false;
                self.status = self.i18n.format("status-saved", &[("name", &self.title())]);
                true
            }
            Err(e) => {
                self.fail(&e);
                false
            }
        }
    }
    pub fn enabled(&self, command: Command) -> bool {
        if let Some(enabled) = self.frame_set_enabled(command) {
            return enabled;
        }
        if let Some(enabled) = self.layer_command_enabled(command) {
            return enabled;
        }
        if let Some(enabled) = self.timeline_enabled(command) {
            return enabled;
        }
        match command {
            Command::Undo => self.editor.undo_label().is_some(),
            Command::Redo => self.editor.redo_label().is_some(),
            Command::Delete | Command::Symbol => {
                !self.editor.selection.is_empty()
                    && self
                        .editor
                        .selection
                        .iter()
                        .all(|s| self.timeline_editable(s.layer))
            }
            Command::Scene => self.editor.editing.is_some(),
            Command::AddLayer => self.editor.timeline().layers.len() < 256,
            _ => true,
        }
    }
    pub fn dispatch(&mut self, command: Command, ctx: &egui::Context) {
        if !self.enabled(command)
            || self.drag.is_some()
            || self.timeline_state.gesture.is_some()
            || self.timeline_state.duration_drag.is_some()
            || self.timeline_state.layer_drag.is_some()
        {
            return;
        }
        if self.dispatch_frame_set(command)
            || self.dispatch_layers(command)
            || self.dispatch_timeline(command)
        {
            self.playing = false;
            self.texture = None;
            self.sync_forms();
            return;
        }
        match command {
            Command::New => self.request(Pending::New, ctx),
            Command::Open => self.request(Pending::Open, ctx),
            Command::Demo => self.request(Pending::Demo, ctx),
            Command::Save => {
                self.save_document(false);
            }
            Command::SaveAs => {
                self.save_document(true);
            }
            Command::Undo => {
                self.editor.undo();
                self.reset_timeline_gesture();
            }
            Command::Redo => {
                self.editor.redo();
                self.reset_timeline_gesture();
            }
            Command::Delete => {
                self.editor.delete_selected();
            }
            Command::AddLayer => {
                self.clear_layer_selection();
                self.editor.add_layer();
                let name = self.i18n.format(
                    "default-layer",
                    &[("number", &self.editor.timeline().layers.len().to_string())],
                );
                let layer = self.editor.layer;
                self.editor.timeline_mut().layers[layer].name = name;
            }
            Command::Tween => self.editor.set_tween(Tween::Classic {
                ease: 0.0,
                rotate_turns: 0,
            }),
            Command::Play => {
                self.playing = !self.playing;
                self.frame_fraction = 0.0;
                self.last_time = Instant::now();
            }
            Command::Fit => {
                self.fit = true;
                self.pan = egui::Vec2::ZERO;
            }
            Command::Scene => {
                self.editor.exit_to_scene();
                self.reset_timeline_gesture();
                self.texture = None;
            }
            Command::Symbol => {
                self.symbol_name = self.i18n.text("default-symbol");
                self.symbol_open = true;
            }
            Command::Export => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter(self.i18n.text("filter-image"), &["png"])
                    .set_file_name("frame.png")
                    .save_file()
                {
                    match self
                        .render_export_pixels(1.0)
                        .and_then(|pix| pix.encode_png().map_err(|e| e.to_string()))
                        .and_then(|bytes| document::atomic_write(&path, &bytes))
                    {
                        Ok(()) => {
                            self.status = self.i18n.format(
                                "status-exported",
                                &[("name", &path.display().to_string())],
                            );
                            self.error = false;
                        }
                        Err(e) => self.fail(&e),
                    }
                }
            }
            _ => unreachable!("timeline commands are handled above"),
        }
        self.sync_forms();
    }
    fn render_pixels(&self, scale: f32) -> Result<tiny_skia::Pixmap, String> {
        self.render_pixels_mode(scale, true)
    }
    fn render_export_pixels(&self, scale: f32) -> Result<tiny_skia::Pixmap, String> {
        self.render_pixels_mode(scale, false)
    }
    fn render_pixels_mode(&self, scale: f32, outlines: bool) -> Result<tiny_skia::Pixmap, String> {
        document::validate(&self.editor.doc)?;
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let w = (self.editor.doc.width * scale).round().max(1.0) as u32;
            let h = (self.editor.doc.height * scale).round().max(1.0) as u32;
            let mut pix = tiny_skia::Pixmap::new(w, h).ok_or_else(|| "error-render".to_owned())?;
            let bg = self.editor.doc.background;
            pix.fill(tiny_skia::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
            let mut items = self.editor.evaluate_current(self.editor.frame);
            if !outlines {
                for item in &mut items {
                    item.outline = None;
                }
            }
            ac_render::draw_items(
                &mut pix.as_mut(),
                &items,
                &ac_render::RenderOpts {
                    view: Matrix::scale(scale, scale),
                    ..Default::default()
                },
            );
            Ok(pix)
        }))
        .unwrap_or_else(|_| Err("error-render".into()))
    }
    // A property transition ends a pending gesture before applying its own edit.
    // An already moved object keeps its separate undo entry; uncommitted drawing
    // previews disappear. Escape still cancels an active move through history.
    pub(super) fn finish_stage_gesture(&mut self) {
        self.drag = None;
    }
    pub(super) fn editable_selection(&self) -> bool {
        !self.editor.selection.is_empty()
            && self
                .editor
                .selection
                .iter()
                .all(|s| self.timeline_editable(s.layer) && self.editor.element_at(*s).is_some())
    }
    pub(super) fn apply_selection_transform(&mut self) {
        if !self.editable_selection() {
            return;
        }
        let Some(element) = self.editor.selected_element() else {
            return;
        };
        let [x, y, alpha] = self.transform_fields;
        if !x.is_finite() || !y.is_finite() || !alpha.is_finite() {
            return;
        }
        self.finish_stage_gesture();
        let dx = x - element.matrix.tx;
        let dy = y - element.matrix.ty;
        let sampled = self.editor.modify_selected("selection.properties", |el| {
            el.matrix.tx += dx;
            el.matrix.ty += dy;
            el.alpha = alpha.clamp(0.0, 1.0);
        });
        self.notice_tween_pose_edit(sampled);
        self.texture = None;
        self.sync_forms();
    }
    pub(super) fn apply_selection_fill(&mut self) {
        if !self.editable_selection() {
            return;
        }
        self.finish_stage_gesture();
        let c = self.fill;
        let sampled = self.editor.modify_selected("selection.fill", |el| {
            if let ac_core::ElementKind::Shape(s) = &mut el.kind {
                s.fill = Some(Color {
                    r: c.r(),
                    g: c.g(),
                    b: c.b(),
                    a: c.a(),
                });
            }
        });
        self.notice_tween_pose_edit(sampled);
        self.texture = None;
    }
    fn notice_tween_pose_edit(&mut self, sampled: bool) {
        if sampled {
            self.status = self.i18n.text("status-tween-boundary-poses");
            self.error = false;
        } else if self.status == self.i18n.text("status-tween-boundary-poses") {
            self.status.clear();
        }
    }
    fn menu_command(&mut self, ui: &mut egui::Ui, command: Command) {
        let mut button =
            egui::Button::new(self.i18n.text(command.label())).min_size(vec2(245.0, 28.0));
        if !command.shortcut().is_empty() {
            button = button.shortcut_text(command.shortcut());
        }
        if ui.add_enabled(self.enabled(command), button).clicked() {
            self.dispatch(command, ui.ctx());
            ui.close();
        }
    }
    fn header(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("studio.header")
            .exact_size(43.2)
            .frame(
                egui::Frame::NONE
                    .fill(self.prefs.theme.palette().panel)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 0,
                        top: 8,
                        bottom: 3,
                    }),
            )
            .show(ui, |ui| {
                egui::MenuBar::new().ui(ui, |ui| {
                    ui.label(
                        RichText::new("Flare")
                            .font(theme::medium(16.0))
                            .strong()
                            .color(self.prefs.theme.palette().wordmark),
                    );
                    ui.add_space(4.0);
                    ui.spacing_mut().item_spacing.x = 9.0;
                    for (key, commands) in [
                        (
                            "menu-file",
                            &[
                                Command::New,
                                Command::Open,
                                Command::Save,
                                Command::SaveAs,
                                Command::Export,
                                Command::Demo,
                            ][..],
                        ),
                        (
                            "menu-edit",
                            &[
                                Command::Undo,
                                Command::Redo,
                                Command::Delete,
                                Command::CopyFrames,
                                Command::CutFrames,
                                Command::PasteFrames,
                                Command::PasteOverwriteFrames,
                                Command::ClearFrames,
                                Command::SelectAllFrames,
                                Command::CopyLayers,
                                Command::CutLayers,
                                Command::PasteLayers,
                                Command::SelectAllLayers,
                            ][..],
                        ),
                        ("menu-view", &[Command::Fit, Command::Scene][..]),
                        (
                            "menu-insert",
                            &[
                                Command::AddLayer,
                                Command::AddFolder,
                                Command::InsertFrame,
                                Command::Keyframe,
                                Command::BlankKeyframe,
                            ][..],
                        ),
                        (
                            "menu-modify",
                            &[
                                Command::Symbol,
                                Command::Tween,
                                Command::RemoveFrame,
                                Command::ClearKeyframe,
                                Command::ReverseFrames,
                                Command::FrameByFrame,
                                Command::MoveFramesEarlier,
                                Command::MoveFramesLater,
                                Command::CopyFramesEarlier,
                                Command::CopyFramesLater,
                                Command::DuplicateLayer,
                                Command::DeleteLayer,
                                Command::LayerUp,
                                Command::LayerDown,
                                Command::IndentLayer,
                                Command::OutdentLayer,
                                Command::ExpandFolders,
                                Command::CollapseFolders,
                            ][..],
                        ),
                        ("menu-control", &[Command::Play][..]),
                    ] {
                        Self::top_menu(ui, key, self.i18n.text(key), |ui| {
                            for command in commands {
                                self.menu_command(ui, *command);
                            }
                        });
                    }
                    Self::top_menu(ui, "menu-window", self.i18n.text("menu-window"), |ui| {
                        ui.checkbox(
                            &mut self.prefs.inspector,
                            self.i18n.text("panel-properties"),
                        );
                        ui.checkbox(&mut self.prefs.timeline, self.i18n.text("panel-timeline"));
                        ui.checkbox(&mut self.prefs.rulers, self.i18n.text("action-rulers"));
                        ui.checkbox(&mut self.prefs.options_bar, self.i18n.text("panel-options"));
                        ui.checkbox(&mut self.prefs.status_bar, self.i18n.text("panel-status"));
                        if widgets::button(ui, self.i18n.text("action-preferences")).clicked() {
                            self.preferences_open = true;
                            ui.close();
                        }
                    });
                    Self::top_menu(ui, "menu-help", self.i18n.text("menu-help"), |ui| {
                        if widgets::button(ui, self.i18n.text("action-about")).clicked() {
                            self.about_open = true;
                            ui.close();
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        self.window_buttons(ui);
                        Self::top_menu(
                            ui,
                            "workspace-title",
                            self.i18n.text("workspace-title"),
                            |ui| {
                                if widgets::button(ui, self.i18n.text("action-reset-layout"))
                                    .clicked()
                                {
                                    self.reset_layout(ui.ctx());
                                    ui.close();
                                }
                            },
                        );
                        widgets::separator(ui);
                        if theme::icon_button(
                            ui,
                            Icon::Settings,
                            false,
                            &self.i18n.text("action-preferences"),
                        )
                        .clicked()
                        {
                            self.preferences_open = true;
                        }
                        self.window_drag_area(ui);
                    });
                });
            });
    }
    fn top_menu(
        ui: &mut egui::Ui,
        key: &'static str,
        label: String,
        content: impl FnOnce(&mut egui::Ui),
    ) {
        // Restrict hover activation to this bar: a color picker or unrelated
        // popup must not put the application menus into tracking mode.
        const KEYS: &[&str] = &[
            "menu-file",
            "menu-edit",
            "menu-view",
            "menu-insert",
            "menu-modify",
            "menu-control",
            "menu-window",
            "menu-help",
            "workspace-title",
        ];
        let id_for = |key| egui::Id::new(("studio.main-menu", key));
        let id = id_for(key);
        ui.push_id(key, |ui| {
            let open = egui::Popup::is_id_open(ui.ctx(), id);
            let display = if key == "workspace-title" {
                format!("{label}    ")
            } else {
                label.clone()
            };
            let response = ui.add(egui::Button::new(display).selected(open));
            if key == "workspace-title" {
                let c = response.rect.right_center() - vec2(7.0, 0.0);
                ui.painter().add(egui::Shape::convex_polygon(
                    vec![
                        c + vec2(-2.4, -1.4),
                        c + vec2(2.4, -1.4),
                        c + vec2(0.0, 1.4),
                    ],
                    ui.visuals().text_color(),
                    Stroke::NONE,
                ));
            }
            response
                .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &label));
            if response.hovered()
                && !response.clicked()
                && !open
                && KEYS
                    .iter()
                    .any(|key| egui::Popup::is_id_open(ui.ctx(), id_for(*key)))
            {
                egui::Popup::open_id(ui.ctx(), id);
            }
            // Retain egui's click toggle, Escape, outside-click dismissal and
            // command-close behavior. IDs never depend on translated labels.
            egui::Popup::menu(&response).id(id).show(content);
        });
    }
    fn tools(&mut self, ui: &mut egui::Ui) {
        let p = self.prefs.theme.palette();
        let panel = if self.tools_on_left() {
            egui::Panel::left("studio.tools.rtl")
        } else {
            egui::Panel::right("studio.tools.ltr")
        };
        let dock = panel
            .show_separator_line(false)
            .exact_size(if self.prefs.tools_collapsed {
                24.0
            } else {
                43.2
            })
            .frame(
                egui::Frame::NONE
                    .fill(p.panel)
                    .inner_margin(egui::Margin::symmetric(7, 0)),
            )
            .show(ui, |ui| {
                ui.spacing_mut().interact_size.x = 26.0;
                ui.spacing_mut().item_spacing.y = 0.0;
                if widgets::dock_cap(
                    ui,
                    p,
                    7.0,
                    !self.prefs.tools_collapsed,
                    &self.i18n.text("dock-toggle-tools"),
                )
                .clicked()
                {
                    self.prefs.tools_collapsed = !self.prefs.tools_collapsed;
                }
                if self.prefs.tools_collapsed {
                    return;
                }
                let menu = widgets::panel_menu(ui, p, &self.i18n.text("dock-tools-menu"));
                egui::Popup::menu(&menu).show(|ui| {
                    for tool in [
                        Tool::Select,
                        Tool::Rectangle,
                        Tool::Ellipse,
                        Tool::Hand,
                        Tool::Zoom,
                    ] {
                        if widgets::button(ui, self.i18n.text(tool.label())).clicked() {
                            self.tool = tool;
                            ui.close();
                        }
                    }
                });
                self.workspace_grip(
                    ui,
                    workspace::DockModule::Tools,
                    vec2(ui.available_width(), 7.0),
                );
                for group in [
                    &[Tool::Select][..],
                    &[Tool::Rectangle, Tool::Ellipse][..],
                    &[Tool::Hand, Tool::Zoom][..],
                ] {
                    for tool in group {
                        let response = theme::neutral_tool_button(
                            ui,
                            tool.icon(),
                            self.tool == *tool,
                            &self.i18n.text(tool.label()),
                        );
                        if response.clicked() && self.drag.is_none() {
                            self.tool = *tool;
                        }
                        if *tool == Tool::Hand && response.double_clicked() {
                            self.dispatch(Command::Fit, ui.ctx());
                        }
                    }
                    widgets::dock_separator(ui, self.prefs.theme.palette(), 5.0);
                }
                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(8.0);
                    ui.color_edit_button_srgba(&mut self.fill)
                        .on_hover_text(self.i18n.text("field-fill"));
                });
            });
        theme::dock_edges(ui.painter(), dock.response.rect, self.prefs.theme.palette());
    }
    fn panel_strip(&mut self, ui: &mut egui::Ui) {
        let panel = if self.inspector_on_left() {
            egui::Panel::left("studio.strip.rtl")
        } else {
            egui::Panel::right("studio.strip.ltr")
        };
        let dock = panel
            .show_separator_line(false)
            .exact_size(42.4)
            .frame(
                egui::Frame::NONE
                    .fill(self.prefs.theme.palette().panel)
                    .inner_margin(egui::Margin::symmetric(5, 0)),
            )
            .show(ui, |ui| {
                if widgets::dock_cap(
                    ui,
                    self.prefs.theme.palette(),
                    5.0,
                    false,
                    &self.i18n.text("dock-toggle-properties"),
                )
                .clicked()
                {
                    self.prefs.inspector = !self.prefs.inspector;
                }
                ui.vertical_centered(|ui| {
                    ui.spacing_mut().interact_size.x = 22.0;
                    self.workspace_grip(
                        ui,
                        workspace::DockModule::Inspector,
                        vec2(ui.available_width(), 7.0),
                    );
                    let palette = theme::icon_button(
                        ui,
                        Icon::Palette,
                        false,
                        &self.i18n.text("panel-color"),
                    );
                    egui::Popup::from_toggle_button_response(&palette).show(|ui| {
                        egui::color_picker::color_picker_color32(
                            ui,
                            &mut self.fill,
                            egui::color_picker::Alpha::OnlyBlend,
                        );
                    });
                    widgets::dock_separator(ui, self.prefs.theme.palette(), 5.0);
                    if theme::icon_button(
                        ui,
                        Icon::Settings,
                        false,
                        &self.i18n.text("panel-properties"),
                    )
                    .clicked()
                    {
                        self.prefs.inspector = true;
                        self.props_tab = true;
                    }
                    if theme::icon_button(ui, Icon::Symbol, false, &self.i18n.text("panel-library"))
                        .clicked()
                    {
                        self.prefs.inspector = true;
                        self.props_tab = false;
                    }
                    widgets::dock_separator(ui, self.prefs.theme.palette(), 5.0);
                    if theme::icon_button(ui, Icon::Key, false, &self.i18n.text("panel-timeline"))
                        .clicked()
                    {
                        self.prefs.timeline = !self.prefs.timeline;
                    }
                });
            });
        theme::dock_edges(ui.painter(), dock.response.rect, self.prefs.theme.palette());
    }
    fn inspector(&mut self, ui: &mut egui::Ui) {
        if !self.prefs.inspector {
            return;
        }
        let panel = if self.inspector_on_left() {
            egui::Panel::left("studio.inspector.rtl")
        } else {
            egui::Panel::right("studio.inspector.ltr")
        };
        panel
            .show_separator_line(false)
            .resizable(true)
            .default_size(285.6)
            .min_size(230.0)
            .max_size(460.0)
            .frame(egui::Frame::NONE.fill(self.prefs.theme.palette().panel))
            .show(ui, |ui| {
                let p = self.prefs.theme.palette();
                ui.spacing_mut().item_spacing.y = 0.0;
                if widgets::dock_cap(ui, p, 0.0, true, &self.i18n.text("dock-hide-properties"))
                    .clicked()
                {
                    self.prefs.inspector = false;
                }
                widgets::panel_tab_strip(ui, p, |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.horizontal(|ui| {
                        if widgets::panel_tab(
                            ui,
                            "properties",
                            &self.i18n.text("panel-properties"),
                            self.props_tab,
                            p,
                        )
                        .clicked()
                        {
                            self.props_tab = true;
                        }
                        if widgets::panel_tab(
                            ui,
                            "library",
                            &self.i18n.text("panel-library"),
                            !self.props_tab,
                            p,
                        )
                        .clicked()
                        {
                            self.props_tab = false;
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let menu =
                                widgets::panel_menu(ui, p, &self.i18n.text("dock-properties-menu"));
                            egui::Popup::menu(&menu).show(|ui| {
                                for (key, properties) in
                                    [("panel-properties", true), ("panel-library", false)]
                                {
                                    if widgets::button(ui, self.i18n.text(key)).clicked() {
                                        self.props_tab = properties;
                                        ui.close();
                                    }
                                }
                                if widgets::button(ui, self.i18n.text("dock-hide-properties"))
                                    .clicked()
                                {
                                    self.prefs.inspector = false;
                                    ui.close();
                                }
                            });
                        });
                    });
                });
                egui::ScrollArea::vertical()
                    .id_salt("studio.inspector.scroll")
                    .show(ui, |ui| {
                        egui::Frame::NONE
                            .inner_margin(egui::Margin::symmetric(6, 2))
                            .show(ui, |ui| {
                                if !self.props_tab {
                                    self.library(ui);
                                    return;
                                }
                                if self.form_selection != self.editor.selection
                                    || self.form_layer != self.editor.layer
                                {
                                    self.sync_forms();
                                }
                                if let Some(element) = self.editor.selected_element() {
                                    ui.heading(&element.name);
                                    ui.label(
                                        RichText::new(self.i18n.text("field-selection"))
                                            .small()
                                            .color(self.prefs.theme.palette().muted),
                                    );
                                    ui.add_space(10.0);
                                    self.section(ui, "section-transform");
                                    egui::Grid::new("studio.transform")
                                        .spacing(vec2(8.0, 4.0))
                                        .show(ui, |ui| {
                                            for (label, index) in [("X", 0), ("Y", 1)] {
                                                ui.label(label);
                                                self.number_field(ui, index, false);
                                                ui.end_row();
                                            }
                                            ui.label(self.i18n.text("field-opacity"));
                                            ui.add(
                                                egui::Slider::new(
                                                    &mut self.transform_fields[2],
                                                    0.0..=1.0,
                                                )
                                                .show_value(true),
                                            );
                                            ui.end_row();
                                        });
                                    if widgets::button(ui, self.i18n.text("action-apply")).clicked()
                                    {
                                        self.apply_selection_transform();
                                    }
                                    ui.add_space(10.0);
                                    self.section(ui, "section-appearance");
                                    ui.horizontal(|ui| {
                                        ui.label(self.i18n.text("field-fill"));
                                        ui.color_edit_button_srgba(&mut self.fill);
                                    });
                                    if widgets::button(ui, self.i18n.text("action-apply")).clicked()
                                    {
                                        self.apply_selection_fill();
                                    }
                                    ui.add_space(10.0);
                                    if widgets::button(ui, self.i18n.text("cmd-symbol")).clicked() {
                                        self.dispatch(Command::Symbol, ui.ctx());
                                    }
                                } else {
                                    ui.horizontal(|ui| {
                                        ui.add_space(14.0);
                                        let (slot, _) = ui
                                            .allocate_exact_size(vec2(42.0, 48.0), Sense::hover());
                                        theme::paint_icon(
                                            ui,
                                            Rect::from_center_size(slot.center(), vec2(20.0, 24.0)),
                                            Icon::File,
                                            p.text,
                                        );
                                        ui.vertical(|ui| {
                                            ui.add_space(6.0);
                                            ui.label(self.i18n.text("field-document"));
                                            ui.add_space(6.0);
                                            ui.label(self.title());
                                        });
                                    });
                                    self.section(ui, "section-document");
                                    for (key, index) in
                                        [("field-fps", 2), ("field-width", 0), ("field-height", 1)]
                                    {
                                        let row = ui.horizontal(|ui| {
                                            ui.set_height(26.4);
                                            ui.add_sized(
                                                [64.0, theme::CONTROL_HEIGHT],
                                                egui::Label::new(self.i18n.text(key))
                                                    .halign(egui::Align::RIGHT),
                                            );
                                            self.number_field(ui, index, true);
                                        });
                                        let rect = Rect::from_min_max(
                                            pos2(
                                                ui.min_rect().left() - 6.0,
                                                row.response.rect.top(),
                                            ),
                                            pos2(
                                                ui.max_rect().right() + 6.0,
                                                row.response.rect.bottom(),
                                            ),
                                        );
                                        let shades: &[u8] = if p.back.r() > 128 {
                                            &[225, 188, 163]
                                        } else {
                                            &[80, 59, 44]
                                        };
                                        theme::edge_bands(ui.painter(), rect, false, true, shades);
                                    }
                                    ui.add_space(2.0);
                                    if widgets::button(ui, self.i18n.text("action-apply")).clicked()
                                    {
                                        let [w, h, fps] = self.doc_fields;
                                        if w.is_finite()
                                            && h.is_finite()
                                            && fps.is_finite()
                                            && (1.0..=4096.0).contains(&w)
                                            && (1.0..=4096.0).contains(&h)
                                            && (1.0..=120.0).contains(&fps)
                                        {
                                            self.editor.set_doc_props(
                                                w,
                                                h,
                                                fps,
                                                self.editor.doc.background,
                                            );
                                            self.fit = true;
                                        } else {
                                            self.fail("error-size");
                                        }
                                    }
                                    ui.add_space(4.0);
                                    self.section(ui, "section-appearance");
                                    let bg = self.editor.doc.background;
                                    let mut c = Color32::from_rgb(bg.r, bg.g, bg.b);
                                    ui.horizontal(|ui| {
                                        ui.label(self.i18n.text("field-background"));
                                        if ui.color_edit_button_srgba(&mut c).changed() {
                                            self.editor.set_doc_props(
                                                self.editor.doc.width,
                                                self.editor.doc.height,
                                                self.editor.doc.fps,
                                                Color::rgb(c.r(), c.g(), c.b()),
                                            );
                                        }
                                    });
                                    ui.add_space(10.0);
                                    ui.label(
                                        RichText::new(self.i18n.text("inspector-hint"))
                                            .small()
                                            .color(self.prefs.theme.palette().muted),
                                    );
                                }
                                ui.add_space(4.0);
                                self.timeline_properties(ui);
                                self.layer_properties(ui);
                                self.section(ui, "section-swatches");
                                widgets::swatches(ui, &mut self.fill, self.prefs.theme.palette());
                                self.section(ui, "panel-navigator");
                                self.navigator(ui);
                            });
                    });
            });
    }
    fn number_field(&mut self, ui: &mut egui::Ui, index: usize, document: bool) {
        let french = self.prefs.locale == Locale::French;
        let suffix = if document && index < 2 { " px" } else { "" };
        let value = if document {
            &mut self.doc_fields[index]
        } else {
            &mut self.transform_fields[index]
        };
        widgets::field(ui, value, french, suffix, 80.0);
    }
    fn section(&self, ui: &mut egui::Ui, key: &str) {
        widgets::section(ui, &self.i18n.text(key), self.prefs.theme.palette());
    }
    fn library(&mut self, ui: &mut egui::Ui) {
        let p = self.prefs.theme.palette();
        ui.label(
            RichText::new(
                self.i18n
                    .count("library-count", self.editor.doc.library.len()),
            )
            .color(p.muted)
            .size(11.0),
        );
        widgets::text_edit(
            ui,
            egui::TextEdit::singleline(&mut self.library_filter)
                .id_salt("studio.library-filter")
                .hint_text(self.i18n.text("library-search"))
                .desired_width(f32::INFINITY),
        );
        ui.add_space(8.0);
        let query = self.library_filter.to_lowercase();
        let symbols: Vec<_> = self
            .editor
            .doc
            .library
            .iter()
            .filter(|s| s.name.to_lowercase().contains(&query))
            .map(|s| (s.id, s.name.clone()))
            .collect();
        if self.editor.doc.library.is_empty() {
            ui.add_space(22.0);
            let (r, _) = ui.allocate_exact_size(vec2(32.0, 32.0), Sense::hover());
            theme::paint_icon(ui, r, Icon::Symbol, p.accent);
            ui.heading(self.i18n.text("library-empty"));
            ui.label(
                RichText::new(self.i18n.text("library-hint"))
                    .small()
                    .color(p.muted),
            );
        } else if symbols.is_empty() {
            ui.label(self.i18n.text("search-empty"));
        }
        for (id, name) in symbols {
            let (rect, response) =
                ui.allocate_exact_size(vec2(ui.available_width(), 42.0), Sense::click());
            if response.hovered() || self.editor.editing == Some(id) {
                ui.painter().rect_filled(
                    rect,
                    3.0,
                    if self.editor.editing == Some(id) {
                        p.select
                    } else {
                        p.raised
                    },
                );
            }
            theme::paint_icon(
                ui,
                Rect::from_center_size(rect.left_center() + vec2(18.0, 0.0), vec2(19.0, 19.0)),
                Icon::Symbol,
                p.accent,
            );
            let label = rect.shrink2(vec2(36.0, 0.0));
            ui.painter().with_clip_rect(label).text(
                label.left_center(),
                egui::Align2::LEFT_CENTER,
                &name,
                theme::medium(12.0),
                p.text,
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::SelectableLabel, true, &name)
            });
            if response.double_clicked() {
                self.editor.enter_symbol(id);
                self.reset_timeline_gesture();
                self.playing = false;
                self.texture = None;
                self.sync_forms();
            }
        }
        ui.add_space(12.0);
        ui.label(
            RichText::new(self.i18n.text("library-edit"))
                .small()
                .color(p.muted),
        );
        if self.editor.editing.is_some()
            && widgets::button(ui, self.i18n.text("cmd-scene")).clicked()
        {
            self.dispatch(Command::Scene, ui.ctx());
        }
    }
    fn stage(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(self.prefs.theme.palette().back))
            .show(ui, |ui| {
                let p = self.prefs.theme.palette();
                ui.spacing_mut().item_spacing.y = 0.0;
                widgets::tab_strip(ui, p, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        let label =
                            format!("{}{}", self.title(), if self.dirty() { " *" } else { "" });
                        widgets::tab(ui, "document", &label, true, p);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let tip = self.i18n.text("action-new-document");
                            let add = ui.add_sized(
                                vec2(24.0, theme::TAB_HEIGHT),
                                egui::Button::new("+").frame(false),
                            );
                            add.widget_info(|| {
                                egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &tip)
                            });
                            if add.on_hover_text(tip).clicked() {
                                self.dispatch(Command::New, ui.ctx());
                            }
                        });
                    });
                });
                let scene_bar = egui::Frame::NONE
                    .fill(Color32::from_gray(if p.back.r() > 128 { 211 } else { 67 }))
                    .inner_margin(egui::Margin::symmetric(8, 2))
                    .show(ui, |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.horizontal(|ui| {
                            let back = ui
                                .add_enabled_ui(self.editor.editing.is_some(), |ui| {
                                    theme::icon_button(
                                        ui,
                                        Icon::Previous,
                                        false,
                                        &self.i18n.text("cmd-scene"),
                                    )
                                })
                                .inner;
                            if back.clicked() {
                                self.dispatch(Command::Scene, ui.ctx());
                            }
                            let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
                            theme::paint_icon(ui, r, Icon::File, p.text);
                            ui.label(
                                RichText::new(&self.editor.timeline().name)
                                    .font(theme::medium(12.0)),
                            );
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    self.zoom_control(ui);
                                    if theme::icon_button(
                                        ui,
                                        Icon::Monitor,
                                        false,
                                        &self.i18n.text("action-fit"),
                                    )
                                    .clicked()
                                    {
                                        self.dispatch(Command::Fit, ui.ctx());
                                    }
                                    if theme::neutral_icon_button(
                                        ui,
                                        Icon::Ruler,
                                        self.prefs.rulers,
                                        &self.i18n.text("action-rulers"),
                                    )
                                    .clicked()
                                    {
                                        self.prefs.rulers = !self.prefs.rulers;
                                    }
                                },
                            );
                        });
                    });
                let seam: &[u8] = if p.back.r() > 128 {
                    &[168, 179]
                } else {
                    &[43, 49]
                };
                theme::edge_bands(ui.painter(), scene_bar.response.rect, false, true, seam);
                let (area, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
                let scrollbar = 13.6;
                let viewport = Rect::from_min_max(area.min, area.max - vec2(scrollbar, scrollbar));
                let response = ui.interact(
                    viewport,
                    ui.id().with("studio.stage.viewport"),
                    Sense::click_and_drag(),
                );
                let painter = ui.painter_at(viewport);
                if self.fit {
                    self.zoom = ((viewport.width() - 80.0) / self.editor.doc.width)
                        .min((viewport.height() - 85.0) / self.editor.doc.height)
                        .clamp(0.1, 8.0);
                }
                let size = vec2(
                    self.editor.doc.width * self.zoom,
                    self.editor.doc.height * self.zoom,
                );
                let stage = Rect::from_center_size(viewport.center() + self.pan, size);
                let cache = (self.editor.revision, self.editor.frame, self.editor.editing);
                if self.texture.is_none() || self.cache != cache {
                    let scale =
                        (1600.0 / self.editor.doc.width.max(self.editor.doc.height)).min(1.0);
                    match self.render_pixels(scale) {
                        Ok(pix) => {
                            let image = egui::ColorImage::from_rgba_unmultiplied(
                                [pix.width() as usize, pix.height() as usize],
                                &ac_render::unpremultiplied(&pix),
                            );
                            if let Some(texture) = &mut self.texture {
                                texture.set(image, egui::TextureOptions::LINEAR);
                            } else {
                                self.texture = Some(ui.ctx().load_texture(
                                    "studio.stage",
                                    image,
                                    egui::TextureOptions::LINEAR,
                                ));
                            }
                            self.cache = cache;
                        }
                        Err(e) => {
                            self.fail(&e);
                        }
                    }
                }
                if let Some(texture) = &self.texture {
                    painter.image(
                        texture.id(),
                        stage,
                        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
                        Color32::WHITE,
                    );
                }
                if let Some((a, b)) = self.editor.selection_bounds() {
                    let selected = Rect::from_min_max(
                        stage.min + vec2(a.x, a.y) * self.zoom,
                        stage.min + vec2(b.x, b.y) * self.zoom,
                    );
                    painter.rect_stroke(
                        selected,
                        0.0,
                        Stroke::new(1.5, self.prefs.theme.palette().accent),
                        egui::StrokeKind::Outside,
                    );
                    for p in [
                        selected.left_top(),
                        selected.right_top(),
                        selected.left_bottom(),
                        selected.right_bottom(),
                    ] {
                        painter.rect_filled(
                            Rect::from_center_size(p, vec2(6.0, 6.0)),
                            0.0,
                            Color32::WHITE,
                        );
                    }
                }
                if self.prefs.rulers {
                    widgets::rulers(ui, viewport, stage, self.zoom, p);
                }
                self.stage_input(ui, &response, stage, &painter);
                self.stage_scrollbars(ui, area, viewport, size, p);
            });
    }
    fn stage_scrollbars(
        &mut self,
        ui: &mut egui::Ui,
        workspace: Rect,
        viewport: Rect,
        stage_size: egui::Vec2,
        p: theme::Palette,
    ) {
        let horizontal = Rect::from_min_max(
            pos2(workspace.left(), viewport.bottom()),
            pos2(viewport.right(), workspace.bottom()),
        );
        let vertical = Rect::from_min_max(
            pos2(viewport.right(), workspace.top()),
            pos2(workspace.right(), viewport.bottom()),
        );
        let corner = Rect::from_min_max(viewport.right_bottom(), workspace.right_bottom());
        self.stage_scrollbar(
            ui,
            horizontal,
            ScrollAxis::Horizontal,
            viewport,
            stage_size,
            p,
        );
        self.stage_scrollbar(ui, vertical, ScrollAxis::Vertical, viewport, stage_size, p);
        ui.painter().rect_filled(corner, 0.0, p.panel);
        theme::edge_bands(ui.painter(), corner, true, true, &[p.line.r()]);
        theme::edge_bands(ui.painter(), corner, false, true, &[p.line.r()]);
    }
    fn stage_scrollbar(
        &mut self,
        ui: &mut egui::Ui,
        rect: Rect,
        axis: ScrollAxis,
        viewport: Rect,
        stage_size: egui::Vec2,
        p: theme::Palette,
    ) {
        let vertical = axis == ScrollAxis::Vertical;
        let (before, track, after) = if vertical {
            let a = Rect::from_min_size(rect.min, vec2(rect.width(), rect.width()));
            let b = Rect::from_min_max(
                pos2(rect.left(), a.bottom()),
                pos2(rect.right(), rect.bottom() - rect.width()),
            );
            let c = Rect::from_min_max(pos2(rect.left(), b.bottom()), rect.max);
            (a, b, c)
        } else {
            let a = Rect::from_min_size(rect.min, vec2(rect.height(), rect.height()));
            let b = Rect::from_min_max(
                pos2(a.right(), rect.top()),
                pos2(rect.right() - rect.height(), rect.bottom()),
            );
            let c = Rect::from_min_max(pos2(b.right(), rect.top()), rect.max);
            (a, b, c)
        };
        let painter = ui.painter();
        theme::scroll_groove(painter, track, vertical, p);
        let (view_len, content_len, pan_value) = if vertical {
            (viewport.height(), stage_size.y, self.pan.y)
        } else {
            (viewport.width(), stage_size.x, self.pan.x)
        };
        // The supplied Flash reference has a deeper vertical pasteboard than
        // horizontal: its vertical thumb is about 27% of the rail versus 56%.
        let virtual_len = content_len.max(view_len * if vertical { 3.6 } else { 1.8 });
        let max_pan = (virtual_len - view_len) * 0.5;
        let axis_len = if vertical {
            track.height()
        } else {
            track.width()
        };
        let thumb_len = (axis_len * view_len / virtual_len).clamp(28.0, axis_len);
        let travel = (axis_len - thumb_len).max(0.0);
        let fraction = if max_pan > 0.0 {
            (0.5 - pan_value / (max_pan * 2.0)).clamp(0.0, 1.0)
        } else {
            0.5
        };
        let start = if vertical { track.top() } else { track.left() } + fraction * travel;
        let thumb = if vertical {
            Rect::from_min_size(
                pos2(track.left() + 1.0, start),
                vec2(track.width() - 2.0, thumb_len),
            )
        } else {
            Rect::from_min_size(
                pos2(start, track.top() + 1.0),
                vec2(thumb_len, track.height() - 2.0),
            )
        };
        let id = ui.id().with(if vertical {
            "stage.vscroll"
        } else {
            "stage.hscroll"
        });
        let response = ui.interact(track, id, Sense::click_and_drag());
        let arrow = |r: Rect, negative: bool, id: egui::Id| {
            let response = ui.interact(r, id, Sense::click());
            theme::scroll_arrow_face(
                ui.painter(),
                r,
                p,
                response.hovered(),
                response.is_pointer_button_down_on(),
            );
            let c = r.center();
            let (a, b, d) = match (vertical, negative) {
                (false, true) => (c + vec2(-3.0, 0.0), c + vec2(2.0, -3.0), c + vec2(2.0, 3.0)),
                (false, false) => (
                    c + vec2(3.0, 0.0),
                    c + vec2(-2.0, -3.0),
                    c + vec2(-2.0, 3.0),
                ),
                (true, true) => (c + vec2(0.0, -3.0), c + vec2(-3.0, 2.0), c + vec2(3.0, 2.0)),
                (true, false) => (
                    c + vec2(0.0, 3.0),
                    c + vec2(-3.0, -2.0),
                    c + vec2(3.0, -2.0),
                ),
            };
            ui.painter().add(egui::Shape::convex_polygon(
                vec![a, b, d],
                p.text,
                Stroke::NONE,
            ));
            response
        };
        let mut step = 0.0;
        let decrement = arrow(before, true, id.with("decrement"));
        let decrement_label = self.i18n.text(if vertical {
            "scroll-view-decrement-vertical"
        } else {
            "scroll-view-decrement-horizontal"
        });
        decrement.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &decrement_label)
        });
        if decrement.on_hover_text(decrement_label).clicked() {
            step = 1.0;
        }
        let increment = arrow(after, false, id.with("increment"));
        let increment_label = self.i18n.text(if vertical {
            "scroll-view-increment-vertical"
        } else {
            "scroll-view-increment-horizontal"
        });
        increment.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &increment_label)
        });
        if increment.on_hover_text(increment_label).clicked() {
            step = -1.0;
        }
        if step != 0.0 {
            let amount = view_len * 0.08;
            if vertical {
                self.pan.y = (self.pan.y + step * amount).clamp(-max_pan, max_pan);
            } else {
                self.pan.x = (self.pan.x + step * amount).clamp(-max_pan, max_pan);
            }
            self.fit = false;
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(pointer) = ui.input(|i| i.pointer.press_origin()) {
                let pointer_axis = axis.coord(pointer);
                let thumb_center = if vertical {
                    thumb.center().y
                } else {
                    thumb.center().x
                };
                let grab_offset = if thumb.contains(pointer) {
                    pointer_axis - thumb_center
                } else {
                    0.0
                };
                self.scroll_drag = Some(ScrollDrag {
                    axis,
                    track_start: if vertical { track.top() } else { track.left() },
                    thumb_len,
                    travel,
                    max_pan,
                    grab_offset,
                });
            }
        }
        if response.dragged_by(egui::PointerButton::Primary) {
            if let (Some(drag), Some(pointer)) =
                (self.scroll_drag, ui.input(|i| i.pointer.interact_pos()))
            {
                if drag.axis == axis && drag.travel > 0.0 {
                    let center = axis.coord(pointer) - drag.grab_offset;
                    let fraction = ((center - drag.track_start - drag.thumb_len * 0.5)
                        / drag.travel)
                        .clamp(0.0, 1.0);
                    let pan = (0.5 - fraction) * drag.max_pan * 2.0;
                    axis.set_pan(&mut self.pan, pan);
                    self.fit = false;
                }
            }
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            self.scroll_drag = None;
        }
        theme::scroll_thumb(
            ui.painter(),
            thumb,
            p,
            response.hovered(),
            response.dragged(),
        );
        let track_label = self.i18n.text(if vertical {
            "scroll-vertical"
        } else {
            "scroll-horizontal"
        });
        response.widget_info(|| {
            egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, &track_label)
        });
        response.on_hover_text(track_label);
    }
    fn stage_input(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        stage: Rect,
        painter: &egui::Painter,
    ) {
        if self.dock_drag.is_some()
            || self.pending.is_some()
            || self.command_open
            || self.new_open
            || self.about_open
            || self.preferences_open
            || self.symbol_open
        {
            return;
        }
        if response.clicked() || response.drag_started() {
            self.timeline_state.focused = false;
            self.editor.frame_sel = None;
            self.reset_timeline_gesture();
        }
        if response.hovered() {
            ui.ctx().set_cursor_icon(match self.tool {
                Tool::Hand => egui::CursorIcon::Grab,
                Tool::Zoom => egui::CursorIcon::ZoomIn,
                Tool::Rectangle | Tool::Ellipse => egui::CursorIcon::Crosshair,
                _ => egui::CursorIcon::Default,
            });
        }
        if response.clicked() && self.tool == Tool::Zoom {
            if let Some(pos) = response.interact_pointer_pos() {
                let next = (self.zoom
                    * if ui.input(|i| i.modifiers.shift) {
                        0.8
                    } else {
                        1.25
                    })
                .clamp(0.1, 8.0);
                let factor = next / self.zoom;
                let offset = pos - (response.rect.center() + vec2(0.0, -6.0));
                self.pan = (self.pan - offset) * factor + offset;
                self.zoom = next;
                self.fit = false;
            }
        }
        let to_doc = |p: egui::Pos2, zoom: f32| {
            let p = (p - stage.min) / zoom;
            pt(p.x, p.y)
        };
        if response.clicked() && self.tool == Tool::Select {
            if let Some(pos) = response
                .interact_pointer_pos()
                .filter(|p| stage.contains(*p))
            {
                self.editor.select(
                    self.stage_hit_test(to_doc(pos, self.zoom)),
                    ui.input(|i| i.modifiers.shift),
                );
                self.sync_forms();
            }
        }
        if response.drag_started_by(egui::PointerButton::Primary) {
            if let Some(start) = ui
                .input(|i| i.pointer.press_origin())
                .filter(|p| stage.contains(*p) || self.tool == Tool::Hand)
            {
                self.playing = false;
                if self.tool == Tool::Select {
                    let selected = self.stage_hit_test(to_doc(start, self.zoom));
                    if !selected.is_some_and(|s| self.editor.is_selected(s)) {
                        self.editor.select(selected, false);
                    }
                }
                self.drag = Some(Drag {
                    start,
                    last: egui::Vec2::ZERO,
                    pan: self.pan,
                    tool: self.tool,
                    moved: false,
                });
            }
        }
        let mut sampled_pose = None;
        if let Some(drag) = &mut self.drag {
            let delta = response.drag_delta();
            match drag.tool {
                Tool::Hand => {
                    self.pan = drag.pan + delta;
                    self.fit = false;
                }
                Tool::Select if !self.editor.selection.is_empty() && delta != drag.last => {
                    if !drag.moved {
                        self.editor.begin_drag();
                        drag.moved = true;
                    }
                    let d = (delta - drag.last) / self.zoom;
                    sampled_pose = Some(self.editor.modify_selected_continuous(|el| {
                        el.matrix.tx += d.x;
                        el.matrix.ty += d.y;
                    }));
                    self.texture = None;
                    drag.last = delta;
                }
                Tool::Rectangle | Tool::Ellipse => {
                    if let Some(current) = response.interact_pointer_pos() {
                        let rect = Rect::from_two_pos(drag.start, current);
                        if drag.tool == Tool::Rectangle {
                            painter.rect_stroke(
                                rect,
                                0.0,
                                Stroke::new(1.5, self.prefs.theme.palette().accent),
                                egui::StrokeKind::Inside,
                            );
                        } else {
                            painter.add(egui::epaint::EllipseShape::stroke(
                                rect.center(),
                                rect.size() / 2.0,
                                Stroke::new(1.5, self.prefs.theme.palette().accent),
                            ));
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(sampled) = sampled_pose {
            self.notice_tween_pose_edit(sampled);
        }
        if response.drag_stopped_by(egui::PointerButton::Primary) {
            if let Some(drag) = self.drag.take() {
                if matches!(drag.tool, Tool::Rectangle | Tool::Ellipse)
                    && self.timeline_editable(self.editor.layer)
                {
                    if let Some(end) = response.interact_pointer_pos() {
                        let a = to_doc(drag.start, self.zoom);
                        let b = to_doc(end, self.zoom);
                        let x = a.x.min(b.x);
                        let y = a.y.min(b.y);
                        let w = (a.x - b.x).abs();
                        let h = (a.y - b.y).abs();
                        if w > 1.0 && h > 1.0 {
                            let path = if drag.tool == Tool::Rectangle {
                                Path::rect(x, y, w, h)
                            } else {
                                Path::ellipse(x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0)
                            };
                            let c = self.fill;
                            self.editor.add_element(Element::shape(
                                &self.i18n.text("default-shape"),
                                Shape::new(
                                    path,
                                    Some(Color {
                                        r: c.r(),
                                        g: c.g(),
                                        b: c.b(),
                                        a: c.a(),
                                    }),
                                    None,
                                ),
                            ));
                        }
                    }
                }
                self.sync_forms();
            }
        }
        if response.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.1 {
                self.zoom = (self.zoom * (scroll * 0.002).exp()).clamp(0.1, 8.0);
                self.fit = false;
            }
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        self.command_dialog(ctx);
        if self.new_open {
            egui::Modal::new(egui::Id::new("studio.new")).show(ctx, |ui| {
                ui.set_width(540.0);
                ui.heading(self.i18n.text("dialog-new"));
                widgets::separator(ui);
                ui.columns(2, |columns| {
                    columns[0].label(self.i18n.text("field-preset"));
                    for (key, w, h) in [
                        ("preset-classic", 550.0, 400.0),
                        ("preset-hd", 1280.0, 720.0),
                        ("preset-square", 1080.0, 1080.0),
                    ] {
                        if widgets::selectable(
                            &mut columns[0],
                            self.new_fields[0] == w && self.new_fields[1] == h,
                            self.i18n.text(key),
                        )
                        .clicked()
                        {
                            self.new_fields[0] = w;
                            self.new_fields[1] = h;
                        }
                    }
                    columns[0].add_space(12.0);
                    widgets::document_preview(
                        &mut columns[0],
                        self.new_fields[0],
                        self.new_fields[1],
                        self.new_background,
                        self.prefs.theme.palette(),
                    );
                    let ui = &mut columns[1];
                    egui::Grid::new("studio.new.fields")
                        .spacing(vec2(12.0, 12.0))
                        .show(ui, |ui| {
                            for (index, key) in
                                [(0, "field-width"), (1, "field-height"), (2, "field-fps")]
                            {
                                ui.label(self.i18n.text(key));
                                let french = self.prefs.locale == Locale::French;
                                widgets::field(
                                    ui,
                                    &mut self.new_fields[index],
                                    french,
                                    if index < 2 { " px" } else { "" },
                                    108.0,
                                );
                                ui.end_row();
                            }
                            ui.label(self.i18n.text("field-background"));
                            ui.color_edit_button_srgba(&mut self.new_background);
                            ui.end_row();
                        });
                });
                ui.add_space(12.0);
                ui.label(
                    RichText::new(self.i18n.text("new-description"))
                        .small()
                        .color(self.prefs.theme.palette().muted),
                );
                widgets::separator(ui);
                ui.horizontal(|ui| {
                    if widgets::primary(
                        ui,
                        self.i18n.text("action-create"),
                        self.prefs.theme.palette(),
                    )
                    .clicked()
                    {
                        let mut doc = document::blank(
                            &self.i18n.text("default-scene"),
                            &self.i18n.format("default-layer", &[("number", "1")]),
                        );
                        [doc.width, doc.height, doc.fps] = self.new_fields;
                        let c = self.new_background;
                        doc.background = Color::rgb(c.r(), c.g(), c.b());
                        if document::validate(&doc).is_ok() {
                            self.replace(doc, None, false);
                            self.fit = false;
                            self.zoom = 1.0;
                            self.new_open = false;
                        } else {
                            self.fail("error-size");
                        }
                    }
                    if widgets::button(ui, self.i18n.text("action-cancel")).clicked() {
                        self.new_open = false;
                    }
                });
            });
        }
        if self.about_open {
            let mut open = true;
            egui::Window::new(self.i18n.text("action-about"))
                .id(egui::Id::new("studio.about"))
                .open(&mut open)
                .show(ctx, |ui| {
                    ui.heading("Flare Studio");
                    ui.label(self.i18n.text("about-description"));
                });
            self.about_open = open;
        }
        if self.preferences_open {
            let mut open = true;
            egui::Window::new(self.i18n.text("dialog-preferences"))
                .id(egui::Id::new("studio.preferences"))
                .open(&mut open)
                .collapsible(false)
                .resizable(true)
                .default_width(450.0)
                .show(ctx, |ui| {
                    self.section(ui, "section-workspace");
                    ui.label(self.i18n.text("field-theme"));
                    let mut selected = self.prefs.theme;
                    ui.horizontal(|ui| {
                        ui.radio_value(
                            &mut selected,
                            theme::Theme::Dark,
                            self.i18n.text("theme-dark"),
                        );
                        ui.radio_value(
                            &mut selected,
                            theme::Theme::Light,
                            self.i18n.text("theme-light"),
                        );
                    });
                    if selected != self.prefs.theme {
                        self.set_theme(ctx, selected);
                    }
                    widgets::separator(ui);

                    ui.label(self.i18n.text("field-language"));
                    let mut locale = self.prefs.locale;
                    for available in Locale::ALL {
                        let label = if matches!(available, Locale::English | Locale::French) {
                            available.label().to_owned()
                        } else {
                            self.i18n.text(available.label())
                        };
                        ui.radio_value(&mut locale, available, label);
                    }
                    if locale != self.prefs.locale {
                        self.set_locale(locale);
                    }
                    ui.add_space(12.0);
                    ui.label(self.i18n.text("preferences-hint"));
                    if matches!(locale, Locale::Expanded | Locale::RtlPreview) {
                        ui.label(
                            RichText::new(self.i18n.text("locale-preview-note"))
                                .small()
                                .color(self.prefs.theme.palette().muted),
                        );
                    }
                    widgets::separator(ui);
                    ui.checkbox(
                        &mut self.prefs.inspector,
                        self.i18n.text("panel-properties"),
                    );
                    ui.checkbox(&mut self.prefs.timeline, self.i18n.text("panel-timeline"));
                    ui.checkbox(&mut self.prefs.rulers, self.i18n.text("action-rulers"));
                    ui.checkbox(
                        &mut self.prefs.span_selection,
                        self.i18n.text("preferences-span-selection"),
                    );
                    if widgets::button(ui, self.i18n.text("action-reset-layout")).clicked() {
                        self.reset_layout(ctx);
                    }
                });
            self.preferences_open = open;
        }
        if self.symbol_open {
            egui::Modal::new(egui::Id::new("studio.symbol")).show(ctx, |ui| {
                ui.heading(self.i18n.text("dialog-symbol"));
                ui.label(self.i18n.text("field-symbol-name"));
                ui.text_edit_singleline(&mut self.symbol_name);
                ui.horizontal_wrapped(|ui| {
                    if ui
                        .add_enabled(
                            !self.symbol_name.trim().is_empty(),
                            egui::Button::new(self.i18n.text("action-create")),
                        )
                        .clicked()
                    {
                        let sampled = self.editor.selection.first().is_some_and(|s| {
                            self.editor.timeline().layers[s.layer]
                                .span_at(self.editor.frame)
                                .is_some_and(|(_, k, _)| k.tween_boundary.is_some())
                        });
                        self.editor
                            .convert_to_symbol(self.symbol_name.trim(), SymbolKind::Graphic);
                        self.notice_tween_pose_edit(sampled);
                        self.symbol_open = false;
                        self.props_tab = false;
                    }
                    if widgets::button(ui, self.i18n.text("action-cancel")).clicked() {
                        self.symbol_open = false;
                    }
                });
            });
        }
        if let Some(action) = self.pending {
            egui::Modal::new(egui::Id::new("studio.unsaved")).show(ctx, |ui| {
                ui.set_max_width(470.0);
                ui.heading(self.i18n.text("dialog-unsaved"));
                ui.label(self.i18n.text("dialog-unsaved-body"));
                ui.horizontal_wrapped(|ui| {
                    if widgets::button(ui, self.i18n.text("action-save")).clicked()
                        && self.save_document(false)
                    {
                        self.pending = None;
                        self.perform(action, ctx);
                    }
                    if widgets::button(ui, self.i18n.text("action-discard")).clicked() {
                        self.pending = None;
                        self.perform(action, ctx);
                    }
                    if widgets::button(ui, self.i18n.text("action-cancel")).clicked() {
                        self.pending = None;
                    }
                });
            });
        }
    }
    fn keyboard(&mut self, ctx: &egui::Context) {
        if self.dock_drag.is_some() {
            if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.dock_drag = None;
            }
            return;
        }
        if !self.preferences_open
            && !self.new_open
            && self.pending.is_none()
            && !self.symbol_open
            && !self.about_open
            && ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K))
        {
            self.command_open = !self.command_open;
            self.command_focus = true;
            self.command_query.clear();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.reset_timeline_gesture();
            if let Some(drag) = self.drag.take() {
                if drag.moved {
                    self.editor.undo();
                    self.texture = None;
                    self.sync_forms();
                }
            }
            self.playing = false;
            self.command_open = false;
        }
        if ctx.text_edit_focused()
            || self.pending.is_some()
            || self.command_open
            || self.new_open
            || self.about_open
            || self.preferences_open
            || self.symbol_open
            || self.drag.is_some()
            || egui::Popup::is_any_open(ctx)
        {
            return;
        }
        let command = ctx.input_mut(|i| {
            for (mods, key, command) in [
                (
                    egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                    egui::Key::C,
                    Command::CopyFrames,
                ),
                (
                    egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                    egui::Key::X,
                    Command::CutFrames,
                ),
                (
                    egui::Modifiers::COMMAND | egui::Modifiers::ALT,
                    egui::Key::V,
                    Command::PasteFrames,
                ),
                (
                    egui::Modifiers::ALT,
                    egui::Key::ArrowLeft,
                    Command::MoveFramesEarlier,
                ),
                (
                    egui::Modifiers::ALT,
                    egui::Key::ArrowRight,
                    Command::MoveFramesLater,
                ),
                (egui::Modifiers::COMMAND, egui::Key::N, Command::New),
                (egui::Modifiers::COMMAND, egui::Key::O, Command::Open),
                (
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::S,
                    Command::SaveAs,
                ),
                (egui::Modifiers::COMMAND, egui::Key::S, Command::Save),
                (
                    egui::Modifiers::COMMAND | egui::Modifiers::SHIFT,
                    egui::Key::Z,
                    Command::Redo,
                ),
                (egui::Modifiers::COMMAND, egui::Key::Z, Command::Undo),
                (egui::Modifiers::NONE, egui::Key::Delete, Command::Delete),
                (egui::Modifiers::SHIFT, egui::Key::F5, Command::RemoveFrame),
                (egui::Modifiers::NONE, egui::Key::F5, Command::InsertFrame),
                (
                    egui::Modifiers::SHIFT,
                    egui::Key::F6,
                    Command::ClearKeyframe,
                ),
                (egui::Modifiers::NONE, egui::Key::F6, Command::Keyframe),
                (egui::Modifiers::NONE, egui::Key::F7, Command::BlankKeyframe),
                (egui::Modifiers::NONE, egui::Key::Space, Command::Play),
            ] {
                if i.consume_key(mods, key) {
                    return Some(command);
                }
            }
            None
        });
        if let Some(command) = command {
            self.dispatch(command, ctx);
        }
        if self.timeline_state.focused {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::F2)) {
                self.begin_layer_rename(self.editor.layer);
            }
            let layers = self.layer_selection_active();
            let command = ctx.input_mut(|i| {
                for (key, command) in [
                    (
                        egui::Key::C,
                        if layers {
                            Command::CopyLayers
                        } else {
                            Command::CopyFrames
                        },
                    ),
                    (
                        egui::Key::X,
                        if layers {
                            Command::CutLayers
                        } else {
                            Command::CutFrames
                        },
                    ),
                    (
                        egui::Key::V,
                        if layers {
                            Command::PasteLayers
                        } else {
                            Command::PasteFrames
                        },
                    ),
                    (
                        egui::Key::A,
                        if layers {
                            Command::SelectAllLayers
                        } else {
                            Command::SelectAllFrames
                        },
                    ),
                ] {
                    if i.consume_key(egui::Modifiers::COMMAND, key) {
                        return Some(command);
                    }
                }
                None
            });
            if let Some(command) = command {
                self.dispatch(command, ctx);
            }
        }
        if self.timeline_state.focused {
            for (key, dl, df) in [
                (egui::Key::ArrowLeft, 0_i64, -1_i64),
                (egui::Key::ArrowRight, 0, 1),
                (egui::Key::ArrowUp, -1, 0),
                (egui::Key::ArrowDown, 1, 0),
            ] {
                let extend = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, key));
                let plain = !extend && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key));
                if extend || plain {
                    let anchor = self
                        .timeline_state
                        .anchor
                        .unwrap_or((self.editor.layer, self.editor.frame));
                    let s = self.editor.frame_target();
                    let current = if extend {
                        (
                            if s.layer0 < anchor.0 {
                                s.layer0
                            } else {
                                s.layer1
                            },
                            if s.f0 < anchor.1 { s.f0 } else { s.f1 },
                        )
                    } else {
                        (self.editor.layer, self.editor.frame)
                    };
                    let target = (
                        (current.0 as i64 + dl)
                            .clamp(0, self.editor.timeline().layers.len() as i64 - 1)
                            as usize,
                        (current.1 as i64 + df).clamp(0, document::MAX_FRAMES as i64 - 1) as u32,
                    );
                    if extend {
                        self.select_frame_range(anchor, target);
                    } else {
                        self.select_timeline_cell(target.0, target.1);
                    }
                }
            }
        }
        for (key, tool) in [
            (egui::Key::V, Tool::Select),
            (egui::Key::R, Tool::Rectangle),
            (egui::Key::O, Tool::Ellipse),
            (egui::Key::H, Tool::Hand),
            (egui::Key::Z, Tool::Zoom),
        ] {
            if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, key)) {
                self.tool = tool;
            }
        }
    }
}

impl eframe::App for Studio {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.dirty() {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.pending = Some(Pending::Quit);
            self.playing = false;
        }
        if !ctx.input(|i| i.focused) {
            self.dock_drag = None;
            self.reset_timeline_gesture();
            if let Some(drag) = self.drag.take() {
                if drag.moved {
                    self.editor.undo();
                    self.texture = None;
                    self.sync_forms();
                }
            }
        }
        self.keyboard(ctx);
        let now = Instant::now();
        let dt = now.duration_since(self.last_time).as_secs_f32().min(0.25);
        self.last_time = now;
        if self.playing && self.pending.is_none() {
            self.frame_fraction += dt * self.editor.doc.fps;
            let count = self.editor.timeline().frame_count().max(1);
            let steps = self.frame_fraction.floor() as u32;
            self.frame_fraction -= steps as f32;
            self.editor.frame = (self.editor.frame + steps) % count;
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
        let title = format!(
            "{}{} — Flare",
            self.title(),
            if self.dirty() { " *" } else { "" }
        );
        if title != self.window_title {
            self.window_title = title.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let window_rect = ui.max_rect();
        if !self.editor.selection.is_empty() && !self.editable_selection() {
            self.finish_stage_gesture();
        }
        self.editor.selection = self
            .editor
            .selection
            .iter()
            .copied()
            .filter(|s| self.timeline_editable(s.layer) && self.editor.element_at(*s).is_some())
            .collect();
        self.header(ui);
        if self.prefs.options_bar {
            self.options_bar(ui);
        }
        if self.prefs.status_bar || self.error || !self.status.is_empty() {
            self.status_bar(ui);
        }
        let available = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(available, 0.0, self.prefs.theme.palette().panel);
        let workspace = Rect::from_min_max(
            available.min + vec2(4.0, 0.0),
            available.max - vec2(0.8, 4.8),
        );
        ui.scope_builder(egui::UiBuilder::new().max_rect(workspace), |ui| {
            self.tools(ui);
            self.inspector(ui);
            self.panel_strip(ui);
            self.timeline(ui);
            self.stage(ui);
        });
        let p = self.prefs.theme.palette();
        theme::edge_bands(
            ui.painter(),
            window_rect,
            true,
            false,
            &[p.line.r(), p.panel.r()],
        );
        theme::edge_bands(ui.painter(), window_rect, true, true, &[p.line.r()]);
        theme::edge_bands(
            ui.painter(),
            window_rect,
            false,
            true,
            &[p.line.r(), p.panel.r()],
        );
        self.workspace_drop_targets(ui, window_rect);
        self.window_resize_edges(ui, window_rect);
        self.dialogs(ui.ctx());
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, "flare.studio.preferences.v1", &self.prefs);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{kittest::Queryable, Harness};

    #[test]
    fn shared_controls_keep_geometry_on_hover_and_keyboard_focus() {
        use egui_kittest::kittest::NodeT;

        // Tooltips repeat the control label in a separate, non-interactive node.
        let rect_for = |h: &Harness<Studio>, label: &str| {
            let mut controls = h
                .query_all_by_label(label)
                .filter(|node| node.accesskit_node().role() != egui::accesskit::Role::Label);
            let rect = controls.next().unwrap().rect();
            assert!(controls.next().is_none(), "Ambiguous control: {label}");
            rect
        };
        for theme in [theme::Theme::Dark, theme::Theme::Light] {
            let mut h = Harness::builder()
                .with_size(vec2(1440.0, 920.0))
                .build_eframe(|cc| {
                    Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale: Locale::English,
                            theme,
                            ..Default::default()
                        },
                    )
                });
            h.run();
            let labels = [
                "File",
                "Rectangle",
                "Rulers",
                "Apply",
                "Rename selected layer",
                "Create document",
            ];
            let rects: Vec<_> = labels.iter().map(|label| rect_for(&h, label)).collect();
            let tabs: Vec<_> = h
                .query_all_by_label("Properties")
                .map(|node| node.rect())
                .collect();
            for (label, rect) in labels.iter().zip(&rects) {
                h.hover_at(rect.center());
                for _ in 0..3 {
                    h.step();
                    assert_eq!(
                        h.query_all_by_label("Properties")
                            .map(|node| node.rect())
                            .collect::<Vec<_>>(),
                        tabs
                    );
                    for (other, expected) in labels.iter().zip(&rects) {
                        assert_eq!(
                            rect_for(&h, other),
                            *expected,
                            "{other} moved when hovering {label}"
                        );
                    }
                }
            }
            for _ in 0..8 {
                h.key_press(egui::Key::Tab);
                h.run();
                for (label, expected) in labels.iter().zip(&rects) {
                    assert_eq!(
                        rect_for(&h, label),
                        *expected,
                        "{label} moved on keyboard focus"
                    );
                }
            }
            assert!(!h.state().dirty());
        }
    }

    #[test]
    fn new_document_hover_and_selection_keep_geometry_stable() {
        for locale in [Locale::English, Locale::French, Locale::Expanded] {
            for theme in [theme::Theme::Dark, theme::Theme::Light] {
                for scale in [1.0, 1.25] {
                    let l = Localizer::new(locale);
                    let mut h = Harness::builder()
                        .with_size(vec2(1440.0, 920.0))
                        .build_eframe(|cc| {
                            cc.egui_ctx.set_zoom_factor(scale);
                            let mut app = Studio::with_preferences(
                                &cc.egui_ctx,
                                Preferences {
                                    locale,
                                    theme,
                                    ..Default::default()
                                },
                            );
                            app.new_open = true;
                            app
                        });
                    h.run();
                    let keys = [
                        "preset-classic",
                        "preset-hd",
                        "preset-square",
                        "action-create",
                        "action-cancel",
                    ];
                    let bounds = h
                        .ctx
                        .memory(|m| m.area_rect(egui::Id::new("studio.new")))
                        .unwrap();
                    // French Undo and Cancel share a label; scope queries to the modal bounds.
                    let rect_for = |h: &Harness<Studio>, key: &str| {
                        h.query_all_by_label(&l.text(key))
                            .find(|node| bounds.contains(node.rect().center()))
                            .unwrap()
                            .rect()
                    };
                    let baseline: Vec<_> = keys.iter().map(|key| rect_for(&h, key)).collect();
                    for key in keys {
                        h.hover_at(rect_for(&h, key).center());
                        // Check intermediate frames too: settling back after a jump is still a bug.
                        for _ in 0..3 {
                            h.step();
                            assert_eq!(
                                h.ctx
                                    .memory(|m| m.area_rect(egui::Id::new("studio.new")))
                                    .unwrap(),
                                bounds,
                                "dialog moved on hover: {key}, {locale:?}, {theme:?}, {scale}"
                            );
                            for (key, rect) in keys.iter().zip(&baseline) {
                                assert_eq!(
                                    rect_for(&h, key),
                                    *rect,
                                    "control moved on hover: {key}"
                                );
                            }
                        }
                    }
                    for key in ["preset-hd", "preset-square", "preset-classic"] {
                        h.get_by_label(&l.text(key)).click();
                        h.run();
                        assert_eq!(
                            h.ctx
                                .memory(|m| m.area_rect(egui::Id::new("studio.new")))
                                .unwrap(),
                            bounds,
                            "preset selection resized the dialog"
                        );
                        for (key, rect) in keys.iter().zip(&baseline) {
                            assert_eq!(
                                rect_for(&h, key),
                                *rect,
                                "control moved after selecting a preset: {key}"
                            );
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn future_frame_ui_extends_saves_and_undoes_in_both_languages() {
        for locale in [Locale::English, Locale::French] {
            let l = Localizer::new(locale);
            let mut h = Harness::builder()
                .with_size(vec2(1600.0, 900.0))
                .build_eframe(|cc| {
                    Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale,
                            ..Default::default()
                        },
                    )
                });
            h.run();
            assert_eq!(h.state().editor.timeline().frame_count(), 1);
            let name = h.state().editor.timeline().layers[0].name.clone();
            h.get_by_label(&l.format("timeline-cell", &[("layer", &name), ("frame", "90")]))
                .click();
            h.run();
            assert_eq!(h.state().editor.frame, 89);
            assert!(
                !h.state().dirty(),
                "selecting future cells is a view operation"
            );
            h.key_press(egui::Key::F5);
            h.run();
            assert_eq!(h.state().editor.timeline().frame_count(), 90);
            assert_eq!(h.state().editor.timeline().layers[0].keyframes.len(), 1);
            let temp = tempfile::tempdir().unwrap();
            let path = temp.path().join("timeline.json");
            document::save(&h.state().editor.doc, &path).unwrap();
            assert_eq!(document::open(&path).unwrap(), h.state().editor.doc);
            for pressed in [true, false] {
                h.event(egui::Event::Key {
                    key: egui::Key::Z,
                    physical_key: None,
                    pressed,
                    repeat: false,
                    modifiers: egui::Modifiers::COMMAND,
                });
                h.run();
            }
            assert_eq!(h.state().editor.timeline().frame_count(), 1);
            let start = h
                .get_by_label(&l.format("timeline-cell", &[("layer", &name), ("frame", "2")]))
                .rect()
                .center();
            let end = h
                .get_by_label(&l.format("timeline-cell", &[("layer", &name), ("frame", "40")]))
                .rect()
                .center();
            h.hover_at(start);
            h.run();
            h.event(egui::Event::PointerButton {
                pos: start,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
            h.hover_at(end);
            h.run();
            h.event(egui::Event::PointerButton {
                pos: end,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            h.run();
            assert_eq!(
                h.state().editor.frame_sel,
                Some(ac_core::FrameSel {
                    layer0: 0,
                    layer1: 0,
                    f0: 1,
                    f1: 39,
                }),
                "drag selects the complete range under the pointer"
            );
            assert_eq!(h.state().editor.timeline().frame_count(), 1);
        }
    }

    #[test]
    fn frame_lifecycle_preserves_content_and_shifts_later_keys() {
        let ctx = egui::Context::default();
        let mut app = Studio::with_preferences(&ctx, Preferences::default());
        app.editor.add_element(Element::shape(
            "box",
            Shape::new(
                Path::rect(0.0, 0.0, 20.0, 20.0),
                Some(Color::rgb(255, 0, 0)),
                None,
            ),
        ));
        app.select_timeline_cell(0, 9);
        app.dispatch(Command::Keyframe, &ctx);
        assert_eq!(
            app.editor.timeline().layers[0].keyframes[&9].elements.len(),
            1
        );
        app.select_timeline_cell(0, 4);
        app.dispatch(Command::InsertFrame, &ctx);
        assert!(app.editor.timeline().layers[0].keyframes.contains_key(&10));
        app.dispatch(Command::RemoveFrame, &ctx);
        assert!(app.editor.timeline().layers[0].keyframes.contains_key(&9));
        app.select_timeline_cell(0, 9);
        app.dispatch(Command::ClearKeyframe, &ctx);
        assert_eq!(app.editor.timeline().layers[0].end, 10);
        assert!(!app.editor.timeline().layers[0].keyframes.contains_key(&9));
        assert_eq!(app.editor.evaluate_current(9).len(), 1);
        app.dispatch(Command::Undo, &ctx);
        assert!(app.editor.timeline().layers[0].keyframes.contains_key(&9));
        document::validate(&app.editor.doc).unwrap();
    }

    #[test]
    fn frame_commands_respect_locks_limits_and_symbol_context() {
        let ctx = egui::Context::default();
        let mut app = Studio::with_preferences(&ctx, Preferences::default());
        app.editor.timeline_mut().layers[0].locked = true;
        let before = app.editor.doc.clone();
        for command in [
            Command::InsertFrame,
            Command::RemoveFrame,
            Command::Keyframe,
            Command::BlankKeyframe,
            Command::ClearKeyframe,
        ] {
            app.dispatch(command, &ctx);
        }
        assert_eq!(app.editor.doc, before);
        app.editor.timeline_mut().layers[0].locked = false;
        app.editor.timeline_mut().layers[0].end = document::MAX_FRAMES;
        assert!(!app.enabled(Command::InsertFrame));
        app.editor.frame = document::MAX_FRAMES;
        assert!(!app.enabled(Command::Keyframe));
        assert!(!app.enabled(Command::BlankKeyframe));
        app.replace(document::example(), None, true);
        let scene = app.editor.doc.scene.clone();
        let id = app.editor.doc.library[0].id;
        app.editor.enter_symbol(id);
        app.select_timeline_cell(0, 99);
        app.dispatch(Command::InsertFrame, &ctx);
        assert_eq!(app.editor.timeline().frame_count(), 100);
        assert_eq!(app.editor.doc.scene, scene);
        document::validate(&app.editor.doc).unwrap();
    }

    #[test]
    fn localized_command_search_dispatches_and_respects_disabled_actions() {
        for locale in [Locale::English, Locale::French] {
            let l = Localizer::new(locale);
            let mut h = Harness::builder()
                .with_size(vec2(1200.0, 800.0))
                .build_eframe(|cc| {
                    Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale,
                            ..Default::default()
                        },
                    )
                });
            h.run();
            for query in [l.text("cmd-layer"), l.text("cmd-delete")] {
                for pressed in [true, false] {
                    h.event(egui::Event::Key {
                        key: egui::Key::K,
                        physical_key: None,
                        pressed,
                        repeat: false,
                        modifiers: egui::Modifiers::COMMAND,
                    });
                    h.run();
                }
                assert!(h.state().command_open);
                h.event(egui::Event::Text(query.clone()));
                h.run();
                assert_eq!(h.state().command_query, query);
                h.key_press(egui::Key::Enter);
                h.run();
                assert_eq!(h.state().editor.timeline().layers.len(), 2);
            }
            assert!(
                h.state().command_open,
                "disabled Delete must not dispatch or close search"
            );
            h.key_press(egui::Key::Escape);
            h.run();
            assert!(!h.state().command_open);
            assert_eq!(
                h.state().tool,
                Tool::Select,
                "typing in search must not change tools"
            );
        }
    }

    #[test]
    fn stage_scrollbars_move_view_without_editing_document() {
        let mut h = Harness::builder()
            .with_size(vec2(1440.0, 920.0))
            .build_eframe(|cc| {
                Studio::with_preferences(
                    &cc.egui_ctx,
                    Preferences {
                        locale: Locale::English,
                        theme: theme::Theme::Light,
                        ..Default::default()
                    },
                )
            });
        h.run();
        let revision = h.state().editor.revision;
        h.get_by_label("Scroll view right").click();
        h.run();
        assert!(h.state().pan.x < 0.0);
        assert_eq!(h.state().pan.y, 0.0);
        assert_eq!(h.state().editor.revision, revision);
        h.get_by_label("Scroll view down").click();
        h.run();
        assert!(h.state().pan.y < 0.0);
        assert_eq!(h.state().editor.revision, revision);
    }

    #[test]
    fn zoom_and_rulers_change_view_without_editing_document() {
        let mut h = Harness::builder()
            .with_size(vec2(1200.0, 800.0))
            .build_eframe(|cc| {
                Studio::with_preferences(
                    &cc.egui_ctx,
                    Preferences {
                        locale: Locale::English,
                        ..Default::default()
                    },
                )
            });
        h.run();
        let doc = h.state().editor.doc.clone();
        let rulers_before = h.state().prefs.rulers;
        h.get_by_label("Rulers").click();
        h.run();
        assert_ne!(h.state().prefs.rulers, rulers_before);
        h.get_by_label("Stage zoom").click();
        h.run();
        h.get_by_label(&Localizer::new(Locale::English).format("zoom-value", &[("value", "200")]))
            .click();
        h.run();
        assert_eq!(h.state().zoom, 2.0);
        assert!(!h.state().fit);
        assert_eq!(h.state().editor.doc, doc);
        assert!(!h.state().dirty());
    }

    #[test]
    fn compact_workspace_keeps_optional_chrome_reachable_and_persisted() {
        for locale in [Locale::English, Locale::French] {
            let l = Localizer::new(locale);
            let mut h = Harness::builder()
                .with_size(vec2(1200.0, 800.0))
                .build_eframe(|cc| {
                    Studio::with_preferences(
                        &cc.egui_ctx,
                        Preferences {
                            locale,
                            ..Default::default()
                        },
                    )
                });
            h.run();
            let before = h.state().editor.doc.clone();
            assert!(h.query_by_label(&l.text("action-search")).is_none());
            for label in ["panel-options", "panel-status"] {
                h.get_by_label(&l.text("menu-window")).click();
                h.run();
                h.get_by_label(&l.text(label)).click();
                h.key_press(egui::Key::Escape);
                h.run();
            }
            assert!(h.query_by_label(&l.text("action-search")).is_some());
            assert!(h.query_by_label(&l.text("status-ready")).is_some());
            let json = serde_json::to_string(&h.state().prefs).unwrap();
            let restored: Preferences = serde_json::from_str(&json).unwrap();
            assert!(restored.options_bar && restored.status_bar);
            assert_eq!(h.state().editor.doc, before);
        }
    }

    #[test]
    fn open_menu_tracks_hover_until_dismissed() {
        for locale in [Locale::English, Locale::French] {
            for theme in [theme::Theme::Dark, theme::Theme::Light] {
                let l = Localizer::new(locale);
                let mut h = Harness::builder()
                    .with_size(vec2(1200.0, 800.0))
                    .build_eframe(|cc| {
                        Studio::with_preferences(
                            &cc.egui_ctx,
                            Preferences {
                                locale,
                                theme,
                                ..Default::default()
                            },
                        )
                    });
                h.run();
                h.get_by_label(&l.text("menu-file")).hover();
                h.run();
                assert!(h
                    .query_by_label(&format!("{} Ctrl+N", l.text("cmd-new")))
                    .is_none());
                h.get_by_label(&l.text("menu-file")).click();
                h.run();
                let outside = pos2(600.0, 400.0);
                h.hover_at(outside);
                h.run();
                for pressed in [true, false] {
                    h.event(egui::Event::PointerButton {
                        pos: outside,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: egui::Modifiers::NONE,
                    });
                    h.run();
                }
                h.get_by_label(&l.text("menu-view")).hover();
                h.run();
                assert!(h.query_by_label(&l.text("cmd-fit")).is_none());
                h.get_by_label(&l.text("menu-file")).click();
                h.run();
                h.get_by_label(&format!("{} Ctrl+N", l.text("cmd-new")));
                h.get_by_label(&l.text("menu-view")).hover();
                h.run();
                h.get_by_label(&l.text("cmd-fit"));
                assert!(h
                    .query_by_label(&format!("{} Ctrl+N", l.text("cmd-new")))
                    .is_none());
                h.get_by_label(&l.text("menu-file")).hover();
                h.run();
                h.get_by_label(&format!("{} Ctrl+N", l.text("cmd-new")));
                assert!(h.query_by_label(&l.text("cmd-fit")).is_none());
                h.key_press(egui::Key::Escape);
                h.run();
                h.get_by_label(&l.text("menu-view")).hover();
                h.run();
                assert!(h.query_by_label(&l.text("cmd-fit")).is_none());
                h.get_by_label(&l.text("menu-file")).click();
                h.run();
                h.get_by_label(&l.text("menu-view")).hover();
                h.run();
                h.get_by_label(&l.text("cmd-fit")).click();
                h.run();
                assert!(h.state().fit);
                h.get_by_label(&l.text("menu-file")).hover();
                h.run();
                assert!(h
                    .query_by_label(&format!("{} Ctrl+N", l.text("cmd-new")))
                    .is_none());
            }
        }
    }

    #[test]
    fn new_dialog_creates_selected_preset_through_ui() {
        let mut h = Harness::builder()
            .with_size(vec2(1200.0, 800.0))
            .build_eframe(|cc| {
                Studio::with_preferences(
                    &cc.egui_ctx,
                    Preferences {
                        locale: Locale::English,
                        ..Default::default()
                    },
                )
            });
        h.run();
        h.get_by_label("File").click();
        h.run();
        h.get_by_label("New document Ctrl+N").click();
        h.run();
        assert!(h.state().new_open);
        h.get_by_label("HD · 1280 × 720").click();
        h.run();
        h.get_by_label("Create").click();
        h.run();
        assert_eq!(
            (h.state().editor.doc.width, h.state().editor.doc.height),
            (1280.0, 720.0)
        );
        assert!(!h.state().new_open);
        assert!(!h.state().dirty());
    }

    #[test]
    fn live_theme_switch_preserves_artwork_and_persists() {
        let mut h = Harness::builder()
            .with_size(vec2(1200.0, 800.0))
            .build_eframe(|cc| {
                let mut app = Studio::with_preferences(
                    &cc.egui_ctx,
                    Preferences {
                        locale: Locale::English,
                        ..Default::default()
                    },
                );
                app.dispatch(Command::Demo, &cc.egui_ctx);
                app.preferences_open = true;
                app
            });
        h.run();
        let doc = h.state().editor.doc.clone();
        let revision = h.state().editor.revision;
        h.get_by_label("Light").click();
        h.run();
        assert_eq!(h.state().prefs.theme, theme::Theme::Light);
        assert_eq!(h.state().editor.doc, doc);
        assert_eq!(h.state().editor.revision, revision);
        let prefs: Preferences =
            serde_json::from_str(&serde_json::to_string(&h.state().prefs).unwrap()).unwrap();
        assert_eq!(prefs.theme, theme::Theme::Light);
        let old: Preferences =
            serde_json::from_str(r#"{"locale":"English","inspector":true,"timeline":true}"#)
                .unwrap();
        assert_eq!(old.theme, theme::Theme::Dark);
    }
    #[test]
    fn language_switch_does_not_change_document_or_history() {
        let ctx = egui::Context::default();
        let mut app = Studio::with_preferences(&ctx, Preferences::default());
        app.dispatch(Command::Demo, &ctx);
        let doc = app.editor.doc.clone();
        let depth = app.editor.undo_depth();
        for locale in Locale::ALL {
            app.set_locale(locale);
            assert_eq!(app.editor.doc, doc);
            assert_eq!(app.editor.undo_depth(), depth);
        }
    }
    #[test]
    fn unsaved_document_is_not_replaced_without_a_decision() {
        let ctx = egui::Context::default();
        let mut app = Studio::with_preferences(&ctx, Preferences::default());
        app.dispatch(Command::AddLayer, &ctx);
        let doc = app.editor.doc.clone();
        app.dispatch(Command::New, &ctx);
        assert!(app.pending.is_some());
        assert_eq!(app.editor.doc, doc);
        app.pending = None; // Cancel path.
        assert!(app.dirty());
        assert_eq!(app.editor.doc, doc);
    }
    #[test]
    fn rendered_symbol_frames_differ_and_still_render_after_edits() {
        let ctx = egui::Context::default();
        let mut app = Studio::with_preferences(&ctx, Preferences::default());
        app.dispatch(Command::Demo, &ctx);
        let first = app.render_pixels(0.25).unwrap();
        app.editor.frame = 24;
        let second = app.render_pixels(0.25).unwrap();
        assert_ne!(first.data(), second.data());
        app.dispatch(Command::Keyframe, &ctx);
        app.dispatch(Command::Undo, &ctx);
        assert!(app.render_pixels(0.25).is_ok());
    }
}
