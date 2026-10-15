//! The Tools panel: the toolbar column at the far left, laid out as in
//! Animate 2024 (docs/animate-ui.md §6). Buttons stack in sections; a group
//! button holds a flyout and shows the tool used last. Under them: Edit
//! Toolbar ("…"), the colors, and the current tool's options.

use std::time::Duration;

use eframe::egui::{
    self, Align2, Color32, Event, FontId, Frame, Id, Key, KeyboardShortcut, LayerId, Margin, ModifierNames, Modifiers, Order,
    Painter, Popup, PopupCloseBehavior, Pos2, Rect, RectAlign, Sense, SetOpenCommand, Shape, Stroke, StrokeKind, Ui,
    WidgetInfo, WidgetType, pos2, vec2,
};

use crate::app::{Action, Editor, PropsTab};
use crate::eraser;
use crate::icons::{self, Icon};
use crate::paint_tools::Gap;
use crate::theme;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tool {
    Selection,
    Subselection,
    FreeTransform,
    GradientTransform,
    Lasso,
    Polygon,
    MagicWand,
    FluidBrush,
    /// The Classic Brush: paints fills.
    Brush,
    Eraser,
    Rectangle,
    RectanglePrimitive,
    Oval,
    OvalPrimitive,
    PolyStar,
    Line,
    Pen,
    AddAnchor,
    DeleteAnchor,
    ConvertAnchor,
    Text,
    PaintBucket,
    InkBottle,
    Eyedropper,
    AssetWarp,
    Hand,
    Rotation,
    TimeScrub,
    Zoom,
    Rotation3D,
    Translation3D,
    PaintBrush,
    Pencil,
    Camera,
    Bone,
    Bind,
    Width,
}

use Tool::*;

/// The Edit Toolbar drawer's fixed layout: each group at (column, first row),
/// its tools down the rows below. A group goes onto the toolbar as one
/// button, its flyout listing the tools in this order.
const DRAWER: &[(usize, usize, &[Tool])] = &[
    (0, 0, &[Selection, Subselection]),
    (0, 2, &[Brush]),
    (0, 3, &[Eraser]),
    (0, 4, &[Rectangle, RectanglePrimitive, Oval, OvalPrimitive, PolyStar]),
    (1, 0, &[FreeTransform, GradientTransform]),
    (1, 2, &[FluidBrush]),
    (1, 3, &[Line]),
    (1, 4, &[PaintBucket, InkBottle]),
    (1, 6, &[Camera]),
    (1, 7, &[Bone, Bind]),
    (2, 0, &[Rotation3D, Translation3D]),
    (2, 2, &[PaintBrush, Pencil]),
    (2, 4, &[Text]),
    (2, 5, &[Eyedropper]),
    (2, 6, &[Hand, Rotation, TimeScrub]),
    (3, 0, &[Lasso, Polygon, MagicWand]),
    (3, 3, &[Pen, AddAnchor, DeleteAnchor, ConvertAnchor]),
    (3, 7, &[Width]),
    (3, 8, &[Zoom]),
    (3, 9, &[AssetWarp]),
];

/// The default toolbar, section by section, each button named by its group's first tool.
const DEFAULT: &[&[Tool]] = &[
    &[Selection, FreeTransform, Lasso],
    &[FluidBrush, Brush, Eraser, Rectangle, Line],
    &[Pen, Text, PaintBucket, Eyedropper, AssetWarp],
    &[Hand, Zoom],
];

/// The drawer group holding `tool`.
fn group(tool: Tool) -> &'static [Tool] {
    DRAWER.iter().map(|g| g.2).find(|g| g.contains(&tool)).expect("every tool is in the drawer")
}

impl Tool {
    pub fn name(self) -> &'static str {
        match self {
            Selection => "Selection Tool",
            Subselection => "Subselection Tool",
            FreeTransform => "Free Transform Tool",
            GradientTransform => "Gradient Transform Tool",
            Lasso => "Lasso Tool",
            Polygon => "Polygon Tool",
            MagicWand => "Magic Wand Tool",
            FluidBrush => "Fluid Brush Tool",
            Brush => "Classic Brush Tool",
            Eraser => "Eraser Tool",
            Rectangle => "Rectangle Tool",
            RectanglePrimitive => "Rectangle Primitive Tool",
            Oval => "Oval Tool",
            OvalPrimitive => "Oval Primitive Tool",
            PolyStar => "PolyStar Tool",
            Line => "Line Tool",
            Pen => "Pen Tool",
            AddAnchor => "Add Anchor Point Tool",
            DeleteAnchor => "Delete Anchor Point Tool",
            ConvertAnchor => "Convert Anchor Point Tool",
            Text => "Text Tool",
            PaintBucket => "Paint Bucket Tool",
            InkBottle => "Ink Bottle Tool",
            Eyedropper => "Eyedropper Tool",
            AssetWarp => "Asset Warp Tool",
            Hand => "Hand Tool",
            Rotation => "Rotation Tool",
            TimeScrub => "Time Scrub Tool",
            Zoom => "Zoom Tool",
            Rotation3D => "3D Rotation Tool",
            Translation3D => "3D Translation Tool",
            PaintBrush => "Paint Brush Tool",
            Pencil => "Pencil Tool",
            Camera => "Camera Tool",
            Bone => "Bone Tool",
            Bind => "Bind Tool",
            Width => "Width Tool",
        }
    }

    /// Its own key, as in Animate 2024's defaults: no key cycles through a group.
    pub fn shortcut(self) -> Option<KeyboardShortcut> {
        let (none, shift) = (Modifiers::NONE, Modifiers::SHIFT);
        let (mods, key) = match self {
            Selection => (none, Key::V),
            Subselection => (none, Key::A),
            FreeTransform => (none, Key::Q),
            GradientTransform => (none, Key::F),
            Lasso => (none, Key::L),
            Polygon => (shift, Key::L),
            FluidBrush => (shift, Key::B),
            Brush => (none, Key::B),
            Eraser => (none, Key::E),
            Rectangle => (none, Key::R),
            RectanglePrimitive => (shift, Key::R),
            Oval => (none, Key::O),
            OvalPrimitive => (shift, Key::O),
            Line => (none, Key::N),
            Pen => (none, Key::P),
            AddAnchor => (none, Key::Equals),
            DeleteAnchor => (none, Key::Minus),
            ConvertAnchor => (shift, Key::C),
            Text => (none, Key::T),
            PaintBucket => (none, Key::K),
            InkBottle => (none, Key::S),
            Eyedropper => (none, Key::I),
            AssetWarp => (none, Key::W),
            Hand => (none, Key::H),
            Rotation => (shift, Key::H),
            TimeScrub => (Modifiers::ALT.plus(shift), Key::H),
            Zoom => (none, Key::Z),
            Rotation3D => (shift, Key::W),
            Translation3D => (none, Key::G),
            PaintBrush => (none, Key::Y),
            Pencil => (shift, Key::Y),
            Camera => (none, Key::C),
            Bone => (none, Key::M),
            Bind => (shift, Key::M),
            Width => (none, Key::U),
            MagicWand | PolyStar => return None,
        };
        Some(KeyboardShortcut::new(mods, key))
    }

    /// The shortcut as Windows writes it, e.g. "Shift+R" or "="; empty without one.
    fn keys(self) -> String {
        let Some(k) = self.shortcut() else { return String::new() };
        let mods = ModifierNames::NAMES.format(&k.modifiers, false);
        let key = k.logical_key.symbol_or_name();
        if mods.is_empty() { key.to_string() } else { format!("{mods}+{key}") }
    }

    pub fn icon(self) -> Icon {
        match self {
            Selection => Icon::Selection,
            Subselection => Icon::Subselection,
            FreeTransform => Icon::FreeTransform,
            GradientTransform => Icon::GradientTransform,
            Lasso => Icon::Lasso,
            Polygon => Icon::Polygon,
            MagicWand => Icon::MagicWand,
            FluidBrush => Icon::FluidBrush,
            Brush => Icon::Brush,
            Eraser => Icon::Eraser,
            Rectangle => Icon::Rectangle,
            RectanglePrimitive => Icon::RectanglePrimitive,
            Oval => Icon::Oval,
            OvalPrimitive => Icon::OvalPrimitive,
            PolyStar => Icon::PolyStar,
            Line => Icon::Line,
            Pen => Icon::Pen,
            AddAnchor => Icon::AddAnchor,
            DeleteAnchor => Icon::DeleteAnchor,
            ConvertAnchor => Icon::ConvertAnchor,
            Text => Icon::Text,
            PaintBucket => Icon::PaintBucket,
            InkBottle => Icon::InkBottle,
            Eyedropper => Icon::Eyedropper,
            AssetWarp => Icon::AssetWarp,
            Hand => Icon::Hand,
            Rotation => Icon::Rotation,
            TimeScrub => Icon::TimeScrub,
            Zoom => Icon::Zoom,
            Rotation3D => Icon::Rotation3D,
            Translation3D => Icon::Translation3D,
            PaintBrush => Icon::PaintBrush,
            Pencil => Icon::Pencil,
            Camera => Icon::Camera,
            Bone => Icon::Bone,
            Bind => Icon::Bind,
            Width => Icon::Width,
        }
    }

    /// Whether minnamate has this tool yet. The others show dimmed, and
    /// neither a click nor their key picks them.
    pub fn ready(self) -> bool {
        !matches!(
            self,
            Subselection
                | Polygon
                | MagicWand
                | FluidBrush
                | RectanglePrimitive
                | OvalPrimitive
                | AddAnchor
                | DeleteAnchor
                | ConvertAnchor
                | AssetWarp
                | Rotation
                | TimeScrub
                | Rotation3D
                | Translation3D
                | PaintBrush
                | Camera
                | Bone
                | Bind
                | Width
        )
    }

    /// Draws with the Object Drawing toggle (J). The primitives don't.
    pub fn object_drawing(self) -> bool {
        matches!(self, Line | Pen | Pencil | Brush | FluidBrush | PaintBrush | Rectangle | Oval | PolyStar)
    }

    fn tooltip(self) -> String {
        let keys = self.keys();
        let name = if keys.is_empty() { self.name().to_string() } else { format!("{} ({keys})", self.name()) };
        if self.ready() { name } else { format!("{name}\nNot in minnamate yet") }
    }
}

