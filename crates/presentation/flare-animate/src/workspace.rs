//! The workspace: the open document, the tool in hand and everything on screen.
//!
//! The type lives here; what it can do is split by topic into the modules below.

use egui::{Pos2, Rect};
use flare_commands::Command;
use flare_core::{DocumentId, FrameRate, LayerId, SceneId, SymbolId};
use flare_dock::{self, Pane};
use flare_document::{Document, Layer, Scene, Timeline};
use flare_editor::frames;
use flare_history::EditorSession;
use flare_runtime::{Mode, Runtime};
use flare_tools::ops::{self, SpanRef};
use flare_tools::pen::PenPath;
use flare_ui::view::StageView;
use std::path::PathBuf;

use flare_input::{InputSample, PenFeed};

mod arrange;
mod assist;
mod background_work;
mod chords;
mod close_prompt;
mod command_labels;
mod commands;
mod core;
mod dock;
mod drag_preview;
mod draw;
mod files;
mod frame_ranges;
mod furniture;
mod gestures;
mod graphic_playback;
mod groups;
mod guides;
mod help;
mod helpers;
mod home;
mod home_custom;
mod home_list;
mod language;
mod layer_names;
mod layers;
mod open_any;
mod outlines;
mod panels;
mod pen_overlay;
mod place;
mod playback;
mod preferences;
mod props_types;
mod publish_settings;
mod publish_window;
mod rail;
mod render;
mod scene_name;
mod selection;
mod shell;
mod shortcut_editor;
mod sound;
mod splash;
mod stage;
mod stage_furniture;
mod stage_menu;
mod symbols;
mod tabs;
mod tools;
mod transform_cursor;
mod window_controls;
use tools::{anchors, stroke_preview, text_preview};
mod new_document;
mod new_document_presets;
#[cfg(test)]
mod tests;
mod timeline;
mod timeline_cells;
mod timeline_labels;
mod timeline_rows;
mod timeline_sound;
mod tool_kind;
mod top_bar_classic;
mod types;

