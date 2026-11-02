//! The Home screen, File > New's New Document dialog and Modify > Document's
//! Document Settings (docs/animate-ui-interaction.md §4). Home and New Document
//! are Spectrum web pages inside Animate: darker than the panels, pill buttons,
//! uppercase headings. Document Settings is a native dialog in the panels' style.

use eframe::egui::{
    self, Align, Align2, Color32, ComboBox, DragValue, FontId, Frame, Grid, Id, Key, Layout, Margin, Rect, Response,
    RichText, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2,
};
use minnamate_core::doc::{Document, Element, Matrix, Rgba};
use minnamate_core::hit;

use crate::app::{Action, Editor, PropsTab};
use crate::icons::{self, Icon};
use crate::theme;

/// New Document's tab strip, and the margin round its Details panel.
const STRIP: Color32 = Color32::from_rgb(0x1C, 0x1C, 0x1C);
/// Outline of New Document's fields.
const FIELD_BORDER: Color32 = Color32::from_rgb(0x36, 0x36, 0x36);
/// Icons of New Document's other tabs.
const TAB_ICON: Color32 = Color32::from_rgb(0x80, 0x80, 0x80);
const TITLE_H: f32 = 32.0;

/// New Document's tabs: name, icon and presets (name, width, height, fps).
/// The last, Advanced, lists `PLATFORMS` instead.
// ponytail: only Character Animation's presets are known (§4.3); the other tabs
// hold stand-in sizes and rates until they're read off a v24 install.
const TABS: [(&str, Icon, &[(&str, f32, f32, f32)]); 7] = [
    ("Character Animation", Icon::NdCharacter, &[
        ("HD", 1280.0, 720.0, 30.0),
        ("Full HD", 1920.0, 1080.0, 30.0),
        ("4K", 3840.0, 2160.0, 30.0),
        ("Standard", 640.0, 480.0, 30.0),
    ]),
    ("Social", Icon::NdSocial, &[
        ("In-stream Photo", 1024.0, 512.0, 30.0),
        ("Square", 1080.0, 1080.0, 30.0),
        ("Landscape", 1200.0, 630.0, 30.0),
        ("HD Video", 1920.0, 1080.0, 30.0),
    ]),
    ("Game", Icon::NdGame, &[
        ("Android 16:9", 1920.0, 1080.0, 30.0),
        ("Phone Portrait", 1080.0, 1920.0, 30.0),
        ("Tablet", 2048.0, 1536.0, 30.0),
    ]),
    ("Education", Icon::NdEducation, &[
        ("iPad 3-4 Gen", 2048.0, 1536.0, 24.0),
        ("Standard", 1024.0, 768.0, 24.0),
        ("Widescreen", 1366.0, 768.0, 24.0),
    ]),
    ("Ads", Icon::NdAds, &[
        ("Square", 250.0, 250.0, 24.0),
        ("Banner", 468.0, 60.0, 24.0),
        ("Medium Rectangle", 300.0, 250.0, 24.0),
        ("Leaderboard", 728.0, 90.0, 24.0),
        ("Wide Skyscraper", 160.0, 600.0, 24.0),
    ]),
    ("Web", Icon::NdWeb, &[
        ("Standard", 640.0, 480.0, 24.0),
        ("Desktop", 1024.0, 768.0, 24.0),
        ("Wide", 1280.0, 800.0, 24.0),
    ]),
    ("Advanced", Icon::NdAdvanced, &[]),
];
const ADVANCED: usize = 6;

/// New Document > Advanced: name, glyph, and whether minnamate makes it.
const PLATFORMS: [(&str, Icon, bool); 2] = [("HTML5 Canvas", Icon::NdCanvas, true), ("ActionScript 3.0", Icon::NdActionScript, true)];
const CANVAS: usize = 0;
const AS3: usize = 1;
const AS3_ABOUT: &str = "Vector animation with symbols and scripts, published as a SWF movie.";
const CANVAS_ABOUT: &str = "Animation and JavaScript frame scripts, published as a web page that draws on a canvas.";
/// The Ads tab, whose presets are HTML5 Canvas documents.
const ADS: usize = 4;

/// Home's quick-start tiles: name, glyph, the New Document tab and tile each
/// stands for, and whether a click creates it straight away (§4.7).
const QUICK: [(&str, Icon, usize, usize, bool); 6] = [
    ("Full HD", Icon::HomeFullHd, 0, 1, true),
    ("Android 16:9", Icon::HomeGamepad, 2, 0, false),
    ("Banner", Icon::HomeBanner, 4, 1, false),
    ("iPad 3-4 Gen", Icon::HomeTablet, 3, 0, false),
    ("Canvas", Icon::HomeCanvas, ADVANCED, CANVAS, false),
    ("More Presets", Icon::HomeMore, 0, 0, false),
];

/// Home and the document dialogs, kept in `Editor::home`.
#[derive(Default)]
pub struct State {
    /// The Home screen is up, in place of the workspace below the app bar.
    pub shown: bool,
    /// A document is open (behind Home while it's up); false after File > Close.
    pub doc_open: bool,
    /// Document Settings' "Apply to pasteboard": the pasteboard takes the stage colour.
    // ponytail: this and `units` last the session and don't go into the .fla:
    // their XFL attributes aren't in any file we saved ourselves.
    pub apply_to_pasteboard: bool,
    /// The units Document Settings shows sizes in.
    units: Units,
    /// What Make Default saved: New Document > Advanced starts from it.
    // ponytail: forgotten at exit; save it beside recent.txt if that bites.
    defaults: Setup,
    new_doc: Option<NewDoc>,
    settings: Option<Settings>,
    /// What Create or a Home tile is opening, waiting on "Save changes?".
    pending: Option<Setup>,
    /// And whether it's an HTML5 Canvas document.
    pending_canvas: bool,
    /// How the last frame ended: Home up, a dialog up, the document's revision.
    drawn: Option<(bool, bool, u32)>,
}