/// One toolbar button: a drawer group, showing the tool used last.
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    pub tools: &'static [Tool],
    pub shown: Tool,
}

/// The toolbar's layout: sections of buttons, top to bottom, as the Edit
/// Toolbar drawer leaves them.
// ponytail: the layout (and Object Drawing) resets each launch; save it with the workspace once workspaces persist.
#[derive(Clone, Debug, PartialEq)]
pub struct Toolbar {
    pub sections: Vec<Vec<Button>>,
    /// The Edit Toolbar drawer is open.
    pub drawer: bool,
}

impl Default for Toolbar {
    fn default() -> Self {
        let button = |&tool: &Tool| Button { tools: group(tool), shown: tool };
        Toolbar { sections: DEFAULT.iter().map(|s| s.iter().map(button).collect()).collect(), drawer: false }
    }
}

impl Toolbar {
    /// Section and index of the button holding `tool`.
    pub fn find(&self, tool: Tool) -> Option<(usize, usize)> {
        self.sections.iter().enumerate().find_map(|(s, b)| Some((s, b.iter().position(|b| b.tools.contains(&tool))?)))
    }

    /// Takes the button holding `tools` off the toolbar.
    pub fn remove(&mut self, tools: &[Tool]) {
        for section in &mut self.sections {
            section.retain(|b| b.tools[0] != tools[0]);
        }
        self.sections.retain(|s| !s.is_empty());
    }

    /// Puts a button for `tools` at `index` of section `section`, moving it
    /// there if it's already on the toolbar.
    pub fn insert(&mut self, tools: &'static [Tool], (section, index): (usize, usize)) {
        let old = self.find(tools[0]);
        let shown = old.map_or(tools[0], |(s, i)| self.sections[s][i].shown);
        if self.sections.is_empty() {
            self.sections.push(Vec::new());
        }
        let s = section.min(self.sections.len() - 1);
        let i = index.min(self.sections[s].len());
        self.sections[s].insert(i, Button { tools, shown });
        if let Some((os, oi)) = old {
            self.sections[os].remove(if os == s && oi >= i { oi + 1 } else { oi });
        }
        self.sections.retain(|s| !s.is_empty());
    }
}

/// Makes `tool` the current tool, as its button, its flyout or its key does.
/// Tools with options bring up the Properties panel's Tool tab.
pub fn select(ed: &mut Editor, tool: Tool) {
    if !tool.ready() {
        return;
    }
    ed.tool = tool;
    if !matches!(tool, Selection | Hand | Zoom) {
        ed.props_tab = PropsTab::Tool;
    }
}

/// D: a black stroke and a white fill.
fn black_and_white(ed: &mut Editor) {
    (ed.stroke_color, ed.fill_color) = (Color32::BLACK, Color32::WHITE);
    (ed.stroke_gradient, ed.fill_gradient) = (None, None);
}

/// "No color": a fully transparent color shows as the swatch with the red slash.
fn no_color(c: Color32) -> bool {
    c.a() == 0
}

/// X: stroke and fill change places, gradients too; not while either is "no color".
fn swap_colors(ed: &mut Editor) {
    if swappable(ed) {
        std::mem::swap(&mut ed.stroke_color, &mut ed.fill_color);
        std::mem::swap(&mut ed.stroke_gradient, &mut ed.fill_gradient);
    }
}

fn swappable(ed: &Editor) -> bool {
    (ed.stroke_gradient.is_some() || !no_color(ed.stroke_color)) && (ed.fill_gradient.is_some() || !no_color(ed.fill_color))
}

/// Tool keys, then D, X and J. egui ignores extra Shift and Alt when matching,
/// so the most-modified keys go first; keys of tools minnamate lacks are taken
/// too, so Shift+R can't fall through to R. Alt+letter is a menu's on Windows.
pub fn keys(ctx: &egui::Context, ed: &mut Editor) {
    let alt = ctx.input(|i| i.modifiers.alt);
    let tools = DRAWER.iter().flat_map(|g| g.2).filter_map(|&t| Some((t.shortcut()?, Some(t))));
    // Shift+D, Global Transform for the 3D tools, isn't built either.
    let mut keys: Vec<_> = tools.chain([(KeyboardShortcut::new(Modifiers::SHIFT, Key::D), None)]).filter(|(k, _)| k.modifiers.alt == alt).collect();
    keys.sort_by_key(|(k, _)| std::cmp::Reverse(k.modifiers.alt as u8 + k.modifiers.shift as u8));
    for (k, tool) in keys {
        if ctx.input_mut(|i| i.consume_shortcut(&k))
            && let Some(tool) = tool
        {
            select(ed, tool);
        }
    }
    if alt {
        return;
    }
    let toggle_object_drawing = |ed: &mut Editor| ed.object_drawing = !ed.object_drawing;
    for (key, run) in [(Key::D, black_and_white as fn(&mut Editor)), (Key::X, swap_colors), (Key::J, toggle_object_drawing)] {
        // Once a press: a slow frame gets key repeats together.
        for _ in 0..ctx.input_mut(|i| i.count_and_consume_key(Modifiers::NONE, key)) {
            run(ed);
        }
    }
}

