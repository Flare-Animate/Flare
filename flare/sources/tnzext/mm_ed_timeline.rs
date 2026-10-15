//! The Timeline panel, laid out as in Animate 2024 (docs/animate-ui.md): a
//! toolbar row, the layer list with its toggle columns, the two-tier ruler and
//! the frame grid. Frame and layer selection, the frame and layer commands and
//! their menus, the loop range and the onion skin settings live here too
//! (docs/animate-ui-interaction.md §2 and §1.10).

use std::borrow::Cow;
use std::collections::BTreeSet;

use eframe::egui::{
    self, Align, Align2, Color32, CursorIcon, FontId, Key, Layout, Modifiers, Painter, Pos2, Rect, Response, RichText,
    Sense, Stroke, StrokeKind, Ui, pos2, vec2,
};
use minnamate_core::doc::{Frame, Layer, LayerKind, Rgba, Timeline, Tween};
use minnamate_core::hit::ElementRef;
use minnamate_core::symbols;

use crate::app::{Action, Editor, PanelId, PropsTab};
use crate::icons::{self, Icon};
use crate::layer_tree::{self, INDENT};
use crate::theme;

/// Medium rows, the default height.
pub(crate) const ROW_H: f32 = 24.0;
/// The panel menu's row heights.
const ROW_HEIGHTS: [(&str, f32); 3] = [("Short", 18.0), ("Medium", ROW_H), ("Tall", 32.0)];
const TOOLBAR_H: f32 = 34.0;
/// The ruler: a 14 px tier for seconds and the loop range, a line, then ticks and numbers.
const RULER_H: f32 = 33.0;
const TOP_TIER: f32 = 14.0;
const LAYER_W: f32 = 280.0;
pub(crate) const DEFAULT_CELL_W: f32 = 10.0;
const MIN_CELL_W: f32 = 4.0;
const MAX_CELL_W: f32 = 30.0;
/// Toggle column centers, from the layer column's right edge: dot, outline, eye, lock.
pub(crate) const DOT_X: f32 = 99.0;
const OUTLINE_X: f32 = 77.0;
const EYE_X: f32 = 51.0;
const LOCK_X: f32 = 26.0;
/// The layer type icon's center, from the panel's left edge.
pub(crate) const TYPE_ICON_X: f32 = 30.0;
/// How far "All frames" onion skin reaches each way.
const ONION_ALL: u32 = 16;

const READOUT: Color32 = Color32::from_rgb(0xA1, 0xA1, 0xA1);
const ICON_INK: Color32 = Color32::from_rgb(0xEB, 0xEB, 0xEB);
const ICON_OFF: Color32 = Color32::from_rgb(0x6E, 0x6E, 0x6E);
const LOOP_BAR: Color32 = Color32::from_rgb(0x2A, 0x5A, 0x90);
const LOOP_HANDLE: Color32 = Color32::from_rgb(0x4C, 0x89, 0xCE);

/// Selected frames, as (layer, frame).
type Cells = BTreeSet<(usize, u32)>;

/// What a selection was made in: document revision, scene, symbol being
/// edited, layer count, active layer and playhead.
type SelKey = (u32, usize, Option<String>, usize, usize, u32);

/// What the timeline remembers besides the playhead and the active layer.
pub struct State {
    /// Selected frames. A folder, which has none, is selected through its frame 0.
    pub sel: Cells,
    /// Where the selection was made; once that changes (the playhead moved,
    /// or the document changed outside the timeline), the selection is the
    /// active layer's current frame again.
    sel_at: Option<SelKey>,
    /// Where Shift+click extends the selection from.
    anchor: (usize, u32),
    drag: Option<Drag>,
    /// The document revision and scene the gesture started on: a key changing
    /// them mid-drag ends it (see `track`).
    drag_on: (u32, usize),
    /// The cell last right-clicked, which the frame menu is about.
    menu_cell: (usize, u32),
    /// Cut or copied frames: a keyframe list per layer, and how many frames they span.
    clip: Option<(Vec<Vec<Frame>>, u32)>,
    /// Cut or copied layers.
    layer_clip: Vec<Layer>,
    /// Row height: Short, Medium or Tall in the panel menu.
    pub row_h: f32,
    /// Span Based Selection: a click selects a keyframe's whole span.
    pub span_select: bool,
    /// The panel menu's Lock: the panel can't be resized.
    pub locked: bool,
    /// The loop range's first and last frame; None is the whole timeline.
    pub loop_range: Option<(u32, u32)>,
    pub onion: Onion,
    /// The Onion skin settings window is open.
    pub onion_window: bool,
    /// The Layer Properties dialog, while open.
    props: Option<LayerProps>,
    /// The toolbar's Delete button, where dragged layers can be dropped.
    trash: Rect,
    /// The Onion Skin button, which its settings window opens above.
    pub(crate) onion_button: Rect,
    /// Show Parenting View: a column of layer heads to link layers by.
    pub parenting: bool,
    /// The layer whose parent list is open in the parenting view, and where it opened.
    pub(crate) parent_menu: Option<(usize, Pos2)>,
}

impl Default for State {
    fn default() -> Self {
        State {
            sel: Cells::new(),
            sel_at: None,
            anchor: (0, 0),
            drag: None,
            drag_on: (0, 0),
            menu_cell: (0, 0),
            clip: None,
            layer_clip: Vec::new(),
            row_h: ROW_H,
            span_select: false,
            locked: false,
            loop_range: None,
            onion: Onion::default(),
            onion_window: false,
            props: None,
            trash: Rect::NOTHING,
            onion_button: Rect::NOTHING,
            parenting: false,
            parent_menu: None,
        }
    }
}

impl State {
    /// For a newly opened document: settings stay, the rest goes.
    // ponytail: copied frames and layers don't take their library symbols along, so they
    // don't outlive their document; carry the symbols over to paste across documents.
    pub fn reset(&mut self) {
        self.clip = None;
        self.layer_clip.clear();
        self.sel_at = None;
        self.drag = None;
        self.loop_range = None;
        self.props = None;
        self.onion.anchor = None;
        self.parent_menu = None;
    }
}

/// Onion skin settings: the Onion button's menu and its settings window.
#[derive(Clone, PartialEq)]
pub struct Onion {
    /// Frames shown before and after the playhead.
    pub before: u32,
    pub after: u32,
    /// Opacity of the nearest onion frames, and how much fainter each further one is.
    pub opacity: f32,
    pub falloff: f32,
    /// "All frames" instead of the range around the playhead.
    pub all: bool,
    /// "Anchor markers": the range stays around this frame instead of following the playhead.
    pub anchor: Option<u32>,
    /// Only keyframes show, not the frames in between.
    pub keyframes_only: bool,
    /// "Constraints": before and after stay equal.
    pub linked: bool,
    pub before_color: Color32,
    pub after_color: Color32,
}

impl Default for Onion {
    fn default() -> Self {
        Onion {
            before: crate::app::ONION_RANGE,
            after: crate::app::ONION_RANGE,
            opacity: 0.7,
            falloff: 0.2,
            all: false,
            anchor: None,
            keyframes_only: false,
            linked: false,
            before_color: theme::ONION_BEFORE,
            after_color: theme::ONION_AFTER,
        }
    }
}

/// A pointer gesture in the timeline, from press to release.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Drag {
    /// Scrubbing the playhead along the ruler.
    Scrub,
    /// Dragging out a rectangle of frames from the anchor.
    Select,
    /// Pressed on a selected cell: a drag moves the selection (copies it with
    /// Alt); a click selects just the cell when `collapse`.
    Grab { cell: (usize, u32), copy: bool, collapse: bool },
    /// Ctrl pressed on a cell: a click adds or removes it, a drag from a
    /// span's last frame changes the span's length.
    Ctrl((usize, u32)),
    /// Moving the selected block by (layers, frames).
    Move { grab: (usize, u32), copy: bool, by: (isize, i64) },
    /// Ctrl+dragging the last frame of a span on `layer` to frame `to`.
    Stretch { layer: usize, last: u32, to: u32 },
    /// Setting a toggle column to `on` on each row the pointer passes.
    Toggle(Toggle, bool),
    /// Pressed on a layer's name: a drag moves the selected layers.
    Layers { row: usize, dragging: bool },
    /// Dragging layer `child`'s head in the parenting view, to the layer it will follow.
    Parent { child: usize },
    /// Dragging the loop range's ends, a new range from a frame, or the whole range (Shift).
    LoopStart,
    LoopEnd,
    LoopNew(u32),
    LoopMove { grab: u32, range: (u32, u32) },
}

/// The Layer Properties dialog's fields.
struct LayerProps {
    layer: usize,
    name: String,
    visible: bool,
    locked: bool,
    kind: LayerKind,
    color: Color32,
    outline: bool,
}

pub fn panel(ui: &mut Ui, ed: &mut Editor) {
    let (tab, tabs) = (ed.bottom_tab, [ed.tab_label(PanelId::Timeline, "Timeline"), ed.tab_label(PanelId::Output, "Output")]);
    if let Some(tab) = theme::tab_strip_menu(ui, &tabs, tab, Some(|ui: &mut Ui| panel_menu(ui, ed))) {
        ed.bottom_tab = tab;
    }
    if ed.bottom_tab == 1 {
        output(ui, ed);
        return;
    }
    toolbar(ui, ed);
    grid(ui, ed);
}

/// The panel menu (docs/animate-ui-interaction.md §2.7).
fn panel_menu(ui: &mut Ui, ed: &mut Editor) {
    ui.set_min_width(200.0);
    theme::menu_toggle(ui, true, true, "Standard");
    theme::menu_toggle(ui, false, false, "Preview");
    theme::menu_toggle(ui, false, false, "Preview in Context");
    ui.separator();
    for (label, h) in ROW_HEIGHTS {
        if theme::menu_toggle(ui, true, ed.tl.row_h == h, label).clicked() {
            ed.tl.row_h = h;
        }
    }
    ui.separator();
    theme::menu_toggle(ui, false, false, "Timeline control - Bottom");
    theme::menu_toggle(ui, false, false, "Match FPS");
    if theme::menu_toggle(ui, true, ed.tl.span_select, "Span Based Selection").clicked() {
        ed.tl.span_select = !ed.tl.span_select;
    }
    theme::menu_item(ui, false, "Customize Timeline Tools", "");
    ui.separator();
    if theme::menu_toggle(ui, true, ed.tl.locked, "Lock").clicked() {
        ed.tl.locked = !ed.tl.locked;
    }
    ui.separator();
    theme::menu_item(ui, false, "Help", "");
    ui.separator();
    for label in ["Close", "Close Group"] {
        if theme::menu_item(ui, true, label, "").clicked() {
            ed.show_bottom = false;
        }
    }
}

fn output(ui: &mut Ui, ed: &Editor) {
    egui::Frame::NONE.fill(theme::INSET).inner_margin(8).show(ui, |ui| {
        egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink(false).show(ui, |ui| {
            for line in &ed.output {
                ui.label(RichText::new(line).monospace().color(theme::TEXT));
            }
        });
    });
}

/// A toolbar button centered at `center`: a 28 px hit box with a 14 px glyph,
/// a chip behind it while toggled on (and pressed while its menu is open), and
/// a corner triangle for grouped buttons, which also sense drags so a long
/// press stays down (`group_menu`).
fn button_at(ui: &mut Ui, center: Pos2, icon: Icon, tooltip: &str, toggled: bool, enabled: bool, grouped: bool) -> Response {
    let rect = Rect::from_center_size(center, vec2(28.0, 28.0));
    let sense = match (enabled, grouped) {
        (false, _) => Sense::hover(),
        (true, false) => Sense::click(),
        (true, true) => Sense::click_and_drag(),
    };
    let resp = ui.interact(rect, ui.id().with(("tl-button", tooltip)), sense);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, tooltip));
    let open = egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    let p = ui.painter();
    if enabled && (resp.is_pointer_button_down_on() || open) {
        p.rect_filled(rect.expand(1.0), 4.0, theme::BUTTON_ACTIVE);
    } else if toggled {
        p.rect_filled(rect.expand(1.0), 4.0, theme::TOGGLED);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect.expand(1.0), 4.0, theme::BUTTON_HOVER);
    }
    let ink = if enabled { ICON_INK } else { ICON_OFF };
    icons::paint(p, Rect::from_center_size(center, vec2(16.0, 16.0)), icon, ink);
    if grouped {
        let c = rect.right_bottom() - vec2(6.0, 6.0);
        p.add(egui::Shape::convex_polygon(vec![c + vec2(3.0, 0.0), c + vec2(3.0, 3.0), c + vec2(0.0, 3.0)], ink, Stroke::NONE));
    }
    resp.on_hover_text(tooltip)
}

/// A grouped button's menu, in the light menu style: right-click the button,
/// or press and hold it. Holding opens it once egui stops counting the press
/// as a click, so letting go leaves the menu open.
fn group_menu(ui: &Ui, button: &Response, add: impl FnOnce(&mut Ui)) {
    let hold = ui.ctx().options(|o| o.input_options.max_click_duration);
    let down = button.is_pointer_button_down_on();
    let held = down && ui.input(|i| i.pointer.primary_down() && i.pointer.press_start_time().is_some_and(|t| i.time - t > hold));
    if down {
        ui.ctx().request_repaint_after_secs(0.05);
    }
    let open = button.secondary_clicked() || held;
    egui::Popup::from_response(button)
        .kind(egui::PopupKind::Menu)
        .layout(Layout::top_down_justified(Align::Min))
        .style(theme::menu_style)
        .open_memory(open.then_some(egui::SetOpenCommand::Bool(true)))
        .show(add);
}

