//! Editor state, commands and the window layout (Animate's Essentials workspace).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::Duration;

use eframe::egui::{
    self, Color32, ColorImage, CornerRadius, FontId, Frame, Key, Modifiers, Rect, Sense, Stroke,
    TextureHandle, TextureOptions, Ui, Vec2, pos2, vec2,
};
use minnamate_core::doc::{Document, Element, LayerKind, Matrix, Paint, Point, Rgba, Shape, Timeline};
use minnamate_core::hit::{self, ElementRef};
use minnamate_core::render::{render_layer, render_timeline};
use minnamate_core::{arrange, draw, swf_export, symbols, transform, xfl, xfl_write};

use crate::icons::Icon;
use crate::tools::Tool;
use crate::{menu, panels, stage, theme, timeline, tools};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PropsTab {
    Tool,
    Object,
    Frame,
    Doc,
}

/// Everything a menu item, button or shortcut can do.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Action {
    /// Shown, but not implemented yet (drawn disabled).
    Todo,
    /// File > New: the New Document dialog.
    New,
    /// Opens what New Document's Create or a Home tile asked for.
    CreateDocument,
    Open,
    /// Opens the file dropped on the window.
    OpenDropped,
    Close,
    /// Modify > Document: the Document Settings dialog.
    DocumentSettings,
    ExportSwf,
    Exit,
    ZoomIn,
    ZoomOut,
    Zoom(f32),
    FitInWindow,
    ShowAll,
    Play,
    Rewind,
    GoToEnd,
    StepForward,
    StepBack,
    LoopPlayback,
    NewLayer,
    DeleteLayer,
    Undo,
    Redo,
    Show(PanelId),
    TestMovie,
    OpenRecent(usize),
    Save,
    SaveAs,
    HidePanels,
    About,
    InsertFrame,
    InsertKeyframe,
    InsertBlankKeyframe,
    RemoveFrames,
    ClearKeyframe,
    CreateClassicTween,
    RemoveTween,
    CreateMotionTween,
    CreateShapeTween,
    /// Insert Keyframe and Clear Keyframe inside a motion tween: one property, or all.
    InsertPropertyKeyframe(Option<minnamate_core::motion::Kind>),
    ClearPropertyKeyframe(Option<minnamate_core::motion::Kind>),
    SplitMotion,
    /// Modify > Shape: a shape tween's hints.
    AddShapeHint,
    RemoveAllHints,
    ConvertToKeyframes,
    ConvertToBlankKeyframes,
    CutFrames,
    CopyFrames,
    PasteFrames,
    PasteOverwriteFrames,
    ClearFrames,
    SelectAllFrames,
    ReverseFrames,
    PrevKeyframe,
    NextKeyframe,
    NewFolder,
    CutLayers,
    CopyLayers,
    PasteLayers,
    DuplicateLayers,
    ShowAllLayers,
    LockOthers,
    HideOthers,
    ToggleGuide,
    LayerProperties,
    // The layer menu's masks, motion guides and folders.
    ToggleMask,
    ShowMasking,
    AddMotionGuide,
    ExpandFolders,
    CollapseFolders,
    ExpandAllFolders,
    CollapseAllFolders,
    SelectAll,
    Deselect,
    DeleteSelection,
    ConvertToSymbol,
    EditSymbol,
    // Modify > Transform.
    FreeTransform,
    FlipHorizontal,
    FlipVertical,
    Rotate90Cw,
    Rotate90Ccw,
    RemoveTransform,
    /// Opens the Scale and Rotate dialog.
    ScaleAndRotate,
    // Edit: the element clipboard, the selection, symbols.
    Cut,
    Copy,
    PasteInCenter,
    PasteInPlace,
    Duplicate,
    InvertSelection,
    EditInPlace,
    // Modify.
    Group,
    Ungroup,
    BreakApart,
    Arrange(arrange::Arrange),
    Align(arrange::Align),
    AlignToStage,
    // Insert.
    NewSymbol,
    InsertScene,
    // View and Control.
    GoToScene(Go),
    ClipToStage,
    OnionSkin,
    /// Window > Workspaces > Essentials: the only layout there is, so nothing changes.
    Workspace,
    ResetWorkspace,
    /// Window > the open document: brings it in front of Home.
    ShowDocument,
    /// View > Rulers, Grid and Guides.
    Stage(crate::stage::Toggle),
}

/// View > Go To: which scene.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Go {
    First,
    Previous,
    Next,
    Last,
    To(usize),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelId {
    Timeline,
    Output,
    Tools,
    Properties,
    Library,
    Assets,
    // In the collapsed icon strip, opening as flyouts.
    Color,
    Swatches,
    Align,
    Transform,
    Info,
}

/// Where Edit > Duplicate puts the copy, from the original [recall].
const DUPLICATE_OFFSET: Point = (10.0, 10.0);

/// Animate's default number of undo levels.
const UNDO_LEVELS: usize = 100;

/// How many frames either side of the playhead the onion skin shows at first.
pub const ONION_RANGE: u32 = 2;

/// The buttons of "Save changes before closing?".
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Answer {
    Save,
    DontSave,
    Cancel,
}

/// A symbol being edited in place.
pub struct EditTarget {
    pub symbol: String,
    /// From the symbol's coordinates to the scene's: its instance's matrix, and
    /// those of any instances it sits inside.
    pub transform: Matrix,
    /// Where the parent timeline's playhead and layer were, to return to.
    pub frame: u32,
    pub layer: usize,
    /// Edited on its own (from the Library), not in place: the rest of the stage doesn't show.
    pub isolated: bool,
}

/// An undo or redo step: the document, what was selected in it, and in which
/// timeline: its scene, by place and name, and the symbols being edited.
type Step = (Document, Vec<ElementRef>, (usize, String, Vec<String>));

/// The timeline being edited, borrowing only what it needs so callers can
/// change other fields at the same time.
fn timeline_of<'a>(doc: &'a Document, scene: usize, editing: &[EditTarget]) -> &'a Timeline {
    match editing.last().and_then(|t| doc.library.get(&t.symbol)) {
        Some(symbol) => &symbol.timeline,
        None => &doc.scenes[scene],
    }
}

fn timeline_of_mut<'a>(doc: &'a mut Document, scene: usize, editing: &[EditTarget]) -> &'a mut Timeline {
    match editing.last().and_then(|t| doc.library.get_mut(&t.symbol)) {
        Some(symbol) => &mut symbol.timeline,
        None => &mut doc.scenes[scene],
    }
}

/// The stage view's canvas: document pixels to canvas pixels, and the canvas
/// size in pixels. Everything on stage renders into it, so only what's visible
/// is drawn, and artwork off the stage shows on the pasteboard.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ViewPort {
    pub matrix: Matrix,
    pub size: (f32, f32),
}

impl ViewPort {
    pub(crate) fn bits(&self) -> [u32; 8] {
        let m = &self.matrix;
        [m.a, m.b, m.c, m.d, m.tx, m.ty, self.size.0, self.size.1].map(f32::to_bits)
    }
}

pub(crate) fn hash(value: impl std::hash::Hash) -> u64 {
    use std::hash::Hasher;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut h);
    h.finish()
}

/// The texture cached under `id`, redrawn by `render` when `key` (a hash of
/// everything it depends on) changed.
fn cached<K: std::hash::Hash + Eq>(
    ctx: &egui::Context,
    cache: &mut std::collections::HashMap<K, (TextureHandle, u64)>,
    id: K,
    key: u64,
    render: impl FnOnce() -> Option<minnamate_core::tiny_skia::Pixmap>,
) -> Option<egui::TextureId> {
    use std::collections::hash_map::Entry;
    if cache.get(&id).map(|(_, k)| *k) == Some(key) {
        return cache.get(&id).map(|(tex, _)| tex.id());
    }
    let pixmap = render()?;
    let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
    Some(match cache.entry(id) {
        Entry::Occupied(mut slot) => {
            let (tex, k) = slot.get_mut();
            tex.set(image, TextureOptions::LINEAR);
            *k = key;
            tex.id()
        }
        Entry::Vacant(slot) => slot.insert((ctx.load_texture("stage", image, TextureOptions::LINEAR), key)).0.id(),
    })
}

pub struct Editor {
    pub doc: Document,
    pub title: String,
    pub path: Option<PathBuf>,
    pub scene: usize,
    /// Current frame within the scene, 0-based.
    pub frame: u32,
    /// While playing: when playback started, and from which frame.
    pub playing: Option<(f64, u32)>,
    pub loop_playback: bool,
    pub tool: Tool,
    /// The Tools panel's buttons, as the Edit Toolbar drawer leaves them.
    pub toolbar: tools::Toolbar,
    /// Object Drawing (J): drawing tools make drawing objects rather than raw shapes.
    pub object_drawing: bool,
    pub layer: usize,
    pub zoom: f32,
    pub pan: Vec2,
    pub view_size: Vec2,
    /// Where the stage was last drawn, in screen points.
    pub stage_rect: Rect,
    pub props_tab: PropsTab,
    pub right_tab: usize,
    pub bottom_tab: usize,
    pub show_tools: bool,
    pub show_right: bool,
    pub show_bottom: bool,
    /// Docked panels closed from the Window menu; their dock shows the others.
    pub closed: Vec<PanelId>,
    pub first_visible_frame: f32,
    pub first_visible_layer: f32,
    pub library_selection: Option<String>,
    pub library_search: String,
    /// Library folders shown open, by path.
    pub library_open: BTreeSet<String>,
    /// The Library item being renamed in place, and the name typed so far.
    pub library_rename: Option<(String, String)>,
    /// The icon strip's panel shown as a flyout.
    pub flyout: Option<crate::panels::Flyout>,
    pub output: Vec<String>,
    pub error: Option<String>,
    pub about: bool,
    pub test: Option<crate::test_movie::TestMovie>,
    pub recent: Vec<PathBuf>,
    pub stroke_color: Color32,
    pub fill_color: Color32,
    /// The fill and stroke as gradients (color.rs); None draws the solid colors above.
    pub fill_gradient: Option<minnamate_core::doc::Gradient>,
    pub stroke_gradient: Option<minnamate_core::doc::Gradient>,
    pub stroke_width: f32,
    pub stroke_cap: minnamate_core::doc::Cap,
    pub stroke_join: minnamate_core::doc::Join,
    pub brush_size: f32,
    pub corner_radius: f32,
    pub polystar_sides: u32,
    pub polystar_star: bool,
    /// How deep a star's points are cut, 0..=1 (Animate's star point size).
    pub polystar_point: f32,
    /// Selected elements of the current frame.
    pub selection: Vec<ElementRef>,
    /// The stage gesture in progress, if any.
    pub drag: Option<crate::stage::Drag>,
    pub transform: crate::free_transform::State,
    pub gradient_transform: crate::gradient_transform::State,
    pub pen: crate::pen::State,
    pub text: crate::text_tool::State,
    pub eraser: crate::eraser::State,
    pub paint: crate::paint_tools::State,
    pub convert: Option<crate::convert_symbol::Dialog>,
    /// The layer whose name is being edited, and the text so far.
    pub renaming: Option<(usize, String)>,
    /// The timeline's frame selection, gestures, clipboards and settings.
    pub tl: crate::timeline::State,
    /// Where the timeline's layer list was last drawn, in screen points.
    pub layer_rows: Rect,
    /// A command waiting on "Save changes before closing?".
    pub confirm: Option<Action>,
    /// The file dropped on the window, for `Action::OpenDropped`.
    dropped: Option<PathBuf>,
    /// Set while running a command the user agreed may drop unsaved changes.
    discard_ok: bool,
    /// The window may close: unsaved changes were dealt with.
    closing: bool,
    /// Symbols being edited in place, innermost last; empty when editing the scene.
    pub editing: Vec<EditTarget>,

    preview: Option<(TextureHandle, (u32, String, u32, u32))>,
    /// Onion skin: neighbouring frames show faded around the current one.
    pub onion: bool,
    /// Frame-cell width in the timeline (its zoom slider).
    pub cell_w: f32,
    /// What the timeline's keyframe button inserts: Keyframe, Blank Keyframe or Frame.
    pub key_command: Action,
    /// Scroll the timeline to center the playhead on the next draw.
    pub center_frame: bool,
    onion_cache: std::collections::HashMap<(usize, u32, Option<[u8; 4]>), (TextureHandle, u64)>,
    pub logo: Option<TextureHandle>,
    /// Bumped on every document change, to invalidate the stage image.
    pub rev: u32,
    /// Unsaved changes since the document was opened.
    pub dirty: bool,
    /// Undo and redo steps: the document, what was selected in it, and the scene it was in.
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The field a run of edits came from (one drag = one undo step).
    gesture: Option<&'static str>,
    /// The widget with the keyboard as the gesture began: Escape there undoes it.
    gesture_focus: Option<egui::Id>,
    /// The widget with the keyboard as the last frame ended. egui drops it on
    /// Escape before `shortcuts` runs, but that Escape is still the field's.
    focused: Option<egui::Id>,
    /// Rendered views of the scene and of the symbol being edited.
    textures: std::collections::HashMap<String, (TextureHandle, u64)>,
    /// Artwork off the stage is hidden, as in the published movie.
    pub clip_to_stage: bool,
    /// The stage view's rulers, grid and guides.
    pub stage: crate::stage::State,
    /// What Edit > Copy took, bottom-most first. It outlives the document, as in Animate.
    // ponytail: instances name their symbol, so pasted into another document they show nothing
    // unless it has a symbol of that name. Animate brings the symbols along; do that if it bites.
    pub clipboard: Vec<Element>,
    /// Modify > Align lines things up with the stage instead of each other.
    pub align_to_stage: bool,
    /// The Scale and Rotate dialog, while open: the scale in percent and the turn in degrees.
    pub scale_rotate: Option<(f32, f32)>,
    reported: BTreeSet<String>,
    untitled: u32,
    /// The Home screen and the New Document and Document Settings dialogs.
    pub home: crate::home::State,
}

impl Editor {
    pub fn new(ctx: &egui::Context, path: Option<PathBuf>) -> Self {
        theme::install(ctx);
        // Ctrl+= and Ctrl+- zoom the stage. egui would zoom its whole UI with them
        // while a dialog keeps them from `shortcuts`.
        ctx.options_mut(|o| o.zoom_with_keyboard = false);
        let logo = eframe::icon_data::from_png_bytes(include_bytes!("assets/logo.png")).ok().map(|icon| {
            let image = ColorImage::from_rgba_unmultiplied([icon.width as usize, icon.height as usize], &icon.rgba);
            ctx.load_texture("logo", image, TextureOptions::LINEAR)
        });
        let mut ed = Editor {
            doc: Document::new(550.0, 400.0, 24.0),
            title: String::new(),
            path: None,
            scene: 0,
            frame: 0,
            playing: None,
            loop_playback: false,
            tool: Tool::Selection,
            toolbar: Default::default(),
            object_drawing: false,
            layer: 0,
            zoom: 1.0,
            pan: Vec2::ZERO,
            view_size: Vec2::ZERO,
            stage_rect: Rect::NOTHING,
            props_tab: PropsTab::Doc,
            right_tab: 0,
            bottom_tab: 0,
            show_tools: true,
            show_right: true,
            show_bottom: true,
            closed: Vec::new(),
            first_visible_frame: 0.0,
            first_visible_layer: 0.0,
            library_selection: None,
            library_search: String::new(),
            library_open: BTreeSet::new(),
            library_rename: None,
            flyout: None,
            output: Vec::new(),
            error: None,
            about: false,
            test: None,
            recent: load_recent(),
            stroke_color: Color32::BLACK,
            fill_color: Color32::from_rgb(0x00, 0x99, 0xFF),
            fill_gradient: None,
            stroke_gradient: None,
            stroke_width: 1.0,
            stroke_cap: minnamate_core::doc::Cap::Round,
            stroke_join: minnamate_core::doc::Join::Round,
            brush_size: 10.0,
            corner_radius: 0.0,
            polystar_sides: 5,
            polystar_star: false,
            polystar_point: 0.5,
            selection: Vec::new(),
            drag: None,
            transform: Default::default(),
            gradient_transform: Default::default(),
            pen: Default::default(),
            text: Default::default(),
            eraser: Default::default(),
            paint: Default::default(),
            convert: None,
            renaming: None,
            tl: Default::default(),
            layer_rows: Rect::NOTHING,
            confirm: None,
            dropped: None,
            discard_ok: false,
            closing: false,
            editing: Vec::new(),

            preview: None,
            onion: false,
            cell_w: crate::timeline::DEFAULT_CELL_W,
            key_command: Action::ConvertToKeyframes,
            center_frame: false,
            onion_cache: Default::default(),
            logo,
            rev: 0,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            gesture_focus: None,
            focused: None,
            textures: Default::default(),
            clip_to_stage: false,
            stage: Default::default(),
            clipboard: Vec::new(),
            align_to_stage: false,
            scale_rotate: None,
            reported: BTreeSet::new(),
            untitled: 0,
            home: Default::default(),
        };
        if let Some(path) = path {
            ed.open(path);
        }
        // Without a document, Animate starts on its Home screen.
        ed.home.shown = !ed.home.doc_open;
        ed
    }

    pub fn log(&mut self, line: impl Into<String>) {
        self.output.push(line.into());
    }

    /// A new, empty Untitled-N document.
    pub fn new_document(&mut self, width: f32, height: f32, frame_rate: f32) {
        self.untitled += 1;
        let title = format!("Untitled-{}", self.untitled);
        self.load(Document::new(width, height, frame_rate), title, None);
    }

    pub fn open_dialog(&mut self) {
        let picked = rfd::FileDialog::new().add_filter("Animate documents", &["fla", "xfl"]).pick_file();
        if let Some(path) = picked {
            self.open(path);
        }
    }

    /// Puts `path` first in File > Open Recent and saves the list.
    fn remember(&mut self, path: &std::path::Path) {
        self.recent.retain(|p| p != path);
        self.recent.insert(0, path.to_path_buf());
        self.recent.truncate(10);
        if let Some(file) = recent_file() {
            let lines: Vec<_> = self.recent.iter().map(|p| p.to_string_lossy()).collect();
            // A lost recent-files list is harmless, so write errors are ignored.
            let _ = file.parent().map(std::fs::create_dir_all);
            let _ = std::fs::write(file, lines.join("\n"));
        }
    }

    pub fn open(&mut self, path: PathBuf) {
        match xfl::load(&path) {
            Ok(doc) => {
                let title = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.log(format!("Opened {}", path.display()));
                self.remember(&path);
                self.load(doc, title, Some(path));
            }
            Err(e) => self.error = Some(format!("Couldn't open {}.\n\n{e}", path.display())),
        }
    }

    pub fn load(&mut self, doc: Document, title: String, path: Option<PathBuf>) {
        // Typing ends in the document and timeline it started in: here, and where symbols are entered and left.
        crate::text_tool::commit(self);
        self.doc = doc;
        self.title = title;
        self.path = path;
        self.scene = 0;
        self.frame = 0;
        self.playing = None;
        self.layer = 0;
        self.pan = Vec2::ZERO;
        // Animate opens documents centered at 100%.
        self.zoom = 1.0;
        self.first_visible_frame = 0.0;
        self.first_visible_layer = 0.0;
        self.library_selection = None;
        self.library_open.clear();
        self.library_rename = None;
        self.selection.clear();
        self.drag = None;
        self.pen = Default::default();
        self.renaming = None;
        self.tl.reset();
        self.editing.clear();
        self.convert = None;
        self.stage.guides.clear();
        self.reported.clear();
        self.undo.clear();
        self.redo.clear();
        self.gesture = None;
        self.dirty = false;
        self.rev += 1;
        // A document takes the place of Home.
        (self.home.shown, self.home.doc_open) = (false, true);
    }