impl State {
    /// Whether `action` can run now: Home hides the document, so there only
    /// the commands that open, create, save or close one work.
    pub fn allows(&self, action: Action) -> bool {
        match action {
            Action::New | Action::CreateDocument | Action::Open | Action::OpenDropped | Action::OpenRecent(_) | Action::Exit | Action::About => true,
            Action::Close | Action::Save | Action::SaveAs | Action::ShowDocument => self.doc_open,
            _ => !self.shown,
        }
    }

    /// New Document or Document Settings is up.
    pub fn dialog_open(&self) -> bool {
        self.new_doc.is_some() || self.settings.is_some()
    }
}

/// A new document's size in pixels, frame rate and units.
#[derive(Clone, Copy, PartialEq, Debug)]
struct Setup {
    w: f32,
    h: f32,
    fps: f32,
    units: Units,
}

impl Default for Setup {
    /// New Document > Advanced's factory values.
    fn default() -> Self {
        Setup { w: 550.0, h: 400.0, fps: 24.0, units: Units::Pixels }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
enum Units {
    Inches,
    InchesDecimal,
    Points,
    Centimeters,
    Millimeters,
    #[default]
    Pixels,
}

impl Units {
    const ALL: [Units; 6] = [Units::Inches, Units::InchesDecimal, Units::Points, Units::Centimeters, Units::Millimeters, Units::Pixels];

    fn name(self) -> &'static str {
        match self {
            Units::Inches => "Inches",
            Units::InchesDecimal => "Inches (decimal)",
            Units::Points => "Points",
            Units::Centimeters => "Centimeters",
            Units::Millimeters => "Millimeters",
            Units::Pixels => "Pixels",
        }
    }

    /// Pixels per unit, at Animate's 72 pixels per inch.
    // ponytail: Inches shows decimals like Inches (decimal), not fractions.
    fn px(self) -> f32 {
        match self {
            Units::Inches | Units::InchesDecimal => 72.0,
            Units::Points | Units::Pixels => 1.0,
            Units::Centimeters => 72.0 / 2.54,
            Units::Millimeters => 72.0 / 25.4,
        }
    }

    /// `px` pixels in these units, as a field shows them.
    fn show(self, px: f32) -> String {
        let v = px / self.px();
        if v.fract() == 0.0 { format!("{v}") } else { format!("{v:.2}") }
    }
}

/// The New Document dialog: the open tab, its selected tile and the Details fields.
struct NewDoc {
    tab: usize,
    tile: usize,
    /// Width and height in `units`, and frames per second, as typed.
    w: String,
    h: String,
    fps: String,
    units: Units,
    /// The Platform Type dropdown (the preset tabs'): HTML5 Canvas rather than ActionScript 3.0.
    canvas: bool,
}

impl NewDoc {
    /// The selected tile's values; Advanced's come from Make Default.
    fn tile_setup(&self, defaults: Setup) -> Setup {
        match TABS[self.tab].2.get(self.tile) {
            Some(&(_, w, h, fps)) => Setup { w, h, fps, units: Units::Pixels },
            None => defaults,
        }
    }

    /// Selects `tile` of `tab` and loads its values into Details.
    fn select(&mut self, tab: usize, tile: usize, defaults: Setup) {
        // A tab brings its platform: the Ads presets are HTML5 Canvas documents.
        if tab != self.tab {
            self.canvas = tab == ADS;
        }
        (self.tab, self.tile) = (tab, tile);
        let s = self.tile_setup(defaults);
        (self.w, self.h, self.fps, self.units) = (s.units.show(s.w), s.units.show(s.h), format!("{:.2}", s.fps), s.units);
    }

    /// Whether it makes an HTML5 Canvas document: the Advanced tab's tile, or the dropdown.
    fn is_canvas(&self) -> bool {
        if self.tab == ADVANCED { self.tile == CANVAS } else { self.canvas }
    }

