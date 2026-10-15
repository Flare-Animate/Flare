//! The application bar, which is also the window's title bar as in Animate on
//! Windows: logo, Home, menus, Test Movie / Share / workspace, window buttons.
//! The menus follow docs/animate-ui.md §7.4; their shortcuts are the editor's.

use eframe::egui::{self, Align, Color32, CursorIcon, Key, Layout, Modifiers, Rect, ResizeDirection, RichText, Sense, Ui, ViewportCommand, vec2};
use minnamate_core::arrange::{Align::*, Arrange::*};
use minnamate_core::doc::Element;

use crate::app::{Action, Editor, Go, PanelId};
use crate::icons::{self, Icon};
use crate::stage::Toggle;
use crate::theme;

enum Item {
    /// Label, shortcut, action.
    Do(&'static str, &'static str, Action),
    /// A submenu; greyed out while it has nothing in it.
    Sub(&'static str, &'static [Item]),
    /// File > Open Recent, filled from the editor's list.
    Recent,
    /// View > Go To: the document's scenes.
    Scenes,
    /// Window: the open documents.
    Documents,
    Sep,
}
use Action::*;
use Item::*;

const FILE: &[Item] = &[
    Do("New...", "Ctrl+N", New),
    Do("Open...", "Ctrl+O", Open),
    Recent,
    Do("Close", "Ctrl+W", Close),
    // One document at a time, so closing them all is closing it.
    Do("Close All", "Ctrl+Alt+W", Close),
    Sep,
    Do("Save", "Ctrl+S", Save),
    Do("Save As...", "Ctrl+Shift+S", SaveAs),
    Do("Save as Template...", "", Todo),
    Do("Save All", "", Todo),
    Do("Revert", "", Todo),
    Sep,
    Sub("Import", &[
        Do("Import to Stage...", "Ctrl+R", Todo),
        Do("Import to Library...", "", Todo),
        Do("Open External Library...", "Ctrl+Shift+O", Todo),
        Do("Import Video...", "", Todo),
    ]),
    Sub("Export", &[
        Do("Export Image...", "", Todo),
        Do("Export Image (Legacy)...", "", Todo),
        Do("Export Movie...", "Ctrl+Shift+Alt+S", ExportSwf),
        Do("Export Video/Media...", "", Todo),
        Do("Export Animated GIF...", "", Todo),
    ]),
    Sep,
    Sub("Convert To", &[
        Do("ActionScript 3.0", "", Todo),
        Do("HTML5 Canvas", "", Todo),
        Do("AIR for Desktop", "", Todo),
        Do("AIR for Android", "", Todo),
        Do("AIR for iOS", "", Todo),
    ]),
    Sep,
    Do("Publish Settings...", "Ctrl+Shift+F12", Todo),
    Do("Publish", "Shift+Alt+F12", ExportSwf),
    Sep,
    Do("AIR Settings...", "", Todo),
    Do("ActionScript Settings...", "", Todo),
    Do("File Info...", "", Todo),
    Sep,
    Do("Exit", "Ctrl+Q", Exit),
];

const TIMELINE_EDIT: &[Item] = &[
    Do("Remove Frames", "Shift+F5", RemoveFrames),
    Do("Cut Frames", "Ctrl+Alt+X", CutFrames),
    Do("Copy Frames", "Ctrl+Alt+C", CopyFrames),
    Do("Paste Frames", "Ctrl+Alt+V", PasteFrames),
    Do("Paste and Overwrite Frames", "", PasteOverwriteFrames),
    Do("Clear Frames", "Alt+Backspace", ClearFrames),
    Do("Select All Frames", "Ctrl+Alt+A", SelectAllFrames),
    Sep,
    Do("Cut Layers", "", CutLayers),
    Do("Copy Layers", "", CopyLayers),
    Do("Paste Layers", "", PasteLayers),
    Do("Duplicate Layers", "", DuplicateLayers),
    Sep,
    Do("Copy Motion", "", Todo),
    Do("Copy Motion as ActionScript 3.0...", "", Todo),
    Do("Paste Motion", "", Todo),
    Do("Paste Motion Special...", "", Todo),
];

const EDIT: &[Item] = &[
    Do("Undo", "Ctrl+Z", Undo),
    Do("Redo", "Ctrl+Y", Redo),
    Sep,
    Do("Cut", "Ctrl+X", Cut),
    Do("Copy", "Ctrl+C", Copy),
    Do("Paste in Center", "Ctrl+V", PasteInCenter),
    Do("Paste in Place", "Ctrl+Shift+V", PasteInPlace),
    Do("Paste Special...", "", Todo),
    Do("Clear", "Backspace", DeleteSelection),
    Sep,
    Do("Duplicate", "Ctrl+D", Duplicate),
    Do("Select All", "Ctrl+A", SelectAll),
    Do("Deselect All", "Ctrl+Shift+A", Deselect),
    Do("Invert Selection", "", InvertSelection),
    Sep,
    Do("Find and Replace", "Ctrl+F", Todo),
    Do("Find Next", "F3", Todo),
    Sep,
    Sub("Timeline", TIMELINE_EDIT),
    Sep,
    Do("Edit Symbols", "Ctrl+E", EditSymbol),
    Do("Edit Selected", "", Todo),
    Do("Edit in Place", "", EditInPlace),
    Do("Edit All", "", Todo),
    Sep,
    Sub("Preferences", &[
        Do("Edit Preferences...", "Ctrl+U", Todo),
        Sep,
        Do("Export Preferences...", "", Todo),
        Do("Import Preferences...", "", Todo),
    ]),
    Do("Font Mapping...", "", Todo),
    Do("Keyboard Shortcuts...", "", Todo),
];