/// A number with a small raised unit after it, as the toolbar readouts show them.
fn readout(p: &Painter, at: Pos2, value: &str, unit: &str, color: Color32) -> f32 {
    let big = p.layout_no_wrap(value.to_string(), FontId::proportional(13.0), color);
    let small = p.layout_no_wrap(unit.to_string(), FontId::proportional(8.0), color);
    let width = big.size().x + 2.0 + small.size().x;
    let top = at.y - big.size().y / 2.0;
    p.galley(pos2(at.x, top), big.clone(), color);
    p.galley(pos2(at.x + big.size().x + 2.0, top + 1.0), small, color);
    width
}

fn toolbar(ui: &mut Ui, ed: &mut Editor) {
    let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), TOOLBAR_H), Sense::hover());
    let p = ui.painter_at(bar);
    p.rect_filled(bar, 0.0, theme::PANEL_BG);
    p.hline(bar.x_range(), bar.bottom() - 0.5, Stroke::new(1.0, theme::BORDER));
    let mut action = None;
    let cy = bar.center().y;
    let (left, column_right) = (bar.left(), bar.left() + LAYER_W);

    // Above the layer list: layer commands, then camera, parenting and depth.
    if button_at(ui, pos2(left + 17.0, cy), Icon::TlNewLayer, "New Layer", false, true, false).clicked() {
        action = Some(Action::NewLayer);
    }
    if button_at(ui, pos2(left + 50.0, cy), Icon::Folder, "New Folder", false, true, false).clicked() {
        action = Some(Action::NewFolder);
    }
    let trash = button_at(ui, pos2(left + 82.0, cy), Icon::Delete, "Delete", false, available(ed, Action::DeleteLayer), false);
    ed.tl.trash = trash.rect;
    if trash.clicked() {
        action = Some(Action::DeleteLayer);
    }
    button_at(ui, pos2(column_right - 92.0, cy), Icon::Camera, "Add/Remove Camera", false, false, false);
    // ponytail: no press-and-hold menu (Propagate Scale, Skew, Flip): parenting always carries the whole transform.
    if button_at(ui, pos2(column_right - 59.0, cy), Icon::TlParenting, "Show Parenting View", ed.tl.parenting, true, true).clicked() {
        ed.tl.parenting = !ed.tl.parenting;
    }
    button_at(ui, pos2(column_right - 28.0, cy), Icon::TlLayerDepth, "Layer Depth", false, false, false);
    p.vline(column_right, bar.y_range(), Stroke::new(1.0, theme::BORDER));

    // Frame block: the readouts at the left, the cluster centered and the zoom
    // controls at the right. Short of room, the zoom controls go first, then
    // the readouts, the cluster's middle group and its keyframe group, so no
    // control is drawn over another.
    // ponytail: what goes has no overflow menu; its keys and menus still work.
    const PITCH: f32 = 30.0;
    const DIVIDER: f32 = 37.0;
    // The readouts' room ("24.00 FPS", then up to "1234 F"), the zoom controls'
    // from the bar's right end, and the space kept from both.
    const READOUTS_W: f32 = 130.0;
    const ZOOM_W: f32 = 148.0;
    const GAP: f32 = 4.0;
    // With `gone` of those given way: where the cluster may go, and its width.
    let span = |gone: u32| {
        let lo = column_right + if gone < 2 { READOUTS_W + GAP } else { 1.0 };
        let hi = bar.right() - if gone < 1 { ZOOM_W + GAP } else { 1.0 };
        let (middle, keys) = ((gone < 3) as u32, (gone < 4) as u32);
        (lo, hi, PITCH * (4 + 4 * middle + 3 * keys) as f32 + DIVIDER * (middle + keys) as f32)
    };
    let gone = (0..4)
        .find(|&gone| {
            let (lo, hi, width) = span(gone);
            hi - lo >= width
        })
        .unwrap_or(4);
    let (lo, hi, width) = span(gone);

    if gone < 2 {
        let p = p.with_clip_rect(Rect::from_x_y_ranges(column_right..=column_right + READOUTS_W, bar.y_range()));
        let mut x = column_right + 35.0;
        x += readout(&p, pos2(x, cy), &format!("{:.2}", ed.doc.frame_rate), "FPS", READOUT) + 14.0;
        readout(&p, pos2(x, cy), &(ed.frame + 1).to_string(), "F", theme::CURRENT_FRAME_TEXT);
    }

    // The cluster: keyframes | onion, edit multiple, tween, center | loop and transport.
    let mut cx = ((column_right + bar.right()) / 2.0 - width / 2.0).min(hi - width).max(lo) + PITCH / 2.0;
    let mut next = |gap: f32| {
        let at = cx;
        cx += gap;
        pos2(at, cy)
    };
    if gone < 4 {
        let (prev_key, next_key) = keyframe_neighbors(ed);
        if button_at(ui, next(PITCH), Icon::ChevronLeft, "Go to Previous Keyframe (Alt+,)", false, prev_key.is_some(), false).clicked() {
            action = Some(Action::PrevKeyframe);
        }
        let key_icon = match ed.key_command {
            Action::ConvertToBlankKeyframes => Icon::TlBlankKeyframe,
            Action::InsertFrame => Icon::TlFrame,
            _ => Icon::TlKeyframe,
        };
        let key = button_at(ui, next(PITCH), key_icon, "Insert Keyframe (hold for more)", false, true, true);
        if key.clicked() {
            action = Some(ed.key_command);
        }
        group_menu(ui, &key, |ui| {
            let commands = [("Keyframe", Action::ConvertToKeyframes), ("Blank Keyframe", Action::ConvertToBlankKeyframes), ("Frame", Action::InsertFrame)];
            for (label, command) in commands {
                if theme::menu_toggle(ui, true, ed.key_command == command, label).clicked() {
                    ed.key_command = command;
                    action = Some(command);
                }
            }
            ui.separator();
            theme::menu_toggle(ui, false, false, "Auto KeyFrame");
        });
        let at = next(PITCH + DIVIDER);
        if button_at(ui, at, Icon::ChevronRight, "Go to Next Keyframe (Alt+.)", false, next_key.is_some(), false).clicked() {
            action = Some(Action::NextKeyframe);
        }
        divider(&p, at.x + (PITCH + DIVIDER) / 2.0, cy);
    }
    if gone < 3 {
        let onion = button_at(ui, next(PITCH), Icon::TlOnion, "Onion Skin (hold for more)", ed.onion, true, true);
        ed.tl.onion_button = onion.rect;
        if onion.clicked() {
            ed.onion = !ed.onion;
        }
        group_menu(ui, &onion, |ui| onion_menu(ui, ed));
        button_at(ui, next(PITCH), Icon::TlEditMultiple, "Edit Multiple Frames", false, false, true);
        // The glyph shows the kind of tween last picked from its menu, which a click creates.
        let (kind, icon, tip) = crate::tweens::tween_button(ui.ctx());
        let tween = button_at(ui, next(PITCH), icon, tip, false, true, true);
        if tween.clicked() {
            action = Some(kind);
        }
        group_menu(ui, &tween, |ui| {
            let kinds = [("Create classic tween", Action::CreateClassicTween), ("Create motion tween", Action::CreateMotionTween), ("Create shape tween", Action::CreateShapeTween)];
            for (label, command) in kinds {
                if theme::menu_toggle(ui, available(ed, command), kind == command, label).clicked() {
                    crate::tweens::pick_tween(ui.ctx(), command);
                    action = Some(command);
                }
            }
        });
        let at = next(PITCH + DIVIDER);
        if button_at(ui, at, Icon::TlCenterFrame, "Center Frame", false, true, false).clicked() {
            ed.center_frame = true;
        }
        divider(&p, at.x + (PITCH + DIVIDER) / 2.0, cy);
    }
    if button_at(ui, next(PITCH), Icon::TlLoop, "Loop", ed.loop_playback, true, false).clicked() {
        action = Some(Action::LoopPlayback);
    }
    if button_at(ui, next(PITCH), Icon::TlStepBack, "Step back one frame (,)", false, true, false).clicked() {
        action = Some(Action::StepBack);
    }
    let (play_icon, play_tip) = if ed.playing.is_some() { (Icon::Pause, "Stop (Enter)") } else { (Icon::Play, "Play (Enter)") };
    if button_at(ui, next(PITCH), play_icon, play_tip, false, true, false).clicked() {
        action = Some(Action::Play);
    }
    if button_at(ui, next(PITCH), Icon::TlStepForward, "Step forward one frame (.)", false, true, false).clicked() {
        action = Some(Action::StepForward);
    }

    // Right: reset zoom, the cell-width slider and its "large" end.
    if gone < 1 {
        let right = bar.right() - 12.0;
        icons::paint(&p, Rect::from_center_size(pos2(right - 9.0, cy), vec2(16.0, 12.0)), Icon::TlMountains, ICON_INK);
        let track = Rect::from_min_max(pos2(right - 104.0, cy - 1.0), pos2(right - 24.0, cy + 1.0));
        p.rect_filled(track, 1.0, theme::SLIDER_TRACK);
        let t = (ed.cell_w - MIN_CELL_W) / (MAX_CELL_W - MIN_CELL_W);
        let knob = pos2(track.left() + t * track.width(), cy);
        let slider = ui.interact(track.expand2(vec2(6.0, 8.0)), ui.id().with("cell-width"), Sense::click_and_drag()).on_hover_text("Resize Timeline View");
        if let Some(pos) = slider.interact_pointer_pos()
            && (slider.dragged() || slider.clicked())
        {
            let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
            ed.cell_w = MIN_CELL_W + t * (MAX_CELL_W - MIN_CELL_W);
        }
        p.circle_filled(knob, 6.0, theme::PANEL_BG);
        p.circle_stroke(knob, 5.0, Stroke::new(2.0, Color32::from_rgb(0xE0, 0xE0, 0xE0)));
        let reset = button_at(ui, pos2(track.left() - 18.0, cy), Icon::TlResetZoom, "Reset Timeline Zoom To The Default Level", false, true, false);
        if reset.clicked() {
            ed.cell_w = DEFAULT_CELL_W;
        }
    }

    if let Some(action) = action {
        ed.run(action, ui.ctx());
    }
}

/// The Onion Skin button's menu: the range mode, anchoring, and the settings window.
fn onion_menu(ui: &mut Ui, ed: &mut Editor) {
    for (label, all) in [("Selected range", false), ("All frames", true)] {
        if theme::menu_toggle(ui, true, ed.tl.onion.all == all, label).clicked() {
            (ed.tl.onion.all, ed.onion) = (all, true);
        }
    }
    if theme::menu_toggle(ui, true, ed.tl.onion.anchor.is_some(), "Anchor markers").clicked() {
        ed.tl.onion.anchor = if ed.tl.onion.anchor.is_some() { None } else { Some(ed.frame) };
    }
    ui.separator();
    if theme::menu_item(ui, true, "Advanced Settings", "").clicked() {
        ed.tl.onion_window = true;
    }
}

fn divider(p: &Painter, x: f32, cy: f32) {
    p.vline(x, cy - 12.5..=cy + 12.5, Stroke::new(1.0, theme::GROUP_DIVIDER));
}

/// The keyframes before and after the playhead on the active layer.
pub(crate) fn keyframe_neighbors(ed: &Editor) -> (Option<u32>, Option<u32>) {
    let Some(layer) = ed.timeline().layers.get(ed.layer) else { return (None, None) };
    let prev = layer.frames.iter().rev().map(|k| k.index).find(|&i| i < ed.frame);
    let next = layer.frames.iter().map(|k| k.index).find(|&i| i > ed.frame);
    (prev, next)
}

/// The text field over a layer's name while it's being renamed: Enter or
/// clicking away commits, Escape cancels.
fn rename_field(ui: &mut Ui, ed: &mut Editor, geo: &Geo, left: f32) {
    let indent = ed.renaming.as_ref().map_or(0.0, |(l, _)| ed.timeline().depth(*l) as f32 * INDENT);
    let Some((layer, name)) = &mut ed.renaming else { return };
    let top = geo.top_of(*layer);
    let rect = Rect::from_min_max(pos2(left + 46.0 + indent, top + 2.0), pos2(geo.frames.left() - DOT_X - 12.0, top + geo.row_h - 2.0));
    let resp = ui.put(rect, egui::TextEdit::singleline(name).margin(egui::vec2(3.0, 1.0)));
    if !resp.has_focus() && !resp.lost_focus() {
        resp.request_focus();
    }
    let cancel = ui.input(|i| i.key_pressed(egui::Key::Escape));
    if resp.lost_focus() || cancel {
        let (layer, name) = ed.renaming.take().unwrap_or_default();
        let name = name.trim().to_string();
        if !cancel && !name.is_empty() && ed.timeline().layers.get(layer).is_some_and(|l| l.name != name) {
            ed.checkpoint();
            ed.timeline_mut().layers[layer].name = name;
        }
    }
}