    /// Details as typed, kept in range; the tile's own value where a field doesn't parse.
    fn typed(&self, defaults: Setup) -> Setup {
        let base = self.tile_setup(defaults);
        let size = |text: &str, tile: f32| number(text).map_or(tile, |v| (v * self.units.px()).round()).clamp(1.0, 8192.0);
        Setup {
            w: size(&self.w, base.w),
            h: size(&self.h, base.h),
            fps: number(&self.fps).unwrap_or(base.fps).clamp(0.01, 120.0),
            units: self.units,
        }
    }
}

fn number(text: &str) -> Option<f32> {
    text.trim().replace(',', "").parse::<f32>().ok().filter(|v| v.is_finite())
}

/// The Document Settings dialog; sizes in `units`.
struct Settings {
    units: Units,
    w: f32,
    h: f32,
    link: bool,
    scale: bool,
    /// The cell the stage grows away from: column and row, 0..=2.
    anchor: (u8, u8),
    color: Color32,
    pasteboard: bool,
    fps: f32,
}

/// File > New and Home's Create New: New Document, with `tab`'s `tile` selected.
pub fn open_new(ed: &mut Editor, tab: usize, tile: usize) {
    let mut nd = NewDoc { tab, tile, w: String::new(), h: String::new(), fps: String::new(), units: Units::Pixels, canvas: tab == ADS };
    nd.select(tab, tile, ed.home.defaults);
    ed.home.new_doc = Some(nd);
}

/// Opens `setup` as Untitled-N, an HTML5 Canvas document if `canvas`, once "Save changes?"
/// lets the open document go.
fn create(ed: &mut Editor, ctx: &egui::Context, setup: Setup, canvas: bool) {
    ed.home.new_doc = None;
    (ed.home.pending, ed.home.pending_canvas) = (Some(setup), canvas);
    ed.run(Action::CreateDocument, ctx);
}

/// Action::CreateDocument: opens what Create or a Home tile asked for, with
/// Properties on the Doc tab as Animate shows a new document.
pub fn create_pending(ed: &mut Editor) {
    if let Some(s) = ed.home.pending.take() {
        ed.new_document(s.w, s.h, s.fps);
        ed.doc.set_canvas(std::mem::take(&mut ed.home.pending_canvas));
        (ed.home.units, ed.props_tab) = (s.units, PropsTab::Doc);
    }
}

/// File > Close: back to Home, with no document open.
pub fn close(ed: &mut Editor) {
    ed.load(Document::new(550.0, 400.0, 24.0), String::new(), None);
    ed.test = None;
    (ed.home.shown, ed.home.doc_open) = (true, false);
}

/// The app bar's Home button: shows Home, or goes back to the open document.
pub fn toggle(ed: &mut Editor) {
    ed.home.shown = !ed.home.shown || !ed.home.doc_open;
}

/// Modify > Document (Ctrl+J): Document Settings, filled from the document.
pub fn open_settings(ed: &mut Editor) {
    let (doc, units) = (&ed.doc, ed.home.units);
    ed.home.settings = Some(Settings {
        units,
        w: doc.width / units.px(),
        h: doc.height / units.px(),
        link: false,
        scale: false,
        anchor: (1, 1),
        color: color32(doc.background),
        pasteboard: ed.home.apply_to_pasteboard,
        fps: doc.frame_rate,
    });
}

fn color32(c: Rgba) -> Color32 {
    let [r, g, b, _] = c.map(|v| (v * 255.0).round() as u8);
    Color32::from_rgb(r, g, b)
}

/// The Home screen below the app bar: Create New and Open in a rail at the
/// left, the quick-start tiles, then the recent files.
pub fn screen(ui: &mut Ui, ed: &mut Editor) {
    let r = ui.max_rect();
    let p = ui.painter_at(r);
    p.rect_filled(r, 0.0, theme::APP_BG);
    let rail = Rect::from_min_size(r.min, vec2(216.0, r.height()));
    p.vline(rail.right(), rail.y_range(), Stroke::new(1.0, theme::BAR));
    if push(ui, Rect::from_min_size(rail.min + vec2(20.0, 24.0), vec2(176.0, 32.0)), "Create New", true, 16.0).clicked() {
        open_new(ed, 0, 0);
    }
    if push(ui, Rect::from_min_size(rail.min + vec2(20.0, 68.0), vec2(176.0, 32.0)), "Open", false, 16.0).clicked() {
        ed.run(Action::Open, ui.ctx());
    }

    // "Start a new file fast": bare glyphs, a box behind the one under the pointer.
    let cx = (rail.right() + r.right()) / 2.0;
    let top = r.top() + 56.0;
    p.text(pos2(cx, top), Align2::CENTER_TOP, "Start a new file fast", theme::semibold(16.0), Color32::WHITE);
    let glyphs = top + 69.0;
    for (i, &(name, icon, tab, tile, now)) in QUICK.iter().enumerate() {
        let x = cx + (i as f32 - 2.5) * 98.0;
        let rect = Rect::from_min_max(pos2(x - 46.0, glyphs - 16.0), pos2(x + 46.0, glyphs + 100.0));
        let resp = ui.interact(rect, ui.id().with(("quick", i)), Sense::click());
        if resp.hovered() {
            p.rect_filled(rect, 4.0, theme::BUTTON);
        }
        icons::paint(&p, Rect::from_center_size(pos2(x, glyphs + 25.0), vec2(72.0, 72.0)), icon, theme::TEXT);
        p.text(pos2(x, glyphs + 70.0), Align2::CENTER_TOP, name, theme::semibold(12.0), Color32::WHITE);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, name));
        if resp.clicked() {
            match TABS[tab].2.get(tile) {
                Some(&(_, w, h, fps)) if now => create(ed, ui.ctx(), Setup { w, h, fps, units: Units::Pixels }, tab == ADS),
                _ => open_new(ed, tab, tile),
            }
        }
    }

    // Recent files, newest first, as in File > Open Recent.
    let left = cx - 360.0;
    let mut y = glyphs + 156.0;
    p.text(pos2(left, y), Align2::LEFT_TOP, "Recent", theme::semibold(16.0), Color32::WHITE);
    y += 36.0;
    if ed.recent.is_empty() {
        p.text(pos2(left + 12.0, y + 8.0), Align2::LEFT_TOP, "Documents you open show up here.", FontId::proportional(13.0), theme::TEXT_DIM);
    }
    let mut picked = None;
    for (i, path) in ed.recent.iter().enumerate() {
        let row = Rect::from_min_size(pos2(left, y + i as f32 * 32.0), vec2(720.0, 32.0));
        let resp = ui.interact(row, ui.id().with(("recent", i)), Sense::click());
        if resp.hovered() {
            p.rect_filled(row, 4.0, theme::BUTTON);
        }
        icons::paint(&p, Rect::from_center_size(row.left_center() + vec2(20.0, 0.0), vec2(16.0, 16.0)), Icon::Layer, theme::TEXT_DIM);
        let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
        p.text(row.left_center() + vec2(40.0, 0.0), Align2::LEFT_CENTER, &name, FontId::proportional(13.0), theme::TEXT);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &name));
        if resp.on_hover_text(path.display().to_string()).clicked() {
            picked = Some(i);
        }
    }
    if let Some(i) = picked {
        ed.run(Action::OpenRecent(i), ui.ctx());
    }
}

/// New Document and Document Settings, while they're up. This runs last in
/// the frame, so it also asks for another frame when Home, a dialog or the
/// document changed after the workspace was drawn (a click shows no stale frame).
pub fn dialogs(ctx: &egui::Context, ed: &mut Editor) {
    new_document(ctx, ed);
    document_settings(ctx, ed);
    let now = (ed.home.shown, ed.home.dialog_open(), ed.rev);
    if ed.home.drawn.replace(now) != Some(now) {
        ctx.request_repaint();
    }
}