const VIEW: &[Item] = &[
    Sub("Go To", &[
        Do("First", "Home", GoToScene(Go::First)),
        Do("Previous", "Page Up", GoToScene(Go::Previous)),
        Do("Next", "Page Down", GoToScene(Go::Next)),
        Do("Last", "End", GoToScene(Go::Last)),
        Sep,
        Scenes,
    ]),
    Sep,
    Do("Zoom In", "Ctrl+=", ZoomIn),
    Do("Zoom Out", "Ctrl+-", ZoomOut),
    Sub("Magnification", &[
        Do("Fit in Window", "", FitInWindow),
        Do("Clip to Stage", "", ClipToStage),
        Sep,
        Do("25%", "", Zoom(0.25)),
        Do("50%", "", Zoom(0.5)),
        Do("100%", "Ctrl+1", Zoom(1.0)),
        Do("200%", "", Zoom(2.0)),
        Do("400%", "Ctrl+4", Zoom(4.0)),
        Do("800%", "Ctrl+8", Zoom(8.0)),
        Sep,
        Do("Show Frame", "Ctrl+2", FitInWindow),
        Do("Show All", "Ctrl+3", ShowAll),
    ]),
    Sep,
    Sub("Preview Mode", &[
        Do("Outlines", "Ctrl+Alt+Shift+O", Todo),
        Do("Fast", "Ctrl+Alt+Shift+F", Todo),
        Do("Anti-Alias", "Ctrl+Alt+Shift+A", Todo),
        Do("Anti-Alias Text", "Ctrl+Alt+Shift+T", Todo),
        Do("Full", "", Todo),
    ]),
    Sep,
    Do("Rulers", "Ctrl+Alt+Shift+R", Stage(Toggle::Rulers)),
    Sub("Grid", &[Do("Show Grid", "Ctrl+'", Stage(Toggle::Grid)), Do("Edit Grid...", "Ctrl+Alt+G", Todo)]),
    Sub("Guides", &[
        Do("Show Guides", "Ctrl+;", Stage(Toggle::Guides)),
        Do("Lock Guides", "Ctrl+Alt+;", Stage(Toggle::LockGuides)),
        Do("Edit Guides...", "Ctrl+Alt+Shift+G", Todo),
        Do("Clear Guides", "", Todo),
    ]),
    Sub("Snapping", &[
        Do("Snap Align", "", Todo),
        Do("Snap to Grid", "Ctrl+Shift+'", Todo),
        Do("Snap to Guides", "Ctrl+Shift+;", Todo),
        Do("Snap to Pixels", "", Todo),
        Do("Snap to Objects", "Ctrl+Shift+U", Todo),
        Sep,
        Do("Edit Snapping...", "Ctrl+/", Todo),
    ]),
    Sep,
    Do("Hide Edges", "Ctrl+Shift+E", Todo),
    Do("Show Shape Hints", "Ctrl+Alt+I", Stage(Toggle::ShapeHints)),
    Do("Show Tab Order", "", Todo),
    Sep,
    Sub("Screen Mode", &[Do("Standard Screen Mode", "", Todo), Do("Full Screen Mode", "", Todo)]),
];

const INSERT: &[Item] = &[
    Do("New Symbol...", "Ctrl+F8", NewSymbol),
    Sep,
    Do("Motion Tween", "", CreateMotionTween),
    Do("Shape Tween", "", CreateShapeTween),
    Do("Classic Tween", "", CreateClassicTween),
    Sep,
    Sub("Timeline", &[
        Do("Layer", "", NewLayer),
        Do("Layer Folder", "", NewFolder),
        Do("Classic Motion Guide", "", AddMotionGuide),
        Sep,
        Do("Frame", "F5", InsertFrame),
        // F6 and F7 turn every selected frame into a keyframe, as Animate does.
        Do("Keyframe", "F6", ConvertToKeyframes),
        Do("Blank Keyframe", "F7", ConvertToBlankKeyframes),
    ]),
    Do("Scene", "", InsertScene),
];