/// The color the timeline shows for layer `i`: the document's, or Animate's sequence.
pub(crate) fn layer_color(layer: &Layer, i: usize) -> Color32 {
    match layer.color() {
        Some([r, g, b, _]) => Color32::from_rgb((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8),
        None => theme::LAYER_COLORS[i % theme::LAYER_COLORS.len()],
    }
}

/// Before layers are added, removed or moved: stores each layer's color in
/// it, so the others keep theirs.
pub(crate) fn pin_colors(tl: &mut Timeline) {
    for (i, l) in tl.layers.iter_mut().enumerate() {
        if l.color().is_none() {
            l.set_color(crate::app::rgba(layer_color(l, i)));
        }
    }
}

/// A new layer's color: the next in Animate's sequence.
pub(crate) fn next_color(tl: &Timeline) -> Rgba {
    crate::app::rgba(theme::LAYER_COLORS[tl.layers.len() % theme::LAYER_COLORS.len()])
}

/// Copies of layers `rows`, folders with what they hold, their colors stored in them, to paste elsewhere.
fn layer_copies(ed: &Editor, rows: &[usize]) -> Vec<Layer> {
    let mut tl = ed.timeline().clone();
    pin_colors(&mut tl);
    tl.copy_layers(rows)
}

/// Which toggle column a point in the layer list is over.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Toggle {
    Highlight,
    Outline,
    Visible,
    Lock,
}

fn toggle_at(x: f32, column_right: f32) -> Option<Toggle> {
    [(DOT_X, Toggle::Highlight), (OUTLINE_X, Toggle::Outline), (EYE_X, Toggle::Visible), (LOCK_X, Toggle::Lock)]
        .into_iter()
        .find(|(dx, _)| (x - (column_right - dx)).abs() <= 12.0)
        .map(|(_, t)| t)
}

/// Whether layer `l` shows `which`'s mark: hidden, locked or drawn as outlines.
fn marked(l: &Layer, which: Toggle) -> bool {
    match which {
        Toggle::Visible => !l.visible(),
        Toggle::Lock => l.locked(),
        Toggle::Outline => l.outline(),
        Toggle::Highlight => false,
    }
}

fn set_marked(l: &mut Layer, which: Toggle, on: bool) {
    match which {
        Toggle::Visible => l.set_visible(!on),
        Toggle::Lock => l.set_locked(on),
        Toggle::Outline => l.set_outline(on),
        Toggle::Highlight => {}
    }
}

/// Sets `which` on one row, and on what's in it if it's a folder; a press and drag
/// down the column is one undo step.
fn set_toggle(ed: &mut Editor, which: Toggle, row: usize, on: bool) {
    if ed.timeline().layers.get(row).is_some_and(|l| marked(l, which) != on) {
        ed.checkpoint_gesture("layer-toggle");
        let tl = ed.timeline_mut();
        for l in tl.with_contents(&[row]) {
            set_marked(&mut tl.layers[l], which, on);
        }
        keep_selection(ed);
    }
}

/// A header icon: clears the mark from every layer if any has it, else marks them all.
fn toggle_all(ed: &mut Editor, which: Toggle) {
    let on = !ed.timeline().layers.iter().any(|l| marked(l, which));
    ed.checkpoint();
    for l in &mut ed.timeline_mut().layers {
        set_marked(l, which, on);
    }
    keep_selection(ed);
}

/// Alt+click: marks every other layer and clears `row` (a folder, with what's in it);
/// when the others all had the mark already, clears it everywhere.
fn solo(ed: &mut Editor, which: Toggle, row: usize) {
    let own = ed.timeline().with_contents(&[row]);
    let others = ed.timeline().layers.iter().enumerate().filter(|(i, _)| !own.contains(i)).all(|(_, l)| marked(l, which));
    ed.checkpoint();
    for (i, l) in ed.timeline_mut().layers.iter_mut().enumerate() {
        set_marked(l, which, !own.contains(&i) && !others);
    }
    keep_selection(ed);
}

// Frame selection (docs/animate-ui-interaction.md §2.1).

fn sel_key(ed: &Editor) -> SelKey {
    (ed.rev, ed.scene, ed.editing.last().map(|t| t.symbol.clone()), ed.timeline().layers.len(), ed.layer, ed.frame)
}

/// Keeps the selection through an edit the timeline made itself.
pub(crate) fn keep_selection(ed: &mut Editor) {
    ed.tl.sel_at = Some(sel_key(ed));
}

/// Keeps the selection to where it was made (see `State::sel_at`). Another
/// scene or symbol starts scrolled to its playhead, with no loop range.
fn sync(ed: &mut Editor) {
    let key = sel_key(ed);
    if ed.tl.sel_at.as_ref() != Some(&key) {
        if ed.tl.sel_at.as_ref().is_some_and(|k| (k.1, &k.2) != (key.1, &key.2)) {
            (ed.tl.loop_range, ed.center_frame) = (None, true);
        }
        ed.tl.sel = Cells::from([(ed.layer, ed.frame)]);
        ed.tl.anchor = (ed.layer, ed.frame);
        ed.tl.sel_at = Some(key);
    }
}

/// The selected frames as they stand, without updating the selection.
fn selection(ed: &Editor) -> Cow<'_, Cells> {
    if ed.tl.sel_at.as_ref() == Some(&sel_key(ed)) { Cow::Borrowed(&ed.tl.sel) } else { Cow::Owned(Cells::from([(ed.layer, ed.frame)])) }
}

/// Selects `cells`, making `layer` the active layer and moving the playhead
/// to `frame`, unless that's past the timeline's end.
fn select(ed: &mut Editor, cells: Cells, (layer, frame): (usize, u32)) {
    ed.layer = layer;
    if frame < ed.frame_count() && frame != ed.frame {
        ed.set_frame(frame);
    }
    ed.tl.sel = cells;
    keep_selection(ed);
}

/// The selected layers, top first: those with selected frames, and the active one.
pub(crate) fn selected_layers(ed: &Editor) -> Vec<usize> {
    let count = ed.timeline().layers.len();
    let layers: BTreeSet<usize> = selection(ed).iter().map(|c| c.0).chain([ed.layer]).filter(|&l| l < count).collect();
    layers.into_iter().collect()
}

/// All of layer `l`'s frames; a folder or empty layer has just its frame 0.
fn layer_cells(ed: &Editor, l: usize) -> impl Iterator<Item = (usize, u32)> + use<> {
    let n = ed.timeline().layers.get(l).map_or(1, |layer| layer.frame_count().max(1));
    (0..n).map(move |f| (l, f))
}

/// The cells from `a` to `b`, corners included.
fn block(a: (usize, u32), b: (usize, u32)) -> Cells {
    let (f0, f1) = (a.1.min(b.1), a.1.max(b.1));
    (a.0.min(b.0)..=a.0.max(b.0)).flat_map(|l| (f0..=f1).map(move |f| (l, f))).collect()
}

/// The rectangle around `cells`: first and last layer, first and last frame,
/// and whether the cells fill it.
fn bounds(cells: &Cells) -> Option<((usize, usize), (u32, u32), bool)> {
    let (l0, l1) = (cells.first()?.0, cells.last()?.0);
    let (f0, f1) = cells.iter().fold((u32::MAX, 0), |(lo, hi), &(_, f)| (lo.min(f), hi.max(f)));
    let filled = cells.len() as u64 == (l1 - l0 + 1) as u64 * u64::from(f1 - f0 + 1);
    Some(((l0, l1), (f0, f1), filled))
}

/// The keyframe holding frame `f` of layer `l`.
fn key_at(ed: &Editor, (l, f): (usize, u32)) -> Option<&Frame> {
    let layer = ed.timeline().layers.get(l)?;
    layer.keyframe_at(f).map(|k| &layer.frames[k])
}

/// The keyframe span holding a cell, as cells.
fn span(ed: &Editor, cell: (usize, u32)) -> Option<Cells> {
    let key = key_at(ed, cell)?;
    Some((key.index..key.index + key.duration).map(|f| (cell.0, f)).collect())
}

/// The cells as runs of frames, (layer, first frame, count), on layers with frames.
fn runs(ed: &Editor, cells: &Cells) -> Vec<(usize, u32, u32)> {
    let layers = &ed.timeline().layers;
    let mut runs: Vec<(usize, u32, u32)> = Vec::new();
    for &(l, f) in cells {
        if layers.get(l).is_none_or(|layer| layer.kind == LayerKind::Folder) {
            continue;
        }
        match runs.last_mut() {
            Some((rl, start, n)) if *rl == l && *start + *n == f => *n += 1,
            _ => runs.push((l, f, 1)),
        }
    }
    runs
}

/// The stage selection becomes layer `l`'s objects at the playhead, as
/// clicking its frame or name does in Animate.
fn select_objects(ed: &mut Editor, l: usize) {
    let Some(layer) = ed.timeline().layers.get(l) else { return };
    ed.selection = match layer.keyframe_at(ed.frame) {
        Some(k) if layer.visible() && !layer.locked() => {
            (0..layer.frames[k].elements.len()).map(|e| ElementRef { layer: l, keyframe: k, element: e }).collect()
        }
        _ => Vec::new(),
    };
}

/// Pressing a frame cell (docs/animate-ui-interaction.md §2.1). Like a press on
/// a layer's name, it ends typing on stage: the keys are the timeline's again.
fn press_cell(ed: &mut Editor, cell: (usize, u32), mods: Modifiers) {
    crate::text_tool::commit(ed);
    ed.props_tab = PropsTab::Frame;
    if mods.command {
        ed.tl.drag = Some(Drag::Ctrl(cell));
        return;
    }
    let tweened = key_at(ed, cell).is_some_and(|k| k.tween != Tween::None);
    if mods.shift {
        // Shift extends the selection to a rectangle, and adds a tween (or, with Span Based Selection, any span) whole.
        let mut cells = if ed.tl.span_select { ed.tl.sel.clone() } else { block(ed.tl.anchor, cell) };
        if ed.tl.span_select || tweened {
            cells.extend(span(ed, cell).unwrap_or_default());
        }
        select(ed, cells, cell);
        return;
    }
    if ed.tl.sel.contains(&cell) {
        // Keep the selection, so a drag moves it all.
        let cells = std::mem::take(&mut ed.tl.sel);
        let layers: BTreeSet<usize> = cells.iter().map(|c| c.0).collect();
        let whole = layers.into_iter().all(|l| layer_cells(ed, l).all(|c| cells.contains(&c)));
        select(ed, cells, cell);
        ed.tl.drag = Some(Drag::Grab { cell, copy: mods.alt, collapse: !whole && !ed.tl.span_select });
        return;
    }
    let cells = match span(ed, cell) {
        Some(span) if ed.tl.span_select => span,
        _ => Cells::from([cell]),
    };
    select(ed, cells, cell);
    ed.tl.anchor = cell;
    select_objects(ed, cell.0);
    // With Span Based Selection the span drags as one; otherwise a drag selects a rectangle.
    ed.tl.drag = Some(if ed.tl.span_select { Drag::Grab { cell, copy: mods.alt, collapse: false } } else { Drag::Select });
}

/// Pressing a layer's name: Shift selects a range of layers, Ctrl adds or
/// removes one; a plain press selects the layer and all its frames.
fn press_layer(ed: &mut Editor, row: usize, mods: Modifiers) {
    crate::text_tool::commit(ed);
    let selected = selected_layers(ed);
    let mut active = row;
    let cells: Cells = if mods.shift {
        let from = ed.tl.anchor.0;
        (from.min(row)..=from.max(row)).flat_map(|l| layer_cells(ed, l)).collect()
    } else if mods.command {
        let mut cells = ed.tl.sel.clone();
        if selected.contains(&row) && selected.len() > 1 {
            cells.retain(|c| c.0 != row);
            // The active layer is always selected, so another selected one takes over.
            active = selected.iter().copied().find(|&l| l != row).unwrap_or(row);
        } else {
            cells.extend(layer_cells(ed, row));
        }
        cells
    } else if selected.contains(&row) && selected.len() > 1 {
        // Pressing one of several selected layers keeps them, so a drag moves them all.
        ed.tl.sel.clone()
    } else {
        layer_cells(ed, row).collect()
    };
    select(ed, cells, (active, ed.frame));
    if !mods.shift {
        ed.tl.anchor = (row, 0);
    }
    if !mods.shift && !mods.command {
        select_objects(ed, row);
        ed.tl.drag = Some(Drag::Layers { row, dragging: false });
    }
}

/// Moves the selected layers, with what's in them, to the gap above layer `gap`
/// (`layers.len()` is the bottom), into `parent` (a folder, mask or guide) or out of any.
fn reorder(ed: &mut Editor, gap: usize, parent: Option<usize>) {
    let moving = selected_layers(ed);
    let mut tl = ed.timeline().clone();
    pin_colors(&mut tl);
    let pinned = tl.clone();
    let Some(went) = tl.move_into(&moving, gap, parent) else { return };
    if tl == pinned {
        return;
    }
    ed.checkpoint();
    *ed.timeline_mut() = tl;
    let row = |l: usize| went.get(l).copied();
    let cells: Cells = ed.tl.sel.iter().filter_map(|&(l, f)| Some((row(l)?, f))).collect();
    let active = row(ed.layer).unwrap_or(0);
    ed.tl.anchor.0 = row(ed.tl.anchor.0).unwrap_or(active);
    ed.follow_layers(row);
    select(ed, cells, (active, ed.frame));
}

// Commands.