// Layout in points, from the column's top left (docs/animate-ui.md §6.2).
const PITCH: f32 = 30.0;
/// From the column's left edge to a button's.
const INSET: f32 = 8.0;
/// Between buttons side by side, in a window too short for one column of them.
const COLUMN_PITCH: f32 = 36.0;
/// The dock strip and the tab row, each with its line under it.
const HEADER: f32 = 29.0;
/// From the "…" slot to the first tool-option slot.
const OPTIONS: f32 = 91.0;
/// A press on a group button held this long opens its flyout.
const HOLD: f64 = 0.5;

// The drawer's own shades (§6.9).
const DRAWER_BORDER: Color32 = Color32::from_gray(0x3E);
const DRAWER_DIVIDER: Color32 = Color32::from_gray(0x4C);
const DRAWER_TITLE: Color32 = Color32::from_gray(0xE0);
/// Tiles of tools already on the toolbar, and their glyphs.
const TILE_ON: Color32 = Color32::from_gray(0x3D);
const TILE_ON_ICON: Color32 = Color32::from_gray(0x6A);
/// Tiles that can go onto the toolbar.
const TILE_ADD: Color32 = Color32::from_gray(0x4B);
/// Tool options that don't apply.
const OPTION_FAINT: Color32 = Color32::from_gray(0x46);
/// The Black and White glyph's outlines, and the hole in the stroke swatch.
const SWATCH_EDGE: Color32 = Color32::from_gray(0xB8);
const SWATCH_HOLE: Color32 = Color32::from_gray(0x40);

/// A drawer group dragged between the drawer and the toolbar.
#[derive(Clone, Copy)]
struct Dragged(&'static [Tool]);

/// The panel's width when it's `height` tall: one column of buttons, or as
/// many side by side as it takes to fit, as when Animate's toolbar is dragged
/// wider (§6.1). It keeps room for two tool options, so picking a tool never
/// changes it.
pub fn width(ed: &Editor, height: f32) -> f32 {
    let fits = |n: usize| HEADER + body_height(Layout::new(&ed.toolbar, n, Pos2::ZERO).more.top(), n, 2) <= height;
    let n = (1..4).find(|&n| fits(n)).unwrap_or(4);
    theme::TOOLS_W + COLUMN_PITCH * (n - 1) as f32
}

pub fn panel(ui: &mut Ui, ed: &mut Editor) {
    let whole = ui.max_rect();
    // As many buttons abreast as `width` made room for.
    let n = ((whole.width() - theme::TOOLS_W) / COLUMN_PITCH).round().max(0.0) as usize + 1;
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;
    header(ui, ed);
    column(ui, ed, n, whole);
    // The tile or button being dragged follows the pointer.
    if let (Some(Dragged(tools)), Some(pos)) = (egui::DragAndDrop::payload::<Dragged>(ui.ctx()).as_deref().copied(), ui.ctx().pointer_latest_pos()) {
        let p = ui.ctx().layer_painter(LayerId::new(Order::Tooltip, Id::new("tool-drag")));
        let tile = egui::Rect::from_center_size(pos, vec2(40.0, 40.0));
        p.rect_filled(tile, 4.0, TILE_ADD);
        icons::paint(&p, Rect::from_center_size(tile.center(), vec2(18.0, 18.0)), tools[0].icon(), theme::TEXT);
    }
}

/// The column's top: the dock strip with «, then a tab row too narrow for its
/// "Tools" tab, which shows only a sliver of the label, and the panel menu.
fn header(ui: &mut Ui, ed: &mut Editor) {
    theme::dock_strip(ui, Icon::ChevronsLeft);
    let (row, _) = ui.allocate_exact_size(vec2(ui.available_width(), 17.0), Sense::hover());
    let p = ui.painter_at(row);
    let strip = Rect::from_min_size(row.min, vec2(row.width(), 16.0));
    p.rect_filled(strip, 0.0, theme::PANEL_HEADER);
    let tab = Rect::from_min_size(strip.min, vec2(10.0, 16.0));
    p.rect_filled(tab, 0.0, theme::PANEL_BG);
    p.with_clip_rect(tab).text(pos2(tab.left() + 7.0, tab.center().y), Align2::LEFT_CENTER, "Tools", FontId::proportional(12.0), theme::TEXT);
    p.vline(tab.right() + 0.5, strip.y_range(), Stroke::new(1.0, theme::BORDER));
    p.hline(row.x_range(), row.bottom() - 0.5, Stroke::new(1.0, theme::INSET));
    // The panel menu: four short lines.
    let glyph = Rect::from_min_size(pos2(strip.right() - 14.0, strip.top() + 4.0), vec2(10.0, 7.0));
    let menu = ui.interact(glyph.expand2(vec2(5.0, 4.0)), ui.id().with("tools-panel-menu"), Sense::click());
    let color = if menu.hovered() { theme::TEXT } else { theme::TEXT_DIM };
    for i in 0..4 {
        p.hline(glyph.x_range(), glyph.top() + 0.5 + 2.0 * i as f32, Stroke::new(1.0, color));
    }
    Popup::menu(&menu).style(theme::menu_style).show(|ui| {
        // ponytail: panels don't move yet, so there's nothing to lock; no help pages either.
        theme::menu_item(ui, false, "Lock", "");
        ui.separator();
        theme::menu_item(ui, false, "Help", "");
        ui.separator();
        if theme::menu_item(ui, true, "Close", "").clicked() {
            ed.show_tools = false;
        }
        if theme::menu_item(ui, true, "Close Group", "").clicked() {
            ed.show_tools = false;
        }
    });
}

/// Where the buttons go under the header at `o`, `n` abreast: section by
/// section, then the "…" slot on a row of its own.
struct Layout {
    /// Each button's section, index and place.
    slots: Vec<(usize, usize, Rect)>,
    separators: Vec<f32>,
    more: Rect,
}

impl Layout {
    fn new(toolbar: &Toolbar, n: usize, o: Pos2) -> Layout {
        let (mut slots, mut separators, mut y) = (Vec::new(), Vec::new(), o.y + 5.0);
        for (s, section) in toolbar.sections.iter().enumerate() {
            if s > 0 {
                separators.push(y);
                y += 6.0;
            }
            slots.extend((0..section.len()).map(|i| (s, i, slot(o.x, n, i, y))));
            y += PITCH * section.len().div_ceil(n) as f32;
        }
        Layout { slots, separators, more: slot(o.x, n, 0, y) }
    }
}

/// The `k`th of the slots `n` abreast from height `y`, in a column whose left edge is at `left`.
fn slot(left: f32, n: usize, k: usize, y: f32) -> Rect {
    Rect::from_min_size(pos2(left + INSET + COLUMN_PITCH * (k % n) as f32, y + PITCH * (k / n) as f32), theme::TOOL_BUTTON)
}

/// The column's height under the header, its "…" slot at `more` and `options` tool options under the colors.
fn body_height(more: f32, n: usize, options: usize) -> f32 {
    more + OPTIONS + PITCH * options.div_ceil(n) as f32 + 4.0
}

/// Buttons section by section, then the "…" slot, the colors and the tool's
/// options. `whole` is the panel, clicks inside which leave the drawer open.
fn column(ui: &mut Ui, ed: &mut Editor, n: usize, whole: Rect) {
    let o = pos2(ui.max_rect().left(), ui.cursor().top());
    let Layout { slots, separators, more } = Layout::new(&ed.toolbar, n, o);
    let options = options(ed);
    let height = body_height(more.top() - o.y, n, options.len());
    let (col, col_resp) = ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::hover());

    let p = ui.painter().clone();
    let right = slot(o.x, n, n - 1, 0.0).right();
    for y in separators {
        p.rect_filled(Rect::from_min_max(pos2(o.x + 9.0, y), pos2(right - 1.0, y + 2.0)), 0.0, theme::BORDER);
    }
    for &(s, i, rect) in &slots {
        tool_button(ui, ed, s, i, rect);
    }
    more_button(ui, ed, more);
    colors(ui, ed, more);
    for (k, &opt) in options.iter().enumerate() {
        option(ui, ed, opt, slot(o.x, n, k, more.top() + OPTIONS));
    }

    // Dragging a drawer tile (or a button) over the column: an insertion line, and on release, the button.
    let first = slot(o.x, n, 0, o.y + 5.0);
    if let Some(pos) = col_resp.dnd_hover_payload::<Dragged>().and(ui.ctx().pointer_latest_pos()) {
        p.line_segment(drop_at(&slots, n, pos, first).1, Stroke::new(2.0, theme::ACCENT));
    }
    if let (Some(d), Some(pos)) = (col_resp.dnd_release_payload::<Dragged>(), ui.ctx().pointer_latest_pos()) {
        ed.toolbar.insert(d.0, drop_at(&slots, n, pos, first).0);
    }
    if ed.toolbar.drawer {
        drawer(ui, ed, whole, pos2(whole.right(), col.top() + 1.0));
    }
}