const MODIFY: &[Item] = &[
    Do("Document...", "Ctrl+J", DocumentSettings),
    Sep,
    Do("Convert to Symbol...", "F8", ConvertToSymbol),
    Do("Convert to Bitmap", "", Todo),
    Do("Break Apart", "Ctrl+B", BreakApart),
    Sep,
    Sub("Bitmap", &[Do("Swap Bitmap...", "", Todo), Do("Trace Bitmap...", "", Todo)]),
    Sub("Symbol", &[Do("Swap Symbol...", "", Todo), Do("Duplicate Symbol...", "", Todo)]),
    Sub("Shape", &[
        Do("Smooth", "", Todo),
        Do("Straighten", "", Todo),
        Do("Advanced Smooth...", "Ctrl+Alt+Shift+M", Todo),
        Do("Advanced Straighten...", "Ctrl+Alt+Shift+N", Todo),
        Do("Optimize...", "Ctrl+Alt+Shift+C", Todo),
        Sep,
        Do("Convert Lines to Fills", "", Todo),
        Do("Expand Fill...", "", Todo),
        Do("Soften Fill Edges...", "", Todo),
        Sep,
        Do("Add Shape Hint", "Ctrl+Shift+H", AddShapeHint),
        Do("Remove All Hints", "", RemoveAllHints),
    ]),
    Sub("Combine Objects", &[
        Do("Delete Envelope", "", Todo),
        Sep,
        Do("Union", "", Todo),
        Do("Intersect", "", Todo),
        Do("Punch", "", Todo),
        Do("Crop", "", Todo),
    ]),
    Sub("Timeline", &[
        Do("Distribute to Layers", "Ctrl+Shift+D", Todo),
        Do("Distribute to Keyframes", "", Todo),
        Do("Layer Properties...", "", LayerProperties),
        Do("Reverse Frames", "", ReverseFrames),
        Do("Synchronize Symbols", "", Todo),
        Sep,
        Do("Convert to Keyframes", "", ConvertToKeyframes),
        Do("Clear Keyframe", "Shift+F6", ClearKeyframe),
        Do("Convert to Blank Keyframes", "", ConvertToBlankKeyframes),
    ]),
    Sub("Convert to Frame-by-Frame Animation", &[
        Do("Keyframe every frame", "", Todo),
        Do("Keyframe every other frame", "", Todo),
        Do("Keyframe every third frame", "", Todo),
        Do("Keyframe every fourth frame", "", Todo),
        Do("Custom...", "", Todo),
    ]),
    Sep,
    Sub("Transform", &[
        Do("Free Transform", "", FreeTransform),
        Do("Distort", "", Todo),
        Do("Envelope", "", Todo),
        Do("Scale", "", Todo),
        Do("Rotate and Skew", "", Todo),
        Do("Scale and Rotate...", "Ctrl+Alt+S", ScaleAndRotate),
        Sep,
        Do("Rotate 90° CW", "Ctrl+Shift+9", Rotate90Cw),
        Do("Rotate 90° CCW", "Ctrl+Shift+7", Rotate90Ccw),
        Sep,
        Do("Flip Vertical", "", FlipVertical),
        Do("Flip Horizontal", "", FlipHorizontal),
        Sep,
        Do("Remove Transform", "Ctrl+Shift+Z", RemoveTransform),
    ]),
    Sub("Arrange", &[
        Do("Bring to Front", "Ctrl+Shift+Up", Arrange(BringToFront)),
        Do("Bring Forward", "Ctrl+Up", Arrange(BringForward)),
        Do("Send Backward", "Ctrl+Down", Arrange(SendBackward)),
        Do("Send to Back", "Ctrl+Shift+Down", Arrange(SendToBack)),
        Sep,
        Do("Lock", "Ctrl+Alt+L", Todo),
        Do("Unlock All", "Ctrl+Alt+Shift+L", Todo),
    ]),
    Sub("Align", &[
        Do("Left", "Ctrl+Alt+1", Align(Edges { y: false, at: 0.0 })),
        Do("Horizontal Center", "Ctrl+Alt+2", Align(Edges { y: false, at: 0.5 })),
        Do("Right", "Ctrl+Alt+3", Align(Edges { y: false, at: 1.0 })),
        Do("Top", "Ctrl+Alt+4", Align(Edges { y: true, at: 0.0 })),
        Do("Vertical Center", "Ctrl+Alt+5", Align(Edges { y: true, at: 0.5 })),
        Do("Bottom", "Ctrl+Alt+6", Align(Edges { y: true, at: 1.0 })),
        Sep,
        Do("Distribute Widths Evenly", "Ctrl+Alt+7", Align(Space { y: false })),
        Do("Distribute Heights Evenly", "Ctrl+Alt+9", Align(Space { y: true })),
        Sep,
        Do("Make Same Width", "Ctrl+Alt+Shift+7", Align(Match { width: true, height: false })),
        Do("Make Same Height", "Ctrl+Alt+Shift+9", Align(Match { width: false, height: true })),
        Sep,
        Do("Align to Stage", "Ctrl+Alt+8", AlignToStage),
    ]),
    Sep,
    Do("Group", "Ctrl+G", Group),
    Do("Ungroup", "Ctrl+Shift+G", Ungroup),
];

const TEXT: &[Item] = &[
    Sub("Font", &[]),
    Sub("Size", &[]),
    Sub("Style", &[
        Do("Plain", "Ctrl+Shift+P", Todo),
        Do("Bold", "Ctrl+Shift+B", Todo),
        Do("Italic", "Ctrl+Shift+I", Todo),
        Do("Faux Bold", "", Todo),
        Do("Faux Italic", "", Todo),
        Do("Subscript", "", Todo),
        Do("Superscript", "", Todo),
    ]),
    Sub("Align", &[
        Do("Align Left", "Ctrl+Shift+L", Todo),
        Do("Align Center", "Ctrl+Shift+C", Todo),
        Do("Align Right", "Ctrl+Shift+R", Todo),
        Do("Justify", "Ctrl+Shift+J", Todo),
    ]),
    Sub("Letter Spacing", &[
        Do("Increase", "Ctrl+Alt+Right", Todo),
        Do("Decrease", "Ctrl+Alt+Left", Todo),
        Do("Reset", "Ctrl+Alt+Up", Todo),
    ]),
    Do("Scrollable", "", Todo),
    Sep,
    Do("Check Spelling...", "", Todo),
    Do("Spelling Setup...", "", Todo),
    Sep,
    Do("Font Embedding...", "", Todo),
];