/// A dialog window: our own title bar sits inside the 1 px outline.
pub(crate) fn dialog(id: &str, ctx: &egui::Context, fill: Color32) -> egui::Modal {
    let frame = Frame::NONE.fill(fill).inner_margin(1).stroke(Stroke::new(1.0, theme::POPUP_BORDER)).shadow(ctx.global_style().visuals.popup_shadow);
    // Native dialogs don't dim the window behind them.
    egui::Modal::new(Id::new(id)).frame(frame).backdrop_color(Color32::TRANSPARENT)
}

/// A dialog's title bar, like the app bar: the title, and a close box that
/// turns red under the pointer. Returns whether the close box was clicked.
pub(crate) fn title_bar(ui: &mut Ui, title: &str, width: f32) -> bool {
    let (rect, _) = ui.allocate_exact_size(vec2(width, TITLE_H), Sense::hover());
    let close = Rect::from_min_max(pos2(rect.right() - 46.0, rect.top()), rect.max);
    let resp = ui.interact(close, ui.id().with((title, "close")), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, theme::BAR);
    p.text(rect.left_center() + vec2(12.0, 0.0), Align2::LEFT_CENTER, title, FontId::proportional(12.0), theme::TEXT);
    if resp.hovered() {
        p.rect_filled(close, 0.0, theme::CLOSE_HOVER);
    }
    let color = if resp.hovered() { Color32::WHITE } else { theme::TEXT };
    icons::paint(p, Rect::from_center_size(close.center(), vec2(14.0, 14.0)), Icon::WinClose, color);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, "Close"));
    resp.clicked()
}

/// A push button drawn in `rect`: blue for the main action, else outlined.
/// Pills (`radius` = half the height) on the web-style pages.
pub(crate) fn push(ui: &mut Ui, rect: Rect, label: &str, cta: bool, radius: f32) -> Response {
    let resp = ui.interact(rect, ui.id().with(("push", label)), Sense::click());
    let p = ui.painter();
    let down = resp.is_pointer_button_down_on();
    if cta {
        p.rect_filled(rect, radius, if down { theme::CTA_DOWN } else if resp.hovered() { theme::CTA_HOVER } else { theme::CTA });
    } else {
        if resp.hovered() {
            p.rect_filled(rect, radius, if down { theme::BUTTON_ACTIVE } else { theme::BUTTON_HOVER });
        }
        p.rect_stroke(rect, radius, Stroke::new(2.0, theme::TEXT_DIM), StrokeKind::Inside);
    }
    p.text(rect.center(), Align2::CENTER_CENTER, label, theme::semibold(13.0), Color32::WHITE);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, label));
    resp
}

/// File > New: tabs of presets on the left, the chosen one's Details on the
/// right, and Create; about 775 × 575 below its title bar.
fn new_document(ctx: &egui::Context, ed: &mut Editor) {
    let defaults = ed.home.defaults;
    let Some(nd) = &mut ed.home.new_doc else { return };
    // An open dropdown takes Enter and Escape for itself.
    let popup_open = egui::Popup::is_any_open(ctx);
    let (mut create_now, mut close, mut make_default) = (false, false, false);
    dialog("new-document", ctx, STRIP).show(ctx, |ui| {
        close = title_bar(ui, "New Document", 775.0);
        let (body, _) = ui.allocate_exact_size(vec2(775.0, 575.0), Sense::hover());
        let strip = Rect::from_min_size(body.min, vec2(body.width(), 109.0));
        if let Some(tab) = tabs(ui, strip, nd.tab).filter(|&tab| tab != nd.tab) {
            nd.select(tab, if tab == ADVANCED { AS3 } else { 0 }, defaults);
        }
        let pane = Rect::from_min_max(pos2(body.left(), strip.bottom()), pos2(body.left() + 502.0, body.bottom()));
        if let Some((tile, double)) = tiles(ui, pane, nd) {
            nd.select(nd.tab, tile, defaults);
            create_now = double;
        }
        let panel = Rect::from_min_max(pos2(pane.right(), strip.bottom()), body.max - vec2(15.0, 9.0));
        let (c, m) = details(ui, panel, nd, defaults);
        (create_now, make_default) = (create_now || c, m);
    });
    if !popup_open {
        create_now |= ctx.input(|i| i.key_pressed(Key::Enter));
        close |= ctx.input(|i| i.key_pressed(Key::Escape));
    }
    let typed = nd.typed(defaults);
    let canvas = nd.is_canvas();
    if make_default {
        ed.home.defaults = typed;
    }
    if close {
        ed.home.new_doc = None;
    } else if create_now {
        create(ed, ctx, typed, canvas);
    }
}

/// New Document's tab strip: an icon over each name, the open tab's name white
/// and underlined, Advanced apart at the right end. Returns a clicked tab.
fn tabs(ui: &mut Ui, strip: Rect, open: usize) -> Option<usize> {
    let p = ui.painter().clone();
    p.rect_filled(strip, 0.0, STRIP);
    let (mut x, mut clicked) = (strip.left() + 25.0, None);
    for (i, &(name, icon, _)) in TABS.iter().enumerate() {
        let galley = p.layout_no_wrap(name.to_string(), FontId::proportional(13.0), Color32::PLACEHOLDER);
        let w = galley.size().x;
        let left = if i == ADVANCED { strip.right() - 25.0 - w } else { x };
        x = left + w + 44.0;
        let resp = ui.interact(Rect::from_x_y_ranges(left..=left + w, strip.y_range()), ui.id().with(("nd-tab", i)), Sense::click());
        let on = i == open;
        let (text, glyph) = match (on, resp.hovered()) {
            (true, _) => (Color32::WHITE, Color32::WHITE),
            (_, true) => (theme::TEXT, theme::TEXT),
            _ => (theme::TEXT_DIM, TAB_ICON),
        };
        icons::paint(&p, Rect::from_center_size(pos2(left + w / 2.0, strip.top() + 50.0), vec2(30.0, 30.0)), icon, glyph);
        p.galley(pos2(left, strip.top() + 77.0), galley, text);
        if on {
            p.hline(left..=left + w, strip.bottom() - 1.0, Stroke::new(2.0, Color32::WHITE));
        }
        resp.widget_info(|| WidgetInfo::selected(WidgetType::SelectableLabel, true, on, name));
        if resp.clicked() {
            clicked = Some(i);
        }
    }
    clicked
}

