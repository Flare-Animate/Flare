//! Colour controls (docs/animate-ui-interaction.md §3): the swatch popup every
//! colour chip opens, the Color and Swatches panels with their gradient
//! controls, and applying a colour, gradient or stroke setting to the tool
//! defaults and the selected shapes.

use eframe::egui::{
    self, Area, Color32, CursorIcon, Frame, Id, LayerId, Mesh, Order, Popup, PopupCloseBehavior, Pos2, Rect, Response, Sense, Shape,
    Stroke, StrokeKind, TextEdit, Ui, Vec2, WidgetInfo, WidgetType, pos2, vec2,
};
use minnamate_core::doc::{Cap, Element, Gradient, GradientKind, Join, Paint, Rgba, Spread};
use minnamate_core::hit;
use minnamate_core::paint::{self, Picked};

use crate::app::{Editor, rgba};
use crate::icons::{self, Icon};
use crate::theme;
use crate::widgets::{self, Number};

/// The swatch grid's pitch: 10 px cells between 1 px black lines.
const PITCH: f32 = 11.0;
const COLS: usize = 20;

/// Web 216 as the swatch popup lays it out, row by row in 12 rows of 20: a
/// column of greys and primaries, a black spacer column, then the cube (§3.1).
pub fn web216() -> Vec<Color32> {
    const K: [u8; 6] = [0x00, 0x33, 0x66, 0x99, 0xCC, 0xFF];
    const COL0: [u32; 12] = [
        0x000000, 0x333333, 0x666666, 0x999999, 0xCCCCCC, 0xFFFFFF, 0xFF0000, 0x00FF00, 0x0000FF, 0xFFFF00, 0x00FFFF, 0xFF00FF,
    ];
    let mut out = Vec::with_capacity(12 * COLS);
    for (row, &c) in COL0.iter().enumerate() {
        out.push(Color32::from_rgb((c >> 16) as u8, (c >> 8) as u8, c as u8));
        out.push(Color32::BLACK);
        let b = K[row % 6];
        let reds = if row < 6 { &K[..3] } else { &K[3..] };
        for &r in reds {
            out.extend(K.iter().map(|&g| Color32::from_rgb(r, g, b)));
        }
    }
    out
}

/// `c` with alpha `a` (0..=255), from its straight colour.
fn with_alpha(c: Color32, a: u8) -> Color32 {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

fn hex(c: Color32) -> String {
    let [r, g, b, _] = c.to_srgba_unmultiplied();
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn parse_hex(s: &str) -> Option<Color32> {
    let s = s.trim().trim_start_matches('#');
    let v = u32::from_str_radix(s, 16).ok().filter(|_| s.len() == 6)?;
    Some(Color32::from_rgb((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

/// The document's straight RGBA as an egui colour.
pub fn color32(c: Rgba) -> Color32 {
    let [r, g, b, a] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(r, g, b, a)
}

/// A paint's colour where a chip or field wants one: a gradient's first stop,
/// a bitmap fill's grey.
pub fn solid(p: &Paint) -> Color32 {
    match p {
        Paint::Solid(c) => color32(*c),
        Paint::Gradient(g) => g.stops.first().map_or(Color32::BLACK, |s| color32(s.1)),
        Paint::Bitmap(_) | Paint::Unsupported { .. } => Color32::GRAY,
    }
}

/// The gradient row of the swatch popup and Swatches panel (§3.1): two greys,
/// three radials fading to black, a rainbow and a gold.
// ponytail: Animate's seven default gradients are unconfirmed (§7.3 item 4); these are the spec's
// guess. Swap in Animate's once a 2024 capture shows them.
pub fn preset_gradients() -> Vec<Gradient> {
    let ramp = |kind: GradientKind, colors: &[u32]| {
        let last = (colors.len() - 1) as f32;
        let stops = colors.iter().enumerate().map(|(i, &c)| {
            let [r, g, b] = [c >> 16, c >> 8, c].map(|v| (v & 0xFF) as f32 / 255.0);
            (i as f32 / last, [r, g, b, 1.0])
        });
        Gradient { stops: stops.collect(), ..Gradient::new(kind) }
    };
    let (linear, radial) = (GradientKind::Linear, GradientKind::Radial { focal: 0.0 });
    vec![
        ramp(radial, &[0xFFFFFF, 0x000000]),
        ramp(linear, &[0x000000, 0xFFFFFF]),
        ramp(radial, &[0xFF0000, 0x000000]),
        ramp(radial, &[0x00FF00, 0x000000]),
        ramp(radial, &[0x0000FF, 0x000000]),
        ramp(linear, &[0xFF0000, 0xFFFF00, 0x00FF00, 0x00FFFF, 0x0000FF, 0xFF00FF, 0xFF0000]),
        ramp(linear, &[0xFFE68A, 0xD99A1A, 0x7A4D00]),
    ]
}

/// `g` filling `rect`, as chips and swatches show it: a linear one left to
/// right, a radial one out from the middle to the longer side. Opaque, as chips
/// show colours, unless `alpha`.
// ponytail: a radial's focal point and the spread past its edge don't show here; sample
// `Gradient::color_at_point` per vertex if chips must show them.
pub fn paint_gradient(p: &egui::Painter, rect: Rect, g: &Gradient, alpha: bool) {
    let radius = rect.width().max(rect.height()) / 2.0;
    gradient(p, rect, |u, v| {
        let t = match g.kind {
            GradientKind::Linear => u,
            GradientKind::Radial { .. } => ((u - 0.5) * rect.width()).hypot((v - 0.5) * rect.height()) / radius,
        };
        let c = g.color_at(t);
        if alpha { color32(c) } else { opaque(c) }
    });
}

/// `c` without its alpha. Taken off the straight colour: egui's are premultiplied,
/// so a clear one keeps no colour to show.
fn opaque([r, g, b, _]: Rgba) -> Color32 {
    color32([r, g, b, 1.0])
}

/// A colour chip showing `paint`: a gradient as itself, a bitmap fill as the
/// bitmap glyph, else as `widgets::chip` shows colours. `label` names it for screen readers.
// ponytail: a bitmap fill's chip shows the glyph, not a thumbnail of the bitmap; that needs the document here.
pub fn paint_chip(ui: &mut Ui, paint: &Paint, label: &str) -> Response {
    let (Paint::Gradient(_) | Paint::Bitmap(_)) = paint else { return widgets::chip(ui, solid(paint), label) };
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 16.0), Sense::click());
    match paint {
        Paint::Gradient(g) => paint_gradient(ui.painter(), rect, g, false),
        _ => {
            ui.painter().rect_filled(rect, 3.0, Color32::GRAY);
            icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(14.0, 14.0)), Icon::Bitmap, Color32::WHITE);
        }
    }
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, theme::BORDER), StrokeKind::Outside);
    let (label, enabled) = (label.to_string(), ui.is_enabled());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ColorButton, enabled, &label));
    resp
}

/// A checkerboard of 6 px squares behind what has alpha.
fn checkerboard(p: &egui::Painter, rect: Rect) {
    p.rect_filled(rect, 0.0, Color32::WHITE);
    let p = p.with_clip_rect(rect.intersect(p.clip_rect()));
    for row in 0..(rect.height() / 6.0).ceil() as i32 {
        for col in (row % 2..(rect.width() / 6.0).ceil() as i32).step_by(2) {
            p.rect_filled(Rect::from_min_size(rect.min + vec2(col as f32 * 6.0, row as f32 * 6.0), vec2(6.0, 6.0)), 0.0, Color32::from_gray(0xA0));
        }
    }
}

/// The swatch popup's own state while it's open.
#[derive(Clone, Default)]
struct PopupState {
    /// Alpha in percent, given to whatever is picked.
    alpha: f32,
    /// The colour picker shows in place of the grid.
    picker: bool,
    /// What the eyedropper was over on the stage last frame.
    seen: Option<Color32>,
}

/// What a fill or stroke chip's popup picked: a colour, or a gradient of its gradient row.
pub enum Pick {
    Color(Color32),
    Gradient(Gradient),
}