// "Get More Commands..." would lead to Adobe's add-ons site, so it's left out.
const COMMANDS: &[Item] = &[
    Do("Manage Saved Commands...", "", Todo),
    Do("Run Command...", "", Todo),
    Sep,
    Do("Copy Font Names for ActionScript", "", Todo),
    Do("Copy Motion as XML", "", Todo),
    Do("Export Motion XML", "", Todo),
    Do("Import Motion XML", "", Todo),
];

const CONTROL: &[Item] = &[
    Do("Play", "Enter", Play),
    Do("Rewind", "Shift+,", Rewind),
    Do("Go To End", "Shift+.", GoToEnd),
    Sep,
    Do("Step Forward One Frame", ".", StepForward),
    Do("Step Backward One Frame", ",", StepBack),
    Do("Step Forward To Next Keyframe", "Alt+.", NextKeyframe),
    Do("Step Backward To Previous Keyframe", "Alt+,", PrevKeyframe),
    Sep,
    Do("Test", "Ctrl+Enter", TestMovie),
    Sub("Test Movie", &[Do("In minnamate", "", TestMovie), Do("In Browser", "", Todo)]),
    Do("Test Scene", "Ctrl+Alt+Enter", Todo),
    Sep,
    Do("Clear Publish Cache", "", Todo),
    // There's no publish cache to clear.
    Do("Clear Publish Cache and Test Movie", "", TestMovie),
    Sep,
    Do("Loop Playback", "", LoopPlayback),
    Do("Play All Scenes", "", Todo),
    Sep,
    Do("Enable Simple Frame Actions", "Ctrl+Alt+F", Todo),
    Do("Enable Simple Buttons", "Ctrl+Alt+B", Todo),
    Do("Enable Live Preview", "", Todo),
    Do("Mute Sounds", "Ctrl+Alt+M", Todo),
];

const DEBUG: &[Item] = &[
    Do("Debug", "Ctrl+Shift+Enter", Todo),
    Sub("Debug Movie", &[
        Do("In minnamate", "", Todo),
        Do("AIR Debug Launcher (Desktop)", "", Todo),
        Do("AIR Debug Launcher (Mobile)", "", Todo),
        Do("On Device via USB", "", Todo),
    ]),
    Sep,
    Do("Continue", "Alt+F5", Todo),
    Do("End Debug Session", "Alt+F12", Todo),
    Sep,
    Do("Step In", "Alt+F6", Todo),
    Do("Step Over", "Alt+F7", Todo),
    Do("Step Out", "Alt+F8", Todo),
    Sep,
    Do("Remove All Breakpoints", "Ctrl+Shift+B", Todo),
    Sep,
    Sub("Begin Remote Debug Session", &[Do("ActionScript 3.0", "", Todo)]),
];

/// Window > Workspaces and the app bar's switcher. Only Essentials is built.
const WORKSPACES: [&str; 8] = ["Animator", "Basic", "Classic", "Debug", "Designer", "Developer", "Essentials", "Small Screen"];

// CC Libraries is an Adobe service, so it's left out.
const WINDOW: &[Item] = &[
    Do("Duplicate Window", "Ctrl+Alt+K", Todo),
    Sep,
    Sub("Toolbars", &[Do("Edit Bar", "", Todo)]),
    Sep,
    Do("Timeline", "Ctrl+Alt+T", Show(PanelId::Timeline)),
    Do("Tools", "Ctrl+F2", Show(PanelId::Tools)),
    Do("Properties", "Ctrl+F3", Show(PanelId::Properties)),
    Do("Library", "Ctrl+L", Show(PanelId::Library)),
    Do("Assets", "", Show(PanelId::Assets)),
    Do("Brush Library", "", Todo),
    Do("Motion Presets", "", Todo),
    Do("Frame Picker", "", Todo),
    Do("Layer Depth", "", Todo),
    Sep,
    Do("Actions", "F9", Todo),
    Do("Code Snippets", "", Todo),
    Do("Compiler Errors", "Alt+F2", Todo),
    Sub("Debug Panels", &[]),
    Do("Output", "F2", Show(PanelId::Output)),
    Sep,
    Do("Align", "Ctrl+K", Show(PanelId::Align)),
    Do("Color", "Ctrl+Shift+F9", Show(PanelId::Color)),
    Do("Info", "Ctrl+I", Show(PanelId::Info)),
    Do("Swatches", "Ctrl+F9", Show(PanelId::Swatches)),
    Do("Transform", "Ctrl+T", Show(PanelId::Transform)),
    Sep,
    Do("Components", "Ctrl+F7", Todo),
    Do("Component Parameters", "", Todo),
    Sep,
    Do("History", "Ctrl+F10", Todo),
    Do("Scene", "Shift+F2", Todo),
    Sep,
    Sub("Extensions", &[]),
    Sep,
    Sub("Workspaces", &[
        Do("Animator", "", Todo),
        Do("Basic", "", Todo),
        Do("Classic", "", Todo),
        Do("Debug", "", Todo),
        Do("Designer", "", Todo),
        Do("Developer", "", Todo),
        Do("Essentials", "", Workspace),
        Do("Small Screen", "", Todo),
        Sep,
        Do("Manage Workspaces...", "", Todo),
        Do("Reset 'Essentials'", "", ResetWorkspace),
    ]),
    Do("Hide Panels", "F4", HidePanels),
    Sep,
    Documents,
];