/// Where a tile dropped at `pos` goes: before or after the button nearest it,
/// by the half it's over (top or bottom in one column, left or right in more),
/// or first on an empty toolbar. Also the insertion line, along that edge.
fn drop_at(slots: &[(usize, usize, Rect)], n: usize, pos: Pos2, first: Rect) -> ((usize, usize), [Pos2; 2]) {
    let nearest = slots.iter().min_by(|a, b| a.2.distance_sq_to_pos(pos).total_cmp(&b.2.distance_sq_to_pos(pos)));
    let Some(&(s, i, r)) = nearest else { return ((0, 0), [first.left_top(), first.right_top()]) };
    let r = r.expand(2.0);
    match (n == 1, if n == 1 { pos.y < r.center().y } else { pos.x < r.center().x }) {
        (true, true) => ((s, i), [r.left_top(), r.right_top()]),
        (true, false) => ((s, i + 1), [r.left_bottom(), r.right_bottom()]),
        (false, true) => ((s, i), [r.left_top(), r.left_bottom()]),
        (false, false) => ((s, i + 1), [r.right_top(), r.right_bottom()]),
    }
}

/// A tool button, and for a group its flyout: press and hold, or right-click.
fn tool_button(ui: &mut Ui, ed: &mut Editor, s: usize, i: usize, rect: Rect) {
    // The button shows the group's tool used last, however it was picked.
    let Button { tools, mut shown } = ed.toolbar.sections[s][i].clone();
    if tools.contains(&ed.tool) && shown != ed.tool {
        shown = ed.tool;
        ed.toolbar.sections[s][i].shown = shown;
    }
    let id = Id::new(("tool", tools[0]));
    let flyout = id.with("flyout");
    let resp = ui.interact(rect, id, Sense::click_and_drag());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, shown.ready(), shown.name()));
    let open = Popup::is_id_open(ui.ctx(), flyout);
    // A tool is picked on mouse-down, as in Animate; a click ends a Pen path too.
    let pressed = resp.is_pointer_button_down_on() && ui.input(|i| i.pointer.primary_pressed());
    if (pressed || resp.clicked()) && shown.ready() {
        crate::pen::finish(ed, false);
        select(ed, shown);
    }
    if resp.double_clicked() && shown.ready() {
        let ctx = ui.ctx().clone();
        match shown {
            // Double-clicking the Eraser erases everything on stage and pasteboard.
            Eraser => {
                ed.run(Action::SelectAll, &ctx);
                ed.run(Action::DeleteSelection, &ctx);
            }
            Hand => ed.run(Action::FitInWindow, &ctx),
            Zoom => ed.run(Action::Zoom(1.0), &ctx),
            _ => {}
        }
    }
    // With the drawer open, buttons drag off the toolbar into it.
    if ed.toolbar.drawer {
        resp.dnd_set_drag_payload(Dragged(tools));
    }
    if tools.len() > 1 {
        let mut command = None;
        let held = ui.input(|i| i.pointer.primary_down().then(|| i.pointer.press_start_time().map(|t| i.time - t)).flatten());
        if resp.is_pointer_button_down_on() && !ed.toolbar.drawer && let Some(held) = held {
            if held < HOLD {
                ui.ctx().request_repaint_after(Duration::from_secs_f64(HOLD - held));
            } else if !open && !resp.dragged() {
                command = Some(SetOpenCommand::Bool(true));
            }
        }
        if resp.secondary_clicked() {
            command = Some(SetOpenCommand::Bool(true));
        } else if pressed && open {
            command = Some(SetOpenCommand::Bool(false));
        }
        let rows: Vec<Row> = tools.iter().map(|&t| Row { icon: t.icon(), name: t.name(), keys: t.keys(), checked: t == shown, enabled: t.ready() }).collect();
        if let Some(k) = dark_menu(ui, rect, flyout, command, &rows) {
            crate::pen::finish(ed, false);
            select(ed, tools[k]);
        }
    }

    let active = shown == ed.tool;
    let p = ui.painter();
    let down = resp.is_pointer_button_down_on() && shown.ready();
    if down || active || Popup::is_id_open(ui.ctx(), flyout) {
        p.rect_filled(rect, 4.0, theme::BUTTON_ACTIVE);
    } else if resp.hovered() && shown.ready() {
        p.rect_filled(rect, 4.0, theme::BUTTON_HOVER);
    }
    let tint = if !shown.ready() {
        theme::ICON_DISABLED
    } else if active {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    icons::paint(p, Rect::from_center_size(rect.center(), vec2(18.0, 18.0)), shown.icon(), tint);
    if tools.len() > 1 {
        flyout_mark(p, rect, tint);
    }
    if !Popup::is_id_open(ui.ctx(), flyout) {
        resp.on_hover_text(shown.tooltip());
    }
}

/// The small triangle in a button's bottom right corner that marks a flyout or a menu.
fn flyout_mark(p: &Painter, button: Rect, color: Color32) {
    let corner = button.min + vec2(30.0, 24.0);
    p.add(Shape::convex_polygon(vec![corner - vec2(3.5, 0.0), corner, corner - vec2(0.0, 3.5)], color, Stroke::NONE));
}

/// One row of a dark popup menu.
struct Row {
    icon: Icon,
    name: &'static str,
    keys: String,
    checked: bool,
    enabled: bool,
}