/// New Document's left pane: the open tab's presets, three to a row
/// (Advanced: its platforms). Returns a clicked tile, and whether it was a double-click.
fn tiles(ui: &mut Ui, pane: Rect, nd: &NewDoc) -> Option<(usize, bool)> {
    let p = ui.painter().clone();
    p.rect_filled(pane, 0.0, theme::APP_BG);
    let presets = TABS[nd.tab].2;
    let (heading, count, size) = match nd.tab {
        ADVANCED => (format!("PLATFORMS ({})", PLATFORMS.len()), PLATFORMS.len(), vec2(135.0, 115.0)),
        _ => (format!("PRESETS ({})", presets.len()), presets.len(), vec2(136.0, 136.0)),
    };
    p.text(pane.min + vec2(26.0, 33.0), Align2::LEFT_TOP, heading, theme::semibold(11.0), theme::TEXT_DIM);
    let mut picked = None;
    // ponytail: no scrolling; every tab's list fits in two rows.
    for i in 0..count {
        let (col, row) = ((i % 3) as f32, (i / 3) as f32);
        let rect = Rect::from_min_size(pane.min + vec2(26.0 + col * (size.x + 18.0), 67.0 + row * (size.y + 14.0)), size);
        let selected = i == nd.tile;
        let resp = match presets.get(i) {
            Some(&(name, w, h, _)) => {
                let size = format!("{} x {}", thousands(w), thousands(h));
                tile(ui, rect, name, Icon::NdPreset, Some(&size), selected, true)
            }
            None => tile(ui, rect, PLATFORMS[i].0, PLATFORMS[i].1, None, selected, PLATFORMS[i].2),
        };
        if resp.clicked() {
            picked = Some((i, resp.double_clicked()));
        }
    }
    picked
}

/// "1,280", as New Document writes sizes.
fn thousands(v: f32) -> String {
    let digits = (v.round() as u64).to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// One New Document tile: glyph, name and size, a blue border when selected.
fn tile(ui: &mut Ui, rect: Rect, name: &str, icon: Icon, size: Option<&str>, selected: bool, enabled: bool) -> Response {
    let resp = ui.interact(rect, ui.id().with(("nd-tile", name)), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    p.rect_filled(rect, 4.0, if enabled && resp.hovered() && !selected { theme::BUTTON_HOVER } else { theme::BUTTON });
    if selected {
        p.rect_stroke(rect, 4.0, Stroke::new(2.0, theme::ACCENT), StrokeKind::Inside);
    }
    let (glyph, text) = if enabled { (theme::TEXT, Color32::WHITE) } else { (theme::TEXT_DISABLED, theme::TEXT_DISABLED) };
    // A preset's glyph starts 28 px down, its name over the size line; a
    // platform tile has no size line, so its name sits lower.
    let (glyph_y, name_y) = if size.is_some() { (53.0, 35.0) } else { (45.0, 14.0) };
    icons::paint(p, Rect::from_center_size(pos2(rect.center().x, rect.top() + glyph_y), vec2(72.0, 72.0)), icon, glyph);
    p.text(pos2(rect.center().x, rect.bottom() - name_y), Align2::CENTER_BOTTOM, name, theme::semibold(12.0), text);
    if let Some(size) = size {
        p.text(pos2(rect.center().x, rect.bottom() - 32.0), Align2::CENTER_TOP, size, FontId::proportional(12.0), theme::TEXT_DIM);
    }
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, selected, name));
    if enabled { resp } else { resp.on_hover_text("minnamate makes ActionScript 3.0 and HTML5 Canvas documents") }
}

/// New Document's Details panel: size, units, frame rate and platform, then
/// Create (and Make Default, on Advanced). Returns (Create, Make Default) clicked.
fn details(ui: &mut Ui, panel: Rect, nd: &mut NewDoc, defaults: Setup) -> (bool, bool) {
    let p = ui.painter().clone();
    p.rect_filled(panel, 0.0, theme::BUTTON);
    let advanced = nd.tab == ADVANCED;
    let left = panel.left() + 23.0;
    p.text(pos2(left, panel.top() + 23.0), Align2::LEFT_TOP, "DETAILS", theme::semibold(11.0), theme::TEXT_DIM);
    // Each label and its field take 70 px.
    let row = |i: f32| panel.top() + 55.0 + i * 70.0;
    let label = |x: f32, y: f32, text: &str| {
        p.text(pos2(x, y), Align2::LEFT_TOP, text, FontId::proportional(12.0), theme::TEXT_DIM);
    };
    let at = |x: f32, i: f32, w: f32| Rect::from_min_size(pos2(x, row(i) + 20.0), vec2(w, 31.0));
    label(left, row(0.0), "Width");
    label(left + 118.0, row(0.0), "Height");
    field(ui, at(left, 0.0, 93.0), "nd-width", &mut nd.w);
    field(ui, at(left + 118.0, 0.0, 93.0), "nd-height", &mut nd.h);

    label(left, row(1.0), "Units");
    let units = dropdown(ui, at(left, 1.0, 211.0), "nd-units", nd.units.name(), advanced);
    let mut picked = None;
    egui::Popup::menu(&units).show(|ui| {
        // As wide as the dropdown, less the popup's 4 px margins.
        ui.set_min_width(203.0);
        for u in Units::ALL {
            if ui.selectable_label(u == nd.units, u.name()).clicked() {
                picked = Some(u);
            }
        }
    });
    if let Some(u) = picked.filter(|&u| u != nd.units) {
        let typed = nd.typed(defaults);
        (nd.w, nd.h, nd.units) = (u.show(typed.w), u.show(typed.h), u);
    }

    label(left, row(2.0), "Frame Rate");
    field(ui, at(left, 2.0, 93.0), "nd-fps", &mut nd.fps);

    let platform = if nd.is_canvas() { CANVAS } else { AS3 };
    if advanced {
        // Advanced names the platform and says what it makes, instead of a dropdown.
        p.text(pos2(left, row(3.0)), Align2::LEFT_TOP, PLATFORMS[platform].0, FontId::proportional(13.0), theme::TEXT_DIM);
        let about = p.layout([CANVAS_ABOUT, AS3_ABOUT][platform].into(), FontId::proportional(12.0), theme::TEXT_DIM, 211.0);
        p.galley(pos2(left, row(3.0) + 22.0), about, theme::TEXT_DIM);
    } else {
        label(left, row(3.0), "Platform Type");
        let field = dropdown(ui, at(left, 3.0, 211.0), "nd-platform", PLATFORMS[platform].0, true);
        egui::Popup::menu(&field).show(|ui| {
            ui.set_min_width(203.0);
            for i in [AS3, CANVAS] {
                if ui.selectable_label(i == platform, PLATFORMS[i].0).clicked() {
                    nd.canvas = i == CANVAS;
                }
            }
        });
    }

    let create = Rect::from_min_size(panel.max - vec2(23.0 + 72.0, 23.0 + 31.0), vec2(72.0, 31.0));
    let make_default = advanced && push(ui, Rect::from_min_size(create.min - vec2(134.0, 0.0), vec2(122.0, 31.0)), "Make Default", false, 15.5).clicked();
    (push(ui, create, "Create", true, 15.5).clicked(), make_default)
}