// Animate's Sign In / Sign Out are for an Adobe account, so they're left out.
const HELP: &[Item] = &[
    Do("minnamate Help", "F1", Todo),
    Do("Support Center", "", Todo),
    Sep,
    Do("Updates...", "", Todo),
    Sep,
    Do("About minnamate", "", About),
];

const MENUS: &[(&str, &[Item])] = &[
    ("File", FILE),
    ("Edit", EDIT),
    ("View", VIEW),
    ("Insert", INSERT),
    ("Modify", MODIFY),
    ("Text", TEXT),
    ("Commands", COMMANDS),
    ("Control", CONTROL),
    ("Debug", DEBUG),
    ("Window", WINDOW),
    ("Help", HELP),
];

/// Whether `action` can run now; menus grey out the rest, as Animate does.
fn enabled(ed: &Editor, action: Action) -> bool {
    let picked = !ed.selection.is_empty();
    let lone_instance = matches!(ed.selection[..], [r] if matches!(ed.element(r), Some(Element::Instance(_))));
    ed.home.allows(action)
        && crate::timeline::available(ed, action)
        && match action {
            Todo => false,
            Undo => ed.can_undo(),
            Redo => ed.can_redo(),
            PasteInCenter | PasteInPlace => !ed.clipboard.is_empty() && ed.drawable(),
            Cut | Copy | DeleteSelection | Duplicate | Group | Arrange(_) | Align(_) | RemoveTransform | ScaleAndRotate
            | FlipHorizontal | FlipVertical | Rotate90Cw | Rotate90Ccw => picked,
            // It converts on a scene's timeline only (convert_symbol::open).
            ConvertToSymbol => picked && ed.editing.is_empty(),
            Ungroup => ed.selection.iter().any(|&r| matches!(ed.element(r), Some(Element::Group(_)))),
            // Shapes and text are as far apart as they go (arrange::broken_apart).
            BreakApart => ed.selection.iter().any(|&r| matches!(ed.element(r), Some(Element::Group(_) | Element::Instance(_)))),
            EditInPlace => lone_instance,
            AddShapeHint | RemoveAllHints => crate::tweens::hint_available(ed, action),
            EditSymbol => !ed.editing.is_empty() || lone_instance,
            // First, Previous, Next and Last need another scene to go to, or a symbol to leave.
            GoToScene(go) => matches!(go, Go::To(_)) || ed.scene_at(go) != ed.scene || !ed.editing.is_empty(),
            _ => true,
        }
}

/// Whether `action`'s item shows a check: toggles that are on, panels that are open.
fn checked(ed: &Editor, action: Action) -> bool {
    match action {
        LoopPlayback => ed.loop_playback,
        ClipToStage => ed.clip_to_stage,
        AlignToStage => ed.align_to_stage,
        Show(panel) => ed.panel_shown(panel),
        Stage(t) => ed.stage.on(t),
        Workspace => true,
        _ => false,
    }
}