/// A dark popup menu beside `anchor`, as a tool flyout is: 24 px rows of
/// icon, name and shortcut, a blue check by the current row. It stays open
/// until a row is picked, a press lands outside it and its button, or Escape.
/// Returns the row picked.
fn dark_menu(ui: &Ui, anchor: Rect, id: Id, command: Option<SetOpenCommand>, rows: &[Row]) -> Option<usize> {
    let ctx = ui.ctx().clone();
    let font = FontId::proportional(12.0);
    let widest = |text: &dyn Fn(&Row) -> String| {
        rows.iter().map(|r| ctx.fonts_mut(|f| f.layout_no_wrap(text(r), font.clone(), theme::TEXT).size().x)).fold(0.0, f32::max)
    };
    let width = (29.0 + widest(&|r| r.name.to_string()) + 12.0 + 10.0 + widest(&|r| r.keys.clone()) + 8.0).max(224.0);
    let frame = Frame::new()
        .fill(theme::POPUP_BG)
        .stroke(Stroke::new(1.0, theme::POPUP_BORDER))
        .corner_radius(4)
        .inner_margin(Margin::symmetric(0, 4))
        .shadow(ui.style().visuals.popup_shadow);
    let popup = Popup::new(id, ctx.clone(), anchor, ui.layer_id())
        .open_memory(command)
        .align(RectAlign::RIGHT_START)
        .align_alternatives(&[RectAlign::LEFT_START])
        .close_behavior(PopupCloseBehavior::IgnoreClicks)
        .frame(frame);
    let shown = popup.show(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let mut picked = None;
        for (k, row) in rows.iter().enumerate() {
            let (rect, resp) = ui.allocate_exact_size(vec2(width, 24.0), Sense::hover());
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, row.enabled, row.name));
            let hot = row.enabled && ui.rect_contains_pointer(rect);
            let p = ui.painter();
            if hot {
                p.rect_filled(rect, 0.0, theme::POPUP_HOVER);
            }
            let (ink, text) = if row.enabled { (theme::TEXT, theme::TEXT) } else { (theme::ICON_DISABLED, theme::TEXT_DISABLED) };
            icons::paint(p, Rect::from_min_size(pos2(rect.left() + 7.0, rect.center().y - 8.0), vec2(16.0, 16.0)), row.icon, ink);
            p.text(pos2(rect.left() + 29.0, rect.center().y), Align2::LEFT_CENTER, row.name, font.clone(), text);
            let keys = p.text(pos2(rect.right() - 8.0, rect.center().y), Align2::RIGHT_CENTER, &row.keys, font.clone(), text);
            if row.checked {
                check(p, pos2(keys.left() - if row.keys.is_empty() { 0.0 } else { 2.0 }, rect.center().y));
            }
            if hot && ui.input(|i| i.pointer.primary_released()) {
                picked = Some(k);
            }
        }
        picked
    })?;
    let area = shown.response.rect;
    let pressed_outside = ctx.input(|i| {
        i.events.iter().any(|e| matches!(e, Event::PointerButton { pos, pressed: true, .. } if !area.contains(*pos) && !anchor.contains(*pos)))
    });
    if shown.inner.is_some() || pressed_outside || ctx.input(|i| i.key_down(Key::Escape)) {
        Popup::close_id(&ctx, id);
    }
    shown.inner
}

/// The flyout's blue check mark, about 10×9, ending at `right` (its middle right).
fn check(p: &Painter, right: Pos2) {
    let o = right - vec2(10.0, 0.0);
    p.line(vec![o + vec2(0.5, 0.0), o + vec2(3.5, 3.5), o + vec2(9.5, -3.5)], Stroke::new(1.6, theme::ACCENT_HOVER));
}

/// The "…" button: it opens and closes the Edit Toolbar drawer.
fn more_button(ui: &mut Ui, ed: &mut Editor, rect: Rect) {
    let resp = ui.interact(rect, Id::new("edit-toolbar"), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Edit Toolbar..."));
    if resp.clicked() {
        ed.toolbar.drawer = !ed.toolbar.drawer;
    }
    let p = ui.painter();
    if ed.toolbar.drawer || resp.hovered() {
        p.rect_filled(Rect::from_center_size(rect.min + vec2(16.0, 14.0), vec2(32.0, 17.0)), 8.5, theme::BUTTON_ACTIVE);
    }
    for x in [10.0, 16.0, 22.0] {
        p.circle_filled(rect.min + vec2(x, 14.0), 2.0, theme::TEXT);
    }
    resp.on_hover_text("Edit Toolbar...");
}

/// Black and White, Swap Colors, then the fill swatch over the stroke swatch,
/// all at fixed offsets from the "…" slot (§6.8).
fn colors(ui: &mut Ui, ed: &mut Editor, more: Rect) {
    let o = more.min - vec2(INSET, 0.0);
    let bw = Rect::from_min_size(o + vec2(8.0, 34.0), vec2(12.0, 12.0));
    let swap = Rect::from_min_size(o + vec2(27.0, 34.0), vec2(12.0, 12.0));
    let fill = Rect::from_min_size(o + vec2(8.0, 52.0), vec2(22.0, 22.0));
    let stroke = fill.translate(vec2(11.0, 11.0));

    let small_button = |ui: &mut Ui, rect: Rect, id: &str, label: &str, enabled: bool| {
        let resp = ui.interact(rect.expand(1.0), Id::new(id), if enabled { Sense::click() } else { Sense::hover() });
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
        if enabled && resp.hovered() {
            ui.painter().rect_filled(rect.expand(1.0), 2.0, theme::BUTTON_HOVER);
        }
        resp.on_hover_text(label)
    };
    if small_button(ui, bw, "black-and-white", "Black and White (D)", true).clicked() {
        black_and_white(ed);
    }
    let swappable = swappable(ed);
    if small_button(ui, swap, "swap-colors", "Swap Colors (X)", swappable).clicked() {
        swap_colors(ed);
    }
    let p = ui.painter();
    // Black and White: two 9×9 squares, the white one in front.
    let back = Rect::from_min_size(bw.min + vec2(3.0, 3.0), vec2(9.0, 9.0));
    let front = Rect::from_min_size(bw.min, vec2(9.0, 9.0));
    p.rect_filled(back, 0.0, SWATCH_EDGE);
    p.rect_filled(back.shrink(1.0), 0.0, Color32::BLACK);
    p.rect_filled(front, 0.0, SWATCH_EDGE);
    p.rect_filled(front.shrink(1.0), 0.0, Color32::BLACK);
    p.rect_filled(front.shrink(2.0), 0.0, Color32::WHITE);
    let ink = if swappable { theme::TEXT } else { theme::ICON_DISABLED };
    icons::paint(p, Rect::from_center_size(swap.center(), vec2(18.0, 18.0)), Icon::Swap, ink);

    // The stroke ring sits behind the fill square, which takes clicks where they overlap.
    let stroke_resp = ui.interact(stroke, Id::new("stroke-swatch"), Sense::click());
    let fill_resp = ui.interact(fill, Id::new("fill-swatch"), Sense::click());
    let p = ui.painter();
    swatch(p, stroke, ed.stroke_color, ed.stroke_gradient.as_ref(), true);
    swatch(p, fill, ed.fill_color, ed.fill_gradient.as_ref(), false);
    // The swatch popup, as from Properties: a pick sets the tool color and recolors the selection.
    for (resp, fill, label) in [(stroke_resp, false, "Stroke color"), (fill_resp, true, "Fill color")] {
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, format!("Tools {}", label.to_lowercase())));
        let paint = if fill { ed.fill() } else { ed.stroke().paint };
        crate::color::paint_popup(ui, ed, &resp, &paint, fill);
        resp.on_hover_text(label);
    }
}

