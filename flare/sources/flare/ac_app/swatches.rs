//! Swatches: a column of colour chips beside the Tools panel (Window ›
//! Swatches, Ctrl+F9). Click a chip for the fill, Shift+click for the
//! stroke; with something selected it is applied to it too. Gradient chips
//! set a gradient fill. Right-click a chip to replace, use or delete it.
//!
//! A document can carry its own swatches (Animate keeps them in the .fla,
//! and .fla import reads them); otherwise the app's set (in the settings) is
//! shown. Palettes import from Photoshop `.aco`, Adobe Swatch Exchange `.ase`
//! and Photoshop gradients `.grd` (readers and writers from PhotoCraft's
//! `photocraft-psd`, MIT OR Apache-2.0, used as a git dependency), GIMP `.gpl`
//! and colour tables `.act` (our own readers), and save as `.ase`, `.aco`,
//! `.gpl` (colours) or `.grd` (gradients).

use crate::{c32, from32, App};
use ac_core::{Color, ElementKind, Gradient, GradientKind, Stroke, Swatch};
use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense};

fn rgba(r: u8, g: u8, b: u8, a: u8) -> Color {
    Color { a, ..Color::rgb(r, g, b) }
}

fn label(s: &Swatch) -> String {
    let c = s.color;
    let hex = if c.a == 255 { format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b) } else { format!("#{:02X}{:02X}{:02X} · {:.0}%", c.r, c.g, c.b, c.a as f32 / 2.55) };
    let what = match &s.gradient {
        Some(g) => format!("{} gradient, {} colours", if g.kind == GradientKind::Linear { "Linear" } else { "Radial" }, g.stops.len()),
        None => hex,
    };
    if s.name.is_empty() { what } else { format!("{}  {what}", s.name) }
}

/// The standard set: a grey ramp, then twelve hues at full, light and dark,
/// and a few gradients.
pub(crate) fn defaults() -> Vec<Swatch> {
    let mut v = Vec::new();
    for i in 0..12u32 {
        let g = (i * 255 / 11) as u8;
        v.push(Swatch::solid("", Color::rgb(g, g, g)));
    }
    for (s, b) in [(1.0, 1.0), (0.45, 1.0), (1.0, 0.55)] {
        for h in 0..12 {
            let [r, g, bl] = hsb(h as f32 * 30.0, s, b);
            v.push(Swatch::solid("", Color::rgb(r, g, bl)));
        }
    }
    let (k, w) = (Color::rgb(0, 0, 0), Color::rgb(255, 255, 255));
    v.push(Swatch::gradient("Black to white", GradientKind::Linear, vec![(0.0, k), (1.0, w)]));
    v.push(Swatch::gradient("White to black", GradientKind::Radial, vec![(0.0, w), (1.0, k)]));
    v.push(Swatch::gradient("Sunset", GradientKind::Linear, vec![(0.0, Color::rgb(0x2B, 0x1B, 0x6B)), (0.5, Color::rgb(0xE0, 0x4A, 0x5F)), (1.0, Color::rgb(0xFF, 0xC8, 0x57))]));
    v.push(Swatch::gradient("Sky", GradientKind::Linear, vec![(0.0, Color::rgb(0x1E, 0x6B, 0xD6)), (1.0, Color::rgb(0xC9, 0xE8, 0xFF))]));
    v.push(Swatch::gradient("Glow", GradientKind::Radial, vec![(0.0, Color::rgb(0xFF, 0xF4, 0xB0)), (1.0, rgba(0xFF, 0xA0, 0x20, 0))]));
    v.push(Swatch::gradient("Spectrum", GradientKind::Linear, (0..=6).map(|i| { let [r, g, b] = hsb(i as f32 * 60.0, 1.0, 1.0); (i as f32 / 6.0, Color::rgb(r, g, b)) }).collect()));
    v
}