/// A New Document text field: dark box, light value, blue outline while typing.
fn field(ui: &mut Ui, rect: Rect, id: &str, text: &mut String) -> Response {
    ui.painter().rect_filled(rect, 2.0, theme::INSET);
    let edit = egui::TextEdit::singleline(text)
        .id(Id::new(id))
        .frame(Frame::NONE.inner_margin(Margin::symmetric(9, 0)))
        .font(FontId::proportional(13.0))
        .text_color(theme::TEXT)
        .vertical_align(Align::Center)
        .min_size(rect.size());
    let resp = ui.put(rect, edit);
    let border = if resp.has_focus() { theme::ACCENT } else { FIELD_BORDER };
    ui.painter().rect_stroke(rect, 2.0, Stroke::new(1.0, border), StrokeKind::Inside);
    resp
}

/// A New Document dropdown: the value and a chevron in a field's box; greyed when off.
fn dropdown(ui: &mut Ui, rect: Rect, id: &str, value: &str, enabled: bool) -> Response {
    let resp = ui.interact(rect, Id::new(id), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    if enabled {
        p.rect_filled(rect, 2.0, theme::INSET);
    }
    let border = if enabled && resp.hovered() { theme::OUTLINE } else { FIELD_BORDER };
    p.rect_stroke(rect, 2.0, Stroke::new(1.0, border), StrokeKind::Inside);
    let color = if enabled { theme::TEXT } else { theme::TEXT_DISABLED };
    p.text(rect.left_center() + vec2(9.0, 0.0), Align2::LEFT_CENTER, value, FontId::proportional(13.0), color);
    icons::paint(p, Rect::from_center_size(rect.right_center() - vec2(15.0, 0.0), vec2(14.0, 14.0)), Icon::ChevronDown, color);
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ComboBox, enabled, value));
    resp
}

/// Modify > Document: units, stage size (scaling or anchoring the content),
/// stage colour and frame rate. OK applies them as one undo step.
fn document_settings(ctx: &egui::Context, ed: &mut Editor) {
    let Some(s) = &mut ed.home.settings else { return };
    let popup_open = egui::Popup::is_any_open(ctx);
    let (mut ok, mut cancel, mut make_default) = (false, false, false);
    dialog("document-settings", ctx, theme::PANEL_BG).show(ctx, |ui| {
        cancel = title_bar(ui, "Document Settings", 470.0);
        Frame::NONE.inner_margin(Margin { left: 20, right: 20, top: 18, bottom: 16 }).show(ui, |ui| {
            ui.set_width(430.0);
            Grid::new("document-settings").num_columns(2).spacing([16.0, 14.0]).show(ui, |ui| settings_rows(ui, s));
            ui.add_space(20.0);
            ui.horizontal(|ui| {
                make_default = ui.add(egui::Button::new("Make Default").min_size(vec2(0.0, 28.0))).clicked();
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (_, rect) = ui.allocate_space(vec2(72.0, 28.0));
                    ok = push(ui, rect, "OK", true, 4.0).clicked();
                    cancel |= ui.add(egui::Button::new("Cancel").min_size(vec2(72.0, 28.0))).clicked();
                });
            });
        });
    });
    if !popup_open {
        ok |= ctx.input(|i| i.key_pressed(Key::Enter));
        cancel |= ctx.input(|i| i.key_pressed(Key::Escape));
    }
    if make_default {
        let px = s.units.px();
        ed.home.defaults = Setup { w: (s.w * px).round(), h: (s.h * px).round(), fps: s.fps, units: s.units };
    }
    if cancel {
        ed.home.settings = None;
    } else if ok && let Some(s) = ed.home.settings.take() {
        apply(ed, s);
    }
}