/// The item's label as it reads now.
fn label(ed: &Editor, label: &'static str, action: Action) -> &'static str {
    match action {
        Redo if !ed.can_redo() => "Repeat",
        EditSymbol if !ed.editing.is_empty() => "Edit Document",
        Play if ed.playing.is_some() => "Stop",
        _ => label,
    }
}

/// An item picked from a list, checked when `on`. Returns whether it was clicked.
fn check_item(ui: &mut Ui, enabled: bool, text: &str, on: bool) -> bool {
    let resp = theme::menu_item(ui, enabled, text, "");
    if on {
        theme::menu_check(ui, &resp);
    }
    resp.clicked()
}

fn items(ui: &mut Ui, list: &[Item], ed: &Editor, picked: &mut Option<Action>) {
    for item in list {
        match item {
            Sep => {
                ui.separator();
            }
            Sub(label, []) => {
                ui.add_enabled_ui(false, |ui| ui.menu_button(*label, |_| {}));
            }
            Sub(label, sub) => {
                ui.menu_button(*label, |ui| items(ui, sub, ed, picked));
            }
            Recent => {
                ui.menu_button("Open Recent", |ui| {
                    if ed.recent.is_empty() {
                        theme::menu_item(ui, false, "(empty)", "");
                    }
                    for (i, path) in ed.recent.iter().enumerate() {
                        if theme::menu_item(ui, true, &path.to_string_lossy(), "").clicked() {
                            *picked = Some(OpenRecent(i));
                        }
                    }
                });
            }
            Scenes => {
                for (i, scene) in ed.doc.scenes.iter().enumerate() {
                    if check_item(ui, enabled(ed, GoToScene(Go::To(i))), &scene.name, i == ed.scene) {
                        *picked = Some(GoToScene(Go::To(i)));
                    }
                }
            }
            Documents => {
                if ed.home.doc_open {
                    let title = if ed.dirty { format!("{}*", ed.title) } else { ed.title.clone() };
                    if check_item(ui, true, &title, true) {
                        *picked = Some(ShowDocument);
                    }
                }
            }
            Do(text, keys, action) => {
                // While a field has the keyboard, what its keys leave to the field is greyed here too.
                // ponytail: Animate's menus send Select All, Copy, Undo... on to the field; add that if missed.
                let the_fields = ui.ctx().egui_wants_keyboard_input() && !crate::app::runs_while_typing(*action);
                let resp = theme::menu_item(ui, enabled(ed, *action) && !the_fields, label(ed, text, *action), keys);
                if checked(ed, *action) {
                    theme::menu_check(ui, &resp);
                }
                if resp.clicked() {
                    *picked = Some(*action);
                }
            }
        }
    }
}

/// A menu item's shortcut, "Ctrl+Shift+Up" say, as modifiers and a key.
fn parse(keys: &str) -> Option<(Modifiers, Key)> {
    let (mods, key) = keys.rsplit_once('+').filter(|(_, key)| !key.is_empty()).unwrap_or(("", keys));
    let mut m = Modifiers::NONE;
    for part in mods.split('+').filter(|p| !p.is_empty()) {
        m = m | match part {
            "Ctrl" => Modifiers::COMMAND,
            "Shift" => Modifiers::SHIFT,
            "Alt" => Modifiers::ALT,
            _ => return None,
        };
    }
    Some((m, Key::from_name(&key.replace(' ', ""))?))
}

/// The shortcut of every menu item that does something, so the keys always
/// match what the menus show.
pub fn shortcuts() -> Vec<(Modifiers, Key, Action)> {
    fn walk(list: &[Item], out: &mut Vec<(Modifiers, Key, Action)>) {
        for item in list {
            match item {
                Do(_, keys, action) if *action != Todo => out.extend(parse(keys).map(|(m, k)| (m, k, *action))),
                Sub(_, sub) => walk(sub, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::new();
    MENUS.iter().for_each(|(_, list)| walk(list, &mut out));
    out
}

pub fn app_bar(ui: &mut Ui, ed: &mut Editor) {
    let mut picked = None;
    let ctx = ui.ctx().clone();
    // Empty bar space moves the window, and double-clicking it maximizes.
    let maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
    let title = ui.interact(ui.max_rect(), ui.id().with("title-bar"), Sense::click_and_drag());
    if title.drag_started() {
        ctx.send_viewport_cmd(ViewportCommand::StartDrag);
    }
    if title.double_clicked() {
        ctx.send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(8.0);
        if let Some(logo) = &ed.logo {
            ui.add(egui::Image::new((logo.id(), vec2(16.0, 16.0))).corner_radius(3));
        }
        if theme::icon_button(ui, 24.0, ed.home.shown, true, "Home", |p, r, c| icons::paint(p, r.shrink(4.0), Icon::Home, c)).clicked() {
            crate::home::toggle(ed);
        }
        theme::menu_bar().ui(ui, |ui| {
            for (title, list) in MENUS {
                ui.menu_button(*title, |ui| {
                    ui.set_min_width(240.0);
                    items(ui, list, ed, &mut picked);
                });
            }
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            window_buttons(ui, maximized);
            ui.add_space(12.0);
            ui.spacing_mut().item_spacing.x = 4.0;
            let workspace = theme::icon_button(ui, 28.0, false, true, "Workspaces", |p, r, c| icons::paint(p, r.shrink(6.0), Icon::Workspace, c));
            theme::dark_popup(&workspace).show(|ui| workspaces(ui, &mut picked));
            let share = theme::icon_button(ui, 28.0, false, true, "Quick Share and Publish", |p, r, c| icons::paint(p, r.shrink(6.0), Icon::Publish, c));
            if share.clicked() {
                ui.data_mut(|d| d.remove::<bool>(egui::Id::new("share-publish")));
            }
            theme::dark_popup(&share).show(|ui| share_and_publish(ui, &mut picked));
            let test = theme::icon_button(ui, 28.0, false, true, "Test Movie (Ctrl+Enter)", |p, r, c| icons::paint(p, r.shrink(6.0), Icon::TestMovie, c));
            if test.clicked() {
                picked = Some(TestMovie);
            }
        });
    });
    if let Some(action) = picked {
        ed.run(action, ui.ctx());
    }
}

/// The workspace switcher: a row per workspace with its reset button, the
/// current one checked, then a field to save the layout as a new workspace.
fn workspaces(ui: &mut Ui, picked: &mut Option<Action>) {
    ui.set_width(230.0);
    ui.spacing_mut().item_spacing.y = 0.0;
    for name in WORKSPACES {
        let current = name == "Essentials";
        let row = theme::popup_row(ui, name, current, current);
        let reset = Rect::from_center_size(row.rect.right_center() - vec2(16.0, 0.0), vec2(20.0, 20.0));
        let resp = ui.interact(reset, ui.id().with(("reset", name)), if current { Sense::click() } else { Sense::hover() });
        let color = if !current { theme::TEXT_DISABLED } else if resp.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
        icons::paint(ui.painter(), reset.shrink(3.0), Icon::Reset, color);
        if resp.on_hover_text(format!("Reset '{name}'")).clicked() {
            *picked = Some(ResetWorkspace);
            ui.close();
        }
    }
    ui.add_space(6.0);
    ui.separator();
    ui.add_space(4.0);
    ui.label(RichText::new("New Workspace").color(theme::TEXT_DIM));
    ui.add_space(4.0);
    ui.horizontal(|ui| {
        let id = ui.id().with("new-workspace");
        let mut name = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_default();
        ui.add(egui::TextEdit::singleline(&mut name).hint_text("Name").desired_width(176.0));
        ui.data_mut(|d| d.insert_temp(id, name));
        // ponytail: layouts can't be saved yet, so the button stays off.
        theme::icon_button(ui, 24.0, false, false, "Save Workspace", |p, r, c| icons::paint(p, r.shrink(4.0), Icon::NewLayer, c));
    });
    ui.add_space(4.0);
}

/// Share: Social Share (not built) and Publish, whose page lists the formats
/// with a Publish button.
fn share_and_publish(ui: &mut Ui, picked: &mut Option<Action>) {
    ui.set_width(240.0);
    ui.spacing_mut().item_spacing.y = 0.0;
    let page = egui::Id::new("share-publish");
    let chevron = |ui: &Ui, row: &egui::Response, icon: Icon, left: bool| {
        let at = if left { row.rect.left_center() + vec2(12.0, 0.0) } else { row.rect.right_center() - vec2(14.0, 0.0) };
        icons::paint(ui.painter(), Rect::from_center_size(at, vec2(12.0, 12.0)), icon, theme::TEXT_DIM);
    };
    if !ui.data(|d| d.get_temp::<bool>(page).unwrap_or(false)) {
        let social = theme::popup_row(ui, "Social Share", false, false);
        chevron(ui, &social, Icon::ChevronRight, false);
        let publish = theme::popup_row(ui, "Publish", false, true);
        chevron(ui, &publish, Icon::ChevronRight, false);
        if publish.clicked() {
            ui.data_mut(|d| d.insert_temp(page, true));
        }
        return;
    }
    let back = theme::popup_row(ui, "Publish", false, true);
    chevron(ui, &back, Icon::ChevronLeft, true);
    if back.clicked() {
        ui.data_mut(|d| d.insert_temp(page, false));
    }
    ui.separator();
    theme::popup_row(ui, "SWF movie", true, true);
    theme::popup_row(ui, "Video (MP4)", false, false);
    theme::popup_row(ui, "Animated GIF", false, false);
    ui.add_space(8.0);
    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
        ui.add_space(8.0);
        let button = egui::Button::new(RichText::new("Publish").color(Color32::WHITE)).fill(theme::CTA).stroke(egui::Stroke::NONE);
        if ui.add(button.min_size(vec2(72.0, 24.0))).clicked() {
            *picked = Some(ExportSwf);
            ui.close();
        }
    });
    ui.add_space(8.0);
}

/// Minimize, maximize (or restore) and close, as Windows 11 draws them.
fn window_buttons(ui: &mut Ui, maximized: bool) {
    let buttons = [
        (Icon::WinClose, "Close", ViewportCommand::Close),
        (if maximized { Icon::WinRestore } else { Icon::WinMaximize }, if maximized { "Restore Down" } else { "Maximize" }, ViewportCommand::Maximized(!maximized)),
        (Icon::WinMinimize, "Minimize", ViewportCommand::Minimized(true)),
    ];
    for (icon, tip, command) in buttons {
        let (rect, resp) = ui.allocate_exact_size(vec2(46.0, ui.available_height()), Sense::click());
        let close = icon == Icon::WinClose;
        if resp.hovered() {
            let hover = if close { theme::CLOSE_HOVER } else { Color32::from_white_alpha(20) };
            ui.painter().rect_filled(rect, 0.0, hover);
        }
        let color = if close && resp.hovered() { Color32::WHITE } else { theme::TEXT };
        // At 18 px the glyphs are on whole pixels: 10 px, in 1 px lines.
        icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(18.0, 18.0)), icon, color);
        if resp.on_hover_text(tip).clicked() {
            ui.ctx().send_viewport_cmd(command);
        }
    }
}