/// A Photoshop gradient as a gradient swatch: colour stops, with the
/// opacity stops sampled at each colour stop.
fn grd_swatch(g: &photocraft_psd::grd::GrdGradient) -> Swatch {
    use photocraft_psd::grd::GrdColor;
    let opacity_at = |t: f32| -> f32 {
        let o = &g.opacity;
        match o.len() {
            0 => 1.0,
            _ if t <= o[0].0 => o[0].1,
            _ if t >= o[o.len() - 1].0 => o[o.len() - 1].1,
            _ => {
                let i = o.windows(2).position(|w| t >= w[0].0 && t <= w[1].0).unwrap_or(0);
                let (a, b) = (o[i], o[i + 1]);
                let u = if b.0 > a.0 { (t - a.0) / (b.0 - a.0) } else { 0.0 };
                a.1 + (b.1 - a.1) * u
            }
        }
    };
    let stops = g
        .stops
        .iter()
        .map(|s| {
            let [r, gg, b] = match s.color {
                GrdColor::Rgb([r, g, b]) => [to8(r), to8(g), to8(b)],
                GrdColor::Hsb([h, s, b]) => hsb(h, s, b),
                GrdColor::Cmyk([c, m, y, k]) => cmyk(c, m, y, k),
                GrdColor::Lab([l, a, b]) => lab(l, a, b),
                GrdColor::Gray(k) => [to8(1.0 - k); 3],
                GrdColor::Foreground => [0, 0, 0],
                GrdColor::Background => [255, 255, 255],
            };
            let t = s.location.clamp(0.0, 1.0);
            (t, rgba(r, gg, b, to8(opacity_at(t))))
        })
        .collect();
    Swatch::gradient(g.name.clone(), GradientKind::Linear, stops)
}

fn hsb(h: f32, s: f32, b: f32) -> [u8; 3] {
    let c = b * s;
    let hp = (h.rem_euclid(360.0)) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, bl) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = b - c;
    [to8(r + m), to8(g + m), to8(bl + m)]
}