/// A 22×22 swatch, 1 px black then 1 px white around it. The fill is a
/// square of color, or of its gradient; the stroke a 2 px band of it around a
/// dark hole. "No color" is white, with a red slash.
fn swatch(p: &Painter, r: Rect, color: Color32, gradient: Option<&minnamate_core::doc::Gradient>, ring: bool) {
    let none = no_color(color) && gradient.is_none();
    p.rect_filled(r, 0.0, Color32::BLACK);
    p.rect_filled(r.shrink(1.0), 0.0, Color32::WHITE);
    match gradient {
        Some(g) => crate::color::paint_gradient(p, r.shrink(2.0), g, false),
        None => {
            p.rect_filled(r.shrink(2.0), 0.0, if none { Color32::WHITE } else { color.to_opaque() });
        }
    }
    if none {
        p.with_clip_rect(r.shrink(1.0)).line_segment([r.left_bottom(), r.right_top()], Stroke::new(2.0, theme::NO_COLOR));
    }
    if ring {
        p.rect_filled(r.shrink(4.0), 0.0, Color32::WHITE);
        p.rect_filled(r.shrink(5.0), 0.0, Color32::BLACK);
        p.rect_filled(r.shrink(6.0), 0.0, SWATCH_HOLE);
    }
}

/// A slot in the options area under the colors (§6.8).
#[derive(Clone, Copy, PartialEq, Debug)]
enum Opt {
    ObjectDrawing,
    Smooth,
    Straighten,
    BrushMode,
    PencilMode,
    EraserMode,
    GapSize,
    LockFill,
}

/// The current tool's options, top to bottom.
fn options(ed: &Editor) -> Vec<Opt> {
    let mut list = Vec::new();
    if ed.tool.object_drawing() {
        list.push(Opt::ObjectDrawing);
    }
    match ed.tool {
        // Only while something on stage is selected.
        Selection if !ed.selection.is_empty() => list.extend([Opt::Smooth, Opt::Straighten]),
        Brush => list.push(Opt::BrushMode),
        Pencil => list.push(Opt::PencilMode),
        Eraser => list.push(Opt::EraserMode),
        PaintBucket => list.extend([Opt::GapSize, Opt::LockFill]),
        _ => {}
    }
    list
}

fn eraser_icon(mode: eraser::Mode) -> Icon {
    match mode {
        eraser::Mode::Normal => Icon::EraseNormal,
        eraser::Mode::Fills => Icon::EraseFills,
        eraser::Mode::Lines => Icon::EraseLines,
        eraser::Mode::SelectedFills => Icon::EraseSelectedFills,
        eraser::Mode::Inside => Icon::EraseInside,
    }
}

fn gap_icon(gap: Gap) -> Icon {
    match gap {
        Gap::DontClose => Icon::GapNone,
        Gap::Small => Icon::GapSmall,
        Gap::Medium => Icon::GapMedium,
        Gap::Large => Icon::GapLarge,
    }
}

/// One option slot: a toggle, a one-shot command, or a mode menu (a glyph
/// with the corner triangle). Options minnamate lacks show faint.
fn option(ui: &mut Ui, ed: &mut Editor, opt: Opt, rect: Rect) {
    // ponytail: Smooth, Straighten and the Brush and Pencil modes aren't built (shapes don't smooth, brushes only paint normally).
    let (icon, label, on, enabled, menu) = match opt {
        Opt::ObjectDrawing => (Icon::ObjectDrawing, "Object Drawing (J)", ed.object_drawing, true, false),
        Opt::Smooth => (Icon::Smooth, "Smooth", false, false, false),
        Opt::Straighten => (Icon::Straighten, "Straighten", false, false, false),
        Opt::BrushMode => (Icon::PaintNormal, "Brush Mode", false, false, true),
        Opt::PencilMode => (Icon::Smooth, "Pencil Mode", false, false, true),
        Opt::EraserMode => (eraser_icon(ed.eraser.mode), "Eraser Mode", false, true, true),
        Opt::GapSize => (gap_icon(ed.paint.gap), "Gap Size", false, true, true),
        Opt::LockFill => (Icon::LockFill, "Lock Fill", ed.paint.lock_fill, true, false),
    };
    let id = Id::new(("tool-option", opt as u8));
    let resp = ui.interact(rect, id, if enabled { Sense::click() } else { Sense::hover() });
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, enabled, label));
    if resp.clicked() {
        match opt {
            Opt::ObjectDrawing => ed.object_drawing = !ed.object_drawing,
            Opt::LockFill => ed.paint.lock_fill = !ed.paint.lock_fill,
            _ => {}
        }
    }
    let menu_id = id.with("menu");
    if menu && enabled {
        let command = resp.clicked().then_some(SetOpenCommand::Toggle);
        let rows: Vec<Row> = match opt {
            Opt::EraserMode => eraser::Mode::ALL.iter().map(|&m| row(eraser_icon(m), m.name(), m == ed.eraser.mode)).collect(),
            _ => Gap::ALL.iter().map(|&g| row(gap_icon(g), g.name(), g == ed.paint.gap)).collect(),
        };
        if let Some(k) = dark_menu(ui, rect, menu_id, command, &rows) {
            match opt {
                Opt::EraserMode => ed.eraser.mode = eraser::Mode::ALL[k],
                _ => ed.paint.gap = Gap::ALL[k],
            }
        }
    }
    let p = ui.painter();
    if enabled && (on || resp.is_pointer_button_down_on() || Popup::is_id_open(ui.ctx(), menu_id)) {
        p.rect_filled(rect, 4.0, theme::BUTTON_ACTIVE);
    } else if enabled && resp.hovered() {
        p.rect_filled(rect, 4.0, theme::BUTTON_HOVER);
    }
    let tint = if !enabled {
        OPTION_FAINT
    } else if on {
        Color32::WHITE
    } else {
        theme::TEXT
    };
    icons::paint(p, Rect::from_center_size(rect.center(), vec2(18.0, 18.0)), icon, tint);
    if menu {
        flyout_mark(p, rect, tint);
    }
    resp.on_hover_text(label);
}

fn row(icon: Icon, name: &'static str, checked: bool) -> Row {
    Row { icon, name, keys: String::new(), checked, enabled: true }
}

