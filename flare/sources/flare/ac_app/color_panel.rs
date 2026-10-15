//! Color panel (Window › Color, Ctrl+Shift+F9): the stroke or fill as None,
//! Solid, Linear gradient, Radial gradient or Bitmap; a colour picker with
//! HSB, RGB, hex and alpha fields; a gradient ramp (click to add a stop,
//! drag to move, drag down off the ramp to remove), flow and focal point;
//! bitmap fills from the library. Changes set the tools' colours and, with
//! something selected, apply to it when the mouse is let go (one undo step).
//! Our own code; the panel's parts follow Animate's Color panel.

use crate::{c32, from32, App};
use ac_core::{ElementKind, Gradient, GradientKind, Spread};
use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Stroke};

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Kind {
    None,
    #[default]
    Solid,
    Linear,
    Radial,
    Bitmap,
}

#[derive(Default)]
pub(crate) struct ColorPanel {
    pub open: bool,
    /// Editing the stroke (else the fill).
    pub stroke: bool,
    /// Selected gradient stop.
    stop: usize,
    /// Stop being dragged on the ramp.
    dragging: Option<usize>,
    /// A change waiting for the mouse to be let go before it goes to the selection.
    pending: bool,
    flow: Spread,
    focal: f32,
}

fn fill_kind(app: &App) -> Kind {
    if !app.fill_on {
        return Kind::None;
    }
    match app.topts.fill_gradient {
        Some(GradientKind::Linear) => Kind::Linear,
        Some(GradientKind::Radial) => Kind::Radial,
        None => Kind::Solid,
    }
}

/// HSB (h in degrees, s and b 0…1) of a colour.
fn to_hsb(c: Color32) -> (f32, f32, f32) {
    let (r, g, b) = (c.r() as f32 / 255.0, c.g() as f32 / 255.0, c.b() as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    let h = if d < 1e-6 {
        0.0
    } else if mx == r {
        60.0 * ((g - b) / d).rem_euclid(6.0)
    } else if mx == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (h, if mx > 0.0 { d / mx } else { 0.0 }, mx)
}

fn from_hsb(h: f32, s: f32, v: f32, a: u8) -> Color32 {
    let c = v * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    let q = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0).round() as u8;
    Color32::from_rgba_unmultiplied(q(r), q(g), q(b), a)
}

/// Colour picker plus HSB / RGB / hex / alpha fields. True when changed.
fn color_fields(ui: &mut egui::Ui, c: &mut Color32) -> bool {
    let mut changed = egui::widgets::color_picker::color_picker_color32(ui, c, egui::widgets::color_picker::Alpha::OnlyBlend);
    let [r0, g0, b0, a0] = c.to_srgba_unmultiplied();
    let (mut h, mut s, mut v) = to_hsb(*c);
    let (mut r, mut g, mut b, mut a) = (r0, g0, b0, (a0 as f32 / 2.55).round());
    egui::Grid::new("color-fields").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
        ui.label("H");
        let hh = ui.add(egui::DragValue::new(&mut h).range(0.0..=360.0).suffix("°").max_decimals(0)).changed();
        ui.label("R");
        let rr = ui.add(egui::DragValue::new(&mut r)).changed();
        ui.end_row();
        let mut sp = s * 100.0;
        let mut vp = v * 100.0;
        ui.label("S");
        let ss = ui.add(egui::DragValue::new(&mut sp).range(0.0..=100.0).suffix("%").max_decimals(0)).changed();
        ui.label("G");
        let gg = ui.add(egui::DragValue::new(&mut g)).changed();
        ui.end_row();
        ui.label("B");
        let bb = ui.add(egui::DragValue::new(&mut vp).range(0.0..=100.0).suffix("%").max_decimals(0)).changed();
        ui.label("B");
        let bl = ui.add(egui::DragValue::new(&mut b)).changed();
        ui.end_row();
        if hh || ss || bb {
            s = sp / 100.0;
            v = vp / 100.0;
            *c = from_hsb(h, s, v, a0);
            changed = true;
        } else if rr || gg || bl {
            *c = Color32::from_rgba_unmultiplied(r, g, b, a0);
            changed = true;
        }
        ui.label("#");
        let [cr, cg, cb, ca] = c.to_srgba_unmultiplied();
        let mut hex = format!("{cr:02X}{cg:02X}{cb:02X}");
        let he = ui.add(egui::TextEdit::singleline(&mut hex).desired_width(76.0).char_limit(6));
        if he.changed() && hex.len() == 6 {
            if let Ok(n) = u32::from_str_radix(&hex, 16) {
                *c = Color32::from_rgba_unmultiplied((n >> 16) as u8, (n >> 8) as u8, n as u8, ca);
                changed = true;
            }
        }
        ui.label("A");
        if ui.add(egui::DragValue::new(&mut a).range(0.0..=100.0).suffix("%").max_decimals(0)).changed() {
            let [r, g, b, _] = c.to_srgba_unmultiplied();
            *c = Color32::from_rgba_unmultiplied(r, g, b, (a * 2.55).round() as u8);
            changed = true;
        }
        ui.end_row();
    });
    changed
}