/// The swatch popup for colour chip `chip`, which a click opens: the Web 216
/// grid, a hex field, alpha, No Color (when `no_color`) and a colour picker.
/// While it's open the pointer is an eyedropper, and a click on the stage
/// picks the colour there. Returns the colour picked this frame; Esc or a
/// click anywhere else closes it unchanged.
pub fn swatch_popup(ui: &Ui, ed: &Editor, chip: &Response, current: Color32, no_color: bool) -> Option<Color32> {
    match popup(ui, ed, chip, chip.clicked(), current, no_color, None) {
        Some(Pick::Color(c)) => Some(c),
        _ => None,
    }
}

/// A fill or stroke chip's swatch popup, which also has the gradient row
/// (§3.1); `shown` is the paint the chip shows. What's picked becomes the
/// drawing default and goes to the selected shapes.
pub fn paint_popup(ui: &Ui, ed: &mut Editor, chip: &Response, shown: &Paint, fill: bool) {
    let gradient = match shown {
        Paint::Gradient(g) => Some(g),
        _ => None,
    };
    match popup(ui, ed, chip, chip.clicked(), solid(shown), true, Some(gradient)) {
        Some(Pick::Color(c)) => restyle(ed, if fill { Style::Fill(c) } else { Style::Stroke(c) }),
        Some(Pick::Gradient(g)) => {
            set_gradient(ed, fill, g);
            ed.end_gesture();
        }
        None => {}
    }
}

/// The swatch popup under `anchor`, which `toggle` opens or closes. `gradients`
/// adds the gradient row, holding the gradient the chip shows, if any.
fn popup(ui: &Ui, ed: &Editor, anchor: &Response, toggle: bool, current: Color32, no_color: bool, gradients: Option<Option<&Gradient>>) -> Option<Pick> {
    let ctx = ui.ctx().clone();
    let id = Popup::default_response_id(anchor);
    if toggle {
        let alpha = if widgets::is_none(current) { 100.0 } else { (current.a() as f32 / 2.55).round() };
        ctx.data_mut(|d| d.insert_temp(id, PopupState { alpha, ..Default::default() }));
    }
    let mut state: PopupState = ctx.data_mut(|d| d.get_temp(id)).unwrap_or_default();
    let mut picked = None;
    let mut close = false;
    let mut hover: Option<Color32> = None;
    let mut hover_gradient: Option<Gradient> = None;
    let mut over_grid = false;
    let frame = Frame::popup(ui.style()).fill(theme::PANEL_BG).stroke(Stroke::new(1.0, theme::BORDER)).inner_margin(4);
    let command = toggle.then_some(egui::SetOpenCommand::Toggle);
    let shown = Popup::from_response(anchor).open_memory(command).id(id).close_behavior(PopupCloseBehavior::IgnoreClicks).frame(frame).show(|ui| {
        ui.set_width(4.0 + COLS as f32 * PITCH + 1.0);
        ui.spacing_mut().item_spacing = vec2(6.0, 4.0);
        let alpha = (state.alpha * 2.55).round() as u8;
        // The grid first, so the header can show what the pointer is over.
        let header = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::hover()).0;
        if state.picker {
            let mut c = if widgets::is_none(current) { Color32::BLACK } else { current };
            if egui::color_picker::color_picker_color32(ui, &mut c, egui::color_picker::Alpha::OnlyBlend) {
                picked = Some(Pick::Color(c));
                state.alpha = (c.a() as f32 / 2.55).round();
            }
        } else {
            let colors = web216();
            // The gradients start a row of their own after the solids.
            let presets = if gradients.is_some() { preset_gradients() } else { Vec::new() };
            let count = colors.len() + presets.len();
            let rows = count.div_ceil(COLS);
            let (grid, resp) = ui.allocate_exact_size(vec2(COLS as f32 * PITCH + 1.0, rows as f32 * PITCH + 1.0), Sense::click());
            resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Swatch grid"));
            let p = ui.painter();
            let cell = |i: usize| Rect::from_min_size(grid.min + vec2((i % COLS) as f32 * PITCH + 1.0, (i / COLS) as f32 * PITCH + 1.0), vec2(10.0, 10.0));
            p.rect_filled(Rect::from_min_max(grid.min, pos2(grid.right(), cell(colors.len() - 1).bottom() + 1.0)), 0.0, Color32::BLACK);
            let cell_at = |pos: Pos2| {
                let (col, row) = (((pos.x - grid.left()) / PITCH) as usize, ((pos.y - grid.top()) / PITCH) as usize);
                (grid.contains(pos) && col < COLS && row < rows).then(|| row * COLS + col).filter(|&i| i < count)
            };
            for (i, &c) in colors.iter().enumerate() {
                p.rect_filled(cell(i), 0.0, c);
            }
            for (k, g) in presets.iter().enumerate() {
                p.rect_filled(cell(colors.len() + k).expand(1.0), 0.0, Color32::BLACK);
                paint_gradient(p, cell(colors.len() + k), g, false);
            }
            if let Some(i) = resp.hover_pos().and_then(cell_at) {
                over_grid = true;
                match i.checked_sub(colors.len()) {
                    Some(k) => hover_gradient = Some(presets[k].clone()),
                    None => hover = Some(colors[i]),
                }
                p.rect_stroke(cell(i), 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
            }
            if let Some(i) = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(cell_at) {
                picked = Some(match i.checked_sub(colors.len()) {
                    Some(k) => Pick::Gradient(presets[k].clone()),
                    None => Pick::Color(with_alpha(colors[i], alpha)),
                });
                close = true;
            }
        }

        // Header: preview, hex, alpha, then No Color and the picker at the right.
        let mut ui = ui.new_child(egui::UiBuilder::new().max_rect(header).layout(egui::Layout::left_to_right(egui::Align::Center)));
        ui.spacing_mut().item_spacing.x = 4.0;
        let shown = hover.or(state.seen).unwrap_or(current);
        let (preview, _) = ui.allocate_exact_size(vec2(30.0, 18.0), Sense::hover());
        let current_gradient = gradients.flatten().filter(|_| hover.is_none() && state.seen.is_none());
        match hover_gradient.as_ref().or(current_gradient) {
            Some(g) => paint_gradient(ui.painter(), preview, g, false),
            None => widgets::paint_swatch(ui.painter(), preview, shown, 0.0),
        }
        if let Some(c) = hex_field(&mut ui, id.with("hex"), shown, 58.0) {
            picked = Some(Pick::Color(with_alpha(c, alpha)));
            close = true;
        }
        widgets::label(&mut ui, "Alpha:", 32.0);
        let a = ui.add(Number::new("Alpha", &mut state.alpha, 0.0..=100.0).suffix(" %").width(36.0));
        // ponytail: on a gradient the alpha waits for a colour to go with; scale every stop's alpha
        // if Animate's field does that.
        if a.changed() && !widgets::is_none(current) && gradients.flatten().is_none() {
            picked = Some(Pick::Color(with_alpha(current, (state.alpha * 2.55).round() as u8)));
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            if theme::icon_button(ui, 18.0, state.picker, true, "Color Picker", |p, r, _| hue_wheel(p, r.shrink(2.0))).clicked() {
                state.picker = !state.picker;
            }
            if no_color && theme::icon_button(ui, 18.0, false, true, "No Color", |p, r, _| widgets::paint_swatch(p, r.shrink(2.0), Color32::TRANSPARENT, 0.0)).clicked() {
                picked = Some(Pick::Color(Color32::TRANSPARENT));
                close = true;
            }
        });
    });
    let open = shown.is_some() && !close;

    // The eyedropper: a pane over the rest of the window takes the pointer, so
    // a click there picks from the stage or just closes, and nothing under it acts.
    state.seen = None;
    if open && !toggle {
        let screen = ctx.content_rect();
        let blocker = Area::new(id.with("eyedropper")).order(Order::Middle).fixed_pos(screen.min).show(&ctx, |ui| ui.allocate_rect(screen, Sense::click()));
        let resp = blocker.inner;
        if resp.hovered() {
            over_grid = true;
            state.seen = resp.hover_pos().and_then(|p| sample(ed, p));
        }
        if resp.clicked() {
            let c = resp.interact_pointer_pos().and_then(|p| sample(ed, p));
            picked = c.map(|c| Pick::Color(with_alpha(c, (state.alpha * 2.55).round() as u8)));
            close = true;
        }
    }
    if over_grid && let Some(pos) = ctx.pointer_hover_pos() {
        ctx.set_cursor_icon(CursorIcon::None);
        eyedropper(&ctx.layer_painter(LayerId::new(Order::Tooltip, id.with("pointer"))), pos);
    }
    if close {
        Popup::close_id(&ctx, id);
    }
    ctx.data_mut(|d| d.insert_temp(id, state));
    picked
}