fn to8(v: f32) -> u8 {
    (v.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// CIE L*a*b* (D50, as Photoshop and ASE store it) to sRGB.
fn lab(l: f32, a: f32, b: f32) -> [u8; 3] {
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let f = |t: f32| if t > 6.0 / 29.0 { t * t * t } else { 3.0 * (6.0f32 / 29.0).powi(2) * (t - 4.0 / 29.0) };
    let (x, y, z) = (0.964_22 * f(fx), f(fy), 0.825_21 * f(fz));
    // XYZ (D50) → linear sRGB with Bradford adaptation to D65
    let r = 3.133_856 * x - 1.616_867 * y - 0.490_615 * z;
    let g = -0.978_768 * x + 1.916_142 * y + 0.033_454 * z;
    let bl = 0.071_945 * x - 0.228_991 * y + 1.405_243 * z;
    let enc = |c: f32| if c <= 0.003_130_8 { 12.92 * c } else { 1.055 * c.powf(1.0 / 2.4) - 0.055 };
    [to8(enc(r)), to8(enc(g)), to8(enc(bl))]
}

fn cmyk(c: f32, m: f32, y: f32, k: f32) -> [u8; 3] {
    [to8((1.0 - c) * (1.0 - k)), to8((1.0 - m) * (1.0 - k)), to8((1.0 - y) * (1.0 - k))]
}

/// Read a palette file by its extension.
pub(crate) fn read(path: &std::path::Path) -> Result<Vec<Swatch>, String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let v = parse(&ext, &data)?;
    if v.is_empty() {
        return Err("no colours in that file".into());
    }
    Ok(v)
}

pub(crate) fn parse(ext: &str, data: &[u8]) -> Result<Vec<Swatch>, String> {
    use photocraft_psd::{aco, ase};
    let rgb = |n: &str, [r, g, b]: [u8; 3]| Swatch::solid(n, Color::rgb(r, g, b));
    match ext {
        "aco" => {
            let f = aco::parse(data).map_err(|e| format!(".aco: {e}"))?;
            Ok(f.swatches
                .iter()
                .filter_map(|s| {
                    let n = |v: u16| v as f32 / 65535.0;
                    let c = match s.color {
                        aco::AcoColor::Rgb([r, g, b]) => [(r >> 8) as u8, (g >> 8) as u8, (b >> 8) as u8],
                        aco::AcoColor::Hsb([h, sa, b]) => hsb(n(h) * 360.0, n(sa), n(b)),
                        aco::AcoColor::Cmyk([c, m, y, k]) => cmyk(n(c), n(m), n(y), n(k)),
                        aco::AcoColor::Lab { l, a, b } => lab(l as f32 / 100.0, a as f32 / 100.0, b as f32 / 100.0),
                        aco::AcoColor::Gray(g) => [to8(g as f32 / 10000.0); 3],
                        aco::AcoColor::Other { .. } => return None,
                    };
                    Some(rgb(&s.name, c))
                })
                .collect())
        }
        "ase" => {
            let f = ase::parse(data).map_err(|e| format!(".ase: {e}"))?;
            let one = |s: &ase::AseSwatch| -> Option<Swatch> {
                let c = match &s.color {
                    ase::AseColor::Rgb([r, g, b]) => [to8(*r), to8(*g), to8(*b)],
                    ase::AseColor::Cmyk([c, m, y, k]) => cmyk(*c, *m, *y, *k),
                    ase::AseColor::Lab([l, a, b]) => lab(l * 100.0, *a, *b),
                    ase::AseColor::Gray(g) => [to8(*g); 3],
                    ase::AseColor::Other { .. } => return None,
                };
                Some(rgb(&s.name, c))
            };
            Ok(f.entries
                .iter()
                .flat_map(|e| match e {
                    ase::AseEntry::Swatch(s) => vec![s],
                    ase::AseEntry::Group { swatches, .. } => swatches.iter().collect(),
                })
                .filter_map(one)
                .collect())
        }
        "gpl" => {
            let text = String::from_utf8_lossy(data);
            let mut lines = text.lines();
            if !lines.next().is_some_and(|l| l.trim_start_matches('\u{feff}').trim() == "GIMP Palette") {
                return Err(".gpl: not a GIMP palette (first line must be \"GIMP Palette\")".into());
            }
            Ok(lines
                .filter_map(|l| {
                    let l = l.trim();
                    if l.is_empty() || l.starts_with('#') || l.contains(':') && !l.starts_with(|c: char| c.is_ascii_digit()) {
                        return None; // comments and "Name:" / "Columns:" headers
                    }
                    let mut it = l.split_whitespace();
                    let mut ch = [0u8; 3];
                    for c in &mut ch {
                        *c = it.next()?.parse().ok()?;
                    }
                    Some(rgb(&it.collect::<Vec<_>>().join(" "), ch))
                })
                .collect())
        }
        "act" => {
            // 256 RGB triples, then (optional) a big-endian count of used colours
            if data.len() < 768 {
                return Err(".act: a colour table is 768 bytes".into());
            }
            let n = if data.len() >= 770 { u16::from_be_bytes([data[768], data[769]]).clamp(1, 256) as usize } else { 256 };
            Ok(data[..n * 3].chunks(3).map(|c| rgb("", [c[0], c[1], c[2]])).collect())
        }
        "grd" => {
            // Photoshop gradients → gradient swatches (PhotoCraft's reader)
            let gs = photocraft_psd::grd::parse(data).map_err(|e| format!(".grd: {e}"))?;
            Ok(gs.iter().filter(|g| !g.noise && !g.stops.is_empty()).map(grd_swatch).collect())
        }
        _ => Err(format!("unknown palette type .{ext} (use .aco, .ase, .gpl, .act or .grd)")),
    }
}


/// Write a palette by extension: colours as .ase, .aco or .gpl; gradients as .grd.
pub(crate) fn write(path: &std::path::Path, v: &[Swatch]) -> Result<usize, String> {
    use photocraft_psd::{aco, ase, grd};
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    let solid: Vec<&Swatch> = v.iter().filter(|s| s.gradient.is_none()).collect();
    let (bytes, n) = match ext.as_str() {
        "ase" => {
            let entries = solid.iter().map(|s| ase::AseEntry::Swatch(ase::AseSwatch { name: label(s), color: ase::AseColor::Rgb([s.color.r as f32 / 255.0, s.color.g as f32 / 255.0, s.color.b as f32 / 255.0]), kind: ase::AseKind::Process })).collect();
            (ase::write(&ase::AseFile { entries }).map_err(|e| e.to_string())?, solid.len())
        }
        "aco" => {
            let sw: Vec<aco::AcoSwatch> = solid.iter().map(|s| aco::AcoSwatch { name: label(s), color: aco::AcoColor::Rgb([s.color.r as u16 * 257, s.color.g as u16 * 257, s.color.b as u16 * 257]) }).collect();
            (aco::write(&sw).map_err(|e| e.to_string())?, solid.len())
        }
        "gpl" => {
            let name = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Flare");
            let mut t = format!("GIMP Palette\nName: {name}\nColumns: 12\n#\n");
            for s in &solid {
                let c = s.color;
                t.push_str(&format!("{:3} {:3} {:3}\t{}\n", c.r, c.g, c.b, if s.name.is_empty() { format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b) } else { s.name.clone() }));
            }
            (t.into_bytes(), solid.len())
        }
        "grd" => {
            let gs: Vec<grd::GrdGradient> = v
                .iter()
                .filter_map(|s| s.gradient.as_ref().map(|g| (s, g)))
                .enumerate()
                .map(|(i, (s, g))| grd::GrdGradient {
                    name: if s.name.is_empty() { format!("Gradient {}", i + 1) } else { s.name.clone() },
                    noise: false,
                    stops: g.stops.iter().map(|(t, c)| grd::GrdStop { location: *t, midpoint: 0.5, color: grd::GrdColor::Rgb([c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0]) }).collect(),
                    opacity: g.stops.iter().map(|(t, c)| (*t, c.a as f32 / 255.0, 0.5)).collect(),
                })
                .collect();
            let n = gs.len();
            (grd::write(&gs), n)
        }
        _ => return Err(format!("can't write .{ext} (use .ase, .aco, .gpl, or .grd for gradients)")),
    };
    if n == 0 {
        return Err(if ext == "grd" { "no gradient swatches to save".into() } else { "no colour swatches to save (gradients go in .grd)".into() });
    }
    std::fs::write(path, bytes).map_err(|e| e.to_string())?;
    Ok(n)
}