/// Document Settings' rows: a label, then its controls.
fn settings_rows(ui: &mut Ui, s: &mut Settings) {
    let label = |ui: &mut Ui, text: &str| ui.label(RichText::new(text).color(theme::TEXT_DIM));
    label(ui, "Units");
    let mut units = s.units;
    ComboBox::from_id_salt("ds-units").selected_text(units.name()).width(150.0).show_ui(ui, |ui| {
        for u in Units::ALL {
            ui.selectable_value(&mut units, u, u.name());
        }
    });
    if units != s.units {
        let k = s.units.px() / units.px();
        (s.w, s.h, s.units) = (s.w * k, s.h * k, units);
    }
    ui.end_row();

    label(ui, "Stage size");
    ui.horizontal(|ui| {
        // Scale Content keeps the proportions, so it forces Link on.
        let (w, h, linked) = (s.w, s.h, s.link || s.scale);
        size_value(ui, &mut s.w, s.units);
        ui.label("W");
        let link = theme::icon_button(ui, 22.0, linked, !s.scale, "Link width and height", |p, r, c| icons::paint(p, r.shrink(3.0), Icon::Link, c));
        if link.clicked() {
            s.link = !s.link;
        }
        size_value(ui, &mut s.h, s.units);
        ui.label("H");
        ui.add_space(8.0);
        // ponytail: Match contents waits on which of its two recalled behaviours Animate has (§4.5).
        ui.add_enabled(false, egui::Button::new("Match contents"));
        if linked && s.w != w {
            s.h = h * s.w / w;
        } else if linked && s.h != h {
            s.w = w * s.h / h;
        }
    });
    ui.end_row();

    label(ui, "Scale");
    ui.checkbox(&mut s.scale, "Scale Content");
    ui.end_row();

    label(ui, "Anchor");
    ui.add_enabled_ui(!s.scale, |ui| crate::convert_symbol::registration_grid(ui, &mut s.anchor));
    ui.end_row();

    label(ui, "Stage color");
    ui.horizontal(|ui| {
        egui::color_picker::color_edit_button_srgba(ui, &mut s.color, egui::color_picker::Alpha::Opaque);
        ui.add_space(12.0);
        ui.checkbox(&mut s.pasteboard, "Apply to pasteboard");
    });
    ui.end_row();

    label(ui, "Frame rate");
    ui.add_sized(vec2(64.0, 22.0), DragValue::new(&mut s.fps).range(0.01..=120.0).speed(0.1).fixed_decimals(2));
    ui.end_row();
}

/// A stage-size field in `units`: whole pixels, or two decimals.
fn size_value(ui: &mut Ui, v: &mut f32, units: Units) {
    let px = units.px();
    let decimals = if units == Units::Pixels { 0 } else { 2 };
    ui.add_sized(vec2(64.0, 22.0), DragValue::new(v).range(1.0 / px..=8192.0 / px).speed(1.0 / px).fixed_decimals(decimals));
}

/// Document Settings' OK: whatever changed, as one undo step. A new stage size
/// scales the content with it (Scale Content) or keeps it at the anchor.
fn apply(ed: &mut Editor, s: Settings) {
    (ed.home.units, ed.home.apply_to_pasteboard) = (s.units, s.pasteboard);
    let px = s.units.px();
    let doc = &ed.doc;
    // Untouched sizes keep their exact value; typed ones land on whole pixels.
    let size = |v: f32, old: f32| if (v * px - old).abs() < 0.01 { old } else { (v * px).round().clamp(1.0, 8192.0) };
    let (w, h, fps) = (size(s.w, doc.width), size(s.h, doc.height), s.fps.clamp(0.01, 120.0));
    let recolor = s.color != color32(doc.background);
    let resized = (w, h) != (doc.width, doc.height);
    if !resized && !recolor && fps == doc.frame_rate {
        return;
    }
    let (sx, sy) = (w / doc.width, h / doc.height);
    let (dx, dy) = ((w - doc.width) * s.anchor.0 as f32 / 2.0, (h - doc.height) * s.anchor.1 as f32 / 2.0);
    ed.checkpoint();
    if resized {
        let elements = ed.doc.scenes.iter_mut().flat_map(|t| &mut t.layers).flat_map(|l| &mut l.frames).flat_map(|f| &mut f.elements);
        for el in elements {
            if s.scale {
                scale(el, sx, sy);
            } else {
                hit::translate(el, dx, dy);
            }
        }
    }
    (ed.doc.width, ed.doc.height, ed.doc.frame_rate) = (w, h, fps);
    if recolor {
        ed.doc.background = crate::app::rgba(s.color);
    }
}