/// What the eyedropper sees at screen point `pos`: a fill or stroke on the
/// stage (a gradient's colour there), else the stage colour; nothing off the stage.
// ponytail: colours as the shapes hold them, before color effects (paint::pick's limit),
// and only from the stage: egui can't read pixels of the rest of the window.
fn sample(ed: &Editor, pos: Pos2) -> Option<Color32> {
    let (x, y) = crate::stage::local_point(ed, ed.stage_rect, pos);
    let scene = ed.to_scene((x, y));
    let on_stage = (0.0..=ed.doc.width).contains(&scene.0) && (0.0..=ed.doc.height).contains(&scene.1);
    let picked = paint::pick(&ed.doc, ed.timeline(), ed.frame, (x, y), 1.0 / ed.zoom);
    let c = match picked {
        Some(Picked::Fill(Paint::Solid(c)) | Picked::Stroke(minnamate_core::doc::Stroke { paint: Paint::Solid(c), .. })) => c,
        Some(Picked::Fill(Paint::Gradient(g)) | Picked::Stroke(minnamate_core::doc::Stroke { paint: Paint::Gradient(g), .. })) => {
            g.color_at_point((x, y))
        }
        _ if on_stage => ed.doc.background,
        _ => return None,
    };
    let [r, g, b, _] = c.map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Some(Color32::from_rgb(r, g, b))
}

/// The Eyedropper glyph as the pointer: black with a white rim, its tip at `pos`.
fn eyedropper(p: &egui::Painter, pos: Pos2) {
    let glyph = Rect::from_min_size(pos - vec2(3.0, 15.0), vec2(18.0, 18.0));
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
        icons::paint(p, glyph.translate(d), Icon::Eyedropper, Color32::WHITE);
    }
    icons::paint(p, glyph, Icon::Eyedropper, Color32::BLACK);
}

/// The Color Picker button's glyph: a hue wheel.
fn hue_wheel(p: &egui::Painter, r: Rect) {
    let (c, radius) = (r.center(), r.width().min(r.height()) / 2.0);
    let mut mesh = Mesh::default();
    mesh.colored_vertex(c, Color32::WHITE);
    const N: u32 = 24;
    for i in 0..=N {
        let t = i as f32 / N as f32;
        let a = t * std::f32::consts::TAU;
        mesh.colored_vertex(c + radius * vec2(a.cos(), a.sin()), hsb(t * 360.0, 1.0, 1.0));
        if i > 0 {
            mesh.add_triangle(0, i, i + 1);
        }
    }
    p.add(Shape::mesh(mesh));
}

/// HSB (hue in degrees, saturation and brightness 0..=1) to an opaque colour, in sRGB as Animate shows it.
fn hsb(h: f32, s: f32, b: f32) -> Color32 {
    let h = h.rem_euclid(360.0) / 60.0;
    let f = h - h.floor();
    let (p, q, t) = (b * (1.0 - s), b * (1.0 - s * f), b * (1.0 - s * (1.0 - f)));
    let (r, g, bl) = match h as u32 {
        0 => (b, t, p),
        1 => (q, b, p),
        2 => (p, b, t),
        3 => (p, q, b),
        4 => (t, p, b),
        _ => (b, p, q),
    };
    let u = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgb(u(r), u(g), u(bl))
}

/// An opaque colour's hue (degrees), saturation and brightness (0..=1).
fn to_hsb(c: Color32) -> (f32, f32, f32) {
    let [r, g, b, _] = c.to_srgba_unmultiplied().map(|v| v as f32 / 255.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let d = max - min;
    let h = if d == 0.0 {
        0.0
    } else if max == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if max == 0.0 { 0.0 } else { d / max }, max)
}

/// A stroke or colour setting from Properties or the Color panels.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Style {
    Fill(Color32),
    Stroke(Color32),
    /// The fill's (true) or the stroke's gradient default, set first (`set_gradient`).
    Gradient(bool),
    Width(f32),
    Cap(Cap),
    Join(Join),
}

/// Sets a drawing default and gives the selected shapes the same: their fills
/// or their strokes (no colour removes them; a shape without strokes gets
/// none). One drag, or one pick, is one undo step.
pub fn restyle(ed: &mut Editor, style: Style) {
    restyle_all(ed, &[style]);
}

/// `restyle` with several settings at once (Swap Colors, Black and White), as one undo step.
pub fn restyle_all(ed: &mut Editor, styles: &[Style]) {
    for &style in styles {
        match style {
            Style::Fill(c) => (ed.fill_color, ed.fill_gradient, ed.bitmaps.fill) = (c, None, None),
            Style::Stroke(c) => (ed.stroke_color, ed.stroke_gradient) = (c, None),
            Style::Gradient(_) => {}
            Style::Width(w) => ed.stroke_width = w,
            Style::Cap(c) => ed.stroke_cap = c,
            Style::Join(j) => ed.stroke_join = j,
        }
    }
    let shapes: Vec<_> = ed.selection.iter().copied().filter(|&r| matches!(ed.element(r), Some(Element::Shape(_)))).collect();
    let Some(&first) = styles.first().filter(|_| !shapes.is_empty()) else { return };
    ed.checkpoint_gesture(match first {
        _ if styles.len() > 1 => "colors",
        Style::Fill(_) => "fill-color",
        Style::Stroke(_) => "stroke-color",
        Style::Gradient(true) => "fill-gradient",
        Style::Gradient(false) => "stroke-gradient",
        Style::Width(_) => "stroke-width",
        Style::Cap(_) | Style::Join(_) => "stroke-ends",
    });
    let gradients = [ed.stroke_gradient.clone(), ed.fill_gradient.clone()];
    for r in shapes {
        let Some(Element::Shape(s)) = ed.element_mut(r) else { continue };
        let b = minnamate_core::gradient::edge_box(s);
        for &style in styles {
            match style {
                Style::Gradient(fill) => {
                    let Some(g) = &gradients[fill as usize] else { continue };
                    let paints: Vec<&mut Paint> = if fill { s.fills.iter_mut().collect() } else { s.strokes.iter_mut().map(|st| &mut st.paint).collect() };
                    for p in paints {
                        *p = Paint::Gradient(placed(g, p, b));
                    }
                }
                Style::Fill(c) if widgets::is_none(c) => {
                    s.fills.clear();
                    s.edges.iter_mut().for_each(|e| (e.fill0, e.fill1) = (0, 0));
                }
                Style::Stroke(c) if widgets::is_none(c) => {
                    s.strokes.clear();
                    s.edges.iter_mut().for_each(|e| e.stroke = 0);
                }
                Style::Fill(c) => s.fills.iter_mut().for_each(|f| *f = Paint::Solid(rgba(c))),
                Style::Stroke(c) => s.strokes.iter_mut().for_each(|st| st.paint = Paint::Solid(rgba(c))),
                Style::Width(w) => s.strokes.iter_mut().for_each(|st| st.width = w),
                Style::Cap(c) => s.strokes.iter_mut().for_each(|st| st.cap = c),
                Style::Join(j) => s.strokes.iter_mut().for_each(|st| st.join = j),
            }
        }
    }
}