#[derive(Default)]
pub(crate) struct State {
    pub open: bool,
    /// Import replaces the swatches instead of adding to them.
    pub replace: bool,
}

/// The swatches shown: the document's own, else the app's.
pub(crate) fn current(app: &App) -> Vec<Swatch> {
    app.ed.doc.swatches.clone().or_else(|| app.settings.swatches.clone()).unwrap_or_else(defaults)
}

/// Store an edited list where it came from (the document: one undo step).
fn store(app: &mut App, label: &str, list: Vec<Swatch>) {
    if app.ed.doc.swatches.is_some() {
        app.ed.set_swatches(label, Some(list));
    } else {
        app.settings.swatches = Some(list);
        app.settings.save();
    }
}

/// Use a swatch as the fill (or stroke) and apply it to the selection.
pub(crate) fn use_swatch(app: &mut App, s: &Swatch, stroke: bool) {
    match (&s.gradient, stroke) {
        (Some(g), false) => use_gradient(app, g),
        _ => use_color(app, c32(s.color), stroke),
    }
}

fn use_gradient(app: &mut App, g: &Gradient) {
    let stops = g.stops.clone();
    app.fill = c32(stops[0].1);
    app.fill_on = true;
    app.topts.fill2 = c32(stops[stops.len() - 1].1);
    app.topts.fill_gradient = Some(g.kind);
    app.topts.grad_stops = Some(stops.clone());
    if app.ed.selection.is_empty() {
        return;
    }
    let (kind, focal, spread) = (g.kind, g.focal, g.spread);
    let fit = move |lo, hi| Gradient { focal, spread, ..Gradient::fit(kind, stops.clone(), lo, hi) };
    let has_canvas = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Canvas(_))));
    if has_canvas {
        if let Some((lo, hi)) = app.ed.selection_bounds() {
            app.ed.merge_set_paint(None, Some(Some(fit(lo, hi))), None);
        }
    }
    if app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Shape(_)))) {
        app.ed.modify_selected("Fill Gradient", move |e| {
            if let ElementKind::Shape(sh) = &mut e.kind {
                if let Some((lo, hi)) = sh.path.bounds() {
                    sh.gradient = Some(fit(lo, hi));
                    sh.fill = sh.gradient.as_ref().map(|g| g.stops[0].1);
                }
            }
        });
    }
}