/// Whether a timeline command can run on the selection now; other commands always can.
pub fn available(ed: &Editor, action: Action) -> bool {
    use Action::*;
    let cells = selection(ed);
    let layers = &ed.timeline().layers;
    let runs = runs(ed, &cells);
    let keys = |hit: &dyn Fn(&Frame, u32, u32) -> bool| runs.iter().any(|&(l, f, n)| layers[l].frames.iter().any(|k| hit(k, f, n)));
    match action {
        InsertFrame | RemoveFrames | InsertKeyframe | InsertBlankKeyframe | ConvertToKeyframes | ConvertToBlankKeyframes | ClearFrames
        | CutFrames | CopyFrames => !runs.is_empty(),
        CreateClassicTween => keys(&|k, f, n| k.index < f + n && k.index + k.duration > f && k.tween == Tween::None),
        RemoveTween => keys(&|k, f, n| k.index < f + n && k.index + k.duration > f && k.tween != Tween::None),
        CreateShapeTween => keys(&|k, f, n| k.index < f + n && k.index + k.duration > f && !matches!(k.tween, Tween::Shape { .. })),
        CreateMotionTween => {
            keys(&|k, f, n| k.index < f + n && k.index + k.duration > f && !k.elements.is_empty() && !matches!(k.tween, Tween::Motion(_)))
        }
        InsertPropertyKeyframe(_) | ClearPropertyKeyframe(_) | SplitMotion => crate::tweens::keyed(ed, &cells, action),
        ClearKeyframe => keys(&|k, f, n| k.index > 0 && (f..f + n).contains(&k.index)),
        PasteFrames | PasteOverwriteFrames => ed.tl.clip.as_ref().is_some_and(|(rows, _)| !nests(ed, rows.iter().flatten())),
        ReverseFrames => !runs.is_empty() && runs.iter().all(|&(l, f, n)| layers[l].can_reverse(f, n)),
        PrevKeyframe => keyframe_neighbors(ed).0.is_some(),
        NextKeyframe => keyframe_neighbors(ed).1.is_some(),
        DeleteLayer | CutLayers | LockOthers | HideOthers => selected_layers(ed).len() < layers.len(),
        CopyLayers | DuplicateLayers | LayerProperties => !selected_layers(ed).is_empty(),
        PasteLayers => !ed.tl.layer_clip.is_empty() && !nests(ed, ed.tl.layer_clip.iter().flat_map(|l| &l.frames)),
        ShowAllLayers => layers.iter().any(|l| !l.visible() || l.locked()),
        ToggleGuide => layers.get(ed.layer).is_some_and(|l| matches!(l.kind, LayerKind::Normal | LayerKind::Guide)),
        ToggleMask | AddMotionGuide | ShowMasking | ExpandFolders | CollapseFolders | ExpandAllFolders | CollapseAllFolders => {
            layer_tree::available(ed, action)
        }
        _ => true,
    }
}

/// Whether pasting `frames` here would put the symbol being edited inside itself.
fn nests<'a>(ed: &'a Editor, frames: impl IntoIterator<Item = &'a Frame>) -> bool {
    ed.editing.last().is_some_and(|t| symbols::draws(&ed.doc, frames.into_iter().flat_map(|k| &k.elements), &t.symbol))
}

/// Runs a timeline command on the selected frames or layers.
pub fn command(ed: &mut Editor, action: Action) {
    sync(ed);
    if !available(ed, action) {
        return;
    }
    let cells = ed.tl.sel.clone();
    match action {
        Action::SelectAllFrames => {
            let all = (0..ed.timeline().layers.len()).flat_map(|l| layer_cells(ed, l)).collect();
            select(ed, all, (ed.layer, ed.frame));
        }
        Action::CopyFrames => ed.tl.clip = copy(ed, &cells),
        Action::PasteFrames | Action::PasteOverwriteFrames => paste(ed, &cells, action == Action::PasteOverwriteFrames),
        Action::PrevKeyframe | Action::NextKeyframe => {
            let (prev, next) = keyframe_neighbors(ed);
            if let Some(f) = if action == Action::PrevKeyframe { prev } else { next } {
                ed.set_frame(f);
            }
        }
        Action::NewFolder => {
            ed.checkpoint();
            let n = ed.timeline().layers.iter().filter(|l| l.kind == LayerKind::Folder).count() + 1;
            // Above the selected layer, or above its mask or guide: a folder can't go between them.
            let at = ed.timeline().mask_of(ed.layer).or(ed.timeline().guide_of(ed.layer)).unwrap_or(ed.layer);
            let tl = ed.timeline_mut();
            pin_colors(tl);
            let color = next_color(tl);
            let at = tl.insert_folder(at, &format!("Folder_{n}"));
            tl.layers[at].set_color(color);
            tl.adopt(at);
            ed.layer = at;
            ed.follow_layers(|l| Some(if l < at { l } else { l + 1 }));
        }
        Action::DeleteLayer => delete_layers(ed),
        Action::InsertPropertyKeyframe(_) | Action::ClearPropertyKeyframe(_) => crate::tweens::property_keys(ed, action, &cells),
        Action::CopyLayers | Action::CutLayers => {
            ed.tl.layer_clip = layer_copies(ed, &selected_layers(ed));
            if action == Action::CutLayers {
                delete_layers(ed);
            }
        }
        Action::PasteLayers | Action::DuplicateLayers => {
            let (at, mut new) = match action {
                Action::PasteLayers => (ed.layer, ed.tl.layer_clip.clone()),
                _ => {
                    let selected = selected_layers(ed);
                    (selected[0], layer_copies(ed, &selected))
                }
            };
            if action == Action::DuplicateLayers {
                new.iter_mut().for_each(|l| l.name.push_str(" copy"));
            }
            ed.checkpoint();
            let count = new.len();
            let tl = ed.timeline_mut();
            pin_colors(tl);
            // Above the selected layer, in with it; pasted onto a folder, into it at its top.
            let (gap, parent) = match tl.layers.get(at) {
                Some(l) if action == Action::PasteLayers && l.kind == LayerKind::Folder => (at + 1, Some(at)),
                _ => (at, tl.parent(at)),
            };
            if parent == Some(at) {
                tl.layers[at].set_open(true);
            }
            let at = tl.paste_layers(gap, parent, new);
            ed.follow_layers(|l| Some(if l < at { l } else { l + count }));
            let cells = (at..at + count).flat_map(|l| layer_cells(ed, l)).collect();
            select(ed, cells, (at, ed.frame));
        }
        Action::ShowAllLayers | Action::LockOthers | Action::HideOthers => {
            let (selected, objects) = (selected_layers(ed), ed.selection.clone());
            // On a copy: with nothing to change, no undo step.
            ed.edit_timeline(|_, tl| {
                for (i, l) in tl.layers.iter_mut().enumerate() {
                    match action {
                        Action::ShowAllLayers => {
                            l.set_visible(true);
                            l.set_locked(false);
                        }
                        Action::LockOthers if !selected.contains(&i) => l.set_locked(true),
                        Action::HideOthers if !selected.contains(&i) => l.set_visible(false),
                        _ => {}
                    }
                }
                objects
            });
            keep_selection(ed);
        }
        Action::ToggleGuide => {
            ed.checkpoint();
            let l = ed.layer;
            let tl = ed.timeline_mut();
            let on = tl.layers[l].kind != LayerKind::Guide;
            tl.set_guide(l, on);
            keep_selection(ed);
        }
        Action::ToggleMask | Action::AddMotionGuide | Action::ShowMasking => layer_tree::command(ed, action),
        Action::ExpandFolders | Action::CollapseFolders | Action::ExpandAllFolders | Action::CollapseAllFolders => {
            layer_tree::command(ed, action)
        }
        Action::LayerProperties => {
            let l = ed.layer;
            let Some(layer) = ed.timeline().layers.get(l) else { return };
            ed.tl.props = Some(LayerProps {
                layer: l,
                name: layer.name.clone(),
                visible: layer.visible(),
                locked: layer.locked(),
                kind: layer.kind,
                color: layer_color(layer, l),
                outline: layer.outline(),
            });
        }
        _ => edit_frames(ed, action, &cells),
    }
}

/// F5, F6, F7 and friends on the selected frames: on each layer's runs of
/// them, or for Insert Frame and Insert (Blank) Keyframe on the first.
fn edit_frames(ed: &mut Editor, action: Action, cells: &Cells) {
    use Action::*;
    let runs = runs(ed, cells);
    if action == CutFrames {
        ed.tl.clip = copy(ed, cells);
    }
    // Per layer: its first selected frame and how many it has selected.
    let mut firsts: Vec<(usize, u32, u32)> = Vec::new();
    for &(l, f, n) in &runs {
        match firsts.last_mut() {
            Some((fl, _, total)) if *fl == l => *total += n,
            _ => firsts.push((l, f, n)),
        }
    }
    let single = cells.len() == 1;
    let mut acted = None;
    let objects = ed.selection.clone();
    let fps = ed.doc.frame_rate;
    // On a copy: frames past the end, say, leave nothing to change and no undo step.
    ed.edit_timeline(|_, tl| {
        // F6 copies keyframes as stored; along a guide or after a parent they then show what showed.
        let before = matches!(action, InsertKeyframe | ConvertToKeyframes).then(|| tl.clone());
        let layers = &mut tl.layers;
        match action {
            InsertFrame => firsts.iter().for_each(|&(l, f, n)| layers[l].insert_frames(f, n)),
            RemoveFrames => runs.iter().rev().for_each(|&(l, f, n)| layers[l].remove_frames(f, n)),
            // On one frame, Convert to (Blank) Keyframes is F6 (F7) as Animate has always had it.
            InsertKeyframe | InsertBlankKeyframe | ConvertToKeyframes | ConvertToBlankKeyframes
                if single || matches!(action, InsertKeyframe | InsertBlankKeyframe) =>
            {
                for &(l, f, _) in &firsts {
                    let at = match action {
                        InsertKeyframe | ConvertToKeyframes => layers[l].insert_keyframe(f),
                        _ => layers[l].insert_blank_keyframe(f),
                    };
                    acted = Some((l, at));
                }
            }
            ConvertToKeyframes | ConvertToBlankKeyframes => {
                runs.iter().for_each(|&(l, f, n)| layers[l].convert_to_keyframes(f, n, action == ConvertToBlankKeyframes))
            }
            ClearKeyframe => runs.iter().for_each(|&(l, f, n)| layers[l].clear_keyframes(f, n)),
            SplitMotion => runs.iter().for_each(|&(l, f, _)| _ = layers[l].split_motion(f)),
            CreateClassicTween | CreateShapeTween | CreateMotionTween | RemoveTween => {
                for &(l, f, n) in &runs {
                    let starts: Vec<u32> = layers[l].frames.iter().filter(|k| k.index < f + n && k.index + k.duration > f).map(|k| k.index).collect();
                    for s in starts {
                        match action {
                            CreateClassicTween => layers[l].create_classic_tween(s),
                            CreateShapeTween => layers[l].create_shape_tween(s),
                            // Spans whose contents aren't one instance or text stay: see tweens::create_motion_tween.
                            CreateMotionTween => _ = layers[l].create_motion_tween(s, fps),
                            _ => layers[l].remove_tween(s),
                        }
                    }
                }
            }
            ClearFrames | CutFrames => runs.iter().for_each(|&(l, f, n)| layers[l].clear_frames(f, n)),
            ReverseFrames => runs.iter().for_each(|&(l, f, n)| layers[l].reverse_frames(f, n)),
            _ => {}
        }
        if let Some(before) = before {
            minnamate_core::layer_motion::keep_shown(&before, tl);
        }
        objects
    });
    // An inserted keyframe takes the selection and playhead along when it lands on the next frame.
    if let Some(cell) = acted.filter(|_| single) {
        select(ed, Cells::from([cell]), cell);
    }
    ed.prune_selection();
    keep_selection(ed);
}

/// Copy Frames: the rectangle around the selection, a keyframe list per layer.
fn copy(ed: &Editor, cells: &Cells) -> Option<(Vec<Vec<Frame>>, u32)> {
    let ((l0, l1), (f0, f1), _) = bounds(cells)?;
    let n = f1 - f0 + 1;
    let layers = &ed.timeline().layers;
    let rows = (l0..=l1).map(|l| layers.get(l).filter(|l| l.kind != LayerKind::Folder).map_or(Vec::new(), |l| l.copy_frames(f0, n)));
    Some((rows.collect(), n))
}

/// Paste Frames, or Paste and Overwrite Frames, at the selection's top left;
/// the pasted frames end up selected.
fn paste(ed: &mut Editor, cells: &Cells, overwrite: bool) {
    let (Some((rows, n)), Some(((top, _), (f, _), _))) = (ed.tl.clip.clone(), bounds(cells)) else { return };
    let (objects, mut pasted) = (ed.selection.clone(), Cells::new());
    // On a copy: frames copied from past the end paste nothing, and leave no undo step.
    ed.edit_timeline(|_, tl| {
        for (i, clip) in rows.iter().enumerate() {
            let Some(layer) = tl.layers.get_mut(top + i).filter(|l| l.kind != LayerKind::Folder && !clip.is_empty()) else { continue };
            layer.paste_frames(f, n, clip, !overwrite);
            pasted.extend((f..f + n).map(|g| (top + i, g)));
        }
        objects
    });
    if !pasted.is_empty() {
        select(ed, pasted, (top, f));
    }
    ed.prune_selection();
}