    /// Call before changing the document: records an undo step.
    pub fn checkpoint(&mut self) {
        if self.undo.len() == UNDO_LEVELS {
            self.undo.remove(0);
        }
        self.undo.push((self.doc.clone(), self.selection.clone(), self.place()));
        self.redo.clear();
        self.gesture = None;
        self.dirty = true;
        self.rev += 1;
    }

    /// Like `checkpoint`, but repeated edits from the same field (a drag, typing)
    /// share one undo step until `end_gesture`.
    pub fn checkpoint_gesture(&mut self, field: &'static str) {
        if self.gesture == Some(field) {
            self.rev += 1;
            return;
        }
        self.checkpoint();
        self.gesture = Some(field);
        self.gesture_focus = self.focused;
    }

    pub fn end_gesture(&mut self) {
        self.gesture = None;
    }

    fn undo(&mut self) {
        if let Some(step) = self.undo.pop() {
            let back = self.swap(step);
            self.redo.push(back);
        }
    }

    fn redo(&mut self) {
        if let Some(step) = self.redo.pop() {
            let back = self.swap(step);
            self.undo.push(back);
        }
    }

    /// The timeline being edited, as an undo step names it: the scene's index and
    /// name, and the symbols being edited in it.
    fn place(&self) -> (usize, String, Vec<String>) {
        let name = self.doc.scenes.get(self.scene).map(|s| s.name.clone()).unwrap_or_default();
        (self.scene, name, self.editing.iter().map(|t| t.symbol.clone()).collect())
    }

    /// Undo and redo: puts `step` in, returning what it replaced. The scene shown
    /// stays while it exists; one the step removes gives way to the scene the step
    /// was made in. The step's selection comes back only to the timeline it was made in.
    fn swap(&mut self, (doc, selection, at): Step) -> Step {
        let here = self.place();
        let back = (std::mem::replace(&mut self.doc, doc), std::mem::take(&mut self.selection), here.clone());
        let scene = self.doc.scenes.iter().position(|s| s.name == here.1).unwrap_or(at.0);
        self.after_history(scene);
        let now = self.place();
        if (&now.1, &now.2) == (&at.1, &at.2) {
            self.selection = selection;
            self.prune_selection();
        }
        back
    }

    /// Keeps the state valid after undo or redo swapped the document and selection,
    /// and goes back to the step's `scene`.
    fn after_history(&mut self, scene: usize) {
        self.gesture = None;
        self.dirty = true;
        self.rev += 1;
        // A symbol edited in place belongs to the scene it was entered from.
        if scene != self.scene {
            self.editing.clear();
        }
        self.scene = scene.min(self.doc.scenes.len().saturating_sub(1));
        // Stop editing symbols the undo or redo removed (and any inside them).
        let kept = self.editing.iter().take_while(|t| self.doc.library.contains_key(&t.symbol)).count();
        self.editing.truncate(kept);
        self.layer = self.layer.min(self.timeline().layers.len().saturating_sub(1));
        self.prune_selection();
    }

    /// Drops selected elements that no longer exist, aren't on the current
    /// frame, or are on a layer since hidden or locked, where the stage can't select.
    pub fn prune_selection(&mut self) {
        let (layers, f) = (&timeline_of(&self.doc, self.scene, &self.editing).layers, self.frame);
        self.selection.retain(|r| {
            let key = layers.get(r.layer).filter(|l| l.visible() && !l.locked()).and_then(|l| l.frames.get(r.keyframe));
            key.is_some_and(|k| (k.index..k.index + k.duration).contains(&f) && r.element < k.elements.len())
        });
    }

    /// After layers were added, removed or moved: the selection stays on its
    /// elements, `row(l)` being where layer `l` went (None: removed).
    pub fn follow_layers(&mut self, row: impl Fn(usize) -> Option<usize>) {
        self.selection = self.selection.iter().filter_map(|&r| Some(ElementRef { layer: row(r.layer)?, ..r })).collect();
    }

    /// Whether drawing and pasting can land on the active layer: a normal, guide
    /// or mask layer, shown and unlocked.
    pub fn drawable(&self) -> bool {
        let layer = self.timeline().layers.get(self.layer);
        layer.is_some_and(|l| matches!(l.kind, LayerKind::Normal | LayerKind::Guide | LayerKind::Mask) && l.visible() && !l.locked())
    }

    pub fn element(&self, r: ElementRef) -> Option<&Element> {
        self.timeline().element(r)
    }

    pub fn element_mut(&mut self, r: ElementRef) -> Option<&mut Element> {
        self.timeline_mut().element_mut(r)
    }

    /// Moves the selection by (dx, dy) document pixels.
    pub fn nudge(&mut self, dx: f32, dy: f32, gesture: &'static str) {
        if self.selection.is_empty() || (dx == 0.0 && dy == 0.0) {
            return;
        }
        self.checkpoint_gesture(gesture);
        // A motion tween's target, inside its span, gets a position keyframe instead.
        let frame = self.frame;
        for r in self.selection.clone() {
            minnamate_core::motion::edit_shown(self.timeline_mut(), r, frame, |el| hit::translate(el, dx, dy));
        }
    }

    /// Every element on stage at the current frame, bottom layer first.
    pub fn all_elements(&self) -> Vec<ElementRef> {
        let mut all = Vec::new();
        for (l, layer) in self.timeline().layers.iter().enumerate().rev() {
            if let Some(k) = layer.keyframe_at(self.frame) {
                all.extend((0..layer.frames[k].elements.len()).map(|e| ElementRef { layer: l, keyframe: k, element: e }));
            }
        }
        all
    }

    /// Runs `edit` on a copy of the edited timeline; it returns the new
    /// selection. If anything changed, that's one undo step.
    pub(crate) fn edit_timeline(&mut self, edit: impl FnOnce(&Document, &mut Timeline) -> Vec<ElementRef>) {
        let mut tl = self.timeline().clone();
        let selection = edit(&self.doc, &mut tl);
        if tl != *self.timeline() {
            self.checkpoint();
            *self.timeline_mut() = tl;
            self.selection = selection;
        }
    }