/// Use a colour as the fill (or stroke) and apply it to the selection.
pub(crate) fn use_color(app: &mut App, c: Color32, stroke: bool) {
    let col = from32(c);
    if stroke {
        app.stroke = c;
        app.stroke_on = true;
    } else {
        app.fill = c;
        app.fill_on = true;
        app.topts.fill_gradient = None;
    }
    if app.ed.selection.is_empty() {
        return;
    }
    let has_canvas = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Canvas(_))));
    if has_canvas {
        if stroke {
            let w = app.stroke_w;
            app.ed.merge_set_paint(None, None, Some(Some(Stroke { color: col, width: w })));
        } else {
            app.ed.merge_set_paint(Some(Some(col)), Some(None), None);
        }
    }
    let w = app.stroke_w;
    let others = app.ed.selection.iter().any(|s| app.ed.element_at(*s).is_some_and(|e| matches!(e.kind, ElementKind::Shape(_) | ElementKind::Text(_))));
    if others {
        app.ed.modify_selected(if stroke { "Stroke Color" } else { "Fill Color" }, move |e| match &mut e.kind {
            ElementKind::Shape(s) if stroke => s.stroke = Some(Stroke { color: col, width: s.stroke.map_or(w, |st| st.width) }),
            ElementKind::Shape(s) => {
                s.fill = Some(col);
                s.gradient = None;
            }
            ElementKind::Text(t) if !stroke => t.color = col,
            _ => {}
        });
    }
}

/// Paint a chip: the colour (on a checkerboard when see-through) or the gradient.
fn chip(ui: &egui::Ui, r: egui::Rect, s: &Swatch) {
    let p = ui.painter();
    p.rect_filled(r, 0.0, Color32::from_gray(205));
    let q = r.size() / 2.0;
    p.rect_filled(egui::Rect::from_min_size(r.min, q), 0.0, Color32::WHITE);
    p.rect_filled(egui::Rect::from_min_size(r.min + q, q), 0.0, Color32::WHITE);
    match &s.gradient {
        None => {
            p.rect_filled(r, 0.0, c32(s.color));
        }
        Some(g) => {
            let sample = |t: f32| -> Color32 {
                let st = &g.stops;
                if t <= st[0].0 {
                    return c32(st[0].1);
                }
                for w in st.windows(2) {
                    if t <= w[1].0 {
                        let u = if w[1].0 > w[0].0 { (t - w[0].0) / (w[1].0 - w[0].0) } else { 0.0 };
                        return c32(w[0].1.lerp(w[1].1, u));
                    }
                }
                c32(st[st.len() - 1].1)
            };
            let n = 12;
            if g.kind == GradientKind::Radial {
                for i in (0..n).rev() {
                    let t = (i as f32 + 0.5) / n as f32;
                    p.circle_filled(r.center(), r.width() * 0.5 * (i + 1) as f32 / n as f32 * 1.15, sample(t));
                }
            } else {
                let w = r.width() / n as f32;
                for i in 0..n {
                    let t = (i as f32 + 0.5) / n as f32;
                    p.rect_filled(egui::Rect::from_min_size(pos2(r.left() + i as f32 * w, r.top()), vec2(w + 0.5, r.height())), 0.0, sample(t));
                }
            }
        }
    }
}