/// The Edit Toolbar drawer, a floating panel at `at` beside the column: drag
/// a bright tile onto the toolbar to add its tools, drag a button off the
/// toolbar into the drawer to remove it. Its ≡ menu resets the toolbar.
fn drawer(ui: &Ui, ed: &mut Editor, column: Rect, at: Pos2) {
    let ctx = ui.ctx().clone();
    let menu_id = Id::new("edit-toolbar-menu");
    let area = egui::Area::new(Id::new("edit-toolbar-drawer")).order(Order::Foreground).fixed_pos(at).show(&ctx, |ui| {
        let frame = Frame::new().fill(theme::PANEL_BG).stroke(Stroke::new(1.0, DRAWER_BORDER)).corner_radius(4).shadow(ui.style().visuals.popup_shadow);
        frame.show(ui, |ui| {
            let (rect, _) = ui.allocate_exact_size(vec2(238.0, 623.0), Sense::click());
            let p = ui.painter();
            p.text(pos2(rect.left() + 15.0, rect.top() + 23.0), Align2::LEFT_CENTER, "Drag and drop tools", FontId::proportional(13.0), DRAWER_TITLE);
            let glyph = Rect::from_min_size(pos2(rect.right() - 25.0, rect.top() + 20.0), vec2(10.0, 7.0));
            let menu = ui.interact(glyph.expand(7.0), menu_id, Sense::click());
            let color = if menu.hovered() { theme::TEXT } else { theme::TEXT_DIM };
            for i in 0..4 {
                p.hline(glyph.x_range(), glyph.top() + 0.5 + 2.0 * i as f32, Stroke::new(1.0, color));
            }
            p.hline(rect.x_range(), rect.top() + 47.5, Stroke::new(1.0, DRAWER_DIVIDER));
            let grid = pos2(rect.left() + 15.0, rect.top() + 64.0);
            for &(c, r, tools) in DRAWER {
                let size = vec2(40.0, 56.0 * tools.len() as f32 - 16.0);
                let tile = Rect::from_min_size(grid + vec2(56.0 * c as f32, 56.0 * r as f32), size);
                let on = ed.toolbar.find(tools[0]).is_some();
                let resp = ui.interact(tile, Id::new(("drawer-tile", tools[0])), if on { Sense::hover() } else { Sense::drag() });
                resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, !on, tools[0].name()));
                if !on {
                    resp.dnd_set_drag_payload(Dragged(tools));
                }
                p.rect_filled(tile, 4.0, if on { TILE_ON } else { TILE_ADD });
                if !on && resp.hovered() {
                    p.rect_stroke(tile, 4.0, Stroke::new(2.0, theme::ACCENT), StrokeKind::Inside);
                }
                for (k, &tool) in tools.iter().enumerate() {
                    let cell = Rect::from_min_size(tile.min + vec2(0.0, 56.0 * k as f32), vec2(40.0, 40.0));
                    icons::paint(p, Rect::from_center_size(cell.center(), vec2(18.0, 18.0)), tool.icon(), if on { TILE_ON_ICON } else { theme::TEXT });
                    if !tool.ready() {
                        badge(p, cell.center() + vec2(-11.0, 11.0));
                    }
                    if ui.rect_contains_pointer(cell) {
                        resp.clone().on_hover_text(tool.tooltip());
                    }
                }
            }
            // ponytail: the Spacer (a separator dragged onto the toolbar) isn't built; its tile shows dim.
            let spacer = Rect::from_min_size(grid + vec2(0.0, 56.0 * 9.0), vec2(152.0, 40.0));
            p.rect_filled(spacer, 4.0, TILE_ON);
            icons::paint(p, Rect::from_center_size(spacer.center(), vec2(18.0, 18.0)), Icon::Spacer, TILE_ON_ICON);
            Popup::menu(&menu).style(theme::menu_style).show(|ui| {
                if theme::menu_item(ui, true, "Reset", "").clicked() {
                    ed.toolbar = Toolbar { drawer: true, ..Default::default() };
                }
            });
        });
    });
    // A button dragged off the toolbar and dropped here leaves it.
    if let Some(d) = area.response.dnd_release_payload::<Dragged>() {
        ed.toolbar.remove(d.0);
    }
    // Escape, or a press outside the drawer and the column, closes it.
    let inside = area.response.rect.union(column);
    let outside = ctx.input(|i| i.events.iter().any(|e| matches!(e, Event::PointerButton { pos, pressed: true, .. } if !inside.contains(*pos))));
    // (Its ≡ menu is a popup of its own, outside it.)
    if ctx.input(|i| i.key_down(Key::Escape)) || (outside && !Popup::is_id_open(&ctx, menu_id.with("popup"))) {
        ed.toolbar.drawer = false;
    }
}

/// The yellow warning badge, about 11×10 with a dark "!", its lower left at `at`.
fn badge(p: &Painter, at: Pos2) {
    p.add(Shape::convex_polygon(vec![at, at + vec2(11.0, 0.0), at + vec2(5.5, -10.0)], theme::WARNING, Stroke::NONE));
    let dark = Color32::from_gray(0x1E);
    p.rect_filled(Rect::from_min_size(at + vec2(5.0, -7.0), vec2(1.0, 3.5)), 0.0, dark);
    p.rect_filled(Rect::from_min_size(at + vec2(5.0, -2.5), vec2(1.0, 1.0)), 0.0, dark);
}

#[cfg(test)]
mod tests {
    use eframe::egui::accesskit::Role;
    use eframe::egui::{Key, Modifiers};
    use egui_kittest::kittest::Queryable;
    use minnamate_core::doc::Element;

    use super::*;
    use crate::app::testkit::*;

    fn press(h: &mut Harness<'_, Editor>, mods: Modifiers, key: Key) {
        h.key_press_modifiers(mods, key);
        h.step();
    }

    #[test]
    fn a_flyout_picks_a_tool_and_its_button_shows_it() {
        let mut h = blank();
        // Right-click opens the Rectangle group's flyout, listing the whole group.
        h.get_by_role_and_label(Role::Button, "Rectangle Tool").click_secondary();
        h.run();
        assert!(h.query_by_role_and_label(Role::Button, "Oval Primitive Tool").is_some());
        h.get_by_role_and_label(Role::Button, "Oval Tool").click();
        h.run();
        let ed = h.state();
        assert_eq!(ed.tool, Oval);
        let (s, i) = ed.toolbar.find(Rectangle).unwrap();
        assert_eq!(ed.toolbar.sections[s][i].shown, Oval, "the group button shows the tool used last");
        assert!(h.query_by_role_and_label(Role::Button, "PolyStar Tool").is_none(), "picking closes the flyout");

        // Holding the Lasso button for half a second opens its flyout too.
        let lasso = h.get_by_role_and_label(Role::Button, "Lasso Tool").rect().center();
        h.hover_at(lasso);
        h.step();
        h.drag_at(lasso);
        for _ in 0..20 {
            h.step();
        }
        assert!(h.query_by_role_and_label(Role::Button, "Polygon Tool").is_none(), "not yet");
        for _ in 0..20 {
            h.step();
        }
        h.drop_at(lasso);
        h.step();
        assert_eq!(h.state().tool, Lasso, "the press picked the tool the button shows");
        // Polygon isn't built: it shows, dimmed, and a click does nothing.
        h.get_by_role_and_label(Role::Button, "Polygon Tool").click();
        h.run();
        assert_eq!(h.state().tool, Lasso);
    }

    #[test]
    fn alt_letters_are_the_menus_not_the_tools() {
        let mut h = blank();
        let colors = (h.state().stroke_color, h.state().fill_color);
        for key in [Key::E, Key::I, Key::T, Key::O, Key::H, Key::X] {
            press(&mut h, Modifiers::ALT, key);
        }
        let ed = h.state();
        assert_eq!((ed.tool, (ed.stroke_color, ed.fill_color)), (Selection, colors));
    }

    #[test]
    fn escape_closes_the_drawer_and_keeps_the_selection() {
        let mut h = blank();
        h.state_mut().tool = Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.state_mut().tool = Selection;
        mouse_click(&mut h, (150.0, 125.0));
        h.get_by_role_and_label(Role::Button, "Edit Toolbar...").click();
        h.run();
        assert!(h.state().toolbar.drawer);
        press(&mut h, Modifiers::NONE, Key::Escape);
        let ed = h.state();
        assert_eq!((ed.toolbar.drawer, ed.selection.len()), (false, 1));
    }