/// Scales `el` by (sx, sy) about its timeline's origin.
fn scale(el: &mut Element, sx: f32, sy: f32) {
    minnamate_core::transform::transform(el, &Matrix::scale(sx, sy));
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use eframe::egui::{Key, Modifiers, Vec2, vec2};
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::app::testkit::*;
    use crate::tools::Tool;

    /// The editor launched without a file, run until idle.
    fn launch(renderer: bool) -> Harness<'static, Editor> {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0);
        let builder = if renderer { builder.wgpu() } else { builder };
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.run();
        h
    }

    #[test]
    fn launching_without_a_file_shows_home() {
        let h = launch(false);
        let ed = h.state();
        assert!(ed.home.shown && !ed.home.doc_open && ed.title.is_empty());
        h.get_by_label("Create New");
        h.get_by_label("More Presets");
    }

    #[test]
    fn create_with_the_defaults() {
        let mut h = launch(false);
        h.get_by_label("Create New").click();
        h.run();
        assert!(h.state().home.new_doc.is_some(), "Create New opens New Document");
        h.get_by_label("Create").click();
        h.run();
        let ed = h.state();
        assert_eq!((ed.doc.width, ed.doc.height, ed.doc.frame_rate, ed.title.as_str()), (1280.0, 720.0, 30.0, "Untitled-1"));
        assert!(!ed.home.shown && ed.home.new_doc.is_none() && !ed.dirty);
        assert_eq!((ed.zoom, ed.pan), (1.0, Vec2::ZERO), "centred at 100%");
    }

    #[test]
    fn document_settings_ok_is_one_undo_step() {
        let mut h = blank();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        h.key_press_modifiers(Modifiers::COMMAND, Key::J);
        h.run();
        h.state_mut().home.settings.as_mut().expect("Ctrl+J opens Document Settings").w = 750.0;
        h.get_by_label("OK").click();
        h.run();
        let ed = h.state();
        assert!(ed.home.settings.is_none());
        // The anchor starts in the centre, so the drawing moves by half the growth.
        assert_eq!(ed.doc.width, 750.0);
        assert!((shape_bounds(ed).min.0 - 199.5).abs() < 1.0, "{:?}", shape_bounds(ed));
        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.step();
        let ed = h.state();
        assert!(ed.doc.width == 550.0 && (shape_bounds(ed).min.0 - 99.5).abs() < 1.0, "one undo restores it");

        // Scale Content scales the drawing with the stage instead.
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::DocumentSettings, &ctx);
        let s = h.state_mut().home.settings.as_mut().unwrap();
        (s.w, s.h, s.scale) = (1100.0, 800.0, true);
        h.step();
        h.key_press(Key::Enter);
        h.step();
        let b = shape_bounds(h.state());
        assert!((b.min.0 - 199.0).abs() < 1.0 && (b.max.0 - 401.0).abs() < 1.0, "{b:?}");
    }

    #[test]
    fn close_returns_to_home() {
        let mut h = blank();
        toggle(h.state_mut());
        assert!(h.state().home.shown, "the Home button shows Home over the document");
        toggle(h.state_mut());
        assert!(!h.state().home.shown, "and takes it away again");
        h.key_press_modifiers(Modifiers::COMMAND, Key::W);
        h.run();
        let ed = h.state();
        assert!(ed.home.shown && !ed.home.doc_open && ed.title.is_empty());
        toggle(h.state_mut());
        assert!(h.state().home.shown, "with no document open, Home stays");
    }

    #[test]
    fn probe_flows() {
        // 1. Full HD tile creates directly.
        let mut h = launch(false);
        h.get_by_label("Full HD").click();
        h.run();
        let ed = h.state();
        eprintln!("P1 full hd: {}x{} {} shown={} title={}", ed.doc.width, ed.doc.height, ed.doc.frame_rate, ed.home.shown, ed.title);

        // 2. Dirty doc, Home, tile -> confirm.
        let ctx = h.ctx.clone();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (900.0, 500.0), (1000.0, 550.0));
        eprintln!("P2 dirty={}", h.state().dirty);
        toggle(h.state_mut());
        h.run();
        h.get_by_label("Android 16:9").click();
        h.run();
        eprintln!("P2 nd tab={:?}", h.state().home.new_doc.as_ref().map(|n| (n.tab, n.tile, n.w.clone(), n.h.clone(), n.fps.clone())));
        h.get_by_label("Create").click();
        h.run();
        eprintln!("P2 confirm={:?} pending={:?}", h.state().confirm, h.state().home.pending);
        h.get_by_label_contains("Don't Save").click();
        h.run();
        let ed = h.state();
        eprintln!("P2 after: {}x{} {} title={} shown={}", ed.doc.width, ed.doc.height, ed.doc.frame_rate, ed.title, ed.home.shown);

        // 3. Escape closes New Document.
        h.key_press_modifiers(Modifiers::COMMAND, Key::N);
        h.run();
        eprintln!("P3 open={}", h.state().home.new_doc.is_some());
        h.key_press(Key::Escape);
        h.run();
        eprintln!("P3 closed={}", h.state().home.new_doc.is_none());

        // 4. Play then Home.
        h.state_mut().run(Action::Play, &ctx);
        h.step();
        toggle(h.state_mut());
        h.step();
        eprintln!("P4 playing on home={}", h.state().playing.is_some());
        toggle(h.state_mut());
        h.state_mut().playing = None;
        h.run();

        // 5. Dropped file while Document Settings is up.
        h.key_press_modifiers(Modifiers::COMMAND, Key::J);
        h.run();
        eprintln!("P5 settings open={}", h.state().home.settings.is_some());
        #[derive(Debug)]
        struct Dropped(PathBuf);
        impl eframe::egui::DroppedFile for Dropped {
            fn path(&self) -> &std::path::Path {
                &self.0
            }
            fn bytes(&self) -> Result<Vec<u8>, String> {
                Err("unused".into())
            }
        }
        h.input_mut().dropped_files.push(std::sync::Arc::new(Dropped(fixture())));
        h.step();
        h.step();
        let ed = h.state();
        eprintln!("P5 after drop: title={} size={}x{} settings_open={}", ed.title, ed.doc.width, ed.doc.height, ed.home.settings.is_some());
        h.key_press(Key::Enter);
        h.run();
        let ed = h.state();
        eprintln!("P5 after OK: title={} size={}x{} dirty={}", ed.title, ed.doc.width, ed.doc.height, ed.dirty);
    }

    /// Writes target/screenshots/home.png: Home at launch, with a few recent files.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn home_screenshot() {
        let mut h = launch(true);
        let docs = PathBuf::from("C:/Users/me/Documents");
        h.state_mut().recent = ["walk-cycle.fla", "logo-sting.fla", "spring-banner.fla"].map(|n| docs.join(n)).into();
        h.run();
        screenshot(&mut h, "home");
    }

    /// Writes target/screenshots/new-document.png: Create New's dialog over Home,
    /// and new-document-advanced.png, its Advanced tab.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn new_document_screenshot() {
        let mut h = launch(true);
        h.get_by_label("Create New").click();
        h.run();
        screenshot(&mut h, "new-document");
        h.get_by_label("Advanced").click();
        h.run();
        screenshot(&mut h, "new-document-advanced");
    }

    /// Writes target/screenshots/document-settings.png: Modify > Document over the tween fixture.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn document_settings_screenshot() {
        let mut h = harness(true);
        h.run();
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::DocumentSettings, &ctx);
        h.run();
        screenshot(&mut h, "document-settings");
    }
}