/// `g` going in place of `old` in a shape whose edges' box is `b`: on a
/// gradient's matrix (and focal point, radial for radial), else across `b`.
fn placed(g: &Gradient, old: &Paint, b: Option<hit::Rect>) -> Gradient {
    let mut g = g.clone();
    match old {
        Paint::Gradient(old) => {
            g.matrix = old.matrix;
            if let (GradientKind::Radial { .. }, GradientKind::Radial { focal }) = (g.kind, old.kind) {
                g.kind = GradientKind::Radial { focal };
            }
        }
        _ => {
            if let Some(b) = b {
                g.fit(b);
            }
        }
    }
    g
}

/// Makes `g` the fill (or stroke) gradient default and gives it to the selected
/// shapes. The default keeps its own matrix, where Lock Fill paints it: at
/// first, across the stage.
// ponytail: the stage's box in scene coordinates, also while editing a symbol; map it through
// `Editor::to_local` if Lock Fill inside symbols must line up with the stage.
pub fn set_gradient(ed: &mut Editor, fill: bool, mut g: Gradient) {
    let stage = hit::Rect { min: (0.0, 0.0), max: (ed.doc.width, ed.doc.height) };
    let slot = if fill { &mut ed.fill_gradient } else { &mut ed.stroke_gradient };
    match slot {
        Some(old) => g.matrix = old.matrix,
        None => g.fit(stage),
    }
    *slot = Some(g);
    if fill {
        ed.bitmaps.fill = None;
    }
    restyle(ed, Style::Gradient(fill));
}

/// Whether the fill (or stroke) default is "no color".
fn none(ed: &Editor, fill: bool) -> bool {
    if fill { ed.fill_gradient.is_none() && widgets::is_none(ed.fill_color) } else { ed.stroke_gradient.is_none() && widgets::is_none(ed.stroke_color) }
}

/// Swap colors: the stroke and fill trade places, gradients too, and so do the selected shapes'.
fn swap(ed: &mut Editor) {
    // ponytail: the stroke default has no bitmap, so a bitmap fill goes rather than swapping.
    ed.bitmaps.fill = None;
    let (s, f) = (ed.stroke_color, ed.fill_color);
    let (sg, fg) = (ed.stroke_gradient.take(), ed.fill_gradient.take());
    (ed.stroke_gradient, ed.fill_gradient) = (fg, sg);
    let stroke = if ed.stroke_gradient.is_some() { Style::Gradient(false) } else { Style::Stroke(f) };
    let fill = if ed.fill_gradient.is_some() { Style::Gradient(true) } else { Style::Fill(s) };
    restyle_all(ed, &[stroke, fill]);
}

/// Which colour the Color and Swatches panels edit: the fill (true) or the stroke.
fn fill_active(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp(Id::new("color-fill-active"))).unwrap_or(true)
}

/// The Color panel's components; the selected one drives the slider.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum Comp {
    #[default]
    H,
    S,
    B,
    R,
    G,
    Bl,
}

/// What the Color panel shows of the fill or stroke: the first selected
/// shape's, as Properties does, else the drawing default.
fn shown_paint(ed: &Editor, fill: bool) -> Paint {
    let selected = ed.selection.iter().find_map(|&r| match ed.element(r) {
        Some(Element::Shape(s)) if fill => s.fills.first().cloned(),
        Some(Element::Shape(s)) => s.strokes.first().map(|st| st.paint.clone()),
        _ => None,
    });
    selected.unwrap_or_else(|| if fill { ed.fill() } else { ed.stroke().paint })
}