    /// The Edit and Modify menus' commands on the selected elements.
    fn element_command(&mut self, action: Action, ctx: &egui::Context) {
        let (sel, frame, layer) = (self.selection.clone(), self.frame, self.layer);
        match action {
            Action::Copy | Action::Cut if !sel.is_empty() => {
                self.clipboard = arrange::copy(self.timeline(), &sel);
                // The system clipboard gets them as XFL. Besides, Ctrl+V only reaches us while it holds text.
                ctx.copy_text(xfl_write::elements_xml(&self.clipboard));
                if action == Action::Cut {
                    self.delete_selection();
                }
            }
            Action::PasteInCenter | Action::PasteInPlace if !self.clipboard.is_empty() => {
                if !self.drawable() {
                    return self.log("Pasting needs a normal layer, shown and unlocked: select one in the timeline.");
                }
                if self.editing.last().is_some_and(|t| symbols::draws(&self.doc, &self.clipboard, &t.symbol)) {
                    return self.log("Pasting that here would put a symbol inside itself.");
                }
                let center = (action == Action::PasteInCenter).then(|| self.view_center());
                let items = self.clipboard.clone();
                self.edit_timeline(|doc, tl| {
                    let pasted = arrange::paste(tl, layer, frame, items);
                    if let Some((x, y)) = center
                        && let Some(b) = hit::bounds(doc, tl, frame, &pasted)
                    {
                        let (dx, dy) = (x - (b.min.0 + b.max.0) / 2.0, y - (b.min.1 + b.max.1) / 2.0);
                        for &r in &pasted {
                            tl.element_mut(r).into_iter().for_each(|el| hit::translate(el, dx, dy));
                        }
                    }
                    pasted
                });
            }
            Action::Duplicate => self.edit_timeline(|_, tl| arrange::duplicate(tl, &sel, DUPLICATE_OFFSET)),
            Action::Group => self.edit_timeline(|_, tl| arrange::group(tl, &sel).into_iter().collect()),
            Action::Ungroup | Action::BreakApart => {
                let tl = self.timeline();
                let parts: std::collections::HashMap<_, _> = sel
                    .iter()
                    .filter_map(|&r| {
                        let el = tl.element(r)?;
                        let offset = frame.saturating_sub(tl.layers[r.layer].frames[r.keyframe].index);
                        let parts = match el {
                            Element::Group(members) => members.clone(),
                            _ if action == Action::BreakApart => arrange::broken_apart(&self.doc, el, offset)?,
                            _ => return None,
                        };
                        Some((r, parts))
                    })
                    .collect();
                self.edit_timeline(|_, tl| arrange::replace(tl, &sel, parts));
            }
            Action::Arrange(how) => self.edit_timeline(|_, tl| arrange::arrange(tl, &sel, how)),
            Action::Align(how) => {
                let boxes: Vec<_> = sel.iter().filter_map(|&r| hit::bounds(&self.doc, self.timeline(), frame, &[r]).map(|b| (r, b))).collect();
                let stage = self.align_to_stage.then(|| self.stage_box());
                let moves = arrange::align(&boxes.iter().map(|&(_, b)| b).collect::<Vec<_>>(), stage, how);
                self.edit_timeline(|_, tl| {
                    for (&(r, _), m) in boxes.iter().zip(&moves) {
                        // A motion tween's target, inside its span, gets keyframes instead.
                        minnamate_core::motion::edit_shown(tl, r, frame, |el| transform::transform(el, m));
                    }
                    sel.clone()
                });
            }
            Action::RemoveTransform => {
                // What stays put: an instance's transformation point, a shape's or text's middle.
                let tl = self.timeline();
                let fixed: Vec<(ElementRef, Point)> = sel
                    .iter()
                    .filter_map(|&r| {
                        let matrix = match tl.element(r)? {
                            Element::Instance(i) => return Some((r, i.pivot)),
                            Element::Shape(s) => s.matrix,
                            Element::Text(t) => t.matrix,
                            Element::Group(_) | Element::Unsupported { .. } => return None,
                        };
                        let b = hit::bounds_through(&self.doc, tl, frame, &[r], matrix.inverse()?)?;
                        Some((r, ((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0)))
                    })
                    .collect();
                self.edit_timeline(|_, tl| {
                    for &(r, p) in &fixed {
                        minnamate_core::motion::edit_shown(tl, r, frame, |el| transform::remove_transform(el, p));
                    }
                    sel.clone()
                });
            }
            _ => {}
        }
        if self.selection != sel {
            if let Some(r) = self.selection.last() {
                self.layer = r.layer;
            }
            stage::show_selection_props(self);
        }
    }

    /// The middle of the stage view, in the edited timeline's coordinates.
    fn view_center(&self) -> Point {
        let c = vec2(self.doc.width, self.doc.height) / 2.0 - self.pan / self.zoom;
        self.to_local((c.x, c.y))
    }

    /// The stage's box in the edited timeline's coordinates.
    fn stage_box(&self) -> hit::Rect {
        let (w, h) = (self.doc.width, self.doc.height);
        let corners = [(0.0, 0.0), (w, 0.0), (w, h), (0.0, h)].map(|p| self.to_local(p));
        let (xs, ys) = (corners.map(|p| p.0), corners.map(|p| p.1));
        let lo = |v: [f32; 4]| v.into_iter().fold(f32::INFINITY, f32::min);
        let hi = |v: [f32; 4]| v.into_iter().fold(f32::NEG_INFINITY, f32::max);
        hit::Rect { min: (lo(xs), lo(ys)), max: (hi(xs), hi(ys)) }
    }

    fn delete_selection(&mut self) {
        // Still selected at the checkpoint, so undo selects them again.
        let sel = self.selection.clone();
        self.edit_timeline(|_, tl| {
            arrange::take(tl, &sel);
            Vec::new()
        });
        self.selection.clear();
    }

    /// Adds a drawn shape to the current layer at the current frame: to the
    /// keyframe showing there, or a new blank keyframe past the layer's end.
    pub fn add_shape(&mut self, mut shape: Shape) {
        let (l, f) = (self.layer, self.frame);
        if !self.drawable() {
            return self.log("Drawing needs a normal layer, shown and unlocked: select one in the timeline.");
        }
        // Only the drawing tools add shapes, and a Pen path may end after another tool is picked.
        // ponytail: raw shapes don't merge here yet, so on stage both modes look alike; the file tells them apart.
        if self.object_drawing {
            shape.extra.set("isDrawingObject", Some("true"));
        }
        self.checkpoint();
        let layer = &mut self.timeline_mut().layers[l];
        if layer.keyframe_at(f).is_none() {
            layer.insert_blank_keyframe(f);
        }
        if let Some(k) = layer.keyframe_at(f) {
            draw::add(&mut layer.frames[k], shape);
        }
    }

    /// The stroke the drawing tools use now.
    pub fn stroke(&self) -> minnamate_core::doc::Stroke {
        use minnamate_core::doc::Stroke;
        let paint = self.stroke_gradient.clone().map_or(Paint::Solid(rgba(self.stroke_color)), Paint::Gradient);
        Stroke { width: self.stroke_width, paint, cap: self.stroke_cap, join: self.stroke_join }
    }

    pub fn fill(&self) -> Paint {
        self.fill_gradient.clone().map_or(Paint::Solid(rgba(self.fill_color)), Paint::Gradient)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// File > Save: back to where the document came from, or Save As for a new one.
    /// Returns whether it was saved.
    pub fn save(&mut self) -> bool {
        match self.path.clone() {
            Some(path) => self.save_to(path),
            None => self.save_as(),
        }
    }

    /// File > Save As: asks where, then saves an .fla there.
    pub fn save_as(&mut self) -> bool {
        let stem = self.title.trim_end_matches(".fla").to_string();
        let picked = rfd::FileDialog::new().add_filter("Animate document", &["fla"]).set_file_name(format!("{stem}.fla")).save_file();
        match picked {
            Some(path) => self.save_to(path),
            None => false,
        }
    }

    fn save_to(&mut self, path: PathBuf) -> bool {
        // Media and settings the model doesn't hold come along from the source.
        match xfl_write::save_preserving(&self.doc, self.path.as_deref(), &path) {
            Ok(()) => {
                self.log(format!("Saved {}", path.display()));
                self.title = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.remember(&path);
                self.path = Some(path);
                self.dirty = false;
                true
            }
            Err(e) => {
                self.error = Some(format!("Couldn't save {}.\n\n{e}", path.display()));
                false
            }
        }
    }

    /// Answers the "Save changes before closing?" prompt for `action`.
    fn resolve(&mut self, answer: Answer, ctx: &egui::Context) {
        let Some(action) = self.confirm.take() else { return };
        let go = match answer {
            Answer::Save => self.save(),
            Answer::DontSave => true,
            Answer::Cancel => false,
        };
        if go {
            if action == Action::Exit {
                self.closing = true;
            }
            self.discard_ok = true;
            self.run(action, ctx);
            self.discard_ok = false;
        }
    }

    pub fn export_swf(&mut self) {
        let stem = self.path.as_ref().and_then(|p| p.file_stem()).map(|s| s.to_string_lossy().into_owned());
        let name = format!("{}.swf", stem.unwrap_or_else(|| self.title.clone()));
        let Some(out) = rfd::FileDialog::new().add_filter("SWF movie", &["swf"]).set_file_name(name).save_file() else {
            return;
        };
        let written = swf_export::export(&self.doc).and_then(|(bytes, skipped)| {
            std::fs::write(&out, bytes)?;
            Ok(skipped)
        });
        match written {
            Ok(skipped) => {
                self.log(format!("Exported {}", out.display()));
                for s in skipped {
                    self.log(format!("Warning: SWF export skipped {s}"));
                }
            }
            Err(e) => self.error = Some(format!("Couldn't export the movie.\n\n{e}")),
        }
    }

    /// The timeline being edited: the scene's, or a symbol's while editing one.
    pub fn timeline(&self) -> &Timeline {
        timeline_of(&self.doc, self.scene, &self.editing)
    }

    pub fn timeline_mut(&mut self) -> &mut Timeline {
        timeline_of_mut(&mut self.doc, self.scene, &self.editing)
    }

    /// Scene coordinates of a point in the edited timeline's coordinates.
    pub fn to_scene(&self, p: Point) -> Point {
        self.editing.last().map_or(p, |t| t.transform.apply(p))
    }

    /// The edited timeline's coordinates of a point given in scene coordinates.
    pub fn to_local(&self, p: Point) -> Point {
        match self.editing.last().and_then(|t| t.transform.inverse()) {
            Some(inverse) => inverse.apply(p),
            None => p,
        }
    }

    /// Double-click on an instance, or Edit Symbols: edit that symbol in place,
    /// with the rest of the stage dimmed behind it.
    pub fn enter_symbol(&mut self, r: ElementRef) {
        // Where it shows now: part way along a classic tween, say.
        let Some(inst) = stage::shown_instance(self, r) else { return };
        if !self.doc.library.contains_key(&inst.symbol) {
            return;
        }
        // A Pen path being drawn stays in the timeline it was drawn in.
        crate::pen::finish(self, false);
        crate::text_tool::commit(self);
        let transform = self.editing.last().map_or(inst.matrix, |t| t.transform * inst.matrix);
        let isolated = self.editing.last().is_some_and(|t| t.isolated);
        self.editing.push(EditTarget { symbol: inst.symbol, transform, frame: self.frame, layer: self.layer, isolated });
        (self.frame, self.layer, self.playing) = (0, 0, None);
        self.selection.clear();
        self.rev += 1;
    }

    /// Double-clicking a symbol in the Library edits it on its own, its
    /// registration point at the stage's center, as Animate does.
    pub fn edit_library_symbol(&mut self, name: &str) {
        if !self.doc.library.contains_key(name) {
            return;
        }
        crate::pen::finish(self, false);
        crate::text_tool::commit(self);
        while !self.editing.is_empty() {
            self.exit_symbol();
        }
        let transform = Matrix { tx: self.doc.width / 2.0, ty: self.doc.height / 2.0, ..Matrix::IDENTITY };
        self.editing.push(EditTarget { symbol: name.to_string(), transform, frame: self.frame, layer: self.layer, isolated: true });
        (self.frame, self.layer, self.playing) = (0, 0, None);
        self.selection.clear();
        self.rev += 1;
    }

    /// The Library's preview of symbol `name`'s first frame, fitted into `size` pixels.
    pub fn library_preview(&mut self, ctx: &egui::Context, name: &str, size: Vec2) -> Option<egui::TextureId> {
        let key = (self.rev, name.to_string(), size.x.to_bits(), size.y.to_bits());
        if self.preview.as_ref().map(|(_, k)| k) != Some(&key) {
            let tl = &self.doc.library.get(name)?.timeline;
            let refs: Vec<ElementRef> = tl
                .layers
                .iter()
                .enumerate()
                .filter_map(|(l, layer)| layer.keyframe_at(0).map(|k| (l, k, layer.frames[k].elements.len())))
                .flat_map(|(l, k, n)| (0..n).map(move |e| ElementRef { layer: l, keyframe: k, element: e }))
                .collect();
            let b = hit::bounds(&self.doc, tl, 0, &refs)?;
            let (w, h) = ((b.max.0 - b.min.0).max(1.0), (b.max.1 - b.min.1).max(1.0));
            let fit = ((size.x - 12.0) / w).min((size.y - 12.0) / h).clamp(0.01, 4.0);
            let matrix = Matrix {
                a: fit,
                d: fit,
                tx: (size.x - w * fit) / 2.0 - b.min.0 * fit,
                ty: (size.y - h * fit) / 2.0 - b.min.1 * fit,
                ..Matrix::IDENTITY
            };
            let (pixmap, _) = render_timeline(&self.doc, tl, 0, &matrix, (size.x, size.y), 1.0).ok()?;
            let image = ColorImage::from_rgba_premultiplied([pixmap.width() as usize, pixmap.height() as usize], pixmap.data());
            self.preview = Some((ctx.load_texture("library-preview", image, TextureOptions::LINEAR), key));
        }
        self.preview.as_ref().map(|(tex, _)| tex.id())
    }

    /// Back to the parent timeline (the scene, or the symbol holding this one).
    pub fn exit_symbol(&mut self) {
        crate::pen::finish(self, false);
        crate::text_tool::commit(self);
        if let Some(t) = self.editing.pop() {
            // Its layer may have gone since, by an undo made inside.
            let layer = t.layer.min(self.timeline().layers.len().saturating_sub(1));
            (self.frame, self.layer, self.playing) = (t.frame, layer, None);
            self.selection.clear();
            self.rev += 1;
        }
    }

    pub fn frame_count(&self) -> u32 {
        self.timeline().frame_count().max(1)
    }

    /// Moves the playhead, stopping playback as Animate does when you scrub. As in
    /// Animate it may go past the last frame, where F5, F6 and F7 extend the layer.
    pub fn set_frame(&mut self, frame: u32) {
        // Flash and Animate stop at 16,000 frames.
        self.frame = frame.min(15_999);
        self.playing = None;
        self.selection.clear();
    }

    pub fn toggle_play(&mut self, now: f64) {
        if self.playing.take().is_none() {
            if self.frame + 1 >= self.frame_count() {
                self.frame = 0;
            }
            self.playing = Some((now, self.frame));
        }
    }

    fn tick(&mut self, ctx: &egui::Context) {
        let Some((start, from)) = self.playing else { return };
        let fps = self.doc.frame_rate.max(0.01) as f64;
        let count = self.frame_count();
        let frame = from + ((ctx.input(|i| i.time) - start) * fps) as u32;
        let (first, last) = timeline::loop_range(self);
        if self.loop_playback && frame > last {
            // Round and round the loop range.
            self.frame = first + (frame - first) % (last - first + 1);
        } else if frame < count {
            self.frame = frame;
        } else {
            self.frame = count - 1;
            self.playing = None;
            return;
        }
        ctx.request_repaint_after(Duration::from_secs_f64(1.0 / fps));
    }

    pub fn zoom_to(&mut self, zoom: f32) {
        // Animate's zoom runs from 8% to 2000%.
        self.zoom = zoom.clamp(0.08, 20.0);
    }

    pub fn fit_in_window(&mut self) {
        if self.view_size.x > 0.0 {
            let fit = (self.view_size / vec2(self.doc.width, self.doc.height)).min_elem() * 0.92;
            self.zoom_to(fit);
            self.pan = Vec2::ZERO;
        }
    }

    /// View > Magnification > Show All: fits everything on the current frame of
    /// the timeline being edited, the scene's or a symbol's.
    pub fn show_all(&mut self) {
        // In the scene's coordinates, where the stage shows them.
        let to_scene = self.editing.last().map_or(Matrix::IDENTITY, |t| t.transform);
        let Some(b) = hit::bounds_through(&self.doc, self.timeline(), self.frame, &self.all_elements(), to_scene) else { return self.fit_in_window() };
        if self.view_size.x > 0.0 {
            let size = vec2(b.max.0 - b.min.0, b.max.1 - b.min.1).max(Vec2::splat(1.0));
            self.zoom_to((self.view_size / size).min_elem() * 0.9);
            // Center the content: the stage's center sits at the view's center plus the pan.
            let center = vec2((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0);
            self.pan = -(center - vec2(self.doc.width, self.doc.height) / 2.0) * self.zoom;
        }
    }

    /// Adds a layer above the selected one, as long as the timeline, as Animate does.
    pub fn new_layer(&mut self) {
        self.checkpoint();
        let at = self.layer;
        let tl = self.timeline_mut();
        let name = format!("Layer_{}", tl.layers.len() + 1);
        timeline::pin_colors(tl);
        let color = timeline::next_color(tl);
        let at = tl.insert_layer(at, &name);
        tl.layers[at].set_color(color);
        tl.adopt(at);
        self.layer = at;
        self.follow_layers(|l| Some(if l < at { l } else { l + 1 }));
    }

    /// Window menu: a hidden panel opens on its tab; a visible one closes,
    /// leaving the other tabs of its dock (the dock goes with its last one).
    fn toggle_panel(&mut self, panel: PanelId) {
        // Each dock's panels in the order of its tabs.
        let (shown, tab, dock): (&mut bool, &mut usize, &[PanelId]) = match panel {
            PanelId::Timeline | PanelId::Output => (&mut self.show_bottom, &mut self.bottom_tab, &[PanelId::Timeline, PanelId::Output]),
            PanelId::Properties | PanelId::Library | PanelId::Assets => {
                (&mut self.show_right, &mut self.right_tab, &[PanelId::Properties, PanelId::Library, PanelId::Assets])
            }
            PanelId::Tools => {
                self.show_tools = !self.show_tools;
                return;
            }
            PanelId::Color | PanelId::Swatches | PanelId::Align | PanelId::Transform | PanelId::Info => {
                let flyout = crate::panels::Flyout::of(panel);
                self.flyout = if self.flyout == flyout { None } else { flyout };
                // The icon strip hosts it, so the right-hand dock shows.
                self.show_right |= self.flyout.is_some();
                return;
            }
        };
        let want = dock.iter().position(|&p| p == panel).unwrap_or(0);
        if !*shown || *tab != want {
            (*shown, *tab) = (true, want);
            self.closed.retain(|&p| p != panel);
        } else if let Some(other) = dock.iter().position(|p| *p != panel && !self.closed.contains(p)) {
            *tab = other;
            self.closed.push(panel);
        } else {
            *shown = false;
        }
    }

    /// Whether the Window menu checks `panel`: it's open, if maybe behind another tab.
    pub fn panel_shown(&self, panel: PanelId) -> bool {
        let docked = |shown: bool| shown && !self.closed.contains(&panel);
        match panel {
            PanelId::Timeline | PanelId::Output => docked(self.show_bottom),
            PanelId::Properties | PanelId::Library | PanelId::Assets => docked(self.show_right),
            PanelId::Tools => self.show_tools,
            PanelId::Color | PanelId::Swatches | PanelId::Align | PanelId::Transform | PanelId::Info => {
                self.show_right && self.flyout.is_some() && self.flyout == crate::panels::Flyout::of(panel)
            }
        }
    }

    /// The tab label of docked `panel`: empty once it's closed, for the tab strip to skip.
    pub fn tab_label(&self, panel: PanelId, label: &'static str) -> &'static str {
        if self.closed.contains(&panel) { "" } else { label }
    }

    /// The scene View > Go To `go` means from here.
    pub fn scene_at(&self, go: Go) -> usize {
        let last = self.doc.scenes.len() - 1;
        match go {
            Go::First => 0,
            Go::Previous => self.scene.saturating_sub(1),
            Go::Next => (self.scene + 1).min(last),
            Go::Last => last,
            Go::To(i) => i.min(last),
        }
    }

    /// Switches to scene `i`, at its first frame. Going to the scene that's open
    /// only leaves any symbol, back to where its playhead was.
    pub fn go_to_scene(&mut self, i: usize) {
        while !self.editing.is_empty() {
            self.exit_symbol();
        }
        let i = i.min(self.doc.scenes.len() - 1);
        if i != self.scene {
            self.scene = i;
            self.set_frame(0);
            self.layer = 0;
        }
    }

    pub fn run(&mut self, action: Action, ctx: &egui::Context) {
        if !self.home.allows(action) {
            return;
        }
        // Commands that drop the open document first ask about unsaved changes.
        let discards = matches!(action, Action::CreateDocument | Action::Open | Action::OpenDropped | Action::OpenRecent(_) | Action::Close | Action::Exit);
        if discards && self.dirty && !self.discard_ok {
            self.confirm = Some(action);
            return;
        }
        let now = ctx.input(|i| i.time);
        match action {
            Action::Todo => {}
            Action::New => crate::home::open_new(self, 0, 0),
            Action::CreateDocument => crate::home::create_pending(self),
            Action::Open => self.open_dialog(),
            Action::OpenDropped => {
                if let Some(path) = self.dropped.take() {
                    self.open(path);
                }
            }
            Action::Close => crate::home::close(self),
            Action::DocumentSettings => crate::home::open_settings(self),
            Action::ExportSwf => self.export_swf(),
            Action::Exit => {
                self.closing = true;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Action::Save => {
                self.save();
            }
            Action::SaveAs => {
                self.save_as();
            }
            Action::ZoomIn => self.zoom_to(self.zoom * 2.0),
            Action::ZoomOut => self.zoom_to(self.zoom / 2.0),
            Action::Zoom(z) => self.zoom_to(z),
            Action::FitInWindow => self.fit_in_window(),
            Action::ShowAll => self.show_all(),
            Action::Play => self.toggle_play(now),
            Action::Rewind => self.set_frame(0),
            Action::GoToEnd => self.set_frame(self.frame_count() - 1),
            Action::InsertFrame
            | Action::InsertKeyframe
            | Action::InsertBlankKeyframe
            | Action::RemoveFrames
            | Action::ClearKeyframe
            | Action::CreateClassicTween
            | Action::RemoveTween
            | Action::CreateShapeTween
            | Action::InsertPropertyKeyframe(_)
            | Action::ClearPropertyKeyframe(_)
            | Action::SplitMotion
            | Action::ConvertToKeyframes
            | Action::ConvertToBlankKeyframes
            | Action::PasteFrames
            | Action::PasteOverwriteFrames
            | Action::ClearFrames
            | Action::SelectAllFrames
            | Action::ReverseFrames
            | Action::PrevKeyframe
            | Action::NextKeyframe
            | Action::NewFolder
            | Action::DeleteLayer
            | Action::CutLayers
            | Action::CopyLayers
            | Action::PasteLayers
            | Action::DuplicateLayers
            | Action::ShowAllLayers
            | Action::LockOthers
            | Action::HideOthers
            | Action::ToggleGuide
            | Action::LayerProperties
            | Action::ToggleMask
            | Action::ShowMasking
            | Action::AddMotionGuide
            | Action::ExpandFolders
            | Action::CollapseFolders
            | Action::ExpandAllFolders
            | Action::CollapseAllFolders => timeline::command(self, action),
            // What can't be motion tweened as it is asks to be converted first.
            Action::CreateMotionTween => crate::tweens::create_motion_tween(self, ctx),
            Action::AddShapeHint | Action::RemoveAllHints => crate::tweens::hint_command(self, action),
            Action::CutFrames | Action::CopyFrames if timeline::available(self, action) => {
                timeline::command(self, action);
                // Ctrl+Alt+V only reaches us as egui-winit's Paste event, which needs text on the clipboard.
                ctx.copy_text("Minnamate frames".into());
            }
            Action::CutFrames | Action::CopyFrames => {}
            Action::SelectAll | Action::InvertSelection => {
                // Elements on locked and hidden layers can't be picked, as in Animate.
                let layers = &self.timeline().layers;
                let pickable: Vec<_> = self.all_elements().into_iter().filter(|r| layers[r.layer].visible() && !layers[r.layer].locked()).collect();
                let invert = action == Action::InvertSelection;
                self.selection = pickable.into_iter().filter(|r| !invert || !self.selection.contains(r)).collect();
            }
            Action::Deselect => {
                // Deselect All (and Escape) also ends a Pen path, as in Animate.
                crate::pen::finish(self, false);
                self.selection.clear();
            }
            Action::DeleteSelection => self.delete_selection(),
            Action::ConvertToSymbol => crate::convert_symbol::open(self),
            Action::NewSymbol => crate::convert_symbol::open_new(self),
            // Edit Symbols edits the selected instance's symbol on its own; inside one it reads
            // Edit Document, and goes back to the scene.
            Action::EditSymbol => {
                if !self.editing.is_empty() {
                    while !self.editing.is_empty() {
                        self.exit_symbol();
                    }
                } else if let [r] = self.selection[..]
                    && let Some(Element::Instance(inst)) = self.element(r)
                {
                    let name = inst.symbol.clone();
                    self.edit_library_symbol(&name);
                }
            }
            Action::EditInPlace => {
                if let [r] = self.selection[..] {
                    self.enter_symbol(r);
                }
            }
            Action::Cut
            | Action::Copy
            | Action::PasteInCenter
            | Action::PasteInPlace
            | Action::Duplicate
            | Action::Group
            | Action::Ungroup
            | Action::BreakApart
            | Action::Arrange(_)
            | Action::Align(_)
            | Action::RemoveTransform => self.element_command(action, ctx),
            Action::AlignToStage => self.align_to_stage = !self.align_to_stage,
            Action::FreeTransform => self.tool = Tool::FreeTransform,
            Action::FlipHorizontal | Action::FlipVertical | Action::Rotate90Cw | Action::Rotate90Ccw => {
                crate::free_transform::modify(self, action)
            }
            Action::ScaleAndRotate => {
                if !self.selection.is_empty() {
                    self.scale_rotate = Some((100.0, 0.0));
                }
            }
            Action::InsertScene => {
                self.checkpoint();
                let at = self.doc.insert_scene(self.scene);
                self.go_to_scene(at);
            }
            Action::GoToScene(go) => self.go_to_scene(self.scene_at(go)),
            Action::ClipToStage => self.clip_to_stage = !self.clip_to_stage,
            Action::OnionSkin => self.onion = !self.onion,
            Action::Workspace => {}
            Action::ResetWorkspace => {
                (self.show_tools, self.show_right, self.show_bottom) = (true, true, true);
                (self.right_tab, self.bottom_tab) = (0, 0);
                self.closed.clear();
                self.flyout = None;
                // The docks' sizes too: egui keeps those, by panel id.
                for id in ["properties", "timeline"] {
                    ctx.data_mut(|d| d.remove::<egui::PanelState>(egui::Id::new(id)));
                }
            }
            Action::ShowDocument => self.home.shown = false,
            // The playhead stops at the last frame.
            Action::StepForward if self.frame + 1 < self.frame_count() => self.set_frame(self.frame + 1),
            Action::StepForward => {}
            Action::StepBack => self.set_frame(self.frame.saturating_sub(1)),
            Action::LoopPlayback => self.loop_playback = !self.loop_playback,
            Action::NewLayer => self.new_layer(),
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::Show(panel) => self.toggle_panel(panel),
            Action::TestMovie => self.test = Some(crate::test_movie::TestMovie::new(now)),
            Action::OpenRecent(i) => {
                if let Some(path) = self.recent.get(i).cloned() {
                    self.open(path);
                }
            }
            Action::HidePanels => {
                // An open icon-strip flyout goes with the right-hand dock, and comes back with it.
                let any = self.show_tools || self.show_right || self.show_bottom;
                (self.show_tools, self.show_right, self.show_bottom) = (!any, !any, !any);
            }
            Action::About => self.about = true,
            Action::Stage(toggle) => self.stage.toggle(toggle),
        }
    }

    /// Keyboard shortcuts: every menu item's (`menu::shortcuts`), plus a few of
    /// Animate's that no menu shows. Each needs exactly its modifiers, so
    /// Ctrl+Shift+D (greyed) doesn't reach Ctrl+D.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        // Dialogs keep the keyboard (Enter, Escape) while up.
        if self.dialog_up(ctx) {
            return;
        }
        let mut all = menu::shortcuts();
        all.extend([
            (Modifiers::NONE, Key::Delete, Action::DeleteSelection),
            (Modifiers::NONE, Key::Escape, Action::Deselect),
            // "+" is Shift+= on many keyboards, and on the keypad it's plain.
            (Modifiers::COMMAND, Key::Plus, Action::ZoomIn),
            (Modifiers::COMMAND | Modifiers::SHIFT, Key::Plus, Action::ZoomIn),
            (Modifiers::COMMAND | Modifiers::ALT, Key::O, Action::OnionSkin),
        ]);
        let (chords, keys): (Vec<_>, Vec<_>) = all.into_iter().partition(|(m, ..)| m.command || m.alt);
        // A text field with the keyboard (the Text tool's too) keeps the keys that edit text,
        // Ctrl+A, Z, arrows and all; commands that don't touch the selection still work.
        // An Escape that just took a field's keyboard away is the field's, to cancel with.
        let typing = ctx.egui_wants_keyboard_input() || self.focused.is_some_and(|id| ctx.memory(|m| m.had_focus_last_frame(id)));
        // The panels' fields apply what's typed as it's typed, a run being one undo step:
        // that Escape undoes the field's run, leaving nothing to redo.
        // ponytail: settings outside the document (stroke width, onion frames) keep what was typed.
        if typing && !ctx.egui_wants_keyboard_input() && self.gesture.is_some() && self.gesture_focus == self.focused {
            self.undo();
            self.redo.pop();
        }
        // From no field, Tab would hand the keyboard to the first widget, the title bar,
        // and every key after it with no sign of it. From a field it goes to the next.
        if !typing {
            ctx.memory_mut(|m| m.move_focus(egui::FocusDirection::None));
        }
        let chords: Vec<_> = chords.into_iter().filter(|&(_, _, a)| !typing || runs_while_typing(a)).collect();
        self.press(ctx, &chords);
        // Home hides the document, so its keys and tool keys wait. Plain keys also go to an
        // open menu or the Edit Toolbar drawer, so Escape closes it rather than deselecting.
        if typing || self.home.shown || egui::Popup::is_any_open(ctx) || self.toolbar.drawer {
            return;
        }
        // Ctrl+X, C and V come as clipboard events rather than keys, Alt held or not; Ctrl+V only
        // while the system clipboard holds text, which Copy sees to. With Alt they're the frames'.
        let (cut, copy, paste, in_place, alt) = ctx.input(|i| {
            let has = |event: fn(&egui::Event) -> bool| i.events.iter().any(event);
            // Ctrl+Shift+C is Text > Align Center.
            let copy = !i.modifiers.shift && has(|e| matches!(e, egui::Event::Copy));
            let paste = has(|e| matches!(e, egui::Event::Paste(_)));
            // Ctrl+Shift+V; Shift+Insert, Windows' other Paste, is the plain one.
            let in_place = i.modifiers.shift && i.modifiers.command;
            (has(|e| matches!(e, egui::Event::Cut)), copy, paste, in_place, i.modifiers.alt)
        });
        let actions = if alt {
            [Action::CutFrames, Action::CopyFrames, Action::PasteFrames]
        } else {
            [Action::Cut, Action::Copy, if in_place { Action::PasteInPlace } else { Action::PasteInCenter }]
        };
        for (on, action) in [cut, copy, paste].into_iter().zip(actions) {
            if on {
                self.run(action, ctx);
            }
        }
        crate::pen::keys(ctx, self);
        self.press(ctx, &keys);
        // Arrow keys nudge the selection a pixel, or ten with Shift. During a stage
        // drag they're the tool's: Up and Down round a rectangle's corners.
        for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
            if self.drag.is_some() {
                break;
            }
            let px = ctx.input_mut(|i| i.count_and_consume_key(Modifiers::SHIFT, key) * 10 + i.count_and_consume_key(Modifiers::NONE, key));
            if px > 0 {
                self.nudge(dx * px as f32, dy * px as f32, "nudge");
            }
        }
        tools::keys(ctx, self);
    }

    /// Runs the action of each shortcut in `list` pressed with exactly its modifiers,
    /// once a press: a slow frame gets key repeats together.
    fn press(&mut self, ctx: &egui::Context, list: &[(Modifiers, Key, Action)]) {
        for &(mods, key, action) in list {
            let presses = ctx.input_mut(|i| {
                let before = i.events.len();
                i.events.retain(|e| !matches!(e, egui::Event::Key { key: k, pressed: true, modifiers, .. } if *k == key && modifiers.matches_exact(mods)));
                before - i.events.len()
            });
            for _ in 0..presses {
                self.run(action, ctx);
            }
        }
    }

    /// A dialog is up, and keeps the window's input.
    fn dialog_up(&self, ctx: &egui::Context) -> bool {
        self.home.dialog_open() || ctx.memory(|m| m.top_modal_layer()).is_some()
    }

    /// A file dropped on the window opens as File > Open would: asking about
    /// unsaved changes first, and not while a dialog is up (a native one takes no drops).
    fn dropped_files(&mut self, ctx: &egui::Context) {
        if self.dialog_up(ctx) {
            return;
        }
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().map(|f| f.path().to_path_buf()).filter(|p| !p.as_os_str().is_empty()).collect());
        if let Some(path) = dropped.into_iter().next() {
            self.dropped = Some(path);
            self.run(Action::OpenDropped, ctx);
        }
    }

    /// The scene at the current frame (or, while editing a symbol, at the frame
    /// it was left on), drawn into the stage view's canvas.
    pub fn scene_texture(&mut self, ctx: &egui::Context, vp: &ViewPort) -> Option<egui::TextureId> {
        let frame = self.editing.first().map_or(self.frame, |t| t.frame);
        // Past the scene's last frame there's nothing on stage, just its color.
        if frame >= self.doc.scenes[self.scene].frame_count() {
            return None;
        }
        let key = hash((self.rev, self.scene, frame, vp.bits()));
        let (doc, scene, reported, output) = (&self.doc, self.scene, &mut self.reported, &mut self.output);
        cached(ctx, &mut self.textures, "scene".to_string(), key, || {
            let (pixmap, skipped) = render_timeline(doc, &doc.scenes[scene], frame, &vp.matrix, vp.size, 1.0).ok()?;
            for s in skipped {
                if reported.insert(s.clone()) {
                    output.push(format!("Warning: can't show {s} yet"));
                }
            }
            Some(pixmap)
        })
    }

    /// The symbol being edited at the current frame, where its instance sits;
    /// None when editing the scene.
    pub fn symbol_texture(&mut self, ctx: &egui::Context, vp: &ViewPort) -> Option<egui::TextureId> {
        let target = self.editing.last()?;
        let key = hash((self.rev, self.editing.len(), self.frame, vp.bits()));
        let (doc, matrix, frame) = (&self.doc, vp.matrix * target.transform, self.frame);
        let tl = &doc.library.get(&target.symbol)?.timeline;
        cached(ctx, &mut self.textures, "symbol".to_string(), key, || {
            render_timeline(doc, tl, frame, &matrix, vp.size, 1.0).ok().map(|(pixmap, _)| pixmap)
        })
    }

    /// A drawing of the stage's own (selection dots, a shape being drawn) the
    /// size of the stage view's canvas, cached as `name` and redrawn by `render`
    /// when `key` changes.
    pub fn canvas_texture(
        &mut self,
        ctx: &egui::Context,
        name: &str,
        key: u64,
        render: impl FnOnce(&Editor) -> Option<minnamate_core::tiny_skia::Pixmap>,
    ) -> Option<egui::TextureId> {
        let mut textures = std::mem::take(&mut self.textures);
        let id = cached(ctx, &mut textures, name.to_string(), key, || render(self));
        self.textures = textures;
        id
    }

    /// One layer of the timeline being edited at `frame`, where it shows on
    /// stage, recolored toward `tint` when given. Onion skinning draws layer by
    /// layer, so a layer's ghosts sit under its own current content but over the
    /// layers below it, as in Animate.
    pub fn layer_texture(&mut self, ctx: &egui::Context, layer: usize, frame: u32, tint: Option<Color32>, vp: &ViewPort) -> Option<egui::TextureId> {
        // Keep only the frames the onion skin can show, and the current one.
        let (lo, hi) = timeline::onion_range(self);
        let now = self.frame;
        self.onion_cache.retain(|(_, f, _), _| (lo..=hi).contains(f) || *f == now);
        // Entering or leaving a symbol changes `rev`; another scene doesn't.
        let key = hash((self.rev, self.scene, vp.bits()));
        let (doc, tl) = (&self.doc, timeline_of(&self.doc, self.scene, &self.editing));
        let matrix = vp.matrix * self.editing.last().map_or(Matrix::IDENTITY, |t| t.transform);
        tl.layers.get(layer)?;
        cached(ctx, &mut self.onion_cache, (layer, frame, tint.map(|c| c.to_array())), key, || {
            // Drawn from the whole timeline, so its mask, guide and parent still count.
            let (mut pixmap, _) = render_layer(doc, tl, frame, layer, &matrix, vp.size, 1.0).ok()?;
            if let Some(tint) = tint {
                // Mostly the tint, with a little of the original color left showing.
                for px in pixmap.data_mut().chunks_exact_mut(4) {
                    let a = px[3] as f32 / 255.0;
                    for (c, t) in px[..3].iter_mut().zip(tint.to_array()) {
                        *c = (*c as f32 * 0.3 + t as f32 * a * 0.7) as u8;
                    }
                }
            }
            Some(pixmap)
        })
    }

    /// The document tab row: the open document's tab, filled like the edit bar
    /// below so the two join, with "*" for unsaved changes and a close box.
    fn doc_tabs(&mut self, ui: &mut Ui) {
        let (rect, _) = ui.allocate_exact_size(ui.available_size(), Sense::hover());
        let painter = ui.painter_at(rect);
        let label = if self.dirty { format!("{}*", self.title) } else { self.title.clone() };
        let galley = painter.layout_no_wrap(label, FontId::proportional(12.0), Color32::WHITE);
        let width = (galley.size().x + 10.0 + 8.0 + 16.0 + 6.0).clamp(80.0, 220.0);
        let tab = Rect::from_min_size(rect.min, vec2(width, rect.height()));
        painter.rect_filled(tab, CornerRadius::ZERO, theme::PANEL_BG);
        painter.vline(tab.right(), tab.y_range(), Stroke::new(1.0, theme::APP_BG));
        painter.galley(pos2(tab.left() + 10.0, tab.center().y - galley.size().y / 2.0), galley, Color32::WHITE);
        let close = Rect::from_center_size(pos2(tab.right() - 6.0 - 8.0, tab.center().y), vec2(16.0, 16.0));
        let resp = ui.interact(close, ui.id().with("close-doc"), Sense::click()).on_hover_text("Close");
        if resp.hovered() {
            painter.rect_filled(close, 2.0, theme::BUTTON_ACTIVE);
        }
        let c = close.shrink(4.0);
        let stroke = Stroke::new(1.5, if resp.hovered() { Color32::WHITE } else { theme::TEXT });
        painter.line_segment([c.left_top(), c.right_bottom()], stroke);
        painter.line_segment([c.right_top(), c.left_bottom()], stroke);
        if resp.clicked() {
            self.run(Action::Close, ui.ctx());
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context) {
        if self.confirm.is_some() {
            let title = self.title.clone();
            let modal = egui::Modal::new(egui::Id::new("save-changes")).show(ctx, |ui| {
                ui.set_width(400.0);
                ui.label(egui::RichText::new("Minnamate").strong());
                ui.add_space(6.0);
                ui.label(format!("Save changes to \"{title}\" before closing?"));
                ui.add_space(12.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                    let mut answer = None;
                    if ui.button("  Cancel  ").clicked() {
                        answer = Some(Answer::Cancel);
                    }
                    if ui.button("  Don't Save  ").clicked() {
                        answer = Some(Answer::DontSave);
                    }
                    if theme::cta_button(ui, "    Save    ").clicked() {
                        answer = Some(Answer::Save);
                    }
                    answer
                })
                .inner
            });
            let answer = if modal.should_close() { Some(Answer::Cancel) } else { modal.inner };
            if let Some(answer) = answer {
                self.resolve(answer, ctx);
            }
        }
        if let Some(message) = self.error.clone() {
            let modal = egui::Modal::new(egui::Id::new("error")).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(egui::RichText::new("Minnamate").strong());
                ui.add_space(6.0);
                ui.label(message);
                ui.add_space(10.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| theme::cta_button(ui, "   OK   ").clicked()).inner
            });
            if modal.inner || modal.should_close() {
                self.error = None;
            }
        }
        if self.about {
            let logo = self.logo.as_ref().map(|t| t.id());
            let modal = egui::Modal::new(egui::Id::new("about")).show(ctx, |ui| {
                ui.set_width(360.0);
                ui.horizontal(|ui| {
                    if let Some(logo) = logo {
                        ui.add(egui::Image::new((logo, vec2(64.0, 64.0))).corner_radius(8));
                    }
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Minnamate").heading().strong());
                        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));
                        ui.label(egui::RichText::new("An open-source animation editor.").color(theme::TEXT_DIM));
                    });
                });
                ui.add_space(8.0);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| theme::cta_button(ui, "   OK   ").clicked()).inner
            });
            if modal.inner || modal.should_close() {
                self.about = false;
            }
        }
    }
}