/// The swatch column beside the tools.
pub(crate) fn panel(app: &mut App, ctx: &egui::Context) {
    if !app.swatch_ui.open {
        return;
    }
    egui::SidePanel::new(app.layout.tools_side(), "swatches").exact_width(138.0).resizable(false).show(ctx, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Swatches").strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").on_hover_text("Close (Ctrl+F9)").clicked() {
                    app.toggle_swatches();
                }
                ui.menu_button("…", |ui| menu(app, ui)).response.on_hover_text("Swatch options");
            });
        });
        let doc_own = app.ed.doc.swatches.is_some();
        ui.label(RichText::new(if doc_own { "This document's swatches" } else { "Click: fill · Shift+click: stroke" }).small().color(if doc_own { Color32::from_rgb(0x6C, 0xB4, 0xFF) } else { Color32::from_gray(140) }))
            .on_hover_text(if doc_own { "Saved with the document (… › Use App Swatches to switch back)" } else { "Click: fill · Shift+click: stroke · right-click: more" });
        ui.add_space(2.0);
        let list = current(app);
        let mut action: Option<(usize, u8)> = None; // 0 fill, 1 stroke, 2 replace, 3 delete
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let cell = 18.0;
            let cols = ((ui.available_width() + 2.0) / (cell + 2.0)).floor().max(1.0) as usize;
            for row in list.chunks(cols).enumerate() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = vec2(2.0, 2.0);
                    for (j, s) in row.1.iter().enumerate() {
                        let i = row.0 * cols + j;
                        let (r, resp) = ui.allocate_exact_size(vec2(cell, cell), Sense::click());
                        chip(ui, r, s);
                        let c = c32(s.color);
                        let current = s.gradient.is_none() && (c == app.fill || c == app.stroke);
                        let edge = if resp.hovered() { Color32::WHITE } else if current { Color32::from_rgb(0x3D, 0x8E, 0xF0) } else { Color32::from_gray(70) };
                        ui.painter().rect_stroke(r, 0.0, egui::Stroke::new(1.0_f32, edge));
                        let resp = resp.on_hover_text(label(s));
                        if resp.clicked() {
                            action = Some((i, if ui.input(|inp| inp.modifiers.shift) { 1 } else { 0 }));
                        }
                        resp.context_menu(|ui| {
                            ui.label(RichText::new(label(s)).small());
                            if ui.button("Use as Fill").clicked() { action = Some((i, 0)); ui.close_menu(); }
                            if s.gradient.is_none() && ui.button("Use as Stroke").clicked() { action = Some((i, 1)); ui.close_menu(); }
                            if ui.button("Replace with Current Fill").clicked() { action = Some((i, 2)); ui.close_menu(); }
                            if ui.button("Delete Swatch").clicked() { action = Some((i, 3)); ui.close_menu(); }
                        });
                    }
                });
            }
            ui.add_space(4.0);
            if ui.button("+ Add fill").on_hover_text("Add the current fill (colour or gradient)").clicked() {
                add_fill(app);
            }
        });
        if let Some((i, op)) = action {
            let mut list = list;
            match op {
                0 | 1 => use_swatch(app, &list[i].clone(), op == 1),
                2 => {
                    list[i] = current_fill(app);
                    store(app, "Replace Swatch", list);
                }
                _ => {
                    list.remove(i);
                    store(app, "Delete Swatch", list);
                }
            }
        }
    });
}

/// The current fill as a swatch (a gradient when the fill type is one).
fn current_fill(app: &App) -> Swatch {
    match app.topts.fill_gradient {
        Some(k) => Swatch::gradient("", k, app.fill_stops()),
        None => Swatch::solid("", from32(app.fill)),
    }
}

fn add_fill(app: &mut App) {
    let s = current_fill(app);
    add(app, s);
}

fn add(app: &mut App, s: Swatch) {
    let mut list = current(app);
    if !list.contains(&s) {
        list.push(s);
        store(app, "Add Swatch", list);
    }
}