fn sample(stops: &[(f32, ac_core::Color)], t: f32) -> Color32 {
    if t <= stops[0].0 {
        return c32(stops[0].1);
    }
    for w in stops.windows(2) {
        if t <= w[1].0 {
            let u = if w[1].0 > w[0].0 { (t - w[0].0) / (w[1].0 - w[0].0) } else { 0.0 };
            return c32(w[0].1.lerp(w[1].1, u));
        }
    }
    c32(stops[stops.len() - 1].1)
}

/// The gradient ramp with its stop markers. True when the stops changed.
fn ramp(ui: &mut egui::Ui, p: &mut ColorPanel, stops: &mut Vec<(f32, ac_core::Color)>) -> bool {
    let w = ui.available_width().min(260.0);
    let (r, resp) = ui.allocate_exact_size(vec2(w, 34.0), Sense::click_and_drag());
    let bar = egui::Rect::from_min_size(r.min, vec2(w, 18.0));
    let painter = ui.painter_at(r.expand(2.0));
    let n = 48;
    for i in 0..n {
        let t = (i as f32 + 0.5) / n as f32;
        painter.rect_filled(egui::Rect::from_min_size(pos2(bar.left() + bar.width() * i as f32 / n as f32, bar.top()), vec2(bar.width() / n as f32 + 0.5, bar.height())), 0.0, sample(stops, t));
    }
    painter.rect_stroke(bar, 0.0, Stroke::new(1.0_f32, Color32::from_gray(90)));
    let x_of = |t: f32| bar.left() + bar.width() * t;
    for (i, (t, c)) in stops.iter().enumerate() {
        let x = x_of(*t);
        let tri = vec![pos2(x, bar.bottom() + 1.0), pos2(x - 6.0, bar.bottom() + 10.0), pos2(x + 6.0, bar.bottom() + 10.0)];
        painter.add(egui::Shape::convex_polygon(tri, if i == p.stop { Color32::WHITE } else { Color32::from_gray(150) }, Stroke::NONE));
        painter.rect_filled(egui::Rect::from_min_size(pos2(x - 6.0, bar.bottom() + 10.0), vec2(12.0, 6.0)), 0.0, c32(*c));
        painter.rect_stroke(egui::Rect::from_min_size(pos2(x - 6.0, bar.bottom() + 10.0), vec2(12.0, 6.0)), 0.0, Stroke::new(1.0_f32, if i == p.stop { Color32::WHITE } else { Color32::from_gray(80) }));
    }
    let mut changed = false;
    let pos = resp.interact_pointer_pos();
    if resp.drag_started() || resp.clicked() {
        if let Some(q) = pos {
            let near = stops.iter().enumerate().map(|(i, (t, _))| (i, (x_of(*t) - q.x).abs())).filter(|(_, d)| *d < 8.0).min_by(|a, b| a.1.total_cmp(&b.1)).map(|(i, _)| i);
            match near {
                Some(i) => p.stop = i,
                None if q.y > bar.bottom() || resp.clicked() => {
                    // a new stop with the colour already there
                    let t = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                    if stops.len() < 15 {
                        stops.push((t, from32(sample(stops, t))));
                        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                        p.stop = stops.iter().position(|s| s.0 == t).unwrap_or(0);
                        changed = true;
                    }
                }
                None => {}
            }
            if resp.drag_started() {
                p.dragging = Some(p.stop);
            }
        }
    }
    if let (true, Some(i), Some(q)) = (resp.dragged(), p.dragging, pos) {
        if i < stops.len() {
            if q.y > r.bottom() + 24.0 && stops.len() > 2 {
                // dragged off the ramp: remove it
                stops.remove(i);
                p.stop = i.min(stops.len() - 1);
                p.dragging = None;
            } else {
                let t = ((q.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
                let col = stops[i].1;
                stops[i].0 = t;
                stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                p.stop = stops.iter().position(|s| s.0 == t && s.1 == col).unwrap_or(0);
                p.dragging = Some(p.stop);
            }
            changed = true;
        }
    }
    if resp.drag_stopped() {
        p.dragging = None;
    }
    resp.on_hover_text("Click to add a colour · drag a marker to move it · drag it down to remove it");
    changed
}

/// Put the panel's state on the selection (one undo step).
fn apply_to_selection(app: &mut App) {
    if app.ed.selection.is_empty() {
        return;
    }
    if app.colors.stroke {
        let c = app.stroke;
        if app.stroke_on {
            crate::swatches::use_color(app, c, true);
        } else {
            let has = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Shape(_))));
            if has {
                app.ed.modify_selected("No Stroke", |e| {
                    if let ElementKind::Shape(s) = &mut e.kind {
                        s.stroke = None;
                    }
                });
            }
        }
        return;
    }
    match fill_kind(app) {
        Kind::None => {
            app.ed.modify_selected("No Fill", |e| {
                if let ElementKind::Shape(s) = &mut e.kind {
                    s.fill = None;
                    s.gradient = None;
                    s.bitmap_fill = None;
                }
            });
        }
        Kind::Solid => {
            let c = app.fill;
            crate::swatches::use_color(app, c, false);
        }
        Kind::Linear | Kind::Radial => {
            let kind = if fill_kind(app) == Kind::Linear { GradientKind::Linear } else { GradientKind::Radial };
            let g = Gradient { spread: app.colors.flow, focal: app.colors.focal, ..Gradient::fit(kind, app.fill_stops(), ac_core::pt(0.0, 0.0), ac_core::pt(1.0, 1.0)) };
            let sw = ac_core::Swatch { name: String::new(), color: g.stops[0].1, gradient: Some(g) };
            crate::swatches::use_swatch(app, &sw, false);
        }
        Kind::Bitmap => {}
    }
}

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    if !app.colors.open {
        return;
    }
    let mut open = true;
    let mut changed = false;
    egui::Window::new("Color").open(&mut open).resizable(false).default_width(270.0).default_pos(app.stage_rect.right_top() + vec2(-300.0, 30.0)).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.colors.stroke, true, "Stroke");
            ui.selectable_value(&mut app.colors.stroke, false, "Fill");
            ui.separator();
            if ui.small_button("Swap").on_hover_text("Swap stroke and fill colours").clicked() {
                std::mem::swap(&mut app.fill, &mut app.stroke);
                changed = true;
            }
        });
        let mut kind = if app.colors.stroke { if app.stroke_on { Kind::Solid } else { Kind::None } } else { fill_kind(app) };
        let before = kind;
        ui.horizontal(|ui| {
            ui.label("Type");
            egui::ComboBox::from_id_salt("color-kind")
                .selected_text(match kind { Kind::None => "None", Kind::Solid => "Solid colour", Kind::Linear => "Linear gradient", Kind::Radial => "Radial gradient", Kind::Bitmap => "Bitmap fill" })
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut kind, Kind::None, "None");
                    ui.selectable_value(&mut kind, Kind::Solid, "Solid colour");
                    if !app.colors.stroke {
                        ui.selectable_value(&mut kind, Kind::Linear, "Linear gradient");
                        ui.selectable_value(&mut kind, Kind::Radial, "Radial gradient");
                        ui.selectable_value(&mut kind, Kind::Bitmap, "Bitmap fill");
                    }
                });
        });
        if kind != before {
            changed = true;
            if app.colors.stroke {
                app.stroke_on = kind != Kind::None;
            } else {
                app.fill_on = kind != Kind::None;
                app.topts.fill_gradient = match kind {
                    Kind::Linear => Some(GradientKind::Linear),
                    Kind::Radial => Some(GradientKind::Radial),
                    _ => None,
                };
            }
        }
        ui.separator();
        match kind {
            Kind::None => {
                ui.label(RichText::new(if app.colors.stroke { "No stroke" } else { "No fill" }).color(Color32::from_gray(150)));
            }
            Kind::Solid => {
                let mut c = if app.colors.stroke { app.stroke } else { app.fill };
                if color_fields(ui, &mut c) {
                    if app.colors.stroke {
                        app.stroke = c;
                    } else {
                        app.fill = c;
                    }
                    changed = true;
                }
            }
            Kind::Linear | Kind::Radial => {
                let mut stops = app.fill_stops();
                let mut sc = changed;
                sc |= ramp(ui, &mut app.colors, &mut stops);
                let i = app.colors.stop.min(stops.len() - 1);
                let mut c = c32(stops[i].1);
                if color_fields(ui, &mut c) {
                    stops[i].1 = from32(c);
                    sc = true;
                }
                ui.horizontal(|ui| {
                    ui.label("Flow");
                    for s in Spread::ALL {
                        if ui.selectable_label(app.colors.flow == s, s.name()).clicked() {
                            app.colors.flow = s;
                            sc = true;
                        }
                    }
                });
                if kind == Kind::Radial {
                    sc |= ui.add(egui::Slider::new(&mut app.colors.focal, -1.0..=1.0).text("Focal point").max_decimals(2)).changed();
                }
                if sc {
                    app.fill = c32(stops[0].1);
                    app.topts.fill2 = c32(stops[stops.len() - 1].1);
                    app.topts.grad_stops = Some(stops);
                    changed = true;
                }
            }
            Kind::Bitmap => {
                if app.ed.doc.bitmaps.is_empty() {
                    ui.label(RichText::new("No bitmaps in the library. Import one first (File › Import).").color(Color32::from_gray(150)));
                }
                let list: Vec<(u32, String)> = app.ed.doc.bitmaps.iter().map(|b| (b.id, b.name.clone())).collect();
                egui::ScrollArea::vertical().max_height(160.0).show(ui, |ui| {
                    for (id, name) in list {
                        if ui.selectable_label(false, format!("🖼 {name}")).on_hover_text("Fill the selection with this bitmap").clicked() {
                            let has_canvas = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Canvas(_))));
                            if has_canvas {
                                app.ed.merge_bitmap_fill(Some(id));
                            }
                            app.ed.modify_selected("Bitmap Fill", move |e| {
                                if let ElementKind::Shape(s) = &mut e.kind {
                                    let at = s.path.bounds().map(|(lo, _)| lo).unwrap_or(ac_core::pt(0.0, 0.0));
                                    s.bitmap_fill = Some(ac_core::BitmapFill { bitmap: id, matrix: ac_core::Matrix::translate(at.x, at.y) });
                                    s.gradient = None;
                                }
                            });
                        }
                    }
                });
            }
        }
    });
    if changed {
        app.colors.pending = true;
    }
    if app.colors.pending && !ctx.input(|i| i.pointer.any_down()) {
        app.colors.pending = false;
        apply_to_selection(app);
    }
    if !open {
        app.colors.open = false;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn hsb_round_trip() {
        use super::*;
        for c in [Color32::from_rgb(255, 0, 0), Color32::from_rgb(12, 200, 99), Color32::from_rgb(128, 128, 128), Color32::from_rgb(0, 0, 0)] {
            let (h, s, v) = to_hsb(c);
            assert_eq!(from_hsb(h, s, v, 255), c, "{c:?}");
        }
    }
}