/// Delete Layers: the selected ones, folders with what's in them, as long as one layer is left.
fn delete_layers(ed: &mut Editor) {
    let doomed = ed.timeline().with_contents(&selected_layers(ed));
    if doomed.is_empty() || doomed.len() >= ed.timeline().layers.len() {
        return;
    }
    ed.checkpoint();
    pin_colors(ed.timeline_mut());
    ed.timeline_mut().remove_layers(&doomed);
    ed.layer = doomed[0].min(ed.timeline().layers.len() - 1);
    ed.follow_layers(|l| (!doomed.contains(&l)).then(|| l - doomed.iter().filter(|&&d| d < l).count()));
}

/// The loop range, first and last frame, within the timeline. A range left
/// past its end (frames removed since) is the whole timeline again.
pub fn loop_range(ed: &Editor) -> (u32, u32) {
    let last = ed.frame_count() - 1;
    let (a, b) = ed.tl.loop_range.filter(|&(a, _)| a <= last).unwrap_or((0, last));
    (a, b.min(last).max(a))
}

/// The frames the onion skin can show: first and last.
pub fn onion_range(ed: &Editor) -> (u32, u32) {
    let o = &ed.tl.onion;
    // ponytail: "All frames" reaches ONION_ALL frames each way, since every onion frame is
    // a layer render of its own; cache those renders across playhead moves to lift it.
    let (before, after) = if o.all { (ONION_ALL, ONION_ALL) } else { (o.before, o.after) };
    let center = o.anchor.unwrap_or(ed.frame);
    (center.saturating_sub(before), (center + after).min(ed.frame_count() - 1))
}

/// What the onion skin shows of layer `l`, farthest first: frame, tint and
/// opacity. Locked layers get none, as in Animate.
pub fn onion_frames(ed: &Editor, l: usize) -> Vec<(u32, Color32, f32)> {
    let o = &ed.tl.onion;
    let Some(layer) = ed.timeline().layers.get(l).filter(|l| !l.locked()) else { return Vec::new() };
    let (lo, hi) = onion_range(ed);
    let mut frames: Vec<(u32, Color32, f32)> = (lo..=hi)
        .filter(|&f| f != ed.frame && (!o.keyframes_only || layer.frames.iter().any(|k| k.index == f)))
        .map(|f| {
            let tint = if f < ed.frame { o.before_color } else { o.after_color };
            (f, tint, (o.opacity - o.falloff * (f.abs_diff(ed.frame) - 1) as f32).max(0.1))
        })
        .collect();
    frames.sort_by_key(|&(f, ..)| std::cmp::Reverse(f.abs_diff(ed.frame)));
    frames
}

/// The frame grid's geometry.
#[derive(Clone)]
pub(crate) struct Geo {
    frames: Rect,
    rows_top: f32,
    pub(crate) row_h: f32,
    cell_w: f32,
    first_frame: f32,
    first_layer: f32,
    /// The layers listed, top first: those in a closed folder aren't.
    pub(crate) rows: Vec<usize>,
}

impl Geo {
    fn new(ed: &Editor, frames: Rect, rows_top: f32) -> Self {
        let tl = ed.timeline();
        let rows = (0..tl.layers.len()).filter(|&l| tl.listed(l)).collect();
        Geo { frames, rows_top, row_h: ed.tl.row_h, cell_w: ed.cell_w, first_frame: ed.first_visible_frame, first_layer: ed.first_visible_layer, rows }
    }

    /// The layer listed at `y`.
    pub(crate) fn layer_at(&self, y: f32) -> Option<usize> {
        let r = self.row_at(y).floor();
        if r < 0.0 { None } else { self.rows.get(r as usize).copied() }
    }

    /// The top of visual row `r` (`rows.len()` is just under the last).
    pub(crate) fn top_of_row(&self, r: usize) -> f32 {
        self.rows_top + (r as f32 - self.first_layer) * self.row_h
    }

    fn x_of(&self, f: u32) -> f32 {
        self.frames.left() + (f as f32 - self.first_frame) * self.cell_w
    }

    fn frame_at(&self, x: f32) -> u32 {
        ((x - self.frames.left()) / self.cell_w + self.first_frame).max(0.0) as u32
    }

    /// The row under `y`, fractional.
    pub(crate) fn row_at(&self, y: f32) -> f32 {
        (y - self.rows_top) / self.row_h + self.first_layer
    }

    /// The top of layer `l`'s row, or of its closed folder's.
    pub(crate) fn top_of(&self, l: usize) -> f32 {
        self.top_of_row(self.rows.iter().rposition(|&r| r <= l).unwrap_or(0))
    }
}

/// Where the primary button went down on `resp` this frame, if it did.
fn pressed(ui: &Ui, resp: &Response) -> Option<Pos2> {
    let pressed = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
    if pressed { ui.input(|i| i.pointer.press_origin()) } else { None }
}

fn grid(ui: &mut Ui, ed: &mut Editor) {
    sync(ed);
    let rect = ui.available_rect_before_wrap();
    ui.allocate_rect(rect, Sense::hover());
    let frames = Rect::from_min_max(pos2(rect.left() + LAYER_W, rect.top()), rect.max);
    let ruler = Rect::from_min_max(frames.min, pos2(frames.right(), frames.top() + RULER_H));
    let rows_top = ruler.bottom();
    let column_right = frames.left();
    let header = Rect::from_min_max(rect.min, pos2(column_right, rows_top));
    let names = Rect::from_min_max(pos2(rect.left(), rows_top), pos2(column_right, rect.bottom()));
    let cells = Rect::from_min_max(pos2(column_right, rows_top), rect.max);
    ed.layer_rows = names;
    let count = layer_tree::listed(ed.timeline());
    let shown = (names.height() / ed.tl.row_h).floor().max(1.0);
    let max_layer = (count as f32 - shown).max(0.0);
    ed.first_visible_layer = ed.first_visible_layer.clamp(0.0, max_layer);
    if std::mem::take(&mut ed.center_frame) {
        ed.first_visible_frame = (ed.frame as f32 - frames.width() / ed.cell_w / 2.0).max(0.0);
    }

    let id = ui.id().with("timeline");
    let header_resp = ui.interact(header, id.with("header"), Sense::click());
    let ruler_resp = ui.interact(ruler, id.with("ruler"), Sense::click_and_drag());
    let names_resp = ui.interact(names, id.with("layers"), Sense::click_and_drag());
    let cells_resp = ui.interact(cells, id.with("frames"), Sense::click_and_drag());
    let thumbs = scrollbars(ui, ed, id, frames, names, shown, max_layer);
    let geo = Geo::new(ed, frames, rows_top);
    let mods = ui.input(|i| i.modifiers);
    let idle = ed.tl.drag.is_none();
    let row_at = |p: Pos2| geo.layer_at(p.y);
    let mut open_parents = false;

    // Presses: the header toggles every layer, the ruler scrubs or sets the
    // loop range, the list selects and toggles layers, the grid selects frames.
    if header_resp.clicked()
        && let Some(p) = header_resp.interact_pointer_pos()
        && let Some(which) = toggle_at(p.x, column_right).filter(|&t| t != Toggle::Highlight)
    {
        toggle_all(ed, which);
    }
    if let Some(p) = pressed(ui, &ruler_resp) {
        let f = geo.frame_at(p.x);
        ed.tl.drag = Some(if ed.loop_playback && p.y < ruler.top() + TOP_TIER {
            let (a, b) = loop_range(ed);
            if (p.x - geo.x_of(a)).abs() <= 5.0 {
                Drag::LoopStart
            } else if (p.x - geo.x_of(b + 1)).abs() <= 5.0 {
                Drag::LoopEnd
            } else if mods.shift {
                Drag::LoopMove { grab: f, range: (a, b) }
            } else {
                Drag::LoopNew(f)
            }
        } else {
            Drag::Scrub
        });
    }
    if let Some(p) = pressed(ui, &names_resp)
        && let Some(row) = row_at(p)
    {
        match toggle_at(p.x, column_right) {
            Some(Toggle::Highlight) => {}
            Some(which) if mods.alt => solo(ed, which, row),
            Some(which) => {
                let on = !marked(&ed.timeline().layers[row], which);
                set_toggle(ed, which, row, on);
                ed.tl.drag = Some(Drag::Toggle(which, on));
            }
            None => match layer_tree::part_at(ed, &geo, row, p, rect.left(), column_right) {
                layer_tree::Part::Triangle => layer_tree::toggle_open(ed, row),
                layer_tree::Part::Head => ed.tl.drag = Some(Drag::Parent { child: row }),
                // Its list opens on the click: see below.
                layer_tree::Part::Strip => {}
                layer_tree::Part::Icon | layer_tree::Part::Name => press_layer(ed, row, mods),
            },
        }
    }
    if let Some(p) = pressed(ui, &cells_resp)
        && let Some(row) = row_at(p)
    {
        press_cell(ed, (row, geo.frame_at(p.x)), mods);
    }
    // Moving past egui's click distance decides what a press was.
    if names_resp.drag_started()
        && let Some(Drag::Layers { row, .. }) = ed.tl.drag
    {
        ed.tl.drag = Some(Drag::Layers { row, dragging: true });
    }
    if cells_resp.drag_started() {
        match ed.tl.drag {
            Some(Drag::Grab { cell, copy, .. }) => {
                let rectangle = bounds(&ed.tl.sel).is_some_and(|b| b.2);
                ed.tl.drag = rectangle.then_some(Drag::Move { grab: cell, copy, by: (0, 0) });
            }
            Some(Drag::Ctrl(cell)) => {
                let last = key_at(ed, cell).map(|k| k.index + k.duration - 1);
                ed.tl.drag = Some(if last == Some(cell.1) {
                    Drag::Stretch { layer: cell.0, last: cell.1, to: cell.1 }
                } else {
                    select(ed, Cells::from([cell]), cell);
                    ed.tl.anchor = cell;
                    Drag::Select
                });
            }
            _ => {}
        }
    }
    // A gesture starting now holds on to the document as it is.
    if idle {
        ed.tl.drag_on = (ed.rev, ed.scene);
    }
    track(ui, ed, &geo);
    // A click in the loop strip moves the playhead too, as anywhere on the ruler.
    if ruler_resp.clicked()
        && let Some(p) = ruler_resp.interact_pointer_pos()
    {
        let f = geo.frame_at(p.x).min(ed.frame_count() - 1);
        if f != ed.frame {
            ed.set_frame(f);
        }
    }
    if cells_resp.double_clicked()
        && let Some(p) = cells_resp.interact_pointer_pos()
        && let Some(row) = row_at(p)
    {
        let cell = (row, geo.frame_at(p.x));
        if let Some(span) = span(ed, cell) {
            select(ed, span, cell);
        }
    }
    if cells_resp.secondary_clicked()
        && let Some(p) = cells_resp.interact_pointer_pos()
        && let Some(row) = row_at(p)
    {
        // Right-clicking outside the selection selects that frame first.
        let cell = (row, geo.frame_at(p.x));
        if !ed.tl.sel.contains(&cell) {
            press_cell(ed, cell, Modifiers::NONE);
            ed.tl.drag = None;
        }
        ed.tl.menu_cell = cell;
    }
    // A click in the parenting view, off the layer's head, lists the layers it can follow.
    if names_resp.clicked()
        && let Some(p) = names_resp.interact_pointer_pos()
        && let Some(row) = row_at(p)
        && toggle_at(p.x, column_right).is_none()
        && matches!(layer_tree::part_at(ed, &geo, row, p, rect.left(), column_right), layer_tree::Part::Strip)
    {
        (ed.tl.parent_menu, open_parents) = (Some((row, p)), true);
    }
    if names_resp.double_clicked()
        && let Some(p) = names_resp.interact_pointer_pos()
        && let Some(row) = row_at(p)
        && toggle_at(p.x, column_right).is_none()
    {
        match layer_tree::part_at(ed, &geo, row, p, rect.left(), column_right) {
            layer_tree::Part::Icon => command(ed, Action::LayerProperties),
            layer_tree::Part::Name => ed.renaming = Some((row, ed.timeline().layers[row].name.clone())),
            _ => {}
        }
    }
    if names_resp.secondary_clicked()
        && let Some(p) = names_resp.interact_pointer_pos()
        && let Some(row) = row_at(p)
        && !selected_layers(ed).contains(&row)
    {
        press_layer(ed, row, Modifiers::NONE);
        ed.tl.drag = None;
    }

    let geo = Geo::new(ed, frames, rows_top);
    paint(ui, ed, rect, &geo, thumbs);
    rename_field(ui, ed, &geo, rect.left());
    let mut picked = None;
    theme::context_menu(&cells_resp, |ui| picked = frame_menu(ui, ed));
    theme::context_menu(&names_resp, |ui| picked = picked.or(layer_menu(ui, ed)));
    if let Some(action) = picked {
        ed.run(action, ui.ctx());
    }
    layer_tree::parent_menu(ui, ed, open_parents);

    // The wheel scrolls layers when they overflow, frames otherwise; sideways always scrolls frames.
    if ui.rect_contains_pointer(rect) {
        let delta = ui.input(|i| i.smooth_scroll_delta);
        let (down, across) = if max_layer > 0.0 { (delta.y, delta.x) } else { (0.0, delta.x + delta.y) };
        ed.first_visible_layer = (ed.first_visible_layer - down / ed.tl.row_h).clamp(0.0, max_layer);
        ed.first_visible_frame = (ed.first_visible_frame - across / ed.cell_w).max(0.0);
    }
    // Keep the playhead in view while playing.
    let visible = (frames.width() / ed.cell_w).floor().max(1.0);
    if ed.playing.is_some() && (ed.frame as f32) >= ed.first_visible_frame + visible - 1.0 {
        ed.first_visible_frame = ed.frame as f32;
    }
    if ed.playing.is_some() && (ed.frame as f32) < ed.first_visible_frame {
        ed.first_visible_frame = ed.frame as f32;
    }
}