/// Window > Color (§3.2): stroke and fill controls, colour type, flow and
/// Linear RGB for gradients, the colour square with its slider and fields
/// (a gradient's selected stop), hex, and a preview bar or the gradient bar.
pub fn color_panel(ui: &mut Ui, ed: &mut Editor) {
    let ctx = ui.ctx().clone();
    let fill = fill_active(&ctx);
    ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
    ui.horizontal(|ui| {
        for (is_fill, icon, tip) in [(false, Icon::Pencil, "Stroke color"), (true, Icon::PaintBucket, "Fill color")] {
            if theme::icon_button(ui, 22.0, fill == is_fill, true, tip, |p, r, c| icons::paint(p, r.shrink(4.0), icon, c)).clicked() {
                ctx.data_mut(|d| d.insert_temp(Id::new("color-fill-active"), is_fill));
            }
            let paint = shown_paint(ed, is_fill);
            let chip = paint_chip(ui, &paint, tip);
            paint_popup(ui, ed, &chip, &paint, is_fill);
            ui.add_space(4.0);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            if theme::icon_button(ui, 22.0, false, true, "Swap colors", |p, r, c| icons::paint(p, r.shrink(4.0), Icon::Swap, c)).clicked()
                && !none(ed, false)
                && !none(ed, true)
            {
                swap(ed);
            }
            if theme::icon_button(ui, 22.0, false, true, "No color", |p, r, _| widgets::paint_swatch(p, r.shrink(5.0), Color32::TRANSPARENT, 0.0)).clicked() {
                restyle(ed, if fill { Style::Fill(Color32::TRANSPARENT) } else { Style::Stroke(Color32::TRANSPARENT) });
            }
            if theme::icon_button(ui, 22.0, false, true, "Black and white", |p, r, _| black_and_white(p, r)).clicked() {
                restyle_all(ed, &[Style::Stroke(Color32::BLACK), Style::Fill(Color32::WHITE)]);
            }
        });
    });
    let shown = shown_paint(ed, fill);
    let ramp = match &shown {
        Paint::Gradient(g) => Some(g.clone()),
        _ => None,
    };
    // A gradient's colour controls edit its selected stop.
    let stop_id = Id::new("color-gradient-stop");
    let mut stop: usize = ctx.data(|d| d.get_temp(stop_id)).unwrap_or(0);
    if let Some(g) = &ramp {
        stop = stop.min(g.stops.len().saturating_sub(1));
    }
    // A stop's colour and alpha are kept apart: a clear stop still has a colour.
    let (current, stop_alpha) = match &ramp {
        Some(g) if !g.stops.is_empty() => (opaque(g.stops[stop].1), Some((g.stops[stop].1[3].clamp(0.0, 1.0) * 255.0).round() as u8)),
        _ => (solid(&shown), None),
    };
    // Sets colour `c` (its alpha aside) with alpha `a`.
    let set = {
        let (ramp, stop) = (ramp.clone(), stop);
        move |ed: &mut Editor, c: Color32, a: u8| match &ramp {
            Some(g) if !g.stops.is_empty() => {
                let mut g = g.clone();
                let [r, gr, b, _] = rgba(c);
                g.stops[stop].1 = [r, gr, b, a as f32 / 255.0];
                set_gradient(ed, fill, g);
            }
            _ => restyle(ed, if fill { Style::Fill(with_alpha(c, a)) } else { Style::Stroke(with_alpha(c, a)) }),
        }
    };
    widgets::row(ui, |ui| {
        widgets::label(ui, "Color type:", 64.0);
        let kind = match &ramp {
            _ if matches!(shown, Paint::Bitmap(_)) => 4,
            Some(Gradient { kind: GradientKind::Linear, .. }) => 2,
            Some(_) => 3,
            None if widgets::is_none(current) => 0,
            None => 1,
        };
        // ponytail: bitmap fills for strokes come only from files; the stroke default has none.
        let items = [(0, "None", true), (1, "Solid color", true), (2, "Linear gradient", true), (3, "Radial gradient", true), (4, "Bitmap fill", fill)];
        let last = if fill { ed.fill_color } else { ed.stroke_color };
        let kinds = [GradientKind::Linear, GradientKind::Radial { focal: 0.0 }];
        let Some(picked) = widgets::pick(ui, "Color type", kind, &items, 150.0) else { return };
        match picked {
            0 => restyle(ed, if fill { Style::Fill(Color32::TRANSPARENT) } else { Style::Stroke(Color32::TRANSPARENT) }),
            // Back to the last solid colour.
            1 => {
                let c = if widgets::is_none(last) { Color32::BLACK } else { last };
                restyle(ed, if fill { Style::Fill(c) } else { Style::Stroke(c) });
            }
            4 => crate::bitmaps::choose_fill(ed),
            k => {
                let kind = kinds[k - 2];
                let g = ramp.clone().map_or_else(|| Gradient::new(kind), |g| Gradient { kind, ..g });
                set_gradient(ed, fill, g);
            }
        }
        ed.end_gesture();
    });
    // A bitmap fill shows the library's bitmaps in place of the colour controls (§3.2 item 7).
    if fill && matches!(shown, Paint::Bitmap(_)) {
        return crate::bitmaps::fill_swatches(ui, ed, &shown);
    }
    if let Some(g) = &ramp {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Flow:", 64.0);
            ui.spacing_mut().item_spacing.x = 2.0;
            for (spread, tip) in [(Spread::Pad, "Extend color"), (Spread::Reflect, "Reflect color"), (Spread::Repeat, "Repeat color")] {
                if theme::icon_button(ui, 22.0, g.spread == spread, true, tip, |p, r, c| flow_glyph(p, r.shrink(4.0), spread, c)).clicked() && g.spread != spread {
                    set_gradient(ed, fill, Gradient { spread, ..g.clone() });
                    ed.end_gesture();
                }
            }
            ui.add_space(10.0);
            let mut linear = g.linear_rgb;
            if widgets::checkbox(ui, &mut linear, "Linear RGB").changed() {
                set_gradient(ed, fill, Gradient { linear_rgb: linear, ..g.clone() });
                ed.end_gesture();
            }
        });
    }

    // The square and slider show the other components of the selected one (Photoshop's way).
    let base = if widgets::is_none(current) { Color32::BLACK } else { current };
    let alpha = stop_alpha.unwrap_or(if widgets::is_none(current) { 255 } else { current.a() });
    let (h, s, b) = to_hsb(base);
    let [r, g, bl, _] = base.to_srgba_unmultiplied().map(|v| v as f32 / 255.0);
    let comp_id = Id::new("color-panel-comp");
    let mut comp: Comp = ctx.data(|d| d.get_temp(comp_id)).unwrap_or_default();
    // (x, y) of the square and the slider's value, each 0..=1, for the selected component.
    let axes = |comp: Comp, x: f32, y: f32, z: f32| -> Color32 {
        let u = |v: f32| (v * 255.0).round() as u8;
        let rgb = |r: f32, g: f32, b: f32| Color32::from_rgb(u(r), u(g), u(b));
        match comp {
            Comp::H => hsb(z * 360.0, x, y),
            Comp::S => hsb(x * 360.0, z, y),
            Comp::B => hsb(x * 360.0, y, z),
            Comp::R => rgb(z, y, x),
            Comp::G => rgb(y, z, x),
            Comp::Bl => rgb(x, y, z),
        }
    };
    let (x, y, z) = match comp {
        Comp::H => (s, b, h / 360.0),
        Comp::S => (h / 360.0, b, s),
        Comp::B => (h / 360.0, s, b),
        Comp::R => (bl, g, r),
        Comp::G => (bl, r, g),
        Comp::Bl => (r, g, bl),
    };
    let (mut new, mut new_alpha) = (None, None);
    // A typed value is an undo step of its own.
    let mut let_go = false;
    ui.horizontal_top(|ui| {
        let (square, sq) = ui.allocate_exact_size(vec2(150.0, 120.0), Sense::click_and_drag());
        gradient(ui.painter(), square, |u, v| axes(comp, u, 1.0 - v, z));
        let (strip, st) = ui.allocate_exact_size(vec2(14.0, 120.0), Sense::click_and_drag());
        gradient(ui.painter(), strip, |_, v| axes(comp, x, y, 1.0 - v));
        let p = ui.painter();
        p.circle_stroke(pos2(square.left() + x * square.width(), square.bottom() - y * square.height()), 4.0, Stroke::new(1.5, Color32::WHITE));
        let sy = strip.bottom() - z * strip.height();
        p.hline(strip.left() - 2.0..=strip.right() + 2.0, sy, Stroke::new(2.0, Color32::WHITE));
        if let Some(pos) = sq.interact_pointer_pos().filter(|_| sq.dragged() || sq.clicked()) {
            let u = ((pos.x - square.left()) / square.width()).clamp(0.0, 1.0);
            let v = ((square.bottom() - pos.y) / square.height()).clamp(0.0, 1.0);
            new = Some(axes(comp, u, v, z));
        }
        if let Some(pos) = st.interact_pointer_pos().filter(|_| st.dragged() || st.clicked()) {
            new = Some(axes(comp, x, y, ((strip.bottom() - pos.y) / strip.height()).clamp(0.0, 1.0)));
        }
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            let mut vals = [h, s * 100.0, b * 100.0, r * 255.0, g * 255.0, bl * 255.0];
            let rows = [(Comp::H, "H:", " °", 360.0), (Comp::S, "S:", " %", 100.0), (Comp::B, "B:", " %", 100.0), (Comp::R, "R:", "", 255.0), (Comp::G, "G:", "", 255.0), (Comp::Bl, "B:", "", 255.0)];
            for (i, (c, name, unit, max)) in rows.into_iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if radio(ui, comp == c).clicked() {
                        comp = c;
                    }
                    widgets::label(ui, name, 14.0);
                    let field = ui.add(Number::new(name, &mut vals[i], 0.0..=max).suffix(unit).width(46.0));
                    if field.changed() {
                        new = Some(if i < 3 {
                            hsb(vals[0], vals[1] / 100.0, vals[2] / 100.0)
                        } else {
                            Color32::from_rgb(vals[3].round() as u8, vals[4].round() as u8, vals[5].round() as u8)
                        });
                    }
                    let_go |= field.drag_stopped() || field.lost_focus();
                });
            }
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                widgets::label(ui, "A:", 14.0);
                let mut a = (alpha as f32 / 2.55).round();
                let field = ui.add(Number::new("A:", &mut a, 0.0..=100.0).suffix(" %").width(46.0));
                if field.changed() {
                    new_alpha = Some((a * 2.55).round() as u8);
                }
                let_go |= field.drag_stopped() || field.lost_focus();
            });
        });
    });
    ctx.data_mut(|d| d.insert_temp(comp_id, comp));
    if new.is_some() || new_alpha.is_some() {
        set(ed, new.unwrap_or(base), new_alpha.unwrap_or(alpha));
    }
    ui.horizontal_top(|ui| {
        if let Some(c) = hex_field(ui, Id::new("color-panel-hex"), current, 64.0) {
            set(ed, c, alpha);
            let_go = true;
        }
        match &ramp {
            // In gradient modes the preview bar is the gradient bar.
            Some(g) => {
                let (changed, done) = gradient_bar(ui, ed, g, &mut stop);
                if let Some(g) = changed {
                    set_gradient(ed, fill, g);
                }
                let_go |= done;
            }
            None => {
                let (bar, _) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::hover());
                widgets::paint_swatch(ui.painter(), bar, current, 0.0);
            }
        }
    });
    ctx.data_mut(|d| d.insert_temp(stop_id, stop));
    if let_go {
        ed.end_gesture();
    }
    ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
        // ponytail: swatches the user adds need a home in the document model.
        ui.add_enabled_ui(false, |ui| widgets::button(ui, "Add to Swatches", 110.0));
    });
}