use dock::*;
use helpers::*;
use types::*;
pub(crate) const ANCHOR_INK: egui::Color32 = egui::Color32::from_rgb(40, 120, 255);
pub use flare_audio::AudioEvent;
pub use flare_tools::brush::BrushPreset;
pub use preferences::{MAX_PREFERENCES_BYTES, Preferences};
pub use publish_settings::PublishSettings;
pub use types::{
    DialogRequest, FrameOp, MAX_ONION_FRAMES, OnionSkin, ShareTarget, Tool, WindowRequest,
    onion_ghosts,
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
    publish: PublishSettings,
    /// The Home page's state: the recent projects and how they are listed.
    home: home::HomeState,
    /// Folders shown shut in the timeline, hiding the layers inside.
    collapsed_folders: std::collections::HashSet<LayerId>,
    /// The library panel's state.
    library: symbols::LibraryState,
    /// The import being read, if any.
    importing: Option<u64>,
    reverting: Option<u64>,
    revert_pending: bool,
    /// How many frames have been rendered (as opposed to taken from the cache).
    frames_rendered: u64,
    drag: Option<Drag>,
    /// What the stage is drawn from while a move or transform drag is held.
    drag_preview: Option<drag_preview::DragPreview>,
    /// The Motion Editor panel's state.
    motion: panels::MotionEditor,
    anchor_drag: Option<anchors::AnchorDrag>,
    /// A bone being drawn, or an armature being posed.
    bone_drag: Option<tools::bone::BoneDrag>,
    /// The joint the Bone or Selection tool last touched, which Properties edits.
    bone_joint: Option<flare_tools::bones::JointRef>,
    /// The point of a shape the Subselection tool has chosen, with the revision of the document it
    /// was chosen in: Delete takes it out, unless the document has changed since.
    selected_anchor: Option<(SpanRef, usize, u64)>,
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
    /// The stage shows what the scene's camera sees, and cannot be edited while it does.
    camera_preview: bool,
    /// The theme and icon loaders have been installed on the egui context.
    chrome_ready: bool,
    /// Corner radius of new rectangles, in stage units.
    rect_radius: f32,
    poly_sides: u32,
    poly_star: bool,
    poly_inner: f32,
    /// The loop being drawn with the lasso, in stage coordinates.
    lasso: Vec<Pos2>,
    /// The Magic Wand's settings and the pixels it has picked.
    wand: tools::magic_wand::WandTool,
    /// The Rotation tool's drag, its dial and the Alt tap that opens it.
    rotate: tools::rotate_view::RotateView,
    /// The points of the pencil or brush stroke being drawn, in stage coordinates.
    freehand: Vec<InputSample>,
    /// The Brush's stabilizer for the stroke being drawn, if it has one.
    stabilizer: Option<flare_input::Stabilizer>,
    /// A pen's pressure from a native source, for windowing that passes the pen as a mouse.
    pen_feed: PenFeed,
    brush_size: f32,
    /// How wide the Eraser rubs, in stage units, and whether it deletes whole shapes instead.
    eraser_size: f32,
    eraser_faucet: bool,
    /// How far the Eraser has rubbed out along the drag in progress, when it rubs as it goes.
    live_erase: Option<tools::eraser::LiveErase>,
    /// The tip the brush paints with, and the nibs kept under a name.
    brush_shelf: flare_tools::brush::BrushShelf,
    /// How thin a brush stroke gets at the lightest touch, as a fraction of its size.
    brush_light: f32,
    /// How quickly brush width grows with pressure.
    brush_pressure_gamma: f32,
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
    /// Shift is held: corner scaling keeps the proportions, rotation goes in steps, and a line
    /// keeps to the nearest 45 degrees.
    uniform_scale: bool,
    /// Alt is held: an edge handle skews instead of scaling.
    skew_modifier: bool,
    /// What Copy remembered, for Paste.
    clipboard: Vec<ops::ClipItem>,
    /// Timeline zoom: 1.0 fits the whole scene in the panel.
    timeline_zoom: f32,
    /// Where the timeline's parts were drawn on the last pass, for pointing at a frame.
    timeline_layout: Option<timeline::Layout>,
    /// The end of the onion-skin range being dragged on the ruler, if one is.
    onion_drag: Option<timeline::OnionEnd>,
    /// The next timeline draw sets the zoom at which the scene fills the panel (the Fit button).
    timeline_fit: bool,
    /// Spans selected along with `selected` (Shift-click). Move and Delete act on all of them.
    also_selected: Vec<SpanRef>,
    stage_view: StageView,
    /// The stage panel's last rectangle, so menu zoom can aim at its center.
    stage_panel: Rect,
    span_drag: Option<SpanDrag>,
    current_layer: Option<LayerId>,
    /// The layer being renamed in the timeline and the text so far.
    layer_renaming: Option<(LayerId, String)>,
    layer_rename_serial: u32,
    /// The scene's name while it is being edited in the bar above the stage.
    scene_renaming: Option<String>,
    scene_rename_serial: u32,
    /// The layer a right click in the timeline's layer column opened a menu for.
    layer_menu_for: Option<LayerId>,
    /// Whether the menu a right click of a frame opens is the one for frames, not for a layer.
    frame_menu_open: bool,
    /// A native menu bar owns the menu and its shortcuts, so the in-window ones step aside.
    native_menu: bool,
    quit_requested: bool,
    /// Where the new-document button of the tab strip was drawn last.
    tab_plus: egui::Rect,
    /// The tab a close was asked for while it held unsaved work: the prompt is open for it.
    close_prompt: Option<usize>,
    /// The document on show is closed once the save asked for by the prompt has been written.
    close_after_save: bool,
    /// The panel arrangement, the panel open beside the strip, and changes asked for while drawn.
    panels: rail::Panels,
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
    /// A context a command asked the Properties panel for (the document's settings), until the
    /// selection or the tool changes.
    props_forced: Option<(PropsContext, PropsKey)>,
    /// The paint of the selected shape being changed in the Properties panel.
    shape_paint_edit: Option<ShapePaintEdit>,
    /// Position and size typed into the Object tab: x, y, width, height.
    /// The first frame being typed or dragged in the Looping section, until it is let go.
    playback_first_edit: Option<u32>,
    object_edit: Option<[f32; 4]>,
    status: Option<String>,
    dialog: Option<DialogRequest>,
    window_request: Option<WindowRequest>,
    titlebar_inset: f32,
    /// The window has no title bar of the system's, so the top bar carries its buttons.
    window_controls: bool,
    /// The window is maximized, which the middle button shows.
    window_maximized: bool,
    top_bar_id: egui::Id,
    preferences_id: egui::Id,
    publish_id: egui::Id,
    /// What the person chose for the interface's language, and the system's own, as a locale tag.
    language_choice: flare_i18n::Choice,
    system_locale: String,
    translator: flare_i18n::Translator,
    /// The language the CJK font was last looked for, so it is not looked for every frame.
    cjk_font_for: Option<flare_i18n::Language>,
    /// The palette in use, and the one the egui styles were last set up for.
    theme_mode: flare_ui::theme::Mode,
    styled_for: Option<flare_ui::theme::Mode>,
    /// Which tool of each tool-strip slot was last picked from its list, by the slot's first name.
    slot_choice: std::collections::HashMap<&'static str, usize>,
    /// The slot whose list of tools is open, by its first name.
    slot_open: Option<&'static str>,
    /// Tools from the "…" list put in the strip, and the one being dragged there now.
    pinned_tools: Vec<&'static str>,
    /// The keyboard shortcuts in force, and the editor's state.
    shortcuts: flare_menubar::ShortcutMap,
    shortcut_editor: shortcut_editor::ShortcutEditor,
    slot_drag: Option<&'static str>,
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
            publish: PublishSettings::default(),
            home: home::HomeState::default(),
            collapsed_folders: std::collections::HashSet::new(),
            library: symbols::LibraryState::default(),
            importing: None,
            reverting: None,
            revert_pending: false,
            frames_rendered: 0,
            drag: None,
            drag_preview: None,
            motion: panels::MotionEditor::default(),
            anchor_drag: None,
            bone_drag: None,
            bone_joint: None,
            selected_anchor: None,
            drag_cancelled: false,
            tool: Tool::Rectangle,
            selected: None,
            color_target: ColorTarget::Fill,
            align_to_stage: false,
            transform_edit: TransformEdit::default(),
            hex_buffer: None,
            pointer_stage: None,
            camera_preview: false,
            chrome_ready: false,
            rect_radius: 0.0,
            poly_sides: 5,
            poly_star: false,
            poly_inner: 0.5,
            lasso: Vec::new(),
            wand: Default::default(),
            rotate: Default::default(),
            freehand: Vec::new(),
            stabilizer: None,
            pen_feed: PenFeed::default(),
            brush_size: 12.0,
            eraser_size: 20.0,
            eraser_faucet: false,
            live_erase: None,
            brush_shelf: flare_tools::brush::BrushShelf::default(),
            brush_light: 0.15,
            brush_pressure_gamma: 1.0,
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
            timeline_layout: None,
            onion_drag: None,
            timeline_fit: false,
            also_selected: Vec::new(),
            stage_view: StageView::default(),
            stage_panel: Rect::NOTHING,
            span_drag: None,
            current_layer: None,
            layer_renaming: None,
            layer_rename_serial: 0,
            scene_renaming: None,
            scene_rename_serial: 0,
            layer_menu_for: None,
            frame_menu_open: false,
            native_menu: false,
            quit_requested: false,
            tab_plus: egui::Rect::NOTHING,
            close_prompt: None,
            close_after_save: false,
            panels: rail::Panels::default(),
            onion: OnionSkin {
                enabled: false,
                before: 1,
                after: 1,
                outlines: false,
                tinted: true,
                before_color: types::ONION_BEFORE_COLOR,
                after_color: types::ONION_AFTER_COLOR,
            },
            test_movie_requested: false,
            pen: PenPath::default(),
            stage_edit: None,
            fps_edit: None,
            loop_playback: true,
            muted: false,
            log: Vec::new(),
            props_forced: None,
            shape_paint_edit: None,
            object_edit: None,
            playback_first_edit: None,
            status: None,
            dialog: None,
            window_request: None,
            titlebar_inset: 0.0,
            window_controls: false,
            window_maximized: false,
            top_bar_id: egui::Id::unique("top-bar"),
            preferences_id: egui::Id::unique("preferences"),
            publish_id: egui::Id::unique("publish"),
            language_choice: flare_i18n::Choice::System,
            system_locale: "en".to_owned(),
            translator: flare_i18n::Translator::new(flare_i18n::Language::English),
            cjk_font_for: None,
            theme_mode: flare_ui::theme::Mode::Dark,
            styled_for: None,
            slot_choice: std::collections::HashMap::new(),
            slot_open: None,
            pinned_tools: Vec::new(),
            shortcuts: flare_menubar::ShortcutMap::default(),
            shortcut_editor: shortcut_editor::ShortcutEditor::default(),
            slot_drag: None,
            text: groups::TextTool::default(),
            paint: groups::Paint::default(),
            display: groups::Display::default(),
            windows: groups::OpenWindows::default(),
            recovery: groups::Recovery::default(),
            audio: groups::AudioState::default(),
        }
    }

    /// A new untitled document: one scene with one empty layer at 24 fps, named in English. A
    /// workspace that knows the language makes its own with [`Self::new_blank_document`].
    pub fn untitled() -> Result<Self, String> {
        Ok(Self::new(Self::blank_named("Scene 1", "Layer 1")?, None))
    }

    /// A blank document with a scene and a layer of these names, at 24 fps.
    fn blank_named(scene: &str, layer: &str) -> Result<Document, String> {
        let layer = Layer::new(LayerId::new(), layer, vec![]).map_err(|e| e.to_string())?;
        let scene = Scene::new(SceneId::new(), scene, Timeline::new(vec![layer]));
        let rate = FrameRate::new(24, 1).map_err(|e| e.to_string())?;
        Document::new(DocumentId::new(), rate, vec![scene], vec![])
            .map_err(|diagnostics| format!("{diagnostics:?}"))
    }

    /// A blank document whose scene and layer are named in the language being spoken.
    pub(super) fn new_blank_document(&self) -> Result<Document, String> {
        let number = [("n", "1")];
        Self::blank_named(
            &tr!(self, "name-new-scene", &number),
            &tr!(self, "name-new-layer", &number),
        )
    }

    /// Gives an untouched new document the names of the language chosen. The document made at
    /// start-up is made before the language is known.
    pub fn localize_new_document(&mut self) {
        if self.is_blank()
            && let Ok(document) = self.new_blank_document()
        {
            self.install_document(document, None);
        }
    }
}