/// Follows the pointer while a gesture's button is down, and finishes the gesture when it comes up.
fn track(ui: &Ui, ed: &mut Editor, geo: &Geo) {
    let Some(drag) = ed.tl.drag else { return };
    // A key that changed the document or the scene mid-gesture (Ctrl+Z, Shift+F5, End) ends it:
    // the layers, frames or loop range it holds on to may be gone. Toggles change it themselves.
    if ed.tl.drag_on != (ed.rev, ed.scene) && !matches!(drag, Drag::Toggle(..)) {
        ed.tl.drag = None;
        return;
    }
    let (pos, down) = ui.input(|i| (i.pointer.interact_pos(), i.pointer.primary_down()));
    let count = ed.timeline().layers.len();
    if let Some(pos) = pos {
        let f = geo.frame_at(pos.x);
        // The layer listed under the pointer, or the nearest end of the list.
        let listed = (geo.row_at(pos.y).floor().max(0.0) as usize).min(geo.rows.len().saturating_sub(1));
        let row = geo.rows.get(listed).copied().unwrap_or(0);
        // Dragging frames past the grid's left or right edge scrolls it.
        let edge = (pos.x - geo.frames.right() + 4.0).max(0.0) - (geo.frames.left() + 4.0 - pos.x).max(0.0);
        if down && edge != 0.0 && matches!(drag, Drag::Select | Drag::Move { .. } | Drag::Stretch { .. }) {
            ed.first_visible_frame = (ed.first_visible_frame + edge.signum() * 0.5).max(0.0);
            ui.ctx().request_repaint();
        }
        match drag {
            Drag::Scrub => {
                let f = f.min(ed.frame_count() - 1);
                if f != ed.frame {
                    ed.set_frame(f);
                }
            }
            Drag::Select => ed.tl.sel = block(ed.tl.anchor, (row, f)),
            Drag::Move { grab, copy, .. } => {
                // The block stays on the layers and at or after frame 1.
                if let Some(((l0, l1), (f0, _), _)) = bounds(&ed.tl.sel) {
                    let dl = (row as isize - grab.0 as isize).clamp(-(l0 as isize), (count - 1 - l1) as isize);
                    let df = (f as i64 - grab.1 as i64).max(-(f0 as i64));
                    ed.tl.drag = Some(Drag::Move { grab, copy, by: (dl, df) });
                }
                ui.ctx().set_cursor_icon(if copy { CursorIcon::Copy } else { CursorIcon::Grabbing });
            }
            Drag::Stretch { layer, last, .. } => {
                let start = key_at(ed, (layer, last)).map_or(last, |k| k.index);
                ed.tl.drag = Some(Drag::Stretch { layer, last, to: f.max(start) });
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
            Drag::Toggle(which, on) if geo.row_at(pos.y) >= 0.0 => set_toggle(ed, which, row, on),
            // A new loop range starts once the pointer leaves the pressed frame.
            Drag::LoopNew(from) if f == from => {}
            Drag::LoopStart | Drag::LoopEnd | Drag::LoopNew(_) | Drag::LoopMove { .. } => {
                let last = ed.frame_count() - 1;
                let (a, b) = loop_range(ed);
                let f = f.min(last);
                ed.tl.loop_range = Some(match drag {
                    Drag::LoopStart => (f.min(b), b),
                    Drag::LoopEnd => (a, f.max(a)),
                    Drag::LoopNew(from) => (from.min(f).min(last), from.max(f).min(last)),
                    Drag::LoopMove { grab, range: (a, b) } => {
                        let shift = (f as i64 - grab as i64).clamp(-(a as i64), (last - b) as i64);
                        ((a as i64 + shift) as u32, (b as i64 + shift) as u32)
                    }
                    _ => (a, b),
                });
            }
            _ => {}
        }
    }
    if down {
        return;
    }
    // Released.
    let drag = ed.tl.drag.take();
    match drag {
        Some(Drag::Grab { cell, collapse: true, .. }) => {
            select(ed, Cells::from([cell]), cell);
            ed.tl.anchor = cell;
            select_objects(ed, cell.0);
        }
        Some(Drag::Ctrl(cell)) => {
            // Ctrl+click adds or removes a frame; with Span Based Selection it selects just that one.
            let mut cells = if ed.tl.span_select { Cells::new() } else { ed.tl.sel.clone() };
            if !cells.remove(&cell) || cells.is_empty() {
                cells.insert(cell);
            }
            select(ed, cells, cell);
            ed.tl.anchor = cell;
        }
        Some(Drag::Move { grab, copy, by }) if by != (0, 0) => {
            let Some(((l0, l1), (f0, f1), _)) = bounds(&ed.tl.sel) else { return };
            let to = ((l0 as isize + by.0) as usize, (f0 as i64 + by.1) as u32);
            let n = f1 - f0 + 1;
            ed.checkpoint();
            ed.timeline_mut().move_frames(l0..l1 + 1, f0, n, to, copy);
            let active = ((grab.0 as isize + by.0) as usize, (grab.1 as i64 + by.1) as u32);
            select(ed, block(to, (to.0 + l1 - l0, to.1 + n - 1)), active);
            ed.prune_selection();
        }
        Some(Drag::Stretch { layer, last, to }) if to != last => {
            ed.checkpoint();
            let l = &mut ed.timeline_mut().layers[layer];
            if to > last {
                l.insert_frames(last, to - last);
            } else {
                l.remove_frames(to + 1, last - to);
            }
            select(ed, Cells::from([(layer, to)]), (layer, to));
            ed.prune_selection();
        }
        Some(Drag::Layers { row, dragging: false }) => {
            if selected_layers(ed).len() > 1 {
                let cells = layer_cells(ed, row).collect();
                select(ed, cells, (row, ed.frame));
            }
        }
        // Dropped on the toolbar's Delete button, the layers go.
        Some(Drag::Layers { dragging: true, .. }) => match pos {
            Some(p) if ed.tl.trash.contains(p) => delete_layers(ed),
            Some(p) => {
                let landing = layer_tree::drop_at(ed, geo, p.y);
                reorder(ed, landing.gap, landing.parent);
            }
            None => {}
        },
        // A head dropped on another layer: that's the layer it follows now.
        Some(Drag::Parent { child }) => {
            if let Some(target) = pos.and_then(|p| geo.layer_at(p.y)).filter(|&t| t != child) {
                layer_tree::set_rig_parent(ed, child, Some(target));
            }
        }
        _ => {}
    }
}

/// The frame grid's and layer list's scrollbars: 7 px pills with no track,
/// dragged to scroll. Returns their thumbs, to paint over the grid.
fn scrollbars(ui: &Ui, ed: &mut Editor, id: egui::Id, frames: Rect, rows: Rect, shown: f32, max_layer: f32) -> [Option<Rect>; 2] {
    let visible = frames.width() / ed.cell_w;
    let total = (ed.frame_count() as f32 + visible).max(ed.first_visible_frame + visible);
    let track = Rect::from_min_max(pos2(frames.left() + 2.0, frames.bottom() - 9.0), pos2(frames.right() - 11.0, frames.bottom() - 2.0));
    let len = (visible / total * track.width()).clamp(20.0, track.width());
    let x = track.left() + ed.first_visible_frame / total * track.width();
    let across = Rect::from_min_size(pos2(x.min(track.right() - len), track.top()), vec2(len, track.height()));
    let resp = ui.interact(across, id.with("frames-scroll"), Sense::drag());
    if resp.dragged() {
        ed.first_visible_frame = (ed.first_visible_frame + resp.drag_delta().x / track.width() * total).max(0.0);
    }
    let down = (max_layer > 0.0).then(|| {
        let count = layer_tree::listed(ed.timeline()) as f32;
        let track = Rect::from_min_max(pos2(frames.right() - 9.0, rows.top() + 2.0), pos2(frames.right() - 2.0, rows.bottom() - 11.0));
        let len = (shown / count * track.height()).clamp(20.0, track.height());
        let y = track.top() + ed.first_visible_layer / count * track.height();
        let thumb = Rect::from_min_size(pos2(track.left(), y.min(track.bottom() - len)), vec2(track.width(), len));
        let resp = ui.interact(thumb, id.with("layers-scroll"), Sense::drag());
        if resp.dragged() {
            ed.first_visible_layer = (ed.first_visible_layer + resp.drag_delta().y / track.height() * count).clamp(0.0, max_layer);
        }
        thumb
    });
    [Some(across), down]
}

fn paint(ui: &Ui, ed: &Editor, rect: Rect, geo: &Geo, thumbs: [Option<Rect>; 2]) {
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, theme::FRAME_CELL);
    let (frames, rows_top, row_h, cell_w) = (geo.frames, geo.rows_top, geo.row_h, geo.cell_w);
    let ruler = Rect::from_min_max(frames.min, pos2(frames.right(), rows_top));
    let column_right = frames.left();
    let first = geo.first_frame.floor() as u32;
    let visible = (frames.width() / cell_w).ceil() as u32 + 1;
    let x_of = |f: u32| geo.x_of(f);
    let hover = ui.input(|i| i.pointer.hover_pos()).filter(|&h| rect.contains(h));
    let hairline = Stroke::new(1.0, theme::BORDER);

    // Header row beside the ruler: Layer View, then the toggle columns' headers.
    let header = Rect::from_min_max(rect.min, pos2(column_right, rows_top));
    p.rect_filled(header, 0.0, theme::PANEL_BG);
    icons::paint(&p, Rect::from_center_size(pos2(header.left() + 17.0, header.center().y), vec2(15.0, 15.0)), Icon::TlLayerView, ICON_OFF);
    p.circle_filled(pos2(column_right - DOT_X, header.center().y), 2.5, ICON_INK);
    for (dx, icon) in [(OUTLINE_X, Icon::TlOutlineBar), (EYE_X, Icon::TlEyeSlash), (LOCK_X, Icon::TlLock)] {
        icons::paint(&p, Rect::from_center_size(pos2(column_right - dx, header.center().y), vec2(14.0, 14.0)), icon, ICON_INK);
    }

    // Ruler: seconds and the loop range on top; ticks and every 5th frame's number below.
    p.rect_filled(ruler, 0.0, theme::PANEL_BG);
    let tier = ruler.top() + TOP_TIER;
    let fps = ed.doc.frame_rate.round().max(1.0) as u32;
    if ed.loop_playback {
        let (a, b) = loop_range(ed);
        let (a, b) = (x_of(a), x_of(b + 1));
        let bar = Rect::from_min_max(pos2(a, ruler.top() + 2.0), pos2(b, tier - 1.0));
        p.rect_filled(bar, 0.0, LOOP_BAR);
        for (x, dir) in [(a, 1.0), (b, -1.0)] {
            let tri = vec![pos2(x, bar.top()), pos2(x, bar.bottom()), pos2(x + dir * 5.0, bar.center().y)];
            p.add(egui::Shape::convex_polygon(tri, LOOP_HANDLE, Stroke::NONE));
        }
    }
    let every = if cell_w < 6.0 { 10 } else { 5 };
    let range = first..first + visible;
    for f in range.clone() {
        let x = x_of(f);
        if (f + 1) % 5 == 0 {
            p.vline(x, ruler.top()..=tier, hairline);
        }
        p.vline(x, tier..=tier + 9.0, hairline);
    }
    // Labels go over the lines: italic seconds up top, numbers under the ticks.
    for f in range {
        let (x, number) = (x_of(f), f + 1);
        if number % fps == 0 {
            let format = egui::TextFormat { font_id: FontId::proportional(10.0), color: theme::RULER_TEXT, italics: true, ..Default::default() };
            let galley = p.layout_job(egui::text::LayoutJob::single_section(format!("{}s", number / fps), format));
            p.galley(pos2(x + 2.0, ruler.top() + 1.0), galley, theme::RULER_TEXT);
        }
        if number % every == 0 {
            p.text(pos2(x + 2.0, tier + 5.0), Align2::LEFT_TOP, number.to_string(), FontId::proportional(11.0), theme::RULER_TEXT);
        }
    }
    for y in [ruler.top() + 0.5, tier] {
        p.hline(ruler.x_range(), y, hairline);
    }
    // The bottom line runs under the layer header too.
    p.hline(rect.x_range(), ruler.bottom() - 0.5, hairline);
    let (lo, hi) = onion_range(ed);
    if ed.onion && lo <= hi {
        // The onion-skin range, lightened between its two markers.
        let (a, b) = (x_of(lo), x_of(hi + 1));
        p.rect_filled(Rect::from_min_max(pos2(a, tier), pos2(b, ruler.bottom())), 0.0, Color32::from_white_alpha(25));
        p.vline(a, tier..=ruler.bottom(), Stroke::new(2.0, ed.tl.onion.before_color));
        p.vline(b, tier..=ruler.bottom(), Stroke::new(2.0, ed.tl.onion.after_color));
    }

    let layers = &ed.timeline().layers;
    let sel = &ed.tl.sel;
    let selected_rows = selected_layers(ed);
    let rows_area = Rect::from_min_max(pos2(rect.left(), rows_top), rect.max);
    let p = p.with_clip_rect(rows_area);
    let hovered_row = hover.filter(|h| h.y >= rows_top).and_then(|h| geo.layer_at(h.y));
    for (r, &i) in geo.rows.iter().enumerate() {
        let layer = &layers[i];
        let top = geo.top_of_row(r);
        if top + row_h < rows_top {
            continue;
        }
        if top > rect.bottom() {
            break;
        }
        let (selected, hot) = (selected_rows.contains(&i), hovered_row == Some(i));
        let color = layer_color(layer, i);
        let cell = Rect::from_min_max(pos2(rect.left(), top), pos2(column_right, top + row_h));
        let row_fill = if selected {
            theme::LAYER_SELECTED
        } else if hot {
            theme::LAYER_HOVER
        } else {
            theme::PANEL_BG
        };
        p.rect_filled(cell, 0.0, row_fill);
        // Inside folders, masks and guides, indented; a folder with its disclosure triangle.
        let cy = cell.center().y;
        let indent = ed.timeline().depth(i) as f32 * INDENT;
        layer_tree::paint_kind(&p, ed.timeline(), i, pos2(cell.left() + indent, cy));
        let name_color = if selected { Color32::WHITE } else { theme::LAYER_NAME };
        let name_right = column_right - DOT_X - 10.0 - if ed.tl.parenting { layer_tree::PARENTING_W } else { 0.0 };
        let name_room = Rect::from_min_max(pos2(cell.left() + 46.0 + indent, top), pos2(name_right, top + row_h));
        p.with_clip_rect(name_room).text(pos2(cell.left() + 50.0 + indent, cy), Align2::LEFT_CENTER, &layer.name, FontId::proportional(12.0), name_color);
        // Toggle columns: highlight dot, outline swatch, eye, lock.
        if selected || hot {
            p.circle_filled(pos2(column_right - DOT_X, cy), 3.0, color);
        }
        let swatch = Rect::from_center_size(pos2(column_right - OUTLINE_X, cy), vec2(6.0, 12.0_f32.min(row_h - 6.0)));
        if layer.outline() {
            p.rect_stroke(swatch, 0.0, Stroke::new(1.5, color), StrokeKind::Inside);
        } else {
            p.rect_filled(swatch, 0.0, color);
        }
        for (dx, icon, on) in [(EYE_X, Icon::TlEyeSlash, !layer.visible()), (LOCK_X, Icon::TlLock, layer.locked())] {
            if on || selected || hot {
                let ink = if on { Color32::from_rgb(0xC8, 0xC8, 0xC8) } else { Color32::from_white_alpha(90) };
                icons::paint(&p, Rect::from_center_size(pos2(column_right - dx, cy), vec2(13.0, 13.0)), icon, ink);
            }
        }

        // Empty cells, with every 5th column lighter (not on folder rows); selected
        // ones blue (spans get theirs in `spans`). Each cell's first pixel column
        // (under the ruler's tick) and each row's last pixel row is a line, drawn
        // over the cell's own fill where the next fill can't reach it.
        let folder = layer.kind == LayerKind::Folder;
        let is_selected = |f: u32| !folder && sel.contains(&(i, f));
        for f in first..first + visible {
            let x = x_of(f);
            let fill = if is_selected(f) && layer.keyframe_at(f).is_none() {
                theme::FRAME_SELECTED
            } else if (f + 1) % 5 == 0 && !folder {
                theme::FRAME_CELL_5TH
            } else {
                theme::FRAME_CELL
            };
            p.rect_filled(Rect::from_min_size(pos2(x, top), vec2(cell_w, row_h)), 0.0, fill);
            p.vline(x, top..=top + row_h, hairline);
        }
        if !folder {
            spans(&p, layer, first, visible, top, geo, &is_selected);
        }
        p.hline(rect.x_range(), top + row_h - 0.5, hairline);
    }
    if ed.tl.parenting {
        layer_tree::paint_parenting(&p, ed, geo, column_right);
    }

    // What a drag will do: the block's new place, a span's new end, or where layers land.
    let outline = Stroke::new(1.5, theme::TEXT);
    match ed.tl.drag {
        Some(Drag::Move { by, .. }) => {
            if let Some(((l0, l1), (f0, f1), _)) = bounds(sel) {
                let (l0, l1) = ((l0 as isize + by.0) as usize, (l1 as isize + by.0) as usize);
                let (f0, f1) = ((f0 as i64 + by.1) as u32, (f1 as i64 + by.1) as u32);
                let block = Rect::from_min_max(pos2(x_of(f0), geo.top_of(l0)), pos2(x_of(f1 + 1), geo.top_of(l1) + row_h));
                p.rect_stroke(block, 0.0, outline, StrokeKind::Inside);
            }
        }
        Some(Drag::Stretch { layer, last, to }) => {
            let start = layers.get(layer).and_then(|l| l.keyframe_at(last)).map_or(last, |k| layers[layer].frames[k].index);
            let top = geo.top_of(layer);
            p.rect_stroke(Rect::from_min_max(pos2(x_of(start), top), pos2(x_of(to + 1), top + row_h)), 0.0, outline, StrokeKind::Inside);
        }
        // Into a folder, mask or guide: its row outlined; else a line at the gap, indented to where they'll sit.
        Some(Drag::Layers { dragging: true, .. }) => {
            if let Some(h) = hover {
                let landing = layer_tree::drop_at(ed, geo, h.y);
                let line = Stroke::new(2.0, theme::PLAYHEAD);
                match landing.onto {
                    Some(l) => {
                        let top = geo.top_of(l);
                        p.rect_stroke(Rect::from_min_max(pos2(rect.left(), top), pos2(column_right, top + row_h)), 0.0, line, StrokeKind::Inside);
                    }
                    None => {
                        let indent = landing.parent.map_or(0.0, |f| (ed.timeline().depth(f) + 1) as f32 * INDENT);
                        p.hline(rect.left() + indent..=column_right, landing.y, line);
                    }
                }
            }
        }
        Some(Drag::Parent { child }) => layer_tree::paint_parent_drag(&p, ed, geo, column_right, child, hover),
        _ => {}
    }
    let p = ui.painter_at(rect);
    p.vline(column_right, rect.y_range(), hairline);

    // Playhead: a translucent blue head one cell wide over the ruler, ending in
    // a point, and a line down through the rows. It carries no number.
    let x = x_of(ed.frame);
    if (frames.left() - cell_w..=frames.right()).contains(&x) {
        let head = Rect::from_min_max(pos2(x, ruler.top()), pos2(x + cell_w, ruler.bottom() - 4.0));
        let translucent = theme::PLAYHEAD.gamma_multiply(0.75);
        p.rect_filled(head, 0.0, translucent);
        let point = vec![head.left_bottom(), head.right_bottom(), pos2(head.center().x, ruler.bottom())];
        p.add(egui::Shape::convex_polygon(point, translucent, Stroke::NONE));
        p.vline(head.center().x, ruler.bottom()..=rect.bottom(), Stroke::new(1.0, theme::PLAYHEAD));
    }
    for thumb in thumbs.into_iter().flatten() {
        p.rect_filled(thumb, 3.5, theme::SCROLLBAR_THUMB);
    }
}

