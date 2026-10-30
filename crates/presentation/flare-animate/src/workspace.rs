//! The workspace: the open document, the tool in hand and everything on screen.
//!
//! The type lives here; what it can do is split by topic into the modules below.

use crate::layout::{self, Pane};
use egui::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_editor::frames;
use flare_history::EditorSession;
use flare_runtime::{Mode, Runtime};
use flare_tools::ops::{self, SpanRef};
use flare_tools::pen::PenPath;
use flare_ui::view::StageView;
use std::path::PathBuf;

use flare_input::InputSample;

mod anchors;
mod arrange;
mod assist;
mod background_work;
mod command_labels;
mod commands;
mod core;
mod dock;
mod draw;
mod files;
mod furniture;
mod gestures;
mod groups;
mod guides;
mod help;
mod helpers;
mod home;
mod layers;
mod open_any;
mod outlines;
mod panels;
mod pen_tool;
mod playback;
mod preferences;
mod render;
mod selection;
mod shell;
mod sound;
mod stage;
mod stroke_preview;
mod symbols;
mod tabs;
#[cfg(test)]
mod tests;
mod text;
mod timeline;
mod types;

use dock::*;
use helpers::*;
use types::*;
pub(crate) const ANCHOR_INK: egui::Color32 = egui::Color32::from_rgb(40, 120, 255);
pub use flare_audio::AudioEvent;
pub use open_any::{IMPORT_EXTENSIONS, PROJECT_EXTENSIONS, PathKind, classify};
pub use preferences::{MAX_PREFERENCES_BYTES, Preferences};
pub use types::{
    DialogRequest, FrameOp, MAX_ONION_FRAMES, OnionSkin, Tool, WindowRequest, onion_ghosts,
};

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
    /// The other documents of the window, whole, in tab order without the current one.
    parked: Vec<Workspace>,
    /// This document's place among the tabs.
    position: usize,
    publish_compress: bool,
    /// Projects opened or saved lately, newest first.
    recent: Vec<PathBuf>,
    /// Folders shown shut in the timeline, hiding the layers inside.
    collapsed_folders: std::collections::HashSet<LayerId>,
    /// The picture of the selected library symbol: which symbol, at which revision of the
    /// project, and the texture.
    library_preview: Option<(SymbolId, u64, egui::TextureHandle)>,
    /// The import being read, if any.
    importing: Option<u64>,
    reverting: Option<u64>,
    revert_pending: bool,
    /// How many frames have been rendered (as opposed to taken from the cache).
    frames_rendered: u64,
    drag: Option<Drag>,
    anchor_drag: Option<anchors::AnchorDrag>,
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
    /// Corner radius of new rectangles, in stage units.
    rect_radius: f32,
    poly_sides: u32,
    poly_star: bool,
    poly_inner: f32,
    /// The loop being drawn with the lasso, in stage coordinates.
    lasso: Vec<Pos2>,
    /// The points of the pencil or brush stroke being drawn, in stage coordinates.
    freehand: Vec<InputSample>,
    brush_size: f32,
    /// How thin a brush stroke gets at the lightest touch, as a fraction of its size.
    brush_light: f32,
    /// How far a smoothed line may stray from the drag, in screen pixels.
    smoothing: f32,
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
    /// What the next symbol made from a selection, or added to the library, is.
    new_symbol_kind: flare_document::SymbolKind,
    library_search: String,
    /// The symbol being renamed in the library and the text so far.
    renaming: Option<(SymbolId, String)>,
    current_layer: Option<LayerId>,
    layer_name_edit: String,
    /// A native menu bar owns the menu and its shortcuts, so the in-window ones step aside.
    native_menu: bool,
    quit_requested: bool,
    /// The dockable panel arrangement.
    layout: egui_tiles::Tree<Pane>,
    onion: OnionSkin,
    test_movie_requested: bool,
    /// The pen tool's path while it is being drawn.
    pen: PenPath,
    /// Stage size being edited in the tools panel, applied when the edit finishes.
    stage_edit: Option<(u32, u32)>,
    fps_edit: Option<f64>,
    /// Playback wraps to the first frame; off, it stops on the last.
    loop_playback: bool,
    muted: bool,
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
    /// The Text tool's settings and the text being typed.
    text: groups::TextTool,
    /// How new shapes are painted.
    paint: groups::Paint,
    /// What the stage view shows besides the artwork.
    display: groups::Display,
    /// Which small windows are open.
    windows: groups::OpenWindows,
    /// Autosave and crash recovery.
    recovery: groups::Recovery,
    /// Sounds being decoded, imported and played.
    audio: groups::AudioState,
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
            parked: Vec::new(),
            position: 0,
            publish_compress: true,
            recent: Vec::new(),
            collapsed_folders: std::collections::HashSet::new(),
            library_preview: None,
            importing: None,
            reverting: None,
            revert_pending: false,
            frames_rendered: 0,
            drag: None,
            anchor_drag: None,
            drag_cancelled: false,
            tool: Tool::Rectangle,
            selected: None,
            color_target: ColorTarget::Fill,
            align_to_stage: false,
            transform_edit: TransformEdit::default(),
            hex_buffer: None,
            pointer_stage: None,
            chrome_ready: false,
            rect_radius: 0.0,
            poly_sides: 5,
            poly_star: false,
            poly_inner: 0.5,
            lasso: Vec::new(),
            freehand: Vec::new(),
            brush_size: 12.0,
            brush_light: 0.15,
            smoothing: 2.0,
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
            new_symbol_kind: flare_document::SymbolKind::Graphic,
            library_search: String::new(),
            renaming: None,
            current_layer: None,
            layer_name_edit: String::new(),
            native_menu: false,
            quit_requested: false,
            layout: layout::default_tree(),
            onion: OnionSkin {
                enabled: false,
                before: 1,
                after: 1,
                outlines: false,
            },
            test_movie_requested: false,
            pen: PenPath::default(),
            stage_edit: None,
            fps_edit: None,
            loop_playback: true,
            muted: false,
            log: Vec::new(),
            props_tab: PropsTab::Tool,
            object_edit: None,
            status: None,
            dialog: None,
            window_request: None,
            titlebar_inset: 0.0,
            top_bar_id: egui::Id::unique("top-bar"),
            text: groups::TextTool::default(),
            paint: groups::Paint::default(),
            display: groups::Display::default(),
            windows: groups::OpenWindows::default(),
            recovery: groups::Recovery::default(),
            audio: groups::AudioState::default(),
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
}