/// The gradient bar (§3.2 item 6): `g` over a checkerboard and, under it, a
/// pointer per stop, the `selected` one (which the colour controls edit) with
/// a black roof. A click on the bar adds a stop; a pointer drags sideways, and
/// down off the panel takes its stop away; a double-click opens the swatch
/// popup for it. Returns the gradient changed, and whether that ended a drag or
/// a pick: each is one undo step.
fn gradient_bar(ui: &mut Ui, ed: &Editor, g: &Gradient, selected: &mut usize) -> (Option<Gradient>, bool) {
    const BAR: f32 = 18.0;
    // Room for the house-shaped pointers: a 4.5 px roof and a 9 × 8.5 body.
    let (rect, resp) = ui.allocate_exact_size(vec2(ui.available_width(), BAR + 14.0), Sense::click_and_drag());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Gradient bar"));
    // Inset so the end pointers show whole.
    let bar = Rect::from_min_size(rect.min + vec2(5.0, 0.0), vec2(rect.width() - 10.0, BAR));
    let x_of = |t: f32| bar.left() + t * bar.width();
    let t_of = |x: f32| ((x - bar.left()) / bar.width()).clamp(0.0, 1.0);
    let pointer_at = |pos: Pos2| {
        let near = g.stops.iter().enumerate().map(|(i, s)| (i, (x_of(s.0) - pos.x).abs())).filter(|&(_, d)| d <= 6.0);
        near.min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i).filter(|_| pos.y >= bar.bottom())
    };
    let (mut changed, mut done) = (None, false);
    // The stop being dragged, which a press on its pointer picked, and the gradient the drag
    // left: anything else changing that (an undo, X) ends the drag.
    let drag_id = resp.id.with("stop");
    let left: Option<(usize, Gradient)> = ui.data(|d| d.get_temp(drag_id)).flatten();
    let mut dragging = left.filter(|(_, left)| left == g).map(|(i, _)| i);
    if resp.drag_started() {
        dragging = ui.input(|i| i.pointer.press_origin()).and_then(pointer_at);
        if let Some(i) = dragging {
            *selected = i;
        }
    }
    if let (Some(i), Some(pos)) = (dragging.filter(|_| resp.dragged()), resp.interact_pointer_pos()) {
        let mut moved = g.clone();
        let (_, color) = moved.stops.remove(i);
        if pos.y > rect.bottom() + 40.0 && g.stops.len() > 2 {
            // Off the panel downward: the stop goes.
            (dragging, *selected) = (None, i.min(moved.stops.len() - 1));
        } else {
            let t = t_of(pos.x);
            let j = moved.stops.iter().position(|s| s.0 > t).unwrap_or(moved.stops.len());
            moved.stops.insert(j, (t, color));
            (dragging, *selected) = (Some(j), j);
        }
        changed = (moved != *g).then_some(moved);
    }
    if resp.drag_stopped() {
        (dragging, done) = (None, true);
    }
    if resp.clicked()
        && let Some(pos) = resp.interact_pointer_pos()
    {
        match pointer_at(pos) {
            Some(i) => *selected = i,
            // A click on the bar adds a stop there, in the colour already there; Animate takes 15.
            None if pos.y < bar.bottom() && g.stops.len() < 15 => {
                let t = t_of(pos.x);
                let mut added = g.clone();
                let j = added.stops.iter().position(|s| s.0 > t).unwrap_or(added.stops.len());
                added.stops.insert(j, (t, g.color_at(t)));
                (changed, done, *selected) = (Some(added), true, j);
            }
            None => {}
        }
    }
    let left = dragging.map(|i| (i, changed.clone().unwrap_or_else(|| g.clone())));
    ui.data_mut(|d| d.insert_temp(drag_id, left));

    // A double-click on a pointer opens the swatch popup for its stop.
    let open = resp.double_clicked() && resp.interact_pointer_pos().and_then(pointer_at).is_some();
    let mut picked = changed.clone().unwrap_or_else(|| g.clone());
    let stop = (*selected).min(picked.stops.len().saturating_sub(1));
    let current = picked.stops.get(stop).map_or(Color32::BLACK, |s| color32(s.1));
    if let Some(Pick::Color(c)) = popup(ui, ed, &resp, open, current, false, None)
        && stop < picked.stops.len()
    {
        picked.stops[stop].1 = rgba(c);
        (changed, done) = (Some(picked), true);
    }

    let shown = changed.as_ref().unwrap_or(g);
    let p = ui.painter();
    checkerboard(p, bar);
    paint_gradient(p, bar, &Gradient { kind: GradientKind::Linear, ..shown.clone() }, true);
    p.rect_stroke(bar, 0.0, Stroke::new(1.0, theme::BORDER), StrokeKind::Outside);
    let dark = Stroke::new(1.0, Color32::from_gray(0x1A));
    for (i, &(t, c)) in shown.stops.iter().enumerate() {
        let x = x_of(t);
        let roof = vec![pos2(x, bar.bottom()), pos2(x + 4.5, bar.bottom() + 4.5), pos2(x - 4.5, bar.bottom() + 4.5)];
        p.add(Shape::convex_polygon(roof, if i == *selected { Color32::BLACK } else { Color32::WHITE }, dark));
        let body = Rect::from_min_max(pos2(x - 4.5, bar.bottom() + 4.5), pos2(x + 4.5, bar.bottom() + 13.0));
        p.rect(body, 0.0, opaque(c), dark, StrokeKind::Inside);
    }
    (changed, done)
}

/// A Flow button's glyph: a black-to-white ramp over the left half, and on the
/// right what follows it with `spread`.
fn flow_glyph(p: &egui::Painter, r: Rect, spread: Spread, ink: Color32) {
    gradient(p, r, |u, _| {
        let t = 2.0 * u;
        let t = match spread {
            Spread::Pad => t.min(1.0),
            Spread::Reflect => 1.0 - (t - 1.0).abs(),
            Spread::Repeat => if t > 1.0 { t - 1.0 } else { t },
        };
        Color32::from_gray((t * 255.0).round() as u8)
    });
    p.rect_stroke(r, 0.0, Stroke::new(1.0, ink), StrokeKind::Outside);
}

/// A "#RRGGBB" field showing `shown`; returns what was typed once Enter or
/// leaving the field commits it (Esc reverts).
fn hex_field(ui: &mut Ui, id: Id, shown: Color32, width: f32) -> Option<Color32> {
    let typed: Option<String> = ui.data(|d| d.get_temp(id));
    let mut text = typed.unwrap_or_else(|| if widgets::is_none(shown) { String::new() } else { hex(shown) });
    let field = ui.add(TextEdit::singleline(&mut text).id(id).desired_width(width).char_limit(7));
    if field.has_focus() {
        ui.data_mut(|d| d.insert_temp(id, text.clone()));
    }
    if !field.lost_focus() {
        return None;
    }
    ui.data_mut(|d| d.remove::<String>(id));
    parse_hex(&text).filter(|_| !ui.input(|i| i.key_pressed(egui::Key::Escape)))
}

/// A small radio button.
fn radio(ui: &mut Ui, on: bool) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(14.0, 20.0), Sense::click());
    let c = rect.center();
    ui.painter().circle_stroke(c, 5.0, Stroke::new(1.2, theme::CHECKBOX_BORDER));
    if on {
        ui.painter().circle_filled(c, 2.5, theme::TEXT);
    }
    resp
}

/// `rect` filled with `color(u, v)`, u and v running 0..=1 left to right and top to bottom.
fn gradient(p: &egui::Painter, rect: Rect, color: impl Fn(f32, f32) -> Color32) {
    const N: u32 = 24;
    let mut mesh = Mesh::default();
    for j in 0..=N {
        for i in 0..=N {
            let (u, v) = (i as f32 / N as f32, j as f32 / N as f32);
            mesh.colored_vertex(rect.min + vec2(u * rect.width(), v * rect.height()), color(u, v));
            if i > 0 && j > 0 {
                let k = j * (N + 1) + i;
                mesh.add_triangle(k - N - 2, k - N - 1, k);
                mesh.add_triangle(k - N - 2, k - 1, k);
            }
        }
    }
    p.add(Shape::mesh(mesh));
}

/// Black and White's glyph: a black square over a white one, on the diagonal.
fn black_and_white(p: &egui::Painter, r: Rect) {
    let back = Rect::from_min_size(r.min + vec2(9.0, 9.0), vec2(8.0, 8.0));
    let front = Rect::from_min_size(r.min + vec2(5.0, 5.0), vec2(8.0, 8.0));
    p.rect_filled(back, 0.0, Color32::WHITE);
    p.rect_stroke(back, 0.0, Stroke::new(1.0, theme::TEXT_DIM), StrokeKind::Inside);
    p.rect_filled(front, 0.0, Color32::BLACK);
    p.rect_stroke(front, 0.0, Stroke::new(1.0, theme::TEXT_DIM), StrokeKind::Inside);
}