/// One layer's keyframe spans: fills, the selection over them, keyframe
/// dots, tween arrows, span ends, labels and frame-script marks.
fn spans(p: &Painter, layer: &Layer, first: u32, visible: u32, top: f32, geo: &Geo, selected: &dyn Fn(u32) -> bool) {
    let (row_h, cell_w, x_of) = (geo.row_h, geo.cell_w, |f: u32| geo.x_of(f));
    let ink = theme::KEYFRAME;
    let dot_y = top + row_h * 0.65;
    for (k, key) in layer.frames.iter().enumerate() {
        let (start, end) = (key.index, key.index + key.duration);
        if end <= first || start > first + visible {
            continue;
        }
        let (x0, x1) = (x_of(start), x_of(end));
        let span = Rect::from_min_max(pos2(x0, top), pos2(x1, top + row_h - 1.0));
        let filled = !key.elements.is_empty();
        let has_next = layer.frames.get(k + 1).is_some_and(|n| n.index == end);
        let fill = match &key.tween {
            Tween::Classic { .. } => theme::CLASSIC_TWEEN,
            Tween::Shape { .. } => theme::SHAPE_TWEEN,
            Tween::Motion(_) => theme::MOTION_TWEEN,
            Tween::Unsupported(kind) if kind == "shape" => theme::SHAPE_TWEEN,
            Tween::Unsupported(_) => theme::MOTION_TWEEN,
            Tween::None if filled => theme::FRAME_SPAN,
            Tween::None => theme::FRAME_SPAN_BLANK,
        };
        p.rect_filled(span.shrink2(vec2(0.0, 0.5)), 2.0, fill);
        // Selected frames: half blue over the span, the marks staying on top.
        for f in start.max(first)..end.min(first + visible) {
            if selected(f) {
                p.rect_filled(Rect::from_min_size(pos2(x_of(f), top), vec2(cell_w, row_h - 1.0)), 0.0, theme::FRAME_SELECTED.gamma_multiply(0.5));
            }
        }
        p.vline(x0, span.y_range(), Stroke::new(1.0, theme::FRAME_SPAN_EDGE));
        // Tiny dark dots along the top edge, one per frame boundary.
        for f in start + 1..end {
            p.circle_filled(pos2(x_of(f), span.top() + 2.0), 0.6, ink);
        }
        let dot = pos2(x0 + cell_w / 2.0, dot_y);
        if filled {
            p.circle_filled(dot, 2.75, ink);
        } else {
            p.circle_stroke(dot, 2.4, Stroke::new(1.5, ink));
        }
        // A frame script shows as a small "a" above the keyframe dot.
        if key.extra.before.iter().chain(&key.extra.after).any(|xml| xml.trim_start().starts_with("<Actionscript")) {
            p.text(pos2(dot.x, top + 2.0), Align2::CENTER_TOP, "a", FontId::proportional(8.0), ink);
        }
        if let Some((name, kind)) = key.label() {
            label(p, span, x0, x1, row_h, name, kind);
        }
        let tweening = !matches!(key.tween, Tween::None);
        if matches!(key.tween, Tween::Motion(_)) {
            // A motion tween has diamonds at its property keyframes, not an arrow.
            crate::tweens::paint_span(p, key, &x_of, cell_w, dot_y, span);
        } else if tweening {
            // An arrow at dot height to the next keyframe; dashed when there's none to reach, or the tween is broken.
            let (from, tip) = (dot.x + 4.0, x1 - 3.0);
            let stroke = Stroke::new(1.0, ink);
            if has_next && crate::tweens::plays(key, layer.frames.get(k + 1)) {
                p.line_segment([pos2(from, dot_y), pos2(tip, dot_y)], stroke);
                p.line_segment([pos2(tip - 4.0, dot_y - 3.0), pos2(tip, dot_y)], stroke);
                p.line_segment([pos2(tip - 4.0, dot_y + 3.0), pos2(tip, dot_y)], stroke);
            } else {
                p.add(egui::Shape::dashed_line(&[pos2(from, dot_y), pos2(tip, dot_y)], stroke, 3.0, 2.0));
            }
        } else if key.duration > 1 {
            // The end of a span: a small dark pill in its last frame.
            let pill = Rect::from_min_max(pos2(x1 - cell_w / 2.0 - 1.0, top + row_h * 0.55), pos2(x1 - cell_w / 2.0 + 1.0, top + row_h * 0.8));
            p.rect_filled(pill, 1.0, ink);
        }
    }
}

/// A label in its span: a flag (red name, gold anchor) or a green "//" (comment), then the text.
fn label(p: &Painter, span: Rect, x0: f32, x1: f32, row_h: f32, name: &str, kind: &str) {
    let color = match kind {
        "comment" => theme::LABEL_COMMENT,
        "anchor" => theme::LABEL_ANCHOR,
        _ => theme::LABEL_NAME,
    };
    let (fx, fy) = (x0 + 2.5, span.top() + 3.0);
    let text_x = if kind == "comment" {
        p.text(pos2(fx - 1.0, fy - 1.0), Align2::LEFT_TOP, "//", FontId::proportional(9.0), color);
        fx + 10.0
    } else {
        p.vline(fx, fy..=fy + 9.0, Stroke::new(1.0, color));
        let flag = vec![pos2(fx, fy), pos2(fx + 5.0, fy + 2.5), pos2(fx, fy + 5.0)];
        p.add(egui::Shape::convex_polygon(flag, color, Stroke::NONE));
        fx + 7.0
    };
    let room = Rect::from_min_max(pos2(text_x, span.top()), pos2(x1 - 2.0, span.top() + row_h * 0.55));
    p.with_clip_rect(room).text(pos2(text_x, span.top()), Align2::LEFT_TOP, name, FontId::proportional(9.0), theme::KEYFRAME);
}