/// What its key or menu item still runs while a field has the keyboard: commands
/// that leave the selection be. The rest are the field's (Ctrl+A, Z...) or wait.
pub fn runs_while_typing(a: Action) -> bool {
    use Action::*;
    matches!(a, New | Open | OpenRecent(_) | Close | Save | SaveAs | Exit | ExportSwf | TestMovie | DocumentSettings | Show(_) | About)
        || matches!(a, ZoomIn | ZoomOut | Zoom(_) | FitInWindow | ShowAll | OnionSkin)
}

/// An egui color as the document's straight RGBA.
pub fn rgba(c: Color32) -> Rgba {
    c.to_srgba_unmultiplied().map(|v| v as f32 / 255.0)
}

/// Where File > Open Recent is kept; tests never touch it.
fn recent_file() -> Option<PathBuf> {
    if cfg!(test) {
        return None;
    }
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("Minnamate").join("recent.txt"))
}

fn load_recent() -> Vec<PathBuf> {
    let text = recent_file().and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    text.lines().filter(|l| !l.is_empty()).map(PathBuf::from).collect()
}

impl eframe::App for Editor {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.dropped_files(&ctx);
        self.shortcuts(&ctx);
        self.tick(&ctx);
        // Closing the window with unsaved changes asks first, as File > Exit does.
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.closing {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Action::Exit);
        }
        // Releasing the mouse ends a drag's undo step (typing ends at focus loss).
        if ctx.input(|i| i.pointer.any_released()) && !ctx.egui_wants_keyboard_input() {
            self.end_gesture();
        }
        let title = match self.home.doc_open {
            true => format!("{}{} - Minnamate", self.title, if self.dirty { "*" } else { "" }),
            false => "Minnamate".to_string(),
        };
        if ctx.data_mut(|d| d.get_temp::<String>(egui::Id::new("window-title"))) != Some(title.clone()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            ctx.data_mut(|d| d.insert_temp(egui::Id::new("window-title"), title));
        }

        // Animate's Essentials layout. The window is ours to draw, title bar included
        // as Animate does on Windows; its dark frame shows in the gaps between groups.
        ui.painter().rect_filled(ui.max_rect(), 0.0, theme::APP_BG);
        menu::window_resize(&ctx);
        let gap = theme::GAP;
        egui::Panel::top("app-bar")
            .exact_size(theme::APP_BAR_H)
            .frame(Frame::NONE.fill(theme::BAR))
            .show_separator_line(false)
            .show(ui, |ui| menu::app_bar(ui, self));
        // Under the app bar the window's frame shows as a 1 px line (docs/animate-ui.md §4.1).
        if self.show_tools && !self.home.shown {
            let width = tools::width(self, ui.available_height() - 1.0);
            // A 1 px line of the window's frame shows along the column's outer edge.
            let panel = egui::Panel::left("tools").exact_size(1.0 + width + gap as f32).resizable(false).show_separator_line(false);
            panel.frame(theme::dock_frame(1, gap, 1)).show(ui, |ui| tools::panel(ui, self));
        }
        if self.show_right && !self.home.shown {
            let panel = egui::Panel::right("properties").default_size(300.0).size_range(250.0..=640.0).show_separator_line(false);
            panel.frame(theme::dock_frame(gap, 0, 1)).show(ui, |ui| {
                theme::dock_strip(ui, Icon::ChevronsRight);
                panels::right(ui, self);
            });
            // Collapsed panels, as icons, just left of Properties.
            let strip = egui::Panel::right("panel-icons").exact_size(36.0 + gap as f32).resizable(false).show_separator_line(false);
            strip.frame(theme::dock_frame(gap, 0, 1)).show(ui, |ui| {
                theme::dock_strip(ui, Icon::ChevronsLeft);
                panels::icon_strip(ui, self);
            });
        }
        // The document column: tabs, edit bar and stage, with the Timeline under them.
        egui::CentralPanel::default().frame(Frame::NONE.outer_margin(egui::Margin { top: 1, ..Default::default() })).show(ui, |ui| {
            // Home takes the place of the whole workspace below the app bar.
            if self.home.shown {
                return crate::home::screen(ui, self);
            }
            egui::Panel::top("doc-tabs")
                .exact_size(theme::DOC_TABS_H)
                .frame(Frame::NONE.fill(theme::BAR))
                .show_separator_line(false)
                .show(ui, |ui| self.doc_tabs(ui));
            if self.show_bottom {
                let panel = egui::Panel::bottom("timeline").default_size(280.0).size_range(120.0..=720.0).resizable(!self.tl.locked).show_separator_line(false);
                panel.frame(theme::dock_frame(0, 0, gap)).show(ui, |ui| timeline::panel(ui, self));
            }
            egui::CentralPanel::default().frame(Frame::NONE.fill(theme::PASTEBOARD)).show(ui, |ui| stage::panel(ui, self));
        });
        self.dialogs(&ctx);
        timeline::dialogs(&ctx, self);
        crate::tweens::dialogs(&ctx, self);
        crate::test_movie::show(&ctx, self);
        crate::convert_symbol::show(&ctx, self);
        crate::arrange_panels::show(&ctx, self);
        crate::home::dialogs(&ctx, self);
        // Escape cancels only what was typed since the field took the keyboard.
        let focused = ctx.memory(|m| m.focused());
        if focused != self.focused {
            self.gesture_focus = None;
        }
        self.focused = focused;
    }
}

/// Helpers for driving the editor in tests: a headless window, and mouse
/// gestures given in document coordinates.
#[cfg(test)]
pub(crate) mod testkit {
    use super::*;
    pub use egui_kittest::Harness;