/// Window > Swatches (§3.3): the Web 216 grid reflowed to the panel's width;
/// a click gives the swatch to the colour the Color panel edits.
pub fn swatches_panel(ui: &mut Ui, ed: &mut Editor) {
    let fill = fill_active(ui.ctx());
    let colors = web216();
    let presets = preset_gradients();
    let cols = ((ui.available_width() / PITCH) as usize).max(1);
    // The gradients start a row of their own.
    let first = colors.len().div_ceil(cols) * cols;
    let rows = (first + presets.len()).div_ceil(cols);
    let (grid, resp) = ui.allocate_exact_size(vec2(cols as f32 * PITCH + 1.0, rows as f32 * PITCH + 1.0), Sense::click());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Other, true, "Swatches"));
    let p = ui.painter();
    let cell = |i: usize| Rect::from_min_size(grid.min + vec2((i % cols) as f32 * PITCH + 1.0, (i / cols) as f32 * PITCH + 1.0), vec2(10.0, 10.0));
    // Each swatch in its black frame; the last row stops at its last swatch.
    for i in (0..colors.len()).chain(first..first + presets.len()) {
        p.rect_filled(cell(i).expand(1.0), 0.0, Color32::BLACK);
    }
    for (i, &c) in colors.iter().enumerate() {
        p.rect_filled(cell(i), 0.0, c);
    }
    for (k, g) in presets.iter().enumerate() {
        paint_gradient(p, cell(first + k), g, false);
    }
    let at = |pos: Pos2| {
        let v: Vec2 = (pos - grid.min) / PITCH;
        let i = v.y as usize * cols + v.x as usize;
        (v.x < cols as f32 && (i < colors.len() || (first..first + presets.len()).contains(&i))).then_some(i)
    };
    let current = if fill { ed.fill_color } else { ed.stroke_color };
    if let Some(i) = colors.iter().position(|&c| c == current) {
        p.rect_stroke(cell(i), 0.0, Stroke::new(1.0, Color32::WHITE), StrokeKind::Inside);
    }
    if let Some(i) = resp.interact_pointer_pos().filter(|_| resp.clicked()).and_then(at) {
        match i.checked_sub(first) {
            Some(k) => {
                set_gradient(ed, fill, presets[k].clone());
                ed.end_gesture();
            }
            None => restyle(ed, if fill { Style::Fill(colors[i]) } else { Style::Stroke(colors[i]) }),
        }
    }
    ui.add_space(ui.available_height() - 26.0);
    // ponytail: tagged swatches need a home in the document model; the bar is greyed till then.
    ui.horizontal(|ui| {
        for (icon, tip) in [(Icon::Tag, "Convert to Tagged Swatch"), (Icon::Pencil, "Edit"), (Icon::Delete, "Delete")] {
            theme::icon_button(ui, theme::ICON_BUTTON, false, false, tip, |p, r, c| icons::paint(p, r.shrink(4.0), icon, c));
        }
    });
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;

    use super::*;
    use crate::app::testkit::*;

    #[test]
    fn web216_layout() {
        let c = web216();
        assert_eq!(c.len(), 240);
        assert_eq!((c[0], c[1], c[COLS * 11 + 19]), (Color32::BLACK, Color32::BLACK, Color32::WHITE), "black at the top left, white at the bottom right");
        assert_eq!(c[COLS * 6 + 2], Color32::from_rgb(0x99, 0, 0), "the lower half starts from the reds");
        let mut cube: Vec<_> = c.iter().enumerate().filter(|(i, _)| i % COLS >= 2).map(|(_, c)| c.to_array()).collect();
        cube.sort();
        cube.dedup();
        assert_eq!(cube.len(), 216, "every web-safe colour once");
    }

    #[test]
    fn restyling_the_selection() {
        use crate::app::testkit::*;
        let mut h = blank();
        h.state_mut().tool = crate::tools::Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (200.0, 150.0));
        let ed = h.state_mut();
        ed.selection = ed.all_elements();
        let shape = |ed: &Editor| match ed.element(ed.selection[0]) {
            Some(Element::Shape(s)) => s.clone(),
            _ => panic!("no shape"),
        };
        let before = shape(ed);
        restyle_all(ed, &[Style::Stroke(Color32::BLACK), Style::Fill(Color32::WHITE)]);
        assert_eq!(shape(ed).fills, [Paint::Solid([1.0; 4])]);
        ed.end_gesture();
        restyle(ed, Style::Fill(Color32::TRANSPARENT));
        let s = shape(ed);
        assert!(s.fills.is_empty() && s.edges.iter().all(|e| e.fill0 == 0 && e.fill1 == 0), "no colour removes the fill");
        let ctx = h.ctx.clone();
        h.state_mut().run(crate::app::Action::Undo, &ctx);
        h.state_mut().run(crate::app::Action::Undo, &ctx);
        assert_eq!(shape(h.state()), before, "black and white was one step");
    }

    #[test]
    fn hsb_round_trip() {
        for c in [Color32::from_rgb(0x00, 0x66, 0xCC), Color32::from_rgb(200, 30, 90), Color32::WHITE, Color32::BLACK] {
            let (h, s, b) = to_hsb(c);
            assert_eq!(hsb(h, s, b), c);
        }
        assert_eq!(parse_hex("#0066cc"), Some(Color32::from_rgb(0, 0x66, 0xCC)));
        assert_eq!(hex(Color32::from_rgb(0, 0x66, 0xCC)), "#0066CC");
    }

    /// A 200 × 100 rectangle at (100, 100), selected, and the Color panel open.
    fn rectangle_and_color_panel() -> Harness<'static, Editor> {
        let mut h = blank();
        h.state_mut().tool = crate::tools::Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (300.0, 200.0));
        let ed = h.state_mut();
        (ed.tool, ed.selection) = (crate::tools::Tool::Selection, ed.all_elements());
        ed.flyout = Some(crate::panels::Flyout::Color);
        h.run();
        h
    }

    fn fill(h: &Harness<'_, Editor>) -> Paint {
        match h.state().element(h.state().selection[0]) {
            Some(Element::Shape(s)) => s.fills[0].clone(),
            other => panic!("{other:?}"),
        }
    }

    fn stops(h: &Harness<'_, Editor>) -> Vec<(f32, Rgba)> {
        match fill(h) {
            Paint::Gradient(g) => g.stops,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn the_gradient_bar_adds_moves_removes_and_colors_stops() {
        let mut h = rectangle_and_color_panel();
        let ctx = h.ctx.clone();
        h.get_by_label("Color type").click();
        h.run();
        h.get_by_label("Linear gradient").click();
        h.run();
        // Black to white, across the selected rectangle.
        let Paint::Gradient(g) = fill(&h) else { panic!("not a gradient") };
        assert_eq!((g.kind, g.stops.clone()), (GradientKind::Linear, vec![(0.0, [0.0, 0.0, 0.0, 1.0]), (1.0, [1.0; 4])]));
        assert_eq!((g.matrix.tx, g.matrix.ty), (200.0, 150.0));
        assert_eq!(h.state().fill_gradient.as_ref().map(|d| d.kind), Some(GradientKind::Linear), "and the drawing default");

        // The bar, inset 5 px so the end pointers show whole; pointers hang under it.
        let rect = h.get_by_label("Gradient bar").rect();
        let x = |t: f32| rect.left() + 5.0 + t * (rect.width() - 10.0);
        let (bar_y, pointer_y) = (rect.top() + 9.0, rect.top() + 26.0);
        // A click on the bar adds a stop in the colour there.
        click_at(&mut h, pos2(x(0.5), bar_y));
        assert_eq!(stops(&h).len(), 3);
        assert!((stops(&h)[1].0 - 0.5).abs() < 0.01 && (stops(&h)[1].1[0] - 0.5).abs() < 0.01, "{:?}", stops(&h));
        // Its pointer drags sideways...
        screen_drag(&mut h, pos2(x(0.5), pointer_y), pos2(x(0.25), pointer_y), egui::Modifiers::NONE);
        assert!((stops(&h)[1].0 - 0.25).abs() < 0.01, "{:?}", stops(&h));
        // ...and the colour fields edit the stop it picked.
        h.get_by_label("R:").click();
        h.run();
        h.get_by_label("R:").type_text("255");
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(color32(stops(&h)[1].1), Color32::from_rgb(255, 128, 128));
        // A double-click opens the swatch popup for it (after the field's click, so as not to make three).
        for _ in 0..40 {
            h.step();
        }
        click_at(&mut h, pos2(x(0.25), pointer_y));
        click_at(&mut h, pos2(x(0.25), pointer_y));
        let grid = h.get_by_label("Swatch grid").rect();
        click_at(&mut h, grid.min + vec2(1.0 + 6.0 * PITCH + 5.0, 1.0 + 6.0 * PITCH + 5.0));
        assert_eq!(color32(stops(&h)[1].1), web216()[6 * COLS + 6]);
        // Dragged down off the panel, the stop goes.
        screen_drag(&mut h, pos2(x(0.25), pointer_y), pos2(x(0.25), pointer_y + 120.0), egui::Modifiers::NONE);
        assert_eq!(stops(&h).len(), 2);

        // Flow and Linear RGB.
        h.get_by_label("Reflect color").click();
        h.run();
        h.get_by_label("Linear RGB").click();
        h.run();
        let Paint::Gradient(g) = fill(&h) else { panic!("not a gradient") };
        assert_eq!((g.spread, g.linear_rgb), (Spread::Reflect, true));

        // Each of those was one undo step, back to the solid fill.
        for _ in 0..7 {
            h.state_mut().run(crate::app::Action::Undo, &ctx);
        }
        assert_eq!(stops(&h).len(), 2);
        assert_eq!(stops(&h), vec![(0.0, [0.0, 0.0, 0.0, 1.0]), (1.0, [1.0; 4])]);
        h.state_mut().run(crate::app::Action::Undo, &ctx);
        assert!(matches!(fill(&h), Paint::Solid(_)), "{:?}", fill(&h));
    }

    /// Writes target/screenshots/color_panel_gradient.png: the Color panel on a
    /// selected rectangle's linear gradient, its middle stop picked, its last half clear.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn color_panel_gradient_screenshot() {
        let builder = Harness::builder().with_size(vec2(1600.0, 900.0)).with_step_dt(1.0 / 60.0).wgpu();
        let mut h = builder.build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        let stops = vec![(0.0, [0.0, 0.4, 0.8, 1.0]), (0.45, [1.0, 1.0, 1.0, 1.0]), (1.0, [1.0, 0.5, 0.0, 0.5])];
        h.state_mut().fill_gradient = Some(Gradient { stops, ..Gradient::new(GradientKind::Linear) });
        h.state_mut().tool = crate::tools::Tool::Rectangle;
        mouse_drag(&mut h, (100.0, 100.0), (400.0, 300.0));
        let ed = h.state_mut();
        (ed.tool, ed.selection, ed.flyout) = (crate::tools::Tool::Selection, ed.all_elements(), Some(crate::panels::Flyout::Color));
        h.run();
        let rect = h.get_by_label("Gradient bar").rect();
        click_at(&mut h, pos2(rect.left() + 5.0 + 0.45 * (rect.width() - 10.0), rect.top() + 26.0));
        h.run();
        screenshot(&mut h, "color_panel_gradient");
    }

    #[test]
    fn the_swatch_popups_gradient_row_and_radial_type() {
        let mut h = rectangle_and_color_panel();
        h.state_mut().flyout = None;
        h.state_mut().props_tab = crate::app::PropsTab::Object;
        h.run();
        // The Object tab's fill chip: its popup ends with the gradient row.
        h.get_by_label("Fill color").click();
        h.run();
        let grid = h.get_by_label("Swatch grid").rect();
        assert_eq!(grid.height(), 13.0 * PITCH + 1.0, "12 rows of solids, then the gradients");
        click_at(&mut h, grid.min + vec2(1.0 + 2.0 * PITCH + 5.0, 1.0 + 12.0 * PITCH + 5.0));
        let Paint::Gradient(g) = fill(&h) else { panic!("not a gradient") };
        assert_eq!((g.kind, g.stops[0].1), (GradientKind::Radial { focal: 0.0 }, [1.0, 0.0, 0.0, 1.0]), "the red radial");
        assert!((g.matrix.a * 2.0 * minnamate_core::gradient::HALF - 200.0).abs() < 1e-3, "as wide as the rectangle: {:?}", g.matrix);
        assert!(h.query_by_label("Swatch grid").is_none());
        // The chip shows it; a solid swatch puts the solid back.
        h.get_by_label("Fill color").click();
        h.run();
        let grid = h.get_by_label("Swatch grid").rect();
        click_at(&mut h, grid.min + vec2(6.0, 6.0));
        assert_eq!(fill(&h), Paint::Solid([0.0, 0.0, 0.0, 1.0]));
        assert!(h.state().fill_gradient.is_none());
    }

    fn type_alpha(h: &mut Harness<'_, Editor>, percent: &str) {
        h.get_by_label("A:").click();
        h.run();
        h.get_by_label("A:").type_text(percent);
        h.key_press(egui::Key::Enter);
        h.run();
    }

    #[test]
    fn a_clear_stop_keeps_its_colour() {
        // The selected rectangle's fill black to white, its white end's stop picked.
        let mut h = rectangle_and_color_panel();
        h.get_by_label("Color type").click();
        h.run();
        h.get_by_label("Linear gradient").click();
        h.run();
        let rect = h.get_by_label("Gradient bar").rect();
        click_at(&mut h, pos2(rect.right() - 5.0, rect.top() + 26.0));
        type_alpha(&mut h, "0");
        assert_eq!(stops(&h)[1].1, [1.0, 1.0, 1.0, 0.0], "white, faded out");
        type_alpha(&mut h, "50");
        assert_eq!(color32(stops(&h)[1].1), Color32::from_rgba_unmultiplied(255, 255, 255, 128), "still white");
        // Chips and pointers show a clear colour as the colour it is.
        assert_eq!(opaque([1.0, 0.5, 0.0, 0.0]), Color32::from_rgb(255, 128, 0));
    }

    #[test]
    fn a_pointer_drag_ends_when_its_gradient_changes_under_it() {
        let mut h = blank();
        let mut three = Gradient::new(GradientKind::Linear);
        three.stops.insert(1, (0.5, [1.0, 0.0, 0.0, 1.0]));
        let two = Gradient::new(GradientKind::Linear);
        (h.state_mut().fill_gradient, h.state_mut().stroke_gradient) = (Some(three), Some(two.clone()));
        h.state_mut().flyout = Some(crate::panels::Flyout::Color);
        h.run();
        let rect = h.get_by_label("Gradient bar").rect();
        let at = |t: f32| pos2(rect.left() + 5.0 + t * (rect.width() - 10.0), rect.top() + 26.0);
        // The third stop's pointer, dragged; X mid-drag puts a two-stop gradient in its place.
        h.hover_at(at(1.0));
        h.step();
        h.drag_at(at(1.0));
        h.step();
        h.hover_at(at(0.9));
        h.step();
        h.key_press(egui::Key::X);
        h.step();
        h.hover_at(at(0.8));
        h.step();
        h.drop_at(at(0.8));
        h.step();
        assert_eq!(h.state().fill_gradient.as_ref().map(|g| &g.stops), Some(&two.stops), "left as it was");
    }

    #[test]
    fn the_panels_chips_show_the_selection_as_the_rest_of_it_does() {
        let mut h = rectangle_and_color_panel();
        // A red rectangle selected, the drawing default blue.
        h.state_mut().props_tab = crate::app::PropsTab::Doc;
        restyle(h.state_mut(), Style::Fill(Color32::RED));
        h.state_mut().fill_color = Color32::BLUE;
        h.run();
        // The panel's fill chip (its bucket button shares the name), its popup's alpha at half: the rectangle's red at half.
        let chip = h.get_all_by_label("Fill color").map(|n| n.rect()).find(|r| r.width() == 30.0).unwrap();
        click_at(&mut h, chip.center());
        h.get_by_label("Alpha").click();
        h.run();
        h.get_by_label("Alpha").type_text("50");
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(fill(&h), Paint::Solid(rgba(Color32::from_rgba_unmultiplied(255, 0, 0, 128))));
    }
}