/// The frame context menu (docs/animate-ui-interaction.md §2.3), with what
/// doesn't apply greyed. Returns the chosen command.
fn frame_menu(ui: &mut Ui, ed: &Editor) -> Option<Action> {
    use Action::*;
    ui.set_min_width(250.0);
    let tween = key_at(ed, ed.tl.menu_cell).map(|k| &k.tween);
    let classic = matches!(tween, Some(Tween::Classic { .. }));
    let shape = matches!(tween, Some(Tween::Shape { .. })) || matches!(tween, Some(Tween::Unsupported(k)) if k == "shape");
    let motion = matches!(tween, Some(Tween::Motion(_))) || matches!(tween, Some(Tween::Unsupported(k)) if k != "shape");
    // Inside a motion tween, its keyframes are its properties'.
    let mut keyframes = None;
    let mut picked = None;
    let mut item = |ui: &mut Ui, label: &str, keys: &str, action: Option<Action>| {
        let enabled = action.is_some_and(|a| available(ed, a));
        if theme::menu_item(ui, enabled, label, keys).clicked() {
            picked = action;
        }
    };
    if classic {
        item(ui, "Remove Classic Tween", "", Some(RemoveTween));
    } else {
        item(ui, "Create Classic Tween", "", Some(CreateClassicTween));
    }
    item(ui, if motion { "Remove Motion Tween" } else { "Create Motion Tween" }, "", Some(if motion { RemoveTween } else { CreateMotionTween }));
    item(ui, if shape { "Remove Shape Tween" } else { "Create Shape Tween" }, "", Some(if shape { RemoveTween } else { CreateShapeTween }));
    ui.add_enabled_ui(false, |ui| ui.menu_button("Convert to Frame-by-Frame Animation", |_| {}));
    let blocks: [&[(&str, &str, Option<Action>)]; 5] = [
        &[("Insert Frame", "F5", Some(InsertFrame)), ("Remove Frames", "Shift+F5", Some(RemoveFrames))],
        &[
            ("Insert Keyframe", "", Some(InsertKeyframe)),
            ("Insert Blank Keyframe", "", Some(InsertBlankKeyframe)),
            ("Clear Keyframe", "Shift+F6", Some(ClearKeyframe)),
            ("Convert to Keyframes", "F6", Some(ConvertToKeyframes)),
            ("Convert to Blank Keyframes", "F7", Some(ConvertToBlankKeyframes)),
        ],
        &[
            ("Cut Frames", "Ctrl+Alt+X", Some(CutFrames)),
            ("Copy Frames", "Ctrl+Alt+C", Some(CopyFrames)),
            ("Paste Frames", "Ctrl+Alt+V", Some(PasteFrames)),
            ("Paste and Overwrite Frames", "", Some(PasteOverwriteFrames)),
            ("Clear Frames", "Alt+Backspace", Some(ClearFrames)),
            ("Select All Frames", "Ctrl+Alt+A", Some(SelectAllFrames)),
        ],
        &[("Copy Motion", "", None), ("Paste Motion", "", None), ("Paste Motion Special...", "", None)],
        &[("Reverse Frames", "", Some(ReverseFrames)), ("Synchronize Symbols", "", None), ("Split Audio", "", None), ("Show In Library", "", None)],
    ];
    for (i, block) in blocks.into_iter().enumerate() {
        ui.separator();
        if i == 1 && matches!(tween, Some(Tween::Motion(_))) {
            keyframes = crate::tweens::keyframe_menu(ui, ed);
            continue;
        }
        if i == 3 && matches!(tween, Some(Tween::Motion(_))) {
            keyframes = keyframes.or(crate::tweens::motion_menu(ui, ed));
            continue;
        }
        for &(label, keys, action) in block {
            item(ui, label, keys, action);
        }
    }
    ui.separator();
    item(ui, "Actions", "F9", None);
    picked.or(keyframes)
}

/// The layer context menu (docs/animate-ui-interaction.md §2.5), with what
/// isn't supported greyed. Returns the chosen command.
fn layer_menu(ui: &mut Ui, ed: &Editor) -> Option<Action> {
    use Action::*;
    ui.set_min_width(227.0);
    let kind = ed.timeline().layers.get(ed.layer).map(|l| l.kind);
    let selected = selected_layers(ed);
    let folders_only = selected.iter().all(|&l| ed.timeline().layers[l].kind == LayerKind::Folder);
    let mut picked = None;
    let mut item = |ui: &mut Ui, label: &str, action: Option<Action>, checked: Option<bool>| {
        let enabled = action.is_some_and(|a| available(ed, a));
        let resp = match checked {
            Some(on) => theme::menu_toggle(ui, enabled, on, label),
            None => theme::menu_item(ui, enabled, label, ""),
        };
        if resp.clicked() {
            picked = action;
        }
    };
    let blocks: [&[(&str, Option<Action>)]; 3] = [
        &[("Show All", Some(ShowAllLayers)), ("Lock Others", Some(LockOthers)), ("Hide Others", Some(HideOthers)), ("Show Others Transparent", None)],
        &[("Insert Layer", Some(NewLayer)), ("Delete Layers", Some(DeleteLayer))],
        &[
            ("Cut Layers", Some(CutLayers)),
            ("Copy Layers", Some(CopyLayers)),
            ("Paste Layers", Some(PasteLayers)),
            ("Duplicate Layers", Some(DuplicateLayers)),
            ("Merge Layers", None),
            ("Convert Layers to Symbol...", None),
        ],
    ];
    for block in blocks {
        for &(label, action) in block {
            item(ui, label, action, None);
        }
        ui.separator();
    }
    item(ui, "Guide", Some(ToggleGuide), Some(kind == Some(LayerKind::Guide)));
    item(ui, "Add Classic Motion Guide", Some(AddMotionGuide), None);
    ui.separator();
    item(ui, "Mask", Some(ToggleMask), Some(kind == Some(LayerKind::Mask)));
    item(ui, "Show Masking", Some(ShowMasking), None);
    ui.separator();
    item(ui, "Insert Folder", Some(NewFolder), None);
    item(ui, "Delete Folders", folders_only.then_some(DeleteLayer), None);
    item(ui, "Expand Folders", Some(ExpandFolders), None);
    item(ui, "Collapse Folders", Some(CollapseFolders), None);
    ui.separator();
    item(ui, "Expand All Folders", Some(ExpandAllFolders), None);
    item(ui, "Collapse All Folders", Some(CollapseAllFolders), None);
    ui.separator();
    item(ui, "Properties...", Some(LayerProperties), None);
    picked
}

/// The timeline's windows: Layer Properties and Onion skin settings.
pub fn dialogs(ctx: &egui::Context, ed: &mut Editor) {
    layer_properties(ctx, ed);
    onion_settings(ctx, ed);
}

/// Layer Properties (docs/animate-ui-interaction.md §2.6), for the active layer.
// ponytail: Animate applies it to every selected layer; do that (changed fields only) when asked.
fn layer_properties(ctx: &egui::Context, ed: &mut Editor) {
    let Some(d) = &mut ed.tl.props else { return };
    let mut ok = ctx.input(|i| i.key_pressed(Key::Enter));
    let mut cancel = false;
    let modal = egui::Modal::new(egui::Id::new("layer-properties")).show(ctx, |ui| {
        ui.set_width(420.0);
        ui.label(RichText::new("Layer Properties").heading().color(theme::TEXT));
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            egui::Grid::new("layer-properties-fields").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
                let label = |ui: &mut Ui, text: &str| {
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| ui.label(RichText::new(text).color(theme::TEXT_DIM)));
                };
                label(ui, "Name:");
                ui.add(egui::TextEdit::singleline(&mut d.name).desired_width(170.0));
                ui.end_row();
                label(ui, "");
                ui.checkbox(&mut d.locked, "Lock");
                ui.end_row();
                label(ui, "");
                ui.add_enabled(false, egui::Checkbox::new(&mut false, "Attach To Camera"));
                ui.end_row();
                label(ui, "Visibility:");
                ui.vertical(|ui| {
                    ui.radio_value(&mut d.visible, true, "Visible");
                    ui.add_enabled(false, egui::RadioButton::new(false, "Transparent"));
                    ui.radio_value(&mut d.visible, false, "Invisible");
                });
                ui.end_row();
                label(ui, "Type:");
                ui.vertical(|ui| {
                    // A folder stays a folder, and a layer can't become one here.
                    let folder = d.kind == LayerKind::Folder;
                    let kinds = [(Some(LayerKind::Normal), "Normal"), (Some(LayerKind::Mask), "Mask"), (None, "Masked"), (Some(LayerKind::Folder), "Folder"), (Some(LayerKind::Guide), "Guide")];
                    for (kind, text) in kinds {
                        let enabled = kind.is_some_and(|k| (k == LayerKind::Folder) == folder);
                        if ui.add_enabled(enabled, egui::RadioButton::new(kind == Some(d.kind), text)).clicked()
                            && let Some(k) = kind
                        {
                            d.kind = k;
                        }
                    }
                });
                ui.end_row();
                label(ui, "Outline color:");
                egui::color_picker::color_edit_button_srgba(ui, &mut d.color, egui::color_picker::Alpha::Opaque);
                ui.end_row();
                label(ui, "");
                ui.checkbox(&mut d.outline, "View layer as outlines");
                ui.end_row();
                label(ui, "Layer height:");
                ui.add_enabled_ui(false, |ui| egui::ComboBox::from_id_salt("layer-height").selected_text("100%").show_ui(ui, |_| {}));
                ui.end_row();
            });
            ui.with_layout(Layout::top_down(Align::Max), |ui| {
                ok |= theme::cta_button(ui, "    OK    ").clicked();
                cancel |= ui.button("  Cancel  ").clicked();
            });
        });
    });
    if modal.should_close() || cancel {
        ed.tl.props = None;
    } else if ok && let Some(d) = ed.tl.props.take() {
        // OK with nothing changed is no undo step.
        let sel = ed.selection.clone();
        ed.edit_timeline(|_, tl| {
            // A mask or guide made another kind lets its layers go.
            if tl.layers.get(d.layer).is_some_and(|l| l.kind != d.kind) {
                tl.set_mask(d.layer, false);
                tl.set_guide(d.layer, false);
            }
            if let Some(l) = tl.layers.get_mut(d.layer) {
                let name = d.name.trim();
                if !name.is_empty() {
                    l.name = name.to_string();
                }
                l.kind = d.kind;
                l.set_visible(d.visible);
                l.set_locked(d.locked);
                l.set_outline(d.outline);
                // An untouched colour isn't written in, which would be a change.
                if d.color != layer_color(l, d.layer) {
                    l.set_color(crate::app::rgba(d.color));
                }
            }
            sel
        });
    }
}

/// The floating "Onion skin settings" window (docs/animate-ui-interaction.md §1.10).
fn onion_settings(ctx: &egui::Context, ed: &mut Editor) {
    let mut open = ed.tl.onion_window;
    let frame = ed.frame;
    let o = &mut ed.tl.onion;
    let (before, after) = (o.before, o.after);
    let window = egui::Window::new("Onion skin settings").open(&mut open).collapsible(false).resizable(false).default_width(240.0);
    let window = window.pivot(Align2::CENTER_BOTTOM).default_pos(ed.tl.onion_button.center_top());
    window.show(ctx, |ui| {
        egui::Grid::new("onion-settings").num_columns(2).spacing([10.0, 8.0]).show(ui, |ui| {
            let dim = |text: &str| RichText::new(text).color(theme::TEXT_DIM);
            ui.label(dim("Mode"));
            egui::ComboBox::from_id_salt("onion-mode").selected_text(if o.all { "All frames" } else { "Range" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut o.all, false, "Range");
                ui.selectable_value(&mut o.all, true, "All frames");
            });
            ui.end_row();
            for (text, color, frames) in [("Prev", &mut o.before_color, &mut o.before), ("Next", &mut o.after_color, &mut o.after)] {
                ui.label(dim(text));
                ui.horizontal(|ui| {
                    egui::color_picker::color_edit_button_srgba(ui, color, egui::color_picker::Alpha::Opaque);
                    ui.add(egui::DragValue::new(frames).range(0..=99).suffix(" frames"));
                });
                ui.end_row();
            }
            ui.label("");
            ui.checkbox(&mut o.linked, "Constraints").on_hover_text("Keep Prev and Next equal");
            ui.end_row();
            ui.label(dim("Draw"));
            ui.add_enabled_ui(false, |ui| {
                ui.horizontal(|ui| {
                    let _ = ui.selectable_label(false, "Outline");
                    let _ = ui.selectable_label(true, "Fill");
                })
            });
            ui.end_row();
            for (text, value) in [("Starting opacity", &mut o.opacity), ("Decrease by", &mut o.falloff)] {
                ui.label(dim(text));
                let mut percent = (*value * 100.0).round();
                if ui.add(egui::Slider::new(&mut percent, 0.0..=100.0).suffix("%").fixed_decimals(0)).changed() {
                    *value = percent / 100.0;
                }
                ui.end_row();
            }
            ui.label("");
            ui.checkbox(&mut o.keyframes_only, "Keyframes only");
            ui.end_row();
            ui.label("");
            let mut anchored = o.anchor.is_some();
            if ui.checkbox(&mut anchored, "Anchor markers").changed() {
                o.anchor = anchored.then_some(frame);
            }
            ui.end_row();
        });
        ui.add_space(4.0);
        if ui.button("Reset").clicked() {
            *o = Onion::default();
        }
    });
    if o.linked {
        if o.before != before {
            o.after = o.before;
        } else if o.after != after {
            o.before = o.after;
        }
    }
    ed.tl.onion_window = open;
}