fn menu(app: &mut App, ui: &mut egui::Ui) {
    if ui.button("Add Fill").on_hover_text("The current fill colour or gradient").clicked() { add_fill(app); ui.close_menu(); }
    if ui.button("Add Stroke Colour").clicked() { let s = Swatch::solid("", from32(app.stroke)); add(app, s); ui.close_menu(); }
    ui.separator();
    if ui.button("Add Colours from File…").on_hover_text(".aco, .ase, .gpl, .act; gradients from .grd").clicked() {
        app.swatch_ui.replace = false;
        app.file(crate::files::FileAction::ImportSwatches);
        ui.close_menu();
    }
    if ui.button("Replace Colours from File…").clicked() {
        app.swatch_ui.replace = true;
        app.file(crate::files::FileAction::ImportSwatches);
        ui.close_menu();
    }
    if ui.button("Save Colours…").on_hover_text("Colours as .ase, .aco or .gpl; gradients as .grd").clicked() {
        app.file(crate::files::FileAction::ExportSwatches);
        ui.close_menu();
    }
    ui.separator();
    if app.ed.doc.swatches.is_some() {
        if ui.button("Save as App Default").on_hover_text("New documents and documents without their own swatches use these").clicked() {
            app.settings.swatches = app.ed.doc.swatches.clone();
            app.settings.save();
            app.status = "Swatches saved as the app default".into();
            ui.close_menu();
        }
        if ui.button("Use App Swatches").on_hover_text("This document stops keeping its own swatches").clicked() {
            app.ed.set_swatches("Use App Swatches", None);
            ui.close_menu();
        }
    } else if ui.button("Keep with This Document").on_hover_text("Save a copy of these swatches in the document (as Animate does)").clicked() {
        let list = current(app);
        app.ed.set_swatches("Document Swatches", Some(list));
        ui.close_menu();
    }
    ui.separator();
    if ui.button("Sort by Colour").clicked() {
        let mut list = current(app);
        list.sort_by(|a, b| sort_key(a).partial_cmp(&sort_key(b)).unwrap_or(std::cmp::Ordering::Equal));
        store(app, "Sort Swatches", list);
        ui.close_menu();
    }
    if ui.button("Reset to Standard").clicked() {
        store(app, "Reset Swatches", defaults());
        ui.close_menu();
    }
    if ui.button("Clear All").clicked() {
        store(app, "Clear Swatches", Vec::new());
        ui.close_menu();
    }
}

/// Greys first (dark to light), then by hue, then lightness; gradients last.
fn sort_key(s: &Swatch) -> (u8, f32, f32) {
    if s.gradient.is_some() {
        return (2, 0.0, 0.0);
    }
    let [r, g, b] = [s.color.r, s.color.g, s.color.b].map(|v| v as f32 / 255.0);
    let (mx, mn) = (r.max(g).max(b), r.min(g).min(b));
    let d = mx - mn;
    if d < 0.04 {
        return (0, mx, 0.0);
    }
    let h = if mx == r { ((g - b) / d).rem_euclid(6.0) } else if mx == g { (b - r) / d + 2.0 } else { (r - g) / d + 4.0 };
    (1, h, mx)
}

/// Swatches › Add / Replace Colours from File (FileAction::ImportSwatches).
pub(crate) fn import(app: &mut App, path: &str) -> Result<String, String> {
    let new = read(std::path::Path::new(path))?;
    let n = new.len();
    let mut list = if app.swatch_ui.replace { Vec::new() } else { current(app) };
    list.extend(new);
    store(app, "Import Swatches", list);
    app.swatch_ui.open = true;
    Ok(format!("{} {n} swatch{} from {}", if app.swatch_ui.replace { "Loaded" } else { "Added" }, if n == 1 { "" } else { "es" }, path))
}