/// Without the system title bar, the window's edges resize it: the pointer
/// within a few pixels of an edge shows a resize cursor, and pressing starts
/// the resize.
pub fn window_resize(ctx: &egui::Context) {
    if ctx.input(|i| i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)) {
        return;
    }
    let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) else { return };
    let r = ctx.viewport_rect();
    let edge = 4.0;
    let (west, east) = (pos.x < r.left() + edge, pos.x > r.right() - edge);
    let (north, south) = (pos.y < r.top() + edge, pos.y > r.bottom() - edge);
    let (direction, cursor) = match (north, south, west, east) {
        (true, _, true, _) => (ResizeDirection::NorthWest, CursorIcon::ResizeNorthWest),
        (true, _, _, true) => (ResizeDirection::NorthEast, CursorIcon::ResizeNorthEast),
        (_, true, true, _) => (ResizeDirection::SouthWest, CursorIcon::ResizeSouthWest),
        (_, true, _, true) => (ResizeDirection::SouthEast, CursorIcon::ResizeSouthEast),
        (true, ..) => (ResizeDirection::North, CursorIcon::ResizeNorth),
        (_, true, ..) => (ResizeDirection::South, CursorIcon::ResizeSouth),
        (_, _, true, _) => (ResizeDirection::West, CursorIcon::ResizeWest),
        (_, _, _, true) => (ResizeDirection::East, CursorIcon::ResizeEast),
        _ => return,
    };
    ctx.set_cursor_icon(cursor);
    if ctx.input(|i| i.pointer.primary_pressed()) {
        ctx.send_viewport_cmd(ViewportCommand::BeginResize(direction));
    }
}