    #[test]
    fn every_tool_has_its_own_key() {
        let mut h = blank();
        let (none, shift) = (Modifiers::NONE, Modifiers::SHIFT);
        let keys = [
            (none, Key::R, Rectangle),
            (none, Key::O, Oval),
            (none, Key::N, Line),
            (none, Key::B, Brush),
            (none, Key::E, Eraser),
            (none, Key::K, PaintBucket),
            (none, Key::I, Eyedropper),
            (none, Key::T, Text),
            (none, Key::P, Pen),
            (none, Key::L, Lasso),
            (none, Key::Q, FreeTransform),
            (shift, Key::Y, Pencil),
            (none, Key::H, Hand),
            (none, Key::Z, Zoom),
            (none, Key::V, Selection),
        ];
        for (mods, key, tool) in keys {
            press(&mut h, mods, key);
            assert_eq!(h.state().tool, tool, "{mods:?} {key:?}");
        }
        // Keys of tools minnamate lacks do nothing, rather than fall through to the plain key.
        for (mods, key) in [(shift, Key::R), (shift, Key::B), (shift, Key::H), (Modifiers::ALT.plus(shift), Key::H), (none, Key::Y)] {
            press(&mut h, mods, key);
            assert_eq!(h.state().tool, Selection, "{mods:?} {key:?}");
        }
        // Flyouts and tooltips write them as Windows does.
        assert_eq!([TimeScrub.keys(), AddAnchor.keys(), RectanglePrimitive.keys(), PolyStar.keys()], ["Alt+Shift+H", "=", "Shift+R", ""]);
        // A key picks a group's member, and the group's button shows it.
        press(&mut h, none, Key::S);
        let ed = h.state();
        let (s, i) = ed.toolbar.find(PaintBucket).unwrap();
        assert_eq!((ed.tool, ed.toolbar.sections[s][i].shown), (InkBottle, InkBottle));

        // D: black stroke, white fill; X swaps them.
        press(&mut h, none, Key::D);
        assert_eq!((h.state().stroke_color, h.state().fill_color), (Color32::BLACK, Color32::WHITE));
        press(&mut h, none, Key::X);
        assert_eq!((h.state().stroke_color, h.state().fill_color), (Color32::WHITE, Color32::BLACK));
        // J turns Object Drawing on: a rectangle drawn now is a drawing object.
        press(&mut h, none, Key::J);
        press(&mut h, none, Key::R);
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let ed = h.state();
        let Some(Element::Shape(shape)) = ed.timeline().layers[0].frames[0].elements.first() else { panic!("no rectangle") };
        assert_eq!(shape.extra.get("isDrawingObject"), Some("true"));
    }

    #[test]
    fn a_pen_path_ended_by_a_tool_key_is_a_drawing_object() {
        let mut h = blank();
        press(&mut h, Modifiers::NONE, Key::J);
        press(&mut h, Modifiers::NONE, Key::P);
        mouse_click(&mut h, (100.0, 100.0));
        mouse_click(&mut h, (200.0, 150.0));
        // V picks the Selection tool before the stage ends the path.
        press(&mut h, Modifiers::NONE, Key::V);
        let ed = h.state();
        assert_eq!(ed.tool, Selection);
        let Some(Element::Shape(path)) = ed.timeline().layers[0].frames[0].elements.first() else { panic!("no path") };
        assert_eq!(path.extra.get("isDrawingObject"), Some("true"));
    }

    #[test]
    fn the_whole_column_fits_and_a_short_window_doubles_it() {
        let mut h = blank();
        press(&mut h, Modifiers::NONE, Key::K);
        let rect = |h: &Harness<'_, Editor>, label: &str| h.get_by_role_and_label(Role::Button, label).rect();
        // 1600×900: one column, down to the Paint Bucket's last option.
        assert_eq!(rect(&h, "Lasso Tool").left(), rect(&h, "Selection Tool").left());
        assert!(rect(&h, "Lock Fill").bottom() <= 900.0);
        // At the window's minimum height the buttons go two abreast, and all still fits.
        h.set_size(vec2(1600.0, 600.0));
        h.run();
        let (select, transform) = (rect(&h, "Selection Tool"), rect(&h, "Free Transform Tool"));
        assert_eq!((transform.top(), transform.left() - select.left()), (select.top(), COLUMN_PITCH));
        assert!(rect(&h, "Lock Fill").bottom() <= 600.0 && rect(&h, "Tools stroke color").bottom() <= 600.0);
    }

    #[test]
    fn the_columns_start_a_pixel_under_the_app_bar() {
        // The window's 1 px line under the app bar, then the columns (docs/animate-ui.md
        // §4.1): Selection at T+35 (§6.2); Back under the document tabs, centered in the
        // 30 px edit bar (§4.4).
        let h = blank();
        assert_eq!(h.get_by_role_and_label(Role::Button, "Selection Tool").rect().top(), theme::APP_BAR_H + 35.0);
        assert_eq!(h.get_by_label("Back").rect().top(), theme::APP_BAR_H + 1.0 + theme::DOC_TABS_H + 3.0);
    }

    #[test]
    fn the_drawer_adds_moves_and_removes_buttons() {
        let mut t = Toolbar::default();
        assert_eq!(t.sections.iter().map(Vec::len).collect::<Vec<_>>(), [3, 5, 5, 2]);
        t.insert(group(Width), (1, 2));
        assert_eq!((t.find(Width), t.find(Eraser)), (Some((1, 2)), Some((1, 3))));
        // Moving a button keeps the tool it shows.
        t.sections[0][0].shown = Subselection;
        t.insert(group(Selection), (0, 2));
        assert_eq!((t.find(Selection), t.find(Lasso)), (Some((0, 1)), Some((0, 2))));
        assert_eq!(t.sections[0][1].shown, Subselection);
        // Emptying a section drops it, and its separator with it.
        t.remove(group(Hand));
        t.remove(group(Zoom));
        assert_eq!(t.sections.len(), 3);
        assert!(t.find(Hand).is_none());
    }

    fn drag(h: &mut Harness<'_, Editor>, from: Pos2, to: Pos2) {
        h.hover_at(from);
        h.step();
        h.drag_at(from);
        h.step();
        for k in 1..=4 {
            h.hover_at(from.lerp(to, k as f32 / 4.0));
            h.step();
        }
        h.drop_at(to);
        h.step();
    }

    #[test]
    fn tiles_drag_onto_the_toolbar_and_buttons_back_off_it() {
        let mut h = blank();
        // Tiles share their labels with the buttons; the button is the one further left.
        let button = |h: &Harness<'_, Editor>, label: &str| {
            h.query_all_by_role_and_label(Role::Button, label).map(|n| n.rect()).min_by(|a, b| a.left().total_cmp(&b.left())).unwrap()
        };
        h.get_by_role_and_label(Role::Button, "Edit Toolbar...").click();
        h.run();
        // The Width tile, dropped on the lower half of the Eraser button, goes in after it.
        let tile = h.get_by_role_and_label(Role::Button, "Width Tool").rect().center();
        let to = button(&h, "Eraser Tool").center() + vec2(0.0, 6.0);
        drag(&mut h, tile, to);
        assert_eq!(h.state().toolbar.find(Width), Some((1, 3)));
        // Dragged back into the drawer, it leaves the toolbar.
        let from = button(&h, "Width Tool").center();
        drag(&mut h, from, pos2(200.0, 400.0));
        assert_eq!(h.state().toolbar.find(Width), None);
        assert!(h.state().toolbar.drawer);
    }

    /// Writes target/screenshots/tools.png: the whole window, the Classic Brush
    /// picked and Object Drawing on; and tools_short.png, the window at its
    /// minimum height.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn tools_screenshot() {
        let mut h = harness(true);
        h.run();
        h.key_press(Key::B);
        h.key_press(Key::J);
        h.run();
        screenshot(&mut h, "tools");
        h.set_size(vec2(960.0, 600.0));
        h.run();
        screenshot(&mut h, "tools_short");
    }

    /// Writes target/screenshots/tools_flyout.png, the Rectangle group's flyout
    /// open, and tools_drawer.png, the Edit Toolbar drawer.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn tools_flyout_screenshot() {
        let mut h = harness(true);
        h.run();
        h.get_by_role_and_label(Role::Button, "Rectangle Tool").click_secondary();
        h.run();
        h.hover_at(h.get_by_role_and_label(Role::Button, "Oval Tool").rect().center());
        h.run();
        screenshot(&mut h, "tools_flyout");
        h.key_press(Key::Escape);
        h.run();
        h.get_by_role_and_label(Role::Button, "Edit Toolbar...").click();
        h.run();
        screenshot(&mut h, "tools_drawer");
    }
}