    pub fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../tnztools/tests/fixtures/tween")
    }

    /// The editor on the tween fixture; `renderer` adds a GPU renderer for `screenshot`.
    pub fn harness(renderer: bool) -> Harness<'static, Editor> {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0);
        let builder = if renderer { builder.wgpu() } else { builder };
        builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, Some(fixture())))
    }

    /// The editor on a new, empty 550×400 document, run until idle.
    pub fn blank() -> Harness<'static, Editor> {
        let mut h = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        // Launching shows Home; the tests draw on Animate's classic stage size.
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h
    }

    /// Screen position of a document point on the stage.
    pub fn on_stage(h: &Harness<'_, Editor>, (x, y): (f32, f32)) -> egui::Pos2 {
        let ed = h.state();
        ed.stage_rect.min + vec2(x, y) * ed.zoom
    }

    pub fn mouse_drag(h: &mut Harness<'_, Editor>, from: (f32, f32), to: (f32, f32)) {
        let (a, b) = (on_stage(h, from), on_stage(h, to));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(a.lerp(b, 0.5));
        h.step();
        h.hover_at(b);
        h.step();
        h.drop_at(b);
        h.step();
    }

    /// Presses at the first point, moves through the rest, releases at the last.
    pub fn mouse_path(h: &mut Harness<'_, Editor>, points: &[(f32, f32)]) {
        let screen: Vec<_> = points.iter().map(|&p| on_stage(h, p)).collect();
        let (Some(&first), Some(&last)) = (screen.first(), screen.last()) else { return };
        h.hover_at(first);
        h.step();
        h.drag_at(first);
        h.step();
        for &p in &screen[1..] {
            h.hover_at(p);
            h.step();
        }
        h.drop_at(last);
        h.step();
    }

    pub fn mouse_click(h: &mut Harness<'_, Editor>, at: (f32, f32)) {
        click_at(h, on_stage(h, at));
    }

    /// A click at a screen position.
    pub fn click_at(h: &mut Harness<'_, Editor>, p: egui::Pos2) {
        h.hover_at(p);
        h.step();
        h.drag_at(p);
        h.drop_at(p);
        h.step();
    }

    /// Screen position of the middle of a timeline cell.
    pub fn cell(h: &Harness<'_, Editor>, layer: usize, frame: u32) -> egui::Pos2 {
        let ed = h.state();
        let rows = ed.layer_rows;
        let x = rows.right() + (frame as f32 - ed.first_visible_frame + 0.5) * ed.cell_w;
        egui::pos2(x, rows.top() + (layer as f32 - ed.first_visible_layer + 0.5) * ed.tl.row_h)
    }

    /// Presses at `a`, moves to `b` in steps and releases there, with `mods` held.
    pub fn screen_drag(h: &mut Harness<'_, Editor>, a: egui::Pos2, b: egui::Pos2, mods: egui::Modifiers) {
        h.event(egui::Event::ModifiersChanged(mods));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        for t in [0.25, 0.5, 0.75, 1.0] {
            h.hover_at(a.lerp(b, t));
            h.step();
        }
        h.drop_at(b);
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.step();
    }

    /// Clicks at a screen position with `mods` held.
    pub fn screen_click(h: &mut Harness<'_, Editor>, p: egui::Pos2, mods: egui::Modifiers) {
        h.event(egui::Event::ModifiersChanged(mods));
        h.hover_at(p);
        h.step();
        h.drag_at(p);
        h.drop_at(p);
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.step();
    }

    /// Clicks a timeline cell, (layer, frame), with `mods` held.
    pub fn click_cell(h: &mut Harness<'_, Editor>, (layer, frame): (usize, u32), mods: egui::Modifiers) {
        let p = cell(h, layer, frame);
        screen_click(h, p, mods);
    }

    /// Drags from one timeline cell to another with `mods` held.
    pub fn drag_cells(h: &mut Harness<'_, Editor>, from: (usize, u32), to: (usize, u32), mods: egui::Modifiers) {
        let (a, b) = (cell(h, from.0, from.1), cell(h, to.0, to.1));
        screen_drag(h, a, b, mods);
    }

    /// Drops the file at `path` on the window, as from Explorer.
    pub fn drop_file(h: &mut Harness<'_, Editor>, path: PathBuf) {
        #[derive(Debug)]
        struct Dropped(PathBuf);
        impl egui::DroppedFile for Dropped {
            fn path(&self) -> &std::path::Path {
                &self.0
            }
            fn bytes(&self) -> Result<Vec<u8>, String> {
                std::fs::read(&self.0).map_err(|e| e.to_string())
            }
        }
        h.input_mut().dropped_files.push(std::sync::Arc::new(Dropped(path)));
        h.step();
    }

    /// The box around everything on stage at the current frame.
    pub fn shape_bounds(ed: &Editor) -> hit::Rect {
        let all = ed.all_elements();
        hit::bounds(&ed.doc, ed.timeline(), ed.frame, &all).expect("something on stage")
    }

    /// Renders the window and saves it as target/screenshots/<name>.png. Needs a
    /// harness built with a renderer (`harness(true)` or `.wgpu()`).
    pub fn screenshot(h: &mut Harness<'_, Editor>, name: &str) {
        let image = h.render().expect("render");
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
        std::fs::create_dir_all(&dir).unwrap();
        image.save(dir.join(format!("{name}.png"))).unwrap();
    }

    /// The window as rendered, to read pixels from as `image[(x, y)]`. Needs a
    /// renderer; the pointer, if over the window, is drawn too.
    pub fn rendered(h: &mut Harness<'_, Editor>) -> ColorImage {
        let image = h.render().expect("render");
        ColorImage::from_rgba_unmultiplied([image.width() as usize, image.height() as usize], image.as_raw())
    }

    /// The texts painted last frame, each with the part of it that shows.
    pub fn painted_texts(h: &Harness<'_, Editor>) -> Vec<(String, Rect)> {
        fn walk(shape: &egui::Shape, clip: Rect, out: &mut Vec<(String, Rect)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.visual_bounding_rect().intersect(clip))),
                egui::Shape::Vec(shapes) => shapes.iter().for_each(|s| walk(s, clip, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for c in &h.output().shapes {
            walk(&c.shape, c.clip_rect, &mut out);
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::testkit::*;
    use super::*;

    #[test]
    fn opens_and_plays_a_document() {
        let mut h = harness(false);
        h.run();
        assert_eq!(h.state().title, "tween");
        assert!(h.state().error.is_none(), "{:?}", h.state().error);
        assert_eq!(h.state().frame_count(), 11);

        let now = h.ctx.input(|i| i.time);
        h.state_mut().toggle_play(now);
        h.step();
        assert!(h.state().playing.is_some());
        h.state_mut().set_frame(99);
        assert_eq!((h.state().frame, h.state().playing), (99, None), "scrubbing stops playback, and may pass the last frame");
        h.run();
    }

    #[test]
    fn undo_and_redo() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let ed = h.state_mut();
        assert!(!ed.can_undo() && !ed.dirty);
        ed.new_layer();
        assert_eq!(ed.timeline().layers.len(), 4);
        // Repeated edits from one field are one step.
        ed.checkpoint_gesture("fps");
        ed.doc.frame_rate = 12.0;
        ed.checkpoint_gesture("fps");
        ed.doc.frame_rate = 6.0;
        ed.end_gesture();
        ed.run(Action::Undo, &ctx);
        assert_eq!((ed.doc.frame_rate, ed.timeline().layers.len()), (24.0, 4));
        ed.run(Action::Undo, &ctx);
        assert_eq!(ed.timeline().layers.len(), 3);
        assert!(!ed.can_undo() && ed.can_redo());
        ed.run(Action::Redo, &ctx);
        assert_eq!(ed.timeline().layers.len(), 4);
    }


    #[test]
    fn draw_select_move_undo_delete() {
        let mut h = blank();

        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let ed = h.state();
        assert_eq!(ed.all_elements().len(), 1, "one rectangle drawn");
        let b = shape_bounds(ed);
        // The 1 px stroke sticks out half a pixel; pointer positions round to the screen grid.
        assert!((b.min.0 - 99.5).abs() < 1.0 && (b.max.1 - 150.5).abs() < 1.0, "{b:?}");

        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(h.state().selection.len(), 1, "clicking the fill selects it");

        mouse_drag(&mut h, (150.0, 125.0), (170.0, 145.0));
        let b = shape_bounds(h.state());
        assert!((b.min.0 - 119.5).abs() < 1.0 && (b.min.1 - 119.5).abs() < 1.0, "moved by (20, 20): {b:?}");

        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.step();
        assert!((shape_bounds(h.state()).min.0 - 99.5).abs() < 1.0, "undo puts it back");

        // F6 on frame 1's keyframe makes frame 2 a keyframe holding a copy.
        h.key_press(egui::Key::F6);
        h.step();
        let ed = h.state();
        assert_eq!((ed.frame, ed.timeline().layers[0].frames.len()), (1, 2));
        assert_eq!(ed.all_elements().len(), 1);

        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(egui::Key::Delete);
        h.step();
        assert!(h.state().all_elements().is_empty(), "Delete removes the selection");
    }

    #[test]
    fn f8_converts_the_selection_to_a_symbol() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(egui::Key::F8);
        h.step();
        assert!(h.state().convert.is_some(), "F8 opens Convert to Symbol");
        h.step();
        h.key_press(egui::Key::Enter);
        h.step();
        let ed = h.state();
        assert!(ed.convert.is_none());
        assert!(ed.doc.library.contains_key("Symbol 1"));
        let r = ed.selection[0];
        let el = &ed.timeline().layers[r.layer].frames[r.keyframe].elements[r.element];
        assert!(matches!(el, Element::Instance(i) if i.symbol == "Symbol 1"), "the stage now holds an instance");
    }

    #[test]
    fn renaming_a_layer() {
        let mut h = blank();
        h.state_mut().renaming = Some((0, "Layer_1".into()));
        h.step();
        // Set the field as typing would; Enter commits, trimmed.
        h.state_mut().renaming.as_mut().unwrap().1 = "  Background ".into();
        h.step();
        h.key_press(egui::Key::Enter);
        h.step();
        let ed = h.state();
        assert_eq!((ed.timeline().layers[0].name.as_str(), ed.renaming.is_none()), ("Background", true));
        assert!(ed.can_undo());
    }

    #[test]
    fn dragging_a_layer_reorders_the_timeline() {
        use crate::timeline::ROW_H;
        let mut h = harness(false);
        h.run();
        let names = |h: &Harness<'_, Editor>| h.state().timeline().layers.iter().map(|l| l.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&h), ["Blink", "Ball", "Background"]);
        let rows = h.state().layer_rows;
        let at = |y: f32| egui::pos2(rows.left() + 60.0, rows.top() + y);
        // Drag the top layer to just past the second row's bottom edge.
        h.hover_at(at(ROW_H * 0.5));
        h.step();
        h.drag_at(at(ROW_H * 0.5));
        h.step();
        for y in [ROW_H, ROW_H * 1.5, ROW_H * 2.1] {
            h.hover_at(at(y));
            h.step();
        }
        h.drop_at(at(ROW_H * 2.1));
        h.step();
        assert_eq!(names(&h), ["Ball", "Blink", "Background"]);
        assert_eq!(h.state().layer, 1, "the moved layer stays selected");
    }

    /// Each layer's keyframes as (index, duration).
    fn keys(ed: &Editor) -> Vec<Vec<(u32, u32)>> {
        ed.timeline().layers.iter().map(|l| l.frames.iter().map(|k| (k.index, k.duration)).collect()).collect()
    }

    #[test]
    fn selecting_frames_and_running_commands_on_them() {
        use egui::Modifiers;
        let mut h = harness(false);
        h.run();
        let before = keys(h.state());
        assert_eq!(before, [vec![(0, 11)], vec![(0, 10), (10, 1)], vec![(0, 11)]]);

        // Dragging across cells selects a block of layers and frames; the playhead follows the press.
        drag_cells(&mut h, (0, 2), (1, 4), Modifiers::NONE);
        let ed = h.state();
        let block: BTreeSet<(usize, u32)> = (0..2).flat_map(|l| (2..5).map(move |f| (l, f))).collect();
        assert_eq!((&ed.tl.sel, ed.layer, ed.frame), (&block, 0, 2));

        // F5 inserts as many frames as are selected, on each selected layer.
        h.key_press(Key::F5);
        h.step();
        assert_eq!(keys(h.state()), [vec![(0, 14)], vec![(0, 13), (13, 1)], vec![(0, 11)]]);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert_eq!(keys(h.state()), before);

        // Shift+click extends from the last click to a rectangle; Ctrl+click toggles one frame.
        click_cell(&mut h, (0, 1), Modifiers::NONE);
        click_cell(&mut h, (2, 3), Modifiers::SHIFT);
        assert_eq!(h.state().tl.sel.len(), 9);
        click_cell(&mut h, (1, 2), Modifiers::COMMAND);
        assert_eq!(h.state().tl.sel.len(), 8);

        // Clicking a layer's name selects all its frames; Clear Frames empties them.
        let (rows, row_h) = (h.state().layer_rows, h.state().tl.row_h);
        let name = |row: f32| egui::pos2(rows.left() + 80.0, rows.top() + row_h * row);
        screen_click(&mut h, name(1.5), Modifiers::NONE);
        let ed = h.state();
        assert_eq!((ed.layer, ed.tl.sel.len()), (1, 11));
        // Ctrl+click adds a layer and its frames; again, it takes them away and the other stays active.
        screen_click(&mut h, name(2.5), Modifiers::COMMAND);
        assert_eq!((h.state().layer, h.state().tl.sel.len()), (2, 22));
        screen_click(&mut h, name(2.5), Modifiers::COMMAND);
        assert_eq!((h.state().layer, h.state().tl.sel.len()), (1, 11));
        h.key_press_modifiers(Modifiers::ALT, Key::Backspace);
        h.step();
        let ball = &h.state().timeline().layers[1].frames;
        assert!(ball.len() == 1 && ball[0].elements.is_empty() && ball[0].duration == 11, "{ball:?}");

        // Copy two frames of Blink and paste them over Background's frames 5 and 6.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        drag_cells(&mut h, (0, 3), (0, 4), Modifiers::NONE);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::C);
        h.step();
        click_cell(&mut h, (2, 5), Modifiers::NONE);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::PasteOverwriteFrames, &ctx);
        let ed = h.state();
        assert_eq!(keys(ed)[2], [(0, 5), (5, 2), (7, 4)]);
        assert_eq!(ed.timeline().layers[2].frames[1].elements, ed.timeline().layers[0].frames[0].elements);
    }

    #[test]
    fn timeline_commands_on_odd_timelines() {
        use Action::*;
        let all = [
            InsertFrame, RemoveFrames, InsertKeyframe, InsertBlankKeyframe, ClearKeyframe, CreateClassicTween, RemoveTween,
            ConvertToKeyframes, ConvertToBlankKeyframes, CopyFrames, CutFrames, PasteFrames, PasteOverwriteFrames, ClearFrames,
            SelectAllFrames, ReverseFrames, PrevKeyframe, NextKeyframe, CopyLayers, DuplicateLayers, CutLayers, PasteLayers,
            ShowAllLayers, LockOthers, HideOthers, ToggleGuide, LayerProperties, DeleteLayer, NewFolder,
        ];
        // The playhead past the end; no layers at all; only a folder.
        let setups: [fn(&mut Editor); 3] = [
            |ed| ed.frame = 40,
            |ed| ed.doc.scenes[0].layers.clear(),
            |ed| {
                ed.doc.scenes[0].layers.clear();
                ed.doc.scenes[0].insert_folder(0, "Folder_1");
            },
        ];
        for setup in setups {
            let mut h = harness(false);
            h.run();
            let ctx = h.ctx.clone();
            setup(h.state_mut());
            h.run();
            for action in all {
                h.state_mut().run(action, &ctx);
                h.run();
            }
        }
    }

    #[test]
    fn ctrl_alt_x_c_v_also_arrive_as_clipboard_events() {
        use egui::Modifiers;
        // In the app, egui-winit turns Ctrl+Alt+C into a Copy event, not a key press.
        let clipboard = |h: &mut Harness<'_, Editor>, mods, event| {
            h.event(egui::Event::ModifiersChanged(mods));
            h.event(event);
            h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
            h.step();
        };
        let mut h = harness(false);
        h.run();
        click_cell(&mut h, (0, 0), Modifiers::NONE);
        // Plain Ctrl+C is the stage's Copy, not Copy Frames.
        clipboard(&mut h, Modifiers::COMMAND, egui::Event::Copy);
        assert!(!timeline::available(h.state(), Action::PasteFrames));
        clipboard(&mut h, Modifiers::COMMAND | Modifiers::ALT, egui::Event::Copy);
        click_cell(&mut h, (2, 3), Modifiers::NONE);
        clipboard(&mut h, Modifiers::COMMAND | Modifiers::ALT, egui::Event::Paste("Minnamate frames".into()));
        assert_eq!(keys(h.state())[2], [(0, 3), (3, 1), (4, 8)]);
    }

    #[test]
    fn dragging_frames_moves_them_and_alt_copies() {
        use egui::Modifiers;
        let mut h = harness(false);
        h.run();
        // Select the tween's end keyframe, then drag it out: the tween gets longer.
        click_cell(&mut h, (1, 10), Modifiers::NONE);
        drag_cells(&mut h, (1, 10), (1, 14), Modifiers::NONE);
        let ed = h.state();
        assert_eq!(keys(ed)[1], [(0, 14), (14, 1)]);
        assert!(matches!(ed.timeline().layers[1].frames[0].tween, minnamate_core::doc::Tween::Classic { .. }));
        assert_eq!((ed.tl.sel.iter().copied().collect::<Vec<_>>(), ed.layer), (vec![(1, 14)], 1));

        // Alt+drag copies Blink's keyframe to frame 5.
        click_cell(&mut h, (0, 0), Modifiers::NONE);
        drag_cells(&mut h, (0, 0), (0, 5), Modifiers::ALT);
        let ed = h.state();
        assert_eq!(keys(ed)[0], [(0, 5), (5, 6)]);
        assert_eq!(ed.timeline().layers[0].frames[0].elements, ed.timeline().layers[0].frames[1].elements);
        assert!(ed.can_undo());
    }

    #[test]
    fn loop_range_and_layer_toggles() {
        use egui::Modifiers;
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        // Playback loops inside the loop range.
        let ed = h.state_mut();
        ed.loop_playback = true;
        ed.tl.loop_range = Some((2, 4));
        ed.frame = 2;
        ed.playing = Some((ctx.input(|i| i.time) - 5.5 / 24.0, 2));
        ed.tick(&ctx);
        assert_eq!(h.state().frame, 4, "frames 2, 3, 4, 2, 3, 4: the 6th is 4");
        h.state_mut().playing = None;
        let rows = h.state().layer_rows;

        // In the ruler's top tier: dragging the end handle moves the end, a click only
        // moves the playhead, and a drag elsewhere makes a new range.
        let cell_w = h.state().cell_w;
        let strip = |frames: f32| egui::pos2(rows.right() + frames * cell_w, rows.top() - 26.0);
        screen_drag(&mut h, strip(5.0), strip(8.5), Modifiers::NONE);
        assert_eq!(h.state().tl.loop_range, Some((2, 8)));
        screen_click(&mut h, strip(11.5), Modifiers::NONE);
        assert_eq!((h.state().tl.loop_range, h.state().frame), (Some((2, 8)), 10));
        screen_drag(&mut h, strip(5.5), strip(7.5), Modifiers::NONE);
        assert_eq!(h.state().tl.loop_range, Some((5, 7)));

        // Dragging down the eye column hides every layer passed; Alt+click solos one layer.
        let row_h = h.state().tl.row_h;
        let eye = |row: f32| egui::pos2(rows.right() - 51.0, rows.top() + row_h * row);
        screen_drag(&mut h, eye(0.5), eye(2.5), Modifiers::NONE);
        assert!(h.state().timeline().layers.iter().all(|l| !l.visible()));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert!(h.state().timeline().layers.iter().all(|l| l.visible()), "one undo step for the whole drag");
        screen_click(&mut h, eye(1.5), Modifiers::ALT);
        let shown: Vec<bool> = h.state().timeline().layers.iter().map(|l| l.visible()).collect();
        assert_eq!(shown, [false, true, false]);
        screen_click(&mut h, eye(1.5), Modifiers::ALT);
        assert!(h.state().timeline().layers.iter().all(|l| l.visible()), "Alt+click again restores");

        // New Folder, then Delete removes the selected layers.
        h.state_mut().run(Action::NewFolder, &ctx);
        let ed = h.state();
        assert_eq!((ed.timeline().layers[0].kind, ed.timeline().layers[0].name.as_str()), (LayerKind::Folder, "Folder_1"));
        h.state_mut().run(Action::DeleteLayer, &ctx);
        assert_eq!(h.state().timeline().layers.len(), 3);
    }

    #[test]
    fn holding_the_onion_button_opens_its_menu() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(false);
        h.run();
        let at = h.state().tl.onion_button.center();
        h.hover_at(at);
        h.step();
        h.drag_at(at);
        h.run_steps(60);
        h.drop_at(at);
        h.run();
        assert!(!h.state().onion, "a long press is not a click");
        h.get_by_label("Advanced Settings").click();
        h.run();
        assert!(h.state().tl.onion_window);
    }

    #[test]
    fn layer_properties_dialog() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().layer = 1;
        h.state_mut().run(Action::LayerProperties, &ctx);
        h.run();
        h.get_by_label("Lock").click();
        h.get_by_label("Guide").click();
        h.run();
        h.key_press(Key::Enter);
        h.run();
        let ball = &h.state().timeline().layers[1];
        assert!(ball.locked() && ball.kind == LayerKind::Guide, "{ball:?}");
        h.key_press_modifiers(egui::Modifiers::COMMAND, Key::Z);
        h.step();
        assert_eq!(h.state().timeline().layers[1].kind, LayerKind::Normal);
    }

    #[test]
    fn editing_a_symbol_in_place() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(egui::Key::F8);
        h.step();
        h.step();
        h.key_press(egui::Key::Enter);
        h.step();
        assert!(h.state().doc.library.contains_key("Symbol 1"));

        // Double-click the instance: now the timeline is the symbol's.
        let p = on_stage(&h, (150.0, 125.0));
        h.hover_at(p);
        h.step();
        for _ in 0..2 {
            h.drag_at(p);
            h.drop_at(p);
            h.step();
        }
        assert_eq!(h.state().editing.len(), 1, "double-click enters the symbol");
        assert_eq!(h.state().timeline().name, "Symbol 1");

        // Drawing lands in the symbol, in its own coordinates (registration at the box's top left).
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (120.0, 110.0), (140.0, 130.0));
        let sym = &h.state().doc.library["Symbol 1"].timeline;
        assert_eq!(sym.layers[0].frames[0].elements.len(), 2);
        let b = hit::bounds(&h.state().doc, sym, 0, &[hit::ElementRef { layer: 0, keyframe: 0, element: 1 }]).unwrap();
        assert!((b.min.0 - 20.0).abs() < 1.5 && (b.min.1 - 10.0).abs() < 1.5, "symbol coordinates: {b:?}");

        h.key_press_modifiers(egui::Modifiers::COMMAND, egui::Key::Z);
        h.step();
        assert_eq!(h.state().doc.library["Symbol 1"].timeline.layers[0].frames[0].elements.len(), 1, "undo inside the symbol");
        h.state_mut().exit_symbol();
        assert!(h.state().editing.is_empty());
        assert_eq!(h.state().timeline().name, "Scene 1");
    }

    #[test]
    fn save_reopen_and_the_unsaved_changes_prompt() {
        let mut h = blank();
        let ctx = h.ctx.clone();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        assert!(h.state().dirty);

        // Close with unsaved changes asks first; Cancel keeps the drawing.
        h.state_mut().run(Action::Close, &ctx);
        assert_eq!(h.state().confirm, Some(Action::Close));
        h.state_mut().resolve(Answer::Cancel, &ctx);
        assert_eq!((h.state().all_elements().len(), h.state().confirm), (1, None));

        let dir = std::env::temp_dir().join(format!("minnamate-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("drawing.fla");
        assert!(h.state_mut().save_to(path.clone()), "{:?}", h.state().error);
        assert_eq!((h.state().title.as_str(), h.state().dirty), ("drawing.fla", false));

        // Saved, so Close goes straight through to Home; then reopen the file.
        h.state_mut().run(Action::Close, &ctx);
        assert!(h.state().confirm.is_none() && h.state().home.shown && h.state().all_elements().is_empty());
        h.state_mut().open(path.clone());
        h.step();
        assert_eq!(h.state().all_elements().len(), 1, "the rectangle came back from the .fla");
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Writes target/screenshots/labels.png: frame labels on the timeline and in Properties.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn labels_screenshot() {
        let mut h = harness(true);
        let ed = h.state_mut();
        ed.timeline_mut().layers[1].frames[0].set_label("intro", "name");
        ed.timeline_mut().layers[2].frames[0].set_label("background", "comment");
        ed.layer = 1;
        ed.props_tab = PropsTab::Frame;
        h.run();
        screenshot(&mut h, "labels");
    }

    /// Writes target/screenshots/onion.png: the tween fixture with onion skin on.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn onion_screenshot() {
        let mut h = harness(true);
        h.state_mut().set_frame(5);
        h.state_mut().onion = true;
        h.run();
        screenshot(&mut h, "onion");
    }

    #[test]
    fn lasso_selects_what_it_encloses() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (50.0, 50.0), (100.0, 100.0));
        mouse_drag(&mut h, (300.0, 50.0), (350.0, 100.0));
        h.state_mut().tool = Tool::Lasso;
        mouse_path(&mut h, &[(30.0, 30.0), (130.0, 30.0), (130.0, 130.0), (30.0, 130.0)]);
        let ed = h.state();
        assert_eq!(ed.selection.len(), 1, "only the left rectangle is inside the loop");
        let b = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection).unwrap();
        assert!(b.max.0 < 150.0, "{b:?}");
    }

    #[test]
    fn test_movie_plays_in_its_own_window() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::TestMovie, &ctx);
        // The player keeps requesting frames, so step instead of running to idle.
        for _ in 0..3 {
            h.step();
        }
        assert!(h.state().test.is_some());
    }

    /// Writes target/screenshots/editor.png, to compare the UI against Animate by eye.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn editor_screenshot() {
        let mut h = harness(true);
        h.state_mut().set_frame(5);
        h.run();
        screenshot(&mut h, "editor");
    }

    /// Writes target/screenshots/timeline.png (a block of frames selected, the
    /// frame menu open), timeline-menu.png (the panel menu open),
    /// timeline-onion.png (Onion skin settings) and timeline-props.png (Layer Properties).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn timeline_screenshot() {
        let mut h = harness(true);
        h.run();
        h.state_mut().loop_playback = true;
        h.state_mut().tl.loop_range = Some((1, 7));
        drag_cells(&mut h, (0, 2), (2, 6), egui::Modifiers::NONE);
        let at = cell(&h, 1, 6);
        h.hover_at(at);
        h.step();
        for pressed in [true, false] {
            h.event(egui::Event::PointerButton { pos: at, button: egui::PointerButton::Secondary, pressed, modifiers: egui::Modifiers::NONE });
            h.step();
        }
        h.run();
        screenshot(&mut h, "timeline");

        h.key_press(Key::Escape);
        h.run();
        // The panel menu's glyph, at the right end of the tab strip over the toolbar and ruler.
        let (rows, width) = (h.state().layer_rows, h.state().view_size.x);
        let glyph = egui::pos2(rows.left() + width - 14.0, rows.top() - 33.0 - 34.0 - 11.0);
        screen_click(&mut h, glyph, egui::Modifiers::NONE);
        h.run();
        screenshot(&mut h, "timeline-menu");

        h.key_press(Key::Escape);
        let ctx = h.ctx.clone();
        h.state_mut().tl.onion_window = true;
        h.run();
        screenshot(&mut h, "timeline-onion");
        h.state_mut().tl.onion_window = false;
        h.state_mut().run(Action::LayerProperties, &ctx);
        h.run();
        screenshot(&mut h, "timeline-props");
    }

    /// Writes target/screenshots/menu.png: the Edit menu open, in the light menu style.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn menu_screenshot() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        h.get_by_label("Edit").click();
        h.run();
        screenshot(&mut h, "menu");
    }

    /// Writes target/screenshots/modify_menu.png: Modify > Transform open, with a selection.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn modify_menu_screenshot() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        h.state_mut().selection = vec![ElementRef { layer: 1, keyframe: 0, element: 0 }];
        h.get_by_label("Modify").click();
        h.run();
        // A submenu's label ends in its arrow.
        h.get_by_label("Transform ⏵").hover();
        h.run();
        screenshot(&mut h, "modify_menu");
    }

    /// Writes target/screenshots/arrange_panels.png: the Align panel open from the
    /// icon strip, and the workspace switcher open.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn arrange_panels_screenshot() {
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        let ed = h.state_mut();
        ed.selection = vec![ElementRef { layer: 1, keyframe: 0, element: 0 }];
        ed.run(Action::Show(PanelId::Align), &ctx);
        h.run();
        // The workspace button sits left of the three 46 px window buttons and a 12 px gap.
        let at = egui::pos2(1600.0 - 3.0 * 46.0 - 12.0 - 14.0, theme::APP_BAR_H / 2.0);
        h.hover_at(at);
        h.step();
        h.drag_at(at);
        h.drop_at(at);
        h.run();
        screenshot(&mut h, "arrange_panels");
    }

    /// However narrow the Timeline, its toolbar draws no control over another
    /// or past the panel: the zoom controls, the readouts and then cluster
    /// groups give way, and Play stays (docs/animate-ui.md §5.2).
    #[test]
    fn a_narrow_timeline_toolbar_gives_way_instead_of_overlapping() {
        use egui_kittest::kittest::Queryable;
        const FIRST: &str = "Go to Previous Keyframe (Alt+,)";
        const LAST: &str = "Step forward one frame (.)";
        let buttons = [
            "New Layer", "Add/Remove Camera", "Show Parenting View", "Layer Depth", FIRST, "Insert Keyframe (hold for more)",
            "Go to Next Keyframe (Alt+.)", "Onion Skin (hold for more)", "Edit Multiple Frames", "Create Classic Tween", "Center Frame",
            "Loop", "Step back one frame (,)", "Play (Enter)", LAST, "Reset Timeline Zoom To The Default Level",
        ];
        let mut h = harness(false);
        for size in [vec2(1600.0, 900.0), vec2(1366.0, 768.0), vec2(1280.0, 800.0), vec2(960.0, 600.0)] {
            h.set_size(size);
            h.run();
            let (rows, width) = (h.state().layer_rows, h.state().view_size.x);
            let (panel, frames) = (rows.left()..=rows.left() + width, rows.right()..=rows.left() + width);
            let shown: Vec<(&str, Rect)> = buttons.iter().filter_map(|&b| h.query_by_label(b).map(|n| (b, n.rect()))).collect();
            // The readouts: what's painted in the toolbar row, over the frames.
            let bar = rows.top() - 33.0 - 34.0..=rows.top() - 33.0;
            let texts: Vec<Rect> = painted_texts(&h).into_iter().map(|(_, r)| r).filter(|r| r.is_positive() && bar.contains(&r.center().y) && frames.contains(&r.left())).collect();
            assert!(shown.iter().any(|(b, _)| *b == "Play (Enter)"), "{size:?}: Play is gone");
            for (i, &(a, ra)) in shown.iter().enumerate() {
                assert!(panel.contains(&ra.left()) && panel.contains(&ra.right()), "{size:?}: {a} is off the panel");
                for &(b, rb) in &shown[i + 1..] {
                    assert!(!ra.shrink(0.5).intersects(rb.shrink(0.5)), "{size:?}: {a} is on {b}");
                }
                for t in &texts {
                    assert!(!ra.intersects(*t), "{size:?}: {a} is on a readout");
                }
            }
            if size.x == 1600.0 {
                assert_eq!(shown.len(), buttons.len(), "all fit at full width");
                let middle = (shown[4].1.left() + shown[14].1.right()) / 2.0;
                assert!((middle - (frames.start() + frames.end()) / 2.0).abs() <= 1.0, "the cluster is centered over the frames");
                assert_eq!((shown[4].0, shown[14].0), (FIRST, LAST));
            }
        }
    }

    /// Panel tabs: the active label #F3F3F3, the others #818181 (docs/animate-ui.md §4.6).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn panel_tabs_dim_all_but_the_active_label() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        let image = rendered(&mut h);
        let brightest = |label: &str| {
            let r = h.get_by_label(label).rect();
            let xs = r.left() as usize..r.right() as usize;
            (r.top() as usize..r.bottom() as usize).flat_map(|y| xs.clone().map(move |x| (x, y))).map(|p| image[p].r()).max().unwrap()
        };
        for label in ["Library", "Assets", "Output"] {
            assert!((0x70..=0x86).contains(&brightest(label)), "{label}: {:#X}", brightest(label));
        }
        for label in ["Properties", "Timeline"] {
            assert!(brightest(label) >= 0xF0, "{label}: {:#X}", brightest(label));
        }
    }

    /// The frame grid's 1 px #252525 lines (docs/animate-ui.md §5.4, §5.6):
    /// between empty frames, between rows in the grid and in the layer list,
    /// and under the layer header. Selected frames over a span stay half blue
    /// up to them.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn frame_grid_lines_show() {
        let mut h = harness(true);
        h.run();
        drag_cells(&mut h, (0, 2), (2, 6), egui::Modifiers::NONE);
        h.remove_cursor();
        h.run();
        let image = rendered(&mut h);
        let ed = h.state();
        let (rows, row) = (ed.layer_rows, ed.tl.row_h);
        let x_of = |f: f32| rows.right() + (f - ed.first_visible_frame) * ed.cell_w;
        let line = Color32::from_gray(0x25);
        let px = |x: f32, y: f32| image[(x as usize, y as usize)];
        // Frame 21 is empty on every layer.
        let (x, edge) = (x_of(21.0), rows.top() + row);
        assert!([x - 1.0, x].iter().any(|&x| px(x, rows.top() + row / 2.0) == line), "between frames");
        for x in [x_of(20.5), rows.left() + 150.0] {
            assert!([edge - 1.0, edge].iter().any(|&y| px(x, y) == line), "between rows at x {x}");
        }
        assert_eq!(px(rows.left() + 150.0, rows.top() - 1.0), line, "under the layer header");
        assert_ne!(px(rows.left() + 150.0, rows.top()), theme::FRAME_CELL, "the first row starts under it");
        for y in [edge - 1.0, edge] {
            assert_ne!(px(x_of(4.5), y), theme::FRAME_SELECTED, "a selected span's edge at y {y}");
        }
    }

    /// The edit bar's small glyphs come out at about their color, not faded:
    /// the menu buttons' carets (text_dim) and the zoom steps (text).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn small_chrome_glyphs_are_not_faint() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        let image = rendered(&mut h);
        let brightest = |r: Rect| {
            let xs = r.left() as usize..r.right() as usize;
            (r.top() as usize..r.bottom() as usize).flat_map(|y| xs.clone().map(move |x| (x, y))).map(|p| image[p].r()).max().unwrap()
        };
        for label in ["Edit Scene", "Edit Symbols"] {
            let button = h.get_by_label(label).rect();
            let caret = Rect::from_min_max(egui::pos2(button.left() + 19.0, button.top()), button.max);
            assert!(brightest(caret) >= 0x88, "{label}'s caret: {:#X}", brightest(caret));
        }
        for label in ["Zoom in", "Zoom out"] {
            let glyph = brightest(h.get_by_label(label).rect());
            assert!(glyph >= 0xC8, "{label}: {glyph:#X}");
        }
    }

    /// Separators in the dark popovers are #3F3F3F, to show on #1E1E1E
    /// (docs/animate-ui.md §4.2 item 7).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn dark_popover_separators_show() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        h.get_by_label("Workspaces").click();
        h.run();
        let label = h.get_by_label("New Workspace").rect();
        h.remove_cursor();
        h.run();
        let image = rendered(&mut h);
        // Between the last workspace and "New Workspace".
        let x = label.center().x as usize;
        let brightest = (label.top() as usize - 12..label.top() as usize).map(|y| image[(x, y)].r()).max().unwrap();
        assert!(brightest >= 0x3A, "{brightest:#X}");
    }

    /// A menu's button keeps its pressed fill while the menu is open, wherever
    /// the pointer goes (docs/animate-ui.md §6.5, §4.2).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn menu_buttons_stay_pressed_while_their_menu_is_open() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        for (label, secondary) in [("Edit Scene", false), ("Insert Keyframe (hold for more)", true), ("Workspaces", false)] {
            let rect = {
                let button = h.get_by_label(label);
                if secondary { button.click_secondary() } else { button.click() }
                button.rect()
            };
            h.run();
            h.remove_cursor();
            h.run();
            let image = rendered(&mut h);
            assert_eq!(image[(rect.left() as usize + 2, rect.center().y as usize)], theme::BUTTON_ACTIVE, "{label}");
            // A click on the pasteboard closes it.
            screen_click(&mut h, egui::pos2(300.0, 590.0), egui::Modifiers::NONE);
            h.run();
        }
    }

    /// A dialog's OK button is the blue call to action (docs/animate-ui.md §4.7).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn dialog_ok_buttons_are_blue() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        let dialogs: [(&str, fn(&mut Editor, &egui::Context)); 3] = [
            ("    OK    ", |ed, ctx| ed.run(Action::LayerProperties, ctx)),
            ("    OK    ", |ed, _| {
                ed.selection = vec![ElementRef { layer: 1, keyframe: 0, element: 0 }];
                crate::convert_symbol::open(ed);
            }),
            ("    Save    ", |ed, _| (ed.dirty, ed.confirm) = (true, Some(Action::Close))),
        ];
        for (label, open) in dialogs {
            open(h.state_mut(), &ctx);
            h.run();
            let rect = h.get_by_label(label).rect();
            let image = rendered(&mut h);
            assert_eq!(image[(rect.left() as usize + 3, rect.center().y as usize)], theme::CTA, "{label}");
            h.key_press(Key::Escape);
            h.run();
        }
    }

    /// Properties > Object names a raw shape "Shape" and an Object Drawing one
    /// "Drawing Object" (docs/animate-ui.md §7.2.4).
    #[test]
    fn the_object_tab_tells_a_shape_from_a_drawing_object() {
        use egui_kittest::kittest::Queryable;
        for (object_drawing, name) in [(false, "Shape"), (true, "Drawing Object")] {
            let mut h = blank();
            (h.state_mut().object_drawing, h.state_mut().tool) = (object_drawing, Tool::Rectangle);
            mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
            h.state_mut().tool = Tool::Selection;
            mouse_click(&mut h, (150.0, 125.0));
            h.state_mut().props_tab = PropsTab::Object;
            h.run();
            assert_eq!(h.state().selection.len(), 1);
            assert!(h.query_by_label(name).is_some(), "{name}");
        }
    }

    /// Every panel menu opens and ends with Lock, Help, Close and Close Group,
    /// Properties' group and the icon strip's flyouts included (docs/animate-ui.md §4.6).
    #[test]
    fn every_panel_menu_opens() {
        use egui_kittest::kittest::Queryable;
        let mut h = harness(false);
        h.run();
        h.get_by_label("Properties panel menu").click();
        h.run();
        assert!(h.query_by_label("Lock").is_some());
        h.get_by_label("Close Group").click();
        h.run();
        assert!(!h.state().show_right);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Show(PanelId::Align), &ctx);
        h.run();
        h.get_by_label("Align panel menu").click();
        h.run();
        h.get_by_label("Close").click();
        h.run();
        assert!(h.state().flyout.is_none());
    }

    /// The blank document with rectangles drawn from (x0, y0) to (x1, y1), in order,
    /// and the Selection tool picked.
    fn rectangles(boxes: &[(f32, f32, f32, f32)]) -> Harness<'static, Editor> {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        for &(x0, y0, x1, y1) in boxes {
            mouse_drag(&mut h, (x0, y0), (x1, y1));
        }
        h.state_mut().tool = Tool::Selection;
        h
    }

    fn selected_bounds(ed: &Editor) -> hit::Rect {
        hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection).expect("a selection on stage")
    }

    fn near(a: Point, b: Point) -> bool {
        (a.0 - b.0).abs() < 0.01 && (a.1 - b.1).abs() < 0.01
    }

    #[test]
    fn copy_cut_paste_and_duplicate() {
        use egui::{Event, Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        let original = selected_bounds(h.state());
        h.key_press_modifiers(Modifiers::COMMAND, Key::C);
        h.step();
        assert_eq!(h.state().clipboard.len(), 1);

        // Ctrl+V: the copy lands in the middle of the view (the stage's, here), selected.
        h.key_press_modifiers(Modifiers::COMMAND, Key::V);
        h.step();
        let b = selected_bounds(h.state());
        assert_eq!(h.state().all_elements().len(), 2);
        assert!(near(((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0), (275.0, 200.0)), "{b:?}");
        // Ctrl+Shift+V: where it was copied from.
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::V);
        h.step();
        assert_eq!(selected_bounds(h.state()), original);
        assert_eq!(h.state().all_elements().len(), 3);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert_eq!(h.state().all_elements().len(), 2, "a paste is one undo step");

        // In the app Ctrl+X, C and V arrive as clipboard events; Shift held pastes in place.
        mouse_click(&mut h, (150.0, 125.0));
        h.event(Event::Cut);
        h.step();
        assert_eq!(h.state().all_elements().len(), 1);
        h.event_modifiers(Event::Paste("<DOMShape/>".into()), Modifiers::COMMAND | Modifiers::SHIFT);
        h.step();
        assert_eq!((h.state().all_elements().len(), selected_bounds(h.state())), (2, original));

        // Ctrl+D: a copy 10 px right and down, on top.
        h.key_press_modifiers(Modifiers::COMMAND, Key::D);
        h.step();
        let ed = h.state();
        let copy = selected_bounds(ed);
        assert_eq!((ed.all_elements().len(), ed.selection[0].element), (3, 2));
        assert!(near(copy.min, (original.min.0 + 10.0, original.min.1 + 10.0)), "{copy:?}");
    }

    #[test]
    fn group_ungroup_and_break_apart() {
        use egui::{Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0), (300.0, 200.0, 350.0, 260.0)]);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::SelectAll, &ctx);
        let both = selected_bounds(h.state());
        h.key_press_modifiers(Modifiers::COMMAND, Key::G);
        h.step();
        let ed = h.state();
        assert_eq!((ed.all_elements().len(), ed.selection.len()), (1, 1));
        assert!(matches!(ed.element(ed.selection[0]), Some(Element::Group(m)) if m.len() == 2));
        assert_eq!(selected_bounds(ed), both);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::G);
        h.step();
        assert_eq!((h.state().all_elements().len(), h.state().selection.len()), (2, 2));

        // A symbol instance breaks apart into its shapes, where it showed them.
        mouse_click(&mut h, (150.0, 125.0));
        let rect = selected_bounds(h.state());
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        assert!(matches!(h.state().element(h.state().selection[0]), Some(Element::Instance(_))));
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        h.step();
        let ed = h.state();
        assert!(matches!(ed.element(ed.selection[0]), Some(Element::Shape(_))));
        assert!(near(selected_bounds(ed).min, rect.min) && near(selected_bounds(ed).max, rect.max));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert!(h.state().all_elements().iter().any(|&r| matches!(h.state().element(r), Some(Element::Instance(_)))));
    }

    #[test]
    fn arrange_restacks_and_keeps_the_selection() {
        use egui::{Key, Modifiers};
        // Three overlapping rectangles, a under b under c; pick a by its corner nothing covers.
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0), (150.0, 120.0, 250.0, 170.0), (170.0, 130.0, 270.0, 180.0)]);
        mouse_click(&mut h, (110.0, 110.0));
        let a = selected_bounds(h.state());
        let place = |h: &Harness<'_, Editor>| h.state().selection[0].element;
        assert_eq!(place(&h), 0);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowUp);
        h.step();
        assert_eq!((place(&h), selected_bounds(h.state())), (2, a), "brought to front, still selected");
        h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowDown);
        h.step();
        assert_eq!(place(&h), 1);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowDown);
        h.step();
        assert_eq!(place(&h), 0);
        // Already at the back: nothing changes, so there's nothing to undo.
        let steps = h.state().undo.len();
        h.key_press_modifiers(Modifiers::COMMAND, Key::ArrowDown);
        h.step();
        assert_eq!((place(&h), h.state().undo.len()), (0, steps));
        // Undo puts it back one place, still selected: the selection comes back with the step.
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert_eq!((place(&h), selected_bounds(h.state())), (1, a));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Y);
        h.step();
        assert_eq!((place(&h), selected_bounds(h.state())), (0, a), "and so does redo");
        // Undoing a delete selects what came back.
        h.key_press(Key::Delete);
        h.step();
        assert_eq!(h.state().all_elements().len(), 2);
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        assert_eq!((place(&h), selected_bounds(h.state())), (0, a));
    }

    #[test]
    fn align_to_each_other_and_to_the_stage() {
        use egui::{Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0), (300.0, 200.0, 350.0, 260.0)]);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::SelectAll, &ctx);
        let lefts = |h: &Harness<'_, Editor>| {
            let ed = h.state();
            ed.selection.iter().map(|&r| hit::bounds(&ed.doc, ed.timeline(), ed.frame, &[r]).unwrap()).collect::<Vec<_>>()
        };
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::Num1);
        h.step();
        let boxes = lefts(&h);
        assert!((boxes[0].min.0 - boxes[1].min.0).abs() < 0.01, "left edges line up: {boxes:?}");
        // Ctrl+Alt+8 turns on Align to Stage; then Top puts both at the stage's top.
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::Num8);
        h.step();
        assert!(h.state().align_to_stage);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::Num4);
        h.step();
        assert!(lefts(&h).iter().all(|b| b.min.1.abs() < 0.01), "{:?}", lefts(&h));
        // Ctrl+Alt+Shift+7: both as wide as the stage.
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT, Key::Num7);
        h.step();
        assert!(lefts(&h).iter().all(|b| (b.max.0 - b.min.0 - 550.0).abs() < 0.01), "{:?}", lefts(&h));

        // The panels open from the Window menu's keys, one at a time, as flyouts of the icon strip.
        use crate::panels::Flyout;
        h.key_press_modifiers(Modifiers::COMMAND, Key::K);
        h.run();
        assert_eq!(h.state().flyout, Some(Flyout::Align));
        h.key_press_modifiers(Modifiers::COMMAND, Key::I);
        h.run();
        assert_eq!(h.state().flyout, Some(Flyout::Info));
        h.key_press_modifiers(Modifiers::COMMAND, Key::I);
        h.run();
        assert_eq!(h.state().flyout, None, "the same key closes it");
    }

    #[test]
    fn scale_and_rotate_then_remove_transform() {
        use egui::{Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        let b = selected_bounds(h.state());
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT, Key::S);
        h.step();
        assert_eq!(h.state().scale_rotate, Some((100.0, 0.0)));
        h.state_mut().scale_rotate = Some((200.0, 90.0));
        h.step();
        h.key_press(Key::Enter);
        h.step();
        let ed = h.state();
        let turned = selected_bounds(ed);
        assert!(ed.scale_rotate.is_none());
        // Twice the size and a quarter turn, about the middle.
        let (w, ht) = (b.max.0 - b.min.0, b.max.1 - b.min.1);
        assert!((turned.max.0 - turned.min.0 - 2.0 * ht).abs() < 0.01 && (turned.max.1 - turned.min.1 - 2.0 * w).abs() < 0.01, "{turned:?}");
        // Ctrl+Shift+Z takes it all off again, the middle staying put.
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::Z);
        h.step();
        let back = selected_bounds(h.state());
        assert!(near(back.min, b.min) && near(back.max, b.max), "{back:?}");
    }

    #[test]
    fn new_symbols_scenes_and_edit_symbols() {
        use egui::{Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        // Ctrl+F8 makes an empty symbol and opens it.
        h.key_press_modifiers(Modifiers::COMMAND, Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        let ed = h.state();
        assert!(ed.doc.library.contains_key("Symbol 1"));
        assert_eq!((ed.editing.len(), ed.timeline().name.as_str()), (1, "Symbol 1"));
        // Ctrl+E inside a symbol reads Edit Document and goes back.
        h.key_press_modifiers(Modifiers::COMMAND, Key::E);
        h.step();
        assert!(h.state().editing.is_empty());

        // Insert > Scene adds one after this and goes there; undo takes it away.
        h.state_mut().run(Action::InsertScene, &ctx);
        let ed = h.state();
        assert_eq!((ed.doc.scenes.len(), ed.scene, ed.doc.scenes[1].name.as_str()), (2, 1, "Scene 2"));
        h.key_press(Key::Home);
        h.step();
        assert_eq!(h.state().scene, 0);
        h.key_press(Key::End);
        h.step();
        assert_eq!(h.state().scene, 1);
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!((h.state().doc.scenes.len(), h.state().scene), (1, 0));
    }

    #[test]
    fn invert_selection_and_keyframe_steps() {
        use egui::{Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0), (300.0, 200.0, 350.0, 260.0)]);
        let ctx = h.ctx.clone();
        mouse_click(&mut h, (150.0, 125.0));
        h.state_mut().run(Action::InvertSelection, &ctx);
        assert_eq!(h.state().selection, [ElementRef { layer: 0, keyframe: 0, element: 1 }]);
        // Alt+. and Alt+, step between keyframes, not frames.
        h.key_press(Key::F7);
        h.key_press(Key::F5);
        h.key_press(Key::F5);
        h.step();
        h.state_mut().set_frame(3);
        h.key_press_modifiers(Modifiers::ALT, Key::Comma);
        h.step();
        assert_eq!(h.state().frame, 1);
        h.key_press_modifiers(Modifiers::ALT, Key::Comma);
        h.step();
        assert_eq!(h.state().frame, 0);
        h.key_press_modifiers(Modifiers::ALT, Key::Period);
        h.step();
        assert_eq!(h.state().frame, 1);
    }

    #[test]
    fn shortcuts_need_their_own_modifiers_and_wait_while_typing() {
        use egui::{Event, Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        let rect = h.state().selection.clone();
        // Ctrl+Shift+D is Distribute to Layers (greyed), not Ctrl+D; Ctrl+Alt+Shift+O is
        // Outlines, not the onion skin; Ctrl+Alt+C is Copy Frames, not Copy.
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::SHIFT, Key::D);
        h.key_press_modifiers(Modifiers::COMMAND | Modifiers::ALT | Modifiers::SHIFT, Key::O);
        h.step();
        h.event_modifiers(Event::Copy, Modifiers::COMMAND | Modifiers::ALT);
        h.step();
        let ed = h.state();
        assert_eq!((ed.all_elements().len(), ed.onion, ed.clipboard.len()), (1, false, 0));

        // While the Text tool types, Ctrl+D and Ctrl+G leave the selection be; Ctrl+K still works.
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (300.0, 300.0));
        h.event(Event::Text("a".into()));
        h.step();
        h.state_mut().selection = rect;
        h.key_press_modifiers(Modifiers::COMMAND, Key::D);
        h.key_press_modifiers(Modifiers::COMMAND, Key::G);
        h.key_press_modifiers(Modifiers::COMMAND, Key::K);
        h.step();
        let ed = h.state();
        assert_eq!(ed.all_elements().len(), 2, "the rectangle and the text, nothing duplicated or grouped");
        assert!(h.ctx.egui_wants_keyboard_input() && ed.flyout == Some(crate::panels::Flyout::Align), "still typing");
    }

    #[test]
    fn select_all_skips_locked_and_hidden_layers() {
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        // A second layer on top, with its own rectangle; then lock the first.
        h.state_mut().new_layer();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (300.0, 200.0), (350.0, 260.0));
        let ed = h.state_mut();
        ed.timeline_mut().layers[1].set_locked(true);
        ed.run(Action::SelectAll, &ctx);
        assert_eq!(ed.selection, [ElementRef { layer: 0, keyframe: 0, element: 0 }]);
        ed.selection.clear();
        ed.run(Action::InvertSelection, &ctx);
        assert_eq!(ed.selection.len(), 1, "inverting skips them too");
        ed.timeline_mut().layers[1].set_locked(false);
        ed.timeline_mut().layers[0].set_visible(false);
        ed.run(Action::SelectAll, &ctx);
        assert_eq!(ed.selection, [ElementRef { layer: 1, keyframe: 0, element: 0 }]);
        // Nor does anything paste onto one.
        ed.run(Action::Copy, &ctx);
        ed.timeline_mut().layers[0].set_visible(true);
        ed.timeline_mut().layers[0].set_locked(true);
        ed.layer = 0;
        ed.run(Action::PasteInPlace, &ctx);
        assert_eq!(ed.all_elements().len(), 2);
    }

    #[test]
    fn escape_cancels_the_field_and_leaves_the_stage_be() {
        use egui::{Event, Key};
        use egui_kittest::kittest::Queryable;
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        let (rect, steps) = (h.state().selection.clone(), h.state().undo.len());
        // A layer's name: Escape keeps the old one.
        h.state_mut().renaming = Some((0, "Layer_1".into()));
        h.step();
        h.event(Event::Text("XYZ".into()));
        h.step();
        h.key_press(Key::Escape);
        h.step();
        assert_eq!((h.state().timeline().layers[0].name.as_str(), h.state().renaming.is_none()), ("Layer_1", true));
        // A number typed in Properties: Escape puts the old one back.
        h.state_mut().props_tab = PropsTab::Doc;
        h.run();
        h.get_by_label("FPS").click();
        h.run();
        h.event(Event::Text("12".into()));
        h.step();
        h.key_press(Key::Escape);
        h.step();
        assert_eq!(h.state().doc.frame_rate, 24.0);
        // The selection's X, which moves the shape as it's typed: it goes back.
        h.state_mut().props_tab = PropsTab::Object;
        h.run();
        let x = selected_bounds(h.state()).min.0;
        h.get_by_label("X").click();
        h.run();
        h.event(Event::Text("300".into()));
        h.step();
        h.key_press(Key::Escape);
        h.step();
        let ed = h.state();
        assert_eq!(selected_bounds(ed).min.0, x);
        // None left an undo or redo step, or touched the stage selection.
        assert_eq!((&ed.selection, ed.undo.len(), ed.can_redo()), (&rect, steps, false));
    }

    #[test]
    fn escape_leaves_a_value_entered_before_be() {
        use egui::{Event, Key};
        use egui_kittest::kittest::Queryable;
        use minnamate_core::doc::Tween;
        let ease = |h: &Harness<'_, Editor>| match h.state().timeline().layers[1].frames[0].tween {
            Tween::Classic { ease, .. } => ease,
            _ => f32::NAN,
        };
        let mut h = harness(false);
        h.run();
        click_cell(&mut h, (1, 3), egui::Modifiers::NONE);
        h.run();
        // Ease 50, entered. Its undo run stays open: Ease doesn't end it when left.
        h.get_by_value("0").click();
        h.run();
        h.event(Event::Text("50".into()));
        h.step();
        h.key_press(Key::Enter);
        h.run();
        assert_eq!(ease(&h), 0.5);
        // Back in the field from the keyboard, Escape has nothing typed there to cancel.
        h.get_by_value("50").focus();
        h.run();
        h.key_press(Key::Escape);
        h.run();
        assert_eq!(ease(&h), 0.5);
    }

    #[test]
    fn ctrl_plus_and_minus_never_scale_the_whole_window() {
        use egui::{Key, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        // A dialog keeps the keys from the editor; egui would zoom its whole UI with them.
        h.key_press(Key::F8);
        h.run();
        assert!(h.state().convert.is_some());
        for key in [Key::Equals, Key::Equals, Key::Minus, Key::Plus] {
            h.key_press_modifiers(Modifiers::COMMAND, key);
            h.step();
        }
        assert_eq!(h.ctx.zoom_factor(), 1.0);
    }

    #[test]
    fn tab_leaves_the_keyboard_to_the_shortcuts() {
        use egui::Key;
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        // With no field to go to, egui's Tab would hand the keyboard to the window's title bar.
        h.key_press(Key::Tab);
        h.key_press(Key::R);
        h.step();
        assert_eq!(h.state().tool, Tool::Rectangle);
        h.key_press(Key::Tab);
        h.key_press(Key::Tab);
        h.key_press(Key::Enter);
        h.step();
        let ed = h.state();
        assert!(!ed.home.shown && ed.playing.is_some(), "Enter plays, rather than pressing a focused Home button");
        assert!(!h.ctx.egui_wants_keyboard_input());
    }

    #[test]
    fn while_typing_the_menus_leave_the_field_its_commands() {
        use egui_kittest::kittest::Queryable;
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        h.state_mut().tool = Tool::Text;
        mouse_click(&mut h, (300.0, 300.0));
        h.event(egui::Event::Text("Hello".into()));
        h.step();
        // Ctrl+A selects the text's characters, so Edit > Select All can't select the stage's objects.
        h.get_by_label("Edit").click();
        h.run();
        h.get_by_label("Select All Ctrl+A").click();
        h.run();
        assert!(h.state().selection.is_empty() && h.ctx.egui_wants_keyboard_input(), "{:?}", h.state().selection);
    }

    #[test]
    fn each_press_counts_when_several_come_in_one_frame() {
        use egui::{Event, Key, Modifiers};
        // A slow frame gets the key repeats together.
        fn presses(h: &mut Harness<'_, Editor>, key: Key, n: usize) {
            let press = Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE };
            h.input_mut().events.extend(std::iter::repeat_n(press, n));
            h.step();
        }
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        let before = selected_bounds(h.state());
        presses(&mut h, Key::ArrowRight, 3);
        assert!((selected_bounds(h.state()).min.0 - before.min.0 - 3.0).abs() < 1e-3);
        presses(&mut h, Key::F5, 3);
        assert_eq!(keys(h.state())[0], [(0, 4)]);
        presses(&mut h, Key::Period, 2);
        assert_eq!(h.state().frame, 2);
        let colors = (h.state().stroke_color, h.state().fill_color);
        presses(&mut h, Key::X, 2);
        assert_eq!((h.state().stroke_color, h.state().fill_color), colors);
    }

    #[test]
    fn shift_insert_pastes_in_the_center() {
        use egui::{Event, Modifiers};
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        mouse_click(&mut h, (150.0, 125.0));
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Copy, &ctx);
        // Windows' other Paste key arrives as a Paste event with Shift held, but no Ctrl.
        h.event_modifiers(Event::Paste("<DOMShape/>".into()), Modifiers::SHIFT);
        h.step();
        let b = selected_bounds(h.state());
        assert!(near(((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0), (275.0, 200.0)), "{b:?}");
    }

    #[test]
    fn show_all_fits_the_symbol_being_edited() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        // How far the middle of what's on stage shows from the middle of the view, in points.
        let off_center = |h: &Harness<'_, Editor>| {
            let ed = h.state();
            let b = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.all_elements()).unwrap();
            let (x, y) = ed.to_scene(((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0));
            (ed.stage_rect.min + vec2(x, y) * ed.zoom - (ed.stage_rect.center() - ed.pan)).length()
        };
        // Blink, a 20×20 square, edited on its own: it fills the view, at the 2000% cap.
        h.state_mut().edit_library_symbol("Blink");
        h.state_mut().run(Action::ShowAll, &ctx);
        h.run();
        assert_eq!(h.state().zoom, 20.0);
        assert!(off_center(&h) < 2.0, "{}", off_center(&h));
        // In place, where its instance sits at (520, 30).
        h.state_mut().exit_symbol();
        h.state_mut().selection = vec![ElementRef { layer: 0, keyframe: 0, element: 0 }];
        h.state_mut().run(Action::EditInPlace, &ctx);
        h.state_mut().run(Action::ShowAll, &ctx);
        h.run();
        assert_eq!((h.state().editing.len(), h.state().zoom), (1, 20.0));
        assert!(off_center(&h) < 2.0, "{}", off_center(&h));
    }

    #[test]
    fn closing_a_panel_leaves_the_rest_of_its_dock() {
        use egui::Modifiers;
        let mut h = harness(false);
        h.run();
        let press = |h: &mut Harness<'_, Editor>, mods, key| {
            h.key_press_modifiers(mods, key);
            h.run();
        };
        // F2 brings Output to the front of the Timeline's dock; F2 again closes just Output.
        press(&mut h, Modifiers::NONE, Key::F2);
        assert_eq!((h.state().show_bottom, h.state().bottom_tab), (true, 1));
        press(&mut h, Modifiers::NONE, Key::F2);
        let ed = h.state();
        assert_eq!((ed.show_bottom, ed.bottom_tab), (true, 0));
        assert!(ed.panel_shown(PanelId::Timeline) && !ed.panel_shown(PanelId::Output));
        // Ctrl+L twice: the Library comes and goes, Properties stays.
        press(&mut h, Modifiers::COMMAND, Key::L);
        assert_eq!((h.state().show_right, h.state().right_tab), (true, 1));
        press(&mut h, Modifiers::COMMAND, Key::L);
        let ed = h.state();
        assert_eq!((ed.show_right, ed.right_tab), (true, 0));
        assert!(ed.panel_shown(PanelId::Properties) && !ed.panel_shown(PanelId::Library));
        // Closing a dock's last panel hides the dock; F2 brings it back on Output.
        press(&mut h, Modifiers::COMMAND | Modifiers::ALT, Key::T);
        assert!(!h.state().show_bottom);
        press(&mut h, Modifiers::NONE, Key::F2);
        assert_eq!((h.state().show_bottom, h.state().bottom_tab), (true, 1));
        assert!(h.state().panel_shown(PanelId::Output));
    }

    #[test]
    fn ctrl_minus_leaves_the_interface_size_alone_while_a_dialog_is_up() {
        let mut h = harness(false);
        h.run();
        h.state_mut().about = true;
        h.run();
        for _ in 0..2 {
            h.key_press_modifiers(egui::Modifiers::COMMAND, Key::Minus);
            h.run();
        }
        assert_eq!((h.ctx.zoom_factor(), h.state().zoom), (1.0, 1.0));
    }

    #[test]
    fn step_forward_stops_at_the_last_frame() {
        let mut h = harness(false);
        h.run();
        h.state_mut().set_frame(9);
        for _ in 0..3 {
            h.key_press(Key::Period);
            h.run();
        }
        assert_eq!(h.state().frame, 10);
    }

    #[test]
    fn timeline_commands_that_change_nothing_leave_no_undo_step() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let ed = h.state_mut();
        // Past the end there's nothing to remove, clear, cut, or copy and paste.
        ed.frame = 15;
        for action in [Action::RemoveFrames, Action::ClearFrames, Action::CutFrames, Action::CopyFrames, Action::PasteFrames] {
            ed.run(action, &ctx);
        }
        // The other layers locked and hidden already.
        ed.frame = 0;
        for l in &mut ed.timeline_mut().layers[1..] {
            l.set_locked(true);
            l.set_visible(false);
        }
        ed.run(Action::LockOthers, &ctx);
        ed.run(Action::HideOthers, &ctx);
        assert!(!ed.dirty && !ed.can_undo());
        ed.run(Action::ShowAllLayers, &ctx);
        assert!(ed.dirty && ed.can_undo(), "a command that does change something still counts");
    }

    #[test]
    fn library_new_symbol_and_the_panel_icons() {
        use egui::accesskit::Role;
        use egui_kittest::kittest::Queryable;
        let mut h = harness(false);
        h.run();
        // The collapsed-panel strip opens the panels that are built, and closes them.
        h.get_by_role_and_label(Role::Button, "Align").click();
        h.run();
        assert_eq!(h.state().flyout, Some(crate::panels::Flyout::Align));
        h.get_by_role_and_label(Role::Button, "Align").click();
        h.run();
        assert_eq!(h.state().flyout, None);
        h.state_mut().right_tab = 1;
        h.run();
        h.get_by_label("New Symbol").click();
        h.run();
        assert!(h.state().convert.is_some(), "the Library's New Symbol opens Create New Symbol");
    }

    #[test]
    fn reset_essentials_restores_the_dock_sizes() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let width = |h: &Harness<'_, Editor>| egui::PanelState::load(&h.ctx, egui::Id::new("properties")).map(|s| s.size().x);
        let default = width(&h);
        let left = egui::PanelState::load(&h.ctx, egui::Id::new("properties")).unwrap().outer_rect.left();
        screen_drag(&mut h, egui::pos2(left, 500.0), egui::pos2(left - 200.0, 500.0), egui::Modifiers::NONE);
        h.run();
        assert_ne!(width(&h), default, "dragged wider");
        h.state_mut().run(Action::ResetWorkspace, &ctx);
        h.run();
        assert_eq!(width(&h), default);
    }

    #[test]
    fn undoing_insert_scene_goes_back_to_the_scene_it_was_inserted_from() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let names = |ed: &Editor| ed.doc.scenes.iter().map(|s| s.name.clone()).collect::<Vec<_>>();
        let ed = h.state_mut();
        ed.doc.insert_scene(0);
        ed.run(Action::InsertScene, &ctx);
        assert_eq!((names(ed), ed.scene), (vec!["Scene 1".to_string(), "Scene 3".into(), "Scene 2".into()], 1));
        ed.run(Action::Undo, &ctx);
        assert_eq!((names(ed), ed.scene), (vec!["Scene 1".to_string(), "Scene 2".into()], 0));
        ed.run(Action::Redo, &ctx);
        assert_eq!((names(ed), ed.scene), (vec!["Scene 1".to_string(), "Scene 3".into(), "Scene 2".into()], 0), "redo keeps the scene shown");
    }

    #[test]
    fn hide_panels_hides_the_open_flyout_too() {
        let mut h = harness(false);
        h.run();
        h.key_press_modifiers(egui::Modifiers::COMMAND, Key::K);
        h.run();
        h.key_press(Key::F4);
        h.run();
        let ed = h.state();
        assert!(!ed.show_tools && !ed.show_right && !ed.show_bottom && !ed.panel_shown(PanelId::Align));
        h.key_press(Key::F4);
        h.run();
        let ed = h.state();
        assert!(ed.show_tools && ed.show_right && ed.show_bottom && ed.panel_shown(PanelId::Align), "F4 again brings them back");
    }

    #[test]
    fn a_dropped_file_asks_about_unsaved_changes_and_waits_for_dialogs() {
        let mut h = blank();
        let ctx = h.ctx.clone();
        // A dialog up keeps the window's input, drops too: Document Settings stays about its document.
        h.key_press_modifiers(Modifiers::COMMAND, Key::J);
        h.run();
        drop_file(&mut h, fixture());
        assert_eq!(h.state().title, "Untitled-1");
        h.key_press(Key::Escape);
        h.run();
        // With unsaved changes, a drop asks first, as File > Open does.
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        drop_file(&mut h, fixture());
        let ed = h.state();
        assert_eq!((ed.title.as_str(), ed.all_elements().len(), ed.confirm.is_some()), ("Untitled-1", 1, true));
        h.state_mut().resolve(Answer::Cancel, &ctx);
        // While Close asks, a drop waits too, and doesn't take Close's place.
        h.state_mut().run(Action::Close, &ctx);
        h.run();
        drop_file(&mut h, fixture());
        assert_eq!((h.state().title.as_str(), h.state().confirm), ("Untitled-1", Some(Action::Close)));
        h.state_mut().resolve(Answer::Cancel, &ctx);
        h.run();
        drop_file(&mut h, fixture());
        h.state_mut().resolve(Answer::DontSave, &ctx);
        assert_eq!(h.state().title, "tween");
    }

    #[test]
    fn moving_and_deleting_layers_keeps_the_stage_selection_on_its_objects() {
        let selected = |ed: &Editor| ed.selection.iter().map(|&r| ed.timeline().layers[r.layer].name.clone()).collect::<Vec<_>>();
        // Pressing Blink's name selects its instance; dragged below Ball, it's still Blink's.
        let mut h = harness(false);
        h.run();
        let (rows, row_h) = (h.state().layer_rows, h.state().tl.row_h);
        let name = |row: f32| egui::pos2(rows.left() + 60.0, rows.top() + row_h * row);
        screen_drag(&mut h, name(0.5), name(2.1), Modifiers::NONE);
        let layers: Vec<_> = h.state().timeline().layers.iter().map(|l| l.name.clone()).collect();
        assert_eq!((layers, selected(h.state())), (vec!["Ball".to_string(), "Blink".into(), "Background".into()], vec!["Blink".to_string()]));
        h.key_press(Key::Delete);
        h.step();
        let ed = h.state();
        assert!(ed.timeline().layers[1].frames[0].elements.is_empty() && ed.timeline().layers[0].frames[0].elements.len() == 1);

        // Deleting the ball's layer takes its selected instance along; nothing else is selected.
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        mouse_click(&mut h, (100.0, 200.0));
        assert_eq!(selected(h.state()), ["Ball"]);
        h.state_mut().run(Action::DeleteLayer, &ctx);
        assert!(h.state().selection.is_empty());

        // Duplicate Layers puts the copy above: the original's instance stays selected.
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        mouse_click(&mut h, (100.0, 200.0));
        h.state_mut().run(Action::DuplicateLayers, &ctx);
        let ed = h.state();
        assert_eq!((ed.timeline().layers[1].name.as_str(), ed.selection.as_slice()), ("Ball copy", &[ElementRef { layer: 2, keyframe: 0, element: 0 }][..]));
        // New Layer and New Folder go in above the active layer, the copy; it stays the ball.
        for action in [Action::NewLayer, Action::NewFolder] {
            h.state_mut().run(action, &ctx);
        }
        assert_eq!(selected(h.state()), ["Ball"]);
    }

    #[test]
    fn a_key_changing_the_document_mid_drag_ends_the_timeline_gesture() {
        // Ctrl+dragging the end of the bottom layer's span while Ctrl+Z takes away the layer above.
        let mut h = harness(false);
        h.run();
        let before = keys(h.state());
        h.state_mut().new_layer();
        h.run();
        let (a, b) = (cell(&h, 3, 10), cell(&h, 3, 14));
        h.event(egui::Event::ModifiersChanged(Modifiers::COMMAND));
        h.hover_at(a);
        h.step();
        h.drag_at(a);
        h.step();
        h.hover_at(b);
        h.step();
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        h.drop_at(b);
        h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
        h.step();
        assert_eq!(keys(h.state()), before);

        // Shift+dragging the loop range while Ctrl+Z takes its frames away.
        let mut h = blank();
        h.state_mut().set_frame(14);
        h.key_press(Key::F5);
        h.step();
        let ed = h.state_mut();
        (ed.loop_playback, ed.tl.loop_range) = (true, Some((10, 14)));
        h.run();
        let (rows, cell_w) = (h.state().layer_rows, h.state().cell_w);
        let strip = |frames: f32| egui::pos2(rows.right() + frames * cell_w, rows.top() - 26.0);
        h.event(egui::Event::ModifiersChanged(Modifiers::SHIFT));
        h.hover_at(strip(12.5));
        h.step();
        h.drag_at(strip(12.5));
        h.step();
        h.event(egui::Event::ModifiersChanged(Modifiers::NONE));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        h.hover_at(strip(11.5));
        h.step();
        h.drop_at(strip(11.5));
        h.step();
        assert_eq!((h.state().frame_count(), h.state().tl.loop_range), (1, Some((10, 14))));
    }

    #[test]
    fn a_symbol_never_goes_inside_itself() {
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        // Its instance copied as an element, as frames and as a layer, then pasted into it every way.
        for action in [Action::Copy, Action::CopyFrames, Action::CopyLayers] {
            h.state_mut().run(action, &ctx);
        }
        let r = h.state().selection[0];
        h.state_mut().enter_symbol(r);
        for action in [Action::PasteInPlace, Action::PasteInCenter, Action::PasteFrames, Action::PasteOverwriteFrames, Action::PasteLayers] {
            h.state_mut().run(action, &ctx);
        }
        // No frame is drawn here: two of itself inside would take 2^32 renders of it.
        let sym = &h.state().doc.library["Symbol 1"].timeline;
        let mut elements = sym.layers.iter().flat_map(|l| &l.frames).flat_map(|k| &k.elements);
        assert!(elements.all(|el| matches!(el, Element::Shape(_))), "{sym:?}");
    }

    #[test]
    fn locking_or_hiding_a_layer_deselects_its_objects() {
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let (rows, row_h) = (h.state().layer_rows, h.state().tl.row_h);
        // The lock column, then the eye.
        for dx in [26.0, 51.0] {
            mouse_click(&mut h, (150.0, 125.0));
            assert_eq!(h.state().selection.len(), 1);
            let column = egui::pos2(rows.right() - dx, rows.top() + row_h * 0.5);
            screen_click(&mut h, column, Modifiers::NONE);
            assert!(h.state().selection.is_empty(), "column {dx}");
            h.key_press(Key::Delete);
            h.step();
            assert_eq!(h.state().all_elements().len(), 1);
            screen_click(&mut h, column, Modifiers::NONE);
        }
    }

    #[test]
    fn locked_and_hidden_layers_take_no_drawing_or_paint() {
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let top = |h: &Harness<'_, Editor>| h.state().timeline().layers[0].clone();
        (h.state_mut().fill_color, h.state_mut().stroke_color) = (Color32::RED, Color32::GREEN);
        for (locked, hidden) in [(true, false), (false, true)] {
            let layer = &mut h.state_mut().timeline_mut().layers[0];
            layer.set_locked(locked);
            layer.set_visible(!hidden);
            let before = top(&h);
            for tool in [Tool::Rectangle, Tool::Brush] {
                h.state_mut().tool = tool;
                mouse_drag(&mut h, (120.0, 110.0), (180.0, 140.0));
            }
            // In the fill, then on the outline.
            for tool in [Tool::Eraser, Tool::PaintBucket, Tool::InkBottle] {
                h.state_mut().tool = tool;
                mouse_click(&mut h, (150.0, 125.0));
                mouse_click(&mut h, (100.0, 125.0));
            }
            h.state_mut().tool = Tool::Text;
            mouse_click(&mut h, (150.0, 125.0));
            h.event(egui::Event::Text("a".into()));
            h.step();
            h.state_mut().tool = Tool::Selection;
            h.step();
            assert!(top(&h) == before, "locked {locked}, hidden {hidden}");
        }

        // Under a hidden layer's shape, the Paint Bucket fills the shape it can see.
        h.state_mut().timeline_mut().layers[0].set_visible(true);
        h.state_mut().new_layer();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().timeline_mut().layers[0].set_visible(false);
        let hidden = top(&h);
        (h.state_mut().tool, h.state_mut().fill_color) = (Tool::PaintBucket, Color32::BLUE);
        mouse_click(&mut h, (150.0, 125.0));
        assert!(top(&h) == hidden);
        let Some(Element::Shape(s)) = h.state().timeline().layers[1].frames[0].elements.first() else { panic!("a shape") };
        assert!(s.fills.contains(&Paint::Solid(rgba(Color32::BLUE))), "{:?}", s.fills);
    }

    #[test]
    fn undo_brings_a_selection_back_only_to_its_own_timeline() {
        // A nudge in Scene 1, undone from Scene 2: Scene 2's rectangle doesn't get selected.
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::InsertScene, &ctx);
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Tool::Selection;
        h.state_mut().run(Action::GoToScene(Go::First), &ctx);
        h.run();
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(Key::ArrowRight);
        h.step();
        h.state_mut().run(Action::GoToScene(Go::Last), &ctx);
        h.run();
        h.state_mut().run(Action::Undo, &ctx);
        assert!(h.state().selection.is_empty());
        h.key_press(Key::Delete);
        h.step();
        assert_eq!(h.state().doc.scenes[1].layers[0].frames[0].elements.len(), 1);

        // A nudge inside a symbol, undone from the scene: the scene's instance doesn't either.
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        let r = h.state().selection[0];
        h.state_mut().enter_symbol(r);
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(Key::ArrowRight);
        h.step();
        h.state_mut().exit_symbol();
        h.state_mut().run(Action::Undo, &ctx);
        assert!(h.state().selection.is_empty());

        // Undoing Insert Scene from a later scene stays on that scene.
        let mut h = blank();
        let ctx = h.ctx.clone();
        for go in [Action::InsertScene, Action::InsertScene, Action::GoToScene(Go::First), Action::InsertScene, Action::GoToScene(Go::To(2))] {
            h.state_mut().run(go, &ctx);
        }
        assert_eq!(h.state().doc.scenes[h.state().scene].name, "Scene 2");
        h.state_mut().run(Action::Undo, &ctx);
        let ed = h.state();
        assert_eq!((ed.doc.scenes.len(), ed.doc.scenes[ed.scene].name.as_str()), (3, "Scene 2"));
    }

    #[test]
    fn edit_in_place_opens_a_tweened_instance_where_it_shows() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        // Frame 6: the ball is halfway along its tween, at (250, 200).
        h.state_mut().set_frame(5);
        h.run();
        mouse_click(&mut h, (250.0, 200.0));
        h.state_mut().run(Action::EditInPlace, &ctx);
        let m = h.state().editing[0].transform;
        assert!((m.tx - 250.0).abs() < 0.01 && (m.ty - 200.0).abs() < 0.01, "{m:?}");
    }

    #[test]
    fn leaving_a_symbol_after_its_parent_lost_layers() {
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        mouse_click(&mut h, (150.0, 125.0));
        h.key_press(Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        // Two layers above the instance's, which is then row 2; inside it, undo takes them away.
        h.state_mut().new_layer();
        h.state_mut().new_layer();
        // Long enough after the last click that this isn't a double-click.
        h.run_steps(40);
        mouse_click(&mut h, (150.0, 125.0));
        assert_eq!(h.state().layer, 2);
        h.state_mut().run(Action::EditInPlace, &ctx);
        h.state_mut().run(Action::Undo, &ctx);
        h.state_mut().run(Action::Undo, &ctx);
        h.state_mut().exit_symbol();
        assert_eq!((h.state().timeline().layers.len(), h.state().layer), (1, 0));
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (300.0, 200.0), (350.0, 250.0));
        assert_eq!(h.state().all_elements().len(), 2);
    }

    #[test]
    fn each_scene_plays_its_own_loop() {
        let mut h = blank();
        let ctx = h.ctx.clone();
        // Scene 1: 30 frames, looping frames 21 to 26.
        h.state_mut().set_frame(29);
        h.key_press(Key::F5);
        h.step();
        let ed = h.state_mut();
        (ed.loop_playback, ed.tl.loop_range) = (true, Some((20, 25)));
        h.run();
        // Scene 2, 40 frames: Scene 1's range doesn't come along.
        h.state_mut().run(Action::InsertScene, &ctx);
        for last in [9, 39] {
            h.state_mut().set_frame(last);
            h.key_press(Key::F5);
            h.step();
        }
        h.run();
        assert_eq!((h.state().frame_count(), timeline::loop_range(h.state())), (40, (0, 39)));
        // Undo takes it back to 10 frames, past a range of its own: playback loops them all
        // rather than sticking on the last.
        h.state_mut().tl.loop_range = Some((30, 35));
        h.state_mut().run(Action::Undo, &ctx);
        h.run();
        assert_eq!((h.state().frame_count(), timeline::loop_range(h.state())), (10, (0, 9)));
        let ed = h.state_mut();
        ed.playing = Some((ctx.input(|i| i.time) - 14.5 / 24.0, 0));
        ed.tick(&ctx);
        assert_eq!((h.state().frame, h.state().playing.is_some()), (4, true));
    }

    #[test]
    fn entering_and_leaving_a_symbol_scrolls_the_timeline_to_the_playhead() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().set_frame(5);
        h.state_mut().first_visible_frame = 300.0;
        h.run();
        mouse_click(&mut h, (250.0, 200.0));
        h.state_mut().run(Action::EditInPlace, &ctx);
        h.run();
        assert_eq!(h.state().first_visible_frame, 0.0, "the symbol's frame 1");
        h.state_mut().first_visible_frame = 300.0;
        h.state_mut().exit_symbol();
        h.run();
        assert!(h.state().first_visible_frame <= 5.0, "the scene's frame 6: {}", h.state().first_visible_frame);
    }

    #[test]
    fn show_all_inside_a_symbol_fits_the_symbol() {
        let mut h = rectangles(&[(100.0, 100.0, 200.0, 150.0)]);
        let ctx = h.ctx.clone();
        // A new symbol, edited on its own with its origin at the stage's middle: a box from (0, 0) to (100, 50).
        h.key_press_modifiers(Modifiers::COMMAND, Key::F8);
        h.step();
        h.step();
        h.key_press(Key::Enter);
        h.step();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (275.0, 200.0), (375.0, 250.0));
        h.state_mut().run(Action::ShowAll, &ctx);
        // The box's middle, (325, 225) on stage, comes to the view's.
        let ed = h.state();
        assert!((ed.pan - vec2(-50.0, -25.0) * ed.zoom).length() < ed.zoom, "{:?} at {}", ed.pan, ed.zoom);
    }

    #[test]
    fn commands_that_change_nothing_leave_no_undo_step() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::LayerProperties, &ctx);
        h.run();
        h.key_press(Key::Enter);
        h.run();
        assert!(!h.state().can_undo() && !h.state().dirty, "Layer Properties, OK with nothing changed");
        // Lock Others twice: the second finds them locked.
        h.state_mut().run(Action::LockOthers, &ctx);
        h.state_mut().run(Action::LockOthers, &ctx);
        // Remove Frames past every layer's end.
        h.state_mut().set_frame(40);
        h.state_mut().run(Action::RemoveFrames, &ctx);
        assert_eq!(h.state().undo.len(), 1);
    }
}