#[cfg(test)]
mod tests {
    use minnamate_core::hit::ElementRef;

    use super::*;
    use crate::app::testkit::*;

    /// Every item, submenus included.
    fn all(list: &'static [Item], out: &mut Vec<&'static Item>) {
        for item in list {
            out.push(item);
            if let Sub(_, sub) = item {
                all(sub, out);
            }
        }
    }

    #[test]
    fn every_shortcut_parses_and_none_is_taken_twice() {
        let mut list = Vec::new();
        MENUS.iter().for_each(|(_, menu)| all(menu, &mut list));
        let mut seen = std::collections::HashMap::new();
        for item in list {
            let Do(label, keys, action) = item else { continue };
            if keys.is_empty() {
                continue;
            }
            let (mods, key) = parse(keys).unwrap_or_else(|| panic!("{label}: can't read {keys:?}"));
            if let Some((other, other_action)) = seen.insert((mods.command, mods.shift, mods.alt, key), (*label, *action)) {
                assert_eq!(other_action, *action, "{keys} is both {other} and {label}");
            }
        }
        assert_eq!(parse("Ctrl+Shift+Up"), Some((Modifiers::COMMAND | Modifiers::SHIFT, Key::ArrowUp)));
        assert_eq!(parse("Ctrl+-"), Some((Modifiers::COMMAND, Key::Minus)));
        assert_eq!(parse("Page Down"), Some((Modifiers::NONE, Key::PageDown)));
        assert_eq!(parse("Shift+,"), Some((Modifiers::SHIFT, Key::Comma)));
        assert!(shortcuts().contains(&(Modifiers::COMMAND | Modifiers::ALT, Key::Num8, AlignToStage)));
        assert!(!shortcuts().iter().any(|&(_, _, a)| a == Todo), "greyed items have no working key");
    }

    // Elements of the tween fixture's frame 1.
    const BLINK: ElementRef = ElementRef { layer: 0, keyframe: 0, element: 0 };
    const BALL: ElementRef = ElementRef { layer: 1, keyframe: 0, element: 0 };
    const BACKGROUND: ElementRef = ElementRef { layer: 2, keyframe: 0, element: 0 };

    #[test]
    fn convert_and_paste_are_greyed_where_they_would_refuse() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let ed = h.state_mut();
        ed.selection = vec![BLINK];
        assert!(enabled(ed, ConvertToSymbol));
        // Convert to Symbol works on the scene only.
        ed.run(EditInPlace, &ctx);
        ed.run(SelectAll, &ctx);
        assert!(!ed.selection.is_empty() && !enabled(ed, ConvertToSymbol));
        ed.exit_symbol();
        // Pasting needs a normal layer, shown and unlocked.
        ed.selection = vec![BLINK];
        ed.run(Copy, &ctx);
        assert!(enabled(ed, PasteInCenter) && enabled(ed, PasteInPlace));
        ed.timeline_mut().layers[0].set_locked(true);
        assert!(!enabled(ed, PasteInCenter) && !enabled(ed, PasteInPlace));
    }

    #[test]
    fn break_apart_needs_a_group_or_an_instance() {
        let mut h = harness(false);
        h.run();
        let ed = h.state_mut();
        // A shape is as far apart as it goes.
        ed.selection = vec![BACKGROUND];
        assert!(!enabled(ed, BreakApart));
        ed.selection.push(BALL);
        assert!(enabled(ed, BreakApart));
    }

    #[test]
    fn go_to_needs_another_scene() {
        let mut h = harness(false);
        h.run();
        let ctx = h.ctx.clone();
        let ed = h.state_mut();
        (ed.frame, ed.layer, ed.selection) = (5, 1, vec![BALL]);
        for go in [Go::First, Go::Previous, Go::Next, Go::Last] {
            assert!(!enabled(ed, GoToScene(go)), "{go:?}");
            ed.run(GoToScene(go), &ctx);
        }
        // Picking the open scene keeps the playhead, as the edit bar's Edit Scene does.
        ed.run(GoToScene(Go::To(0)), &ctx);
        assert_eq!((ed.frame, ed.layer, &ed.selection[..]), (5, 1, &[BALL][..]));
        // With a second scene, Next goes there, to its first frame.
        ed.doc.insert_scene(0);
        assert!(enabled(ed, GoToScene(Go::Next)) && !enabled(ed, GoToScene(Go::Previous)));
        ed.run(GoToScene(Go::Next), &ctx);
        assert_eq!((ed.scene, ed.frame), (1, 0));
    }

    #[test]
    fn window_brings_the_document_forward_and_home_greys_the_scenes() {
        use egui_kittest::kittest::{NodeT, Queryable};
        let mut h = blank();
        crate::home::toggle(h.state_mut());
        h.run();
        assert!(h.state().home.shown);
        h.get_by_label("View").click();
        h.run();
        h.get_by_label("Go To ⏵").hover();
        // The submenu opens after a hover delay, repainting meanwhile.
        h.run_steps(10);
        assert!(h.get_by_label("Scene 1").accesskit_node().is_disabled(), "Home hides the scenes");
        h.key_press(egui::Key::Escape);
        h.run();
        h.get_by_label("Window").click();
        h.run();
        h.get_by_label("Untitled-1").click();
        h.run();
        assert!(!h.state().home.shown);
    }
}