pub(crate) fn export(app: &mut App, path: &str) -> Result<String, String> {
    let n = write(std::path::Path::new(path), &current(app))?;
    Ok(format!("Saved {n} swatches to {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(v: &[Swatch]) -> Vec<[u8; 4]> {
        v.iter().map(|s| [s.color.r, s.color.g, s.color.b, s.color.a]).collect()
    }

    #[test]
    fn round_trips_every_writable_format() {
        let v = vec![Swatch::solid("Sky", Color::rgb(0x40, 0x90, 0xE0)), Swatch::solid("", Color::rgb(255, 0, 0)), Swatch::solid("", Color::rgb(17, 17, 17))];
        let dir = std::env::temp_dir().join(format!("flare-swatches-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for ext in ["ase", "aco", "gpl"] {
            let p = dir.join(format!("t.{ext}"));
            assert_eq!(write(&p, &v).unwrap(), 3);
            let back = read(&p).unwrap();
            assert_eq!(colors(&back), colors(&v), "{ext}");
            assert!(back[0].name.contains("Sky"), "{ext}: {:?}", back[0].name);
        }
        // gradients go to .grd (PhotoCraft's writer and reader) and come back
        let g = vec![Swatch::gradient("Fade", GradientKind::Linear, vec![(0.0, Color::rgb(255, 0, 0)), (0.5, Color::rgb(0, 255, 0)), (1.0, rgba(0, 0, 255, 0))])];
        let p = dir.join("t.grd");
        assert_eq!(write(&p, &g).unwrap(), 1);
        let back = read(&p).unwrap();
        let bg = back[0].gradient.as_ref().unwrap();
        assert_eq!(back[0].name, "Fade");
        assert_eq!(bg.stops.len(), 3);
        assert_eq!(bg.stops[1].1, Color::rgb(0, 255, 0));
        assert_eq!(bg.stops[2].1.a, 0);
        assert!(write(&dir.join("t.ase"), &g).is_err(), "no colours to save");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn reads_gimp_and_act() {
        let gpl = "GIMP Palette\nName: Test\nColumns: 4\n# comment\n255   0   0\tRed\n  0 128 255 Deep Sky\n";
        let v = parse("gpl", gpl.as_bytes()).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].color, Color::rgb(0, 128, 255));
        assert_eq!(v[1].name, "Deep Sky");
        assert!(parse("gpl", b"not a palette").is_err());
        let mut act = vec![0u8; 772];
        act[3..6].copy_from_slice(&[10, 20, 30]);
        act[768..770].copy_from_slice(&2u16.to_be_bytes());
        let v = parse("act", &act).unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!(v[1].color, Color::rgb(10, 20, 30));
        assert!(parse("act", &[0; 10]).is_err());
    }

    #[test]
    fn converts_lab_cmyk_and_hsb() {
        assert_eq!(lab(100.0, 0.0, 0.0), [255, 255, 255]);
        assert_eq!(lab(0.0, 0.0, 0.0), [0, 0, 0]);
        let red = lab(54.29, 80.81, 69.89); // sRGB red in Lab (D50)
        assert!(red[0] > 245 && red[1] < 20 && red[2] < 20, "{red:?}");
        assert_eq!(cmyk(0.0, 1.0, 1.0, 0.0), [255, 0, 0]);
        assert_eq!(hsb(120.0, 1.0, 1.0), [0, 255, 0]);
        use photocraft_psd::aco::{write as w, AcoColor, AcoSwatch};
        let bytes = w(&[AcoSwatch { name: "C".into(), color: AcoColor::Cmyk([65535, 0, 0, 0]) }, AcoSwatch { name: "L".into(), color: AcoColor::Lab { l: 10000, a: 0, b: 0 } }]).unwrap();
        let v = parse("aco", &bytes).unwrap();
        assert_eq!(v[0].color, Color::rgb(0, 255, 255));
        assert_eq!(v[1].color, Color::rgb(255, 255, 255));
    }

    #[test]
    fn defaults_and_sorting() {
        let d = defaults();
        assert_eq!(d.iter().filter(|s| s.gradient.is_none()).count(), 48);
        assert!(d.iter().filter(|s| s.gradient.is_some()).count() >= 4);
        assert_eq!(d[0].color, Color::rgb(0, 0, 0));
        assert_eq!(d[11].color, Color::rgb(255, 255, 255));
        let mut s = d.clone();
        s.reverse();
        s.sort_by(|a, b| sort_key(a).partial_cmp(&sort_key(b)).unwrap());
        assert_eq!(s[0].color, Color::rgb(0, 0, 0));
        assert!(s.last().unwrap().gradient.is_some());
    }
}
