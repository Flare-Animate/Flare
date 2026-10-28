//! Bitmaps in the editor (docs/animate-ui.md §7.3, §7.4; interaction §1.3,
//! §3.2): File > Import to the stage or the library, and image files dropped on
//! the window; the Library's bitmap preview, Bitmap Properties and Update; a
//! bitmap's Object tab; the Color panel's Bitmap fill; the Magic Wand; and
//! Modify > Bitmap > Trace Bitmap. minnamate_core::bitmap and ::trace do the work.

use std::path::{Path, PathBuf};

use eframe::egui::{
    self, Align, Color32, ColorImage, CursorIcon, Id, Key, Layout, Painter, Rect, Response, RichText, Sense, Stroke, StrokeKind,
    TextureHandle, TextureOptions, Ui, pos2, vec2,
};
use minnamate_core::bitmap;
use minnamate_core::doc::{BitmapFill, BitmapInstance, BitmapItem, Element, Extra, Matrix, Media, Paint, Shape};
use minnamate_core::hit::ElementRef;
use minnamate_core::tiny_skia as sk;
use minnamate_core::trace::{self, Smoothing};

use crate::app::{Action, Editor};
use crate::icons::{self, Icon};
use crate::theme;
use crate::tools::Tool;
use crate::widgets::{self, Number};

/// The editor's bitmap settings and dialogs.
pub struct State {
    /// The fill default when it's a bitmap (the Color panel's Bitmap fill, the
    /// Eyedropper): its matrix is in the edited timeline's coordinates, where
    /// new shapes and Lock Fill lay it.
    pub fill: Option<BitmapFill>,
    /// The Magic Wand's Threshold and Smoothing (Tool tab).
    pub threshold: f32,
    pub smoothing: Smoothing,
    properties: Option<Properties>,
    /// Trace Bitmap while it's open: Color threshold and Minimum area.
    trace: Option<(f32, f32)>,
}

impl Default for State {
    fn default() -> Self {
        // ponytail: Animate's Threshold default, from recall; how its scale reads colours is a guess (trace.rs).
        State { fill: None, threshold: 10.0, smoothing: Smoothing::Normal, properties: None, trace: None }
    }
}

/// The Bitmap Properties dialog: the item, and its settings as edited.
struct Properties {
    name: String,
    new_name: String,
    smoothing: bool,
    lossless: bool,
    /// A custom JPEG quality; None follows the publish settings.
    quality: Option<f32>,
}

/// File > Import (to the stage or the library) and Modify > Bitmap > Trace Bitmap.
pub fn command(ed: &mut Editor, action: Action) {
    match action {
        // Images come in here, sounds through sound.rs.
        Action::ImportToStage | Action::ImportToLibrary => {
            let picked = rfd::FileDialog::new()
                .add_filter("All supported files", &["png", "jpg", "jpeg", "gif", "wav", "mp3"])
                .add_filter("Images", &["png", "jpg", "jpeg", "gif"])
                .add_filter("Sound", &["wav", "mp3"])
                .pick_files();
            if let Some(paths) = picked {
                let to_stage = action == Action::ImportToStage;
                let (sounds, images): (Vec<_>, Vec<_>) = paths.into_iter().partition(|p| crate::sound::is_sound(p));
                if !images.is_empty() {
                    import(ed, &images, to_stage);
                }
                for path in &sounds {
                    crate::sound::import(ed, path, to_stage);
                }
            }
        }
        // Animate's defaults.
        Action::TraceBitmap if traceable(ed) => ed.bitmaps.trace = Some((100.0, 8.0)),
        _ => {}
    }
}

/// Whether a file is an image Import takes, by its name.
pub fn is_image(path: &Path) -> bool {
    path.extension().is_some_and(|e| ["png", "jpg", "jpeg", "gif"].iter().any(|i| e.eq_ignore_ascii_case(i)))
}

/// Imports image files as library bitmaps, in one undo step. `to_stage` puts
/// each on the current layer at the current frame too, its top left at the
/// stage's, and selects them.
// ponytail: the stage's top left is Animate's place for them as recalled; and a file dropped on the
// window goes there too, not where it's dropped.
pub fn import(ed: &mut Editor, paths: &[PathBuf], to_stage: bool) {
    if to_stage && !ed.drawable() {
        return ed.log("Importing to the stage needs a normal layer, shown and unlocked: select one in the timeline.");
    }
    let mut doc = ed.doc.clone();
    let mut names = Vec::new();
    for path in paths {
        match bitmap::import(&mut doc, path) {
            Ok(name) => names.push(name),
            Err(e) => ed.error = Some(format!("Couldn't import {}.\n\n{e}", path.display())),
        }
    }
    if names.is_empty() {
        return;
    }
    ed.checkpoint();
    ed.doc.media = doc.media;
    ed.library_selection = names.last().cloned();
    ed.log(format!("Imported {}", names.join(", ")));
    if !to_stage {
        return;
    }
    let (l, f) = (ed.layer, ed.frame);
    let (tx, ty) = ed.to_local((0.0, 0.0));
    let layer = &mut ed.timeline_mut().layers[l];
    if layer.keyframe_at(f).is_none() {
        layer.insert_blank_keyframe(f);
    }
    let Some(k) = layer.keyframe_at(f) else { return };
    let elements = &mut layer.frames[k].elements;
    let first = elements.len();
    let matrix = Matrix { tx, ty, ..Matrix::IDENTITY };
    elements.extend(names.into_iter().map(|name| Element::Bitmap(BitmapInstance { name, matrix, extra: Extra::default() })));
    let placed = first..elements.len();
    ed.selection = placed.map(|element| ElementRef { layer: l, keyframe: k, element }).collect();
    crate::stage::show_selection_props(ed);
}

/// The bitmaps selected on stage.
fn selected(ed: &Editor) -> Vec<(ElementRef, BitmapInstance)> {
    ed.selection.iter().filter_map(|&r| if let Some(Element::Bitmap(b)) = ed.element(r) { Some((r, b.clone())) } else { None }).collect()
}

/// Whether Trace Bitmap has something to trace.
pub fn traceable(ed: &Editor) -> bool {
    !selected(ed).is_empty()
}

/// Bitmap Properties, Trace Bitmap.
pub fn dialogs(ctx: &egui::Context, ed: &mut Editor) {
    properties_dialog(ctx, ed);
    trace_dialog(ctx, ed);
}

// ---- Trace Bitmap ------------------------------------------------------------------------

fn trace_dialog(ctx: &egui::Context, ed: &mut Editor) {
    let Some((mut threshold, mut area)) = ed.bitmaps.trace else { return };
    let (mut ok, mut cancel) = (ctx.input(|i| i.key_pressed(Key::Enter)), false);
    let modal = egui::Modal::new(Id::new("trace-bitmap")).show(ctx, |ui| {
        ui.set_width(340.0);
        ui.label(RichText::new("Trace Bitmap").heading().color(theme::TEXT));
        ui.add_space(8.0);
        egui::Grid::new("trace-fields").num_columns(2).spacing([12.0, 8.0]).show(ui, |ui| {
            ui.label(RichText::new("Color threshold:").color(theme::TEXT_DIM));
            ui.add(Number::new("Color threshold", &mut threshold, 0.0..=500.0).fixed(0).width(60.0));
            ui.end_row();
            ui.label(RichText::new("Minimum area:").color(theme::TEXT_DIM));
            ui.horizontal(|ui| {
                ui.add(Number::new("Minimum area", &mut area, 1.0..=1000.0).fixed(0).width(60.0));
                ui.label(RichText::new("pixels").color(theme::TEXT_DIM));
            });
            ui.end_row();
            // ponytail: outlines follow the pixels (trace.rs); the smoother fits and corners aren't built.
            ui.label(RichText::new("Curve fit:").color(theme::TEXT_DIM));
            ui.add_enabled_ui(false, |ui| widgets::pick(ui, "Curve fit", 0, &[(0, "Pixels", true)], 140.0));
            ui.end_row();
            ui.label(RichText::new("Corner threshold:").color(theme::TEXT_DIM));
            ui.add_enabled_ui(false, |ui| widgets::pick(ui, "Corner threshold", 0, &[(0, "Normal", true)], 140.0));
            ui.end_row();
        });
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            cancel |= ui.button("  Cancel  ").clicked();
            ok |= theme::cta_button(ui, "    OK    ").clicked();
        });
    });
    ed.bitmaps.trace = Some((threshold, area));
    if modal.should_close() || cancel {
        ed.bitmaps.trace = None;
    } else if ok {
        ed.bitmaps.trace = None;
        trace_selected(ed, threshold, area);
    }
}

/// Traces each selected bitmap into a shape of flat colours, in its place.
pub fn trace_selected(ed: &mut Editor, threshold: f32, area: f32) {
    let traced: Vec<(ElementRef, Shape)> = selected(ed)
        .into_iter()
        .filter_map(|(r, b)| Some((r, Shape { matrix: b.matrix, ..trace::trace(ed.doc.bitmap(&b.name)?, threshold as u32, area as u32)? })))
        .collect();
    if traced.is_empty() {
        return ed.log("Trace Bitmap needs a bitmap on stage whose image minnamate can read.");
    }
    ed.checkpoint();
    for (r, s) in traced {
        if let Some(el) = ed.element_mut(r) {
            *el = Element::Shape(s);
        }
    }
}

// ---- The Magic Wand ----------------------------------------------------------------------

/// The Magic Wand on the stage: a click on a broken-apart bitmap selects the
/// colours like the one there, cut out into a shape of their own that Delete
/// takes away and a fill colour recolours. Shift adds to the selection.
pub fn wand_input(ui: &Ui, ed: &mut Editor, resp: &Response, stage: Rect) {
    ed.drag = None;
    let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.clicked()) else { return };
    let p = crate::stage::local_point(ed, stage, pos);
    let shift = ui.input(|i| i.modifiers.shift);
    let hit = crate::stage::hit_at(ed, p);
    let Some((r, Element::Shape(mut s))) = hit.and_then(|r| Some((r, ed.element(r)?.clone()))) else {
        if !shift {
            ed.selection.clear();
        }
        if hit.is_some() {
            ed.log("The Magic Wand selects in bitmaps broken apart (Modify > Break Apart).");
        }
        return;
    };
    let Some(part) = trace::magic_wand(&ed.doc, &mut s, p, ed.bitmaps.threshold as u32, ed.bitmaps.smoothing) else {
        return ed.log("The Magic Wand selects in bitmaps broken apart (Modify > Break Apart).");
    };
    ed.checkpoint();
    let mut selection = if shift { ed.selection.clone() } else { Vec::new() };
    let rest = !s.edges.is_empty();
    let key = &mut ed.timeline_mut().layers[r.layer].frames[r.keyframe];
    if rest {
        key.elements[r.element] = Element::Shape(s);
        key.elements.insert(r.element + 1, Element::Shape(part));
        for q in selection.iter_mut().filter(|q| (q.layer, q.keyframe) == (r.layer, r.keyframe) && q.element > r.element) {
            q.element += 1;
        }
    } else {
        // All of it was alike: the part takes its place.
        key.elements[r.element] = Element::Shape(part);
    }
    let part = ElementRef { element: r.element + rest as usize, ..r };
    selection.retain(|q| *q != part && *q != r);
    selection.push(part);
    ed.selection = selection;
    ed.layer = r.layer;
    crate::stage::show_selection_props(ed);
}

/// The wand itself as the pointer, black on a white rim, working at its tip.
pub fn wand_overlay(p: &Painter, ed: &Editor) {
    let ctx = p.ctx();
    let hidden = ctx.output(|o| o.cursor_icon == CursorIcon::None);
    let Some(pos) = ctx.pointer_hover_pos().filter(|_| hidden && ed.tool == Tool::MagicWand) else { return };
    let glyph = Rect::from_min_size(pos - vec2(2.5, 15.5), vec2(18.0, 18.0));
    for d in [vec2(-1.0, 0.0), vec2(1.0, 0.0), vec2(0.0, -1.0), vec2(0.0, 1.0)] {
        icons::paint(p, glyph.translate(d), Icon::MagicWand, Color32::WHITE);
    }
    icons::paint(p, glyph, Icon::MagicWand, Color32::BLACK);
}

/// The Magic Wand's Tool tab options.
pub fn wand_options(ui: &mut Ui, ed: &mut Editor) {
    theme::section(ui, "Magic Wand Options", |ui| {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Threshold", widgets::WORD_W);
            ui.add(Number::new("Threshold", &mut ed.bitmaps.threshold, 1.0..=200.0).fixed(0));
        });
        widgets::row(ui, |ui| {
            widgets::label(ui, "Smoothing", widgets::WORD_W);
            let items: Vec<_> = Smoothing::ALL.iter().map(|&s| (s, s.name(), true)).collect();
            if let Some(s) = widgets::pick(ui, "Smoothing", ed.bitmaps.smoothing, &items, ui.available_width() - 4.0) {
                ed.bitmaps.smoothing = s;
            }
        });
    });
}

// ---- Library -----------------------------------------------------------------------------

/// Bitmap `b` as a texture, made once per image (and smoothing), at most 512 px across.
// ponytail: textures stay for the session, one per image seen.
pub fn texture(ctx: &egui::Context, b: &BitmapItem) -> Option<TextureHandle> {
    let px = b.data.pixels()?;
    let id = Id::new(("bitmap-texture", b.data.bytes().as_ptr() as usize, b.data.bytes().len(), b.smoothing));
    if let Some(t) = ctx.data(|d| d.get_temp::<TextureHandle>(id)) {
        return Some(t);
    }
    let k = (512.0 / px.width().max(px.height()) as f32).min(1.0);
    let small;
    let px = if k < 1.0 {
        let size = |v: u32| ((v as f32 * k).round() as u32).max(1);
        let mut out = sk::Pixmap::new(size(px.width()), size(px.height()))?;
        let paint = sk::PixmapPaint { quality: sk::FilterQuality::Bilinear, ..Default::default() };
        out.draw_pixmap(0, 0, px.as_ref(), &paint, sk::Transform::from_scale(k, k), None);
        small = out;
        &small
    } else {
        px
    };
    let image = ColorImage::from_rgba_premultiplied([px.width() as usize, px.height() as usize], px.data());
    let t = ctx.load_texture("bitmap", image, if b.smoothing { TextureOptions::LINEAR } else { TextureOptions::NEAREST });
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    Some(t)
}

/// Paints bitmap `b` fitted into `rect`, centred, at most 4× its size.
fn paint_fitted(ui: &Ui, b: &BitmapItem, rect: Rect) {
    let (Some(tex), Some((w, h))) = (texture(ui.ctx(), b), b.size()) else { return };
    let fit = (rect.width() / w as f32).min(rect.height() / h as f32).min(4.0);
    let shown = Rect::from_center_size(rect.center(), vec2(w as f32, h as f32) * fit);
    ui.painter().with_clip_rect(rect).image(tex.id(), shown, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

/// The Tools panel's fill swatch, `rect`, while the fill is a bitmap: the bitmap in it.
pub fn fill_swatch(ui: &Ui, ed: &Editor, rect: Rect) {
    if let Paint::Bitmap(f) = ed.fill()
        && let Some(b) = ed.doc.bitmap(&f.name)
    {
        ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
        paint_fitted(ui, b, rect);
    }
}

/// The Library's preview of bitmap `name`.
pub fn preview(ui: &Ui, ed: &Editor, name: &str, rect: Rect) {
    if let Some(b) = ed.doc.bitmap(name) {
        paint_fitted(ui, b, rect.shrink(6.0));
    }
}

/// The Library's Properties on bitmap `name`: Bitmap Properties.
pub fn open_properties(ed: &mut Editor, name: &str) {
    let Some(b) = ed.doc.bitmap(name) else { return };
    let new_name = name.rsplit('/').next().unwrap_or(name).to_string();
    ed.bitmaps.properties = Some(Properties { name: name.into(), new_name, smoothing: b.smoothing, lossless: b.lossless, quality: b.quality.map(f32::from) });
}

/// Bitmap Properties: the bitmap and what it is, its name, Allow smoothing,
/// Compression and Quality; Update reads it again from its file.
// ponytail: no Import (replace from another file) or Test (the compressed size) buttons, and no
// Enable deblocking.
fn properties_dialog(ctx: &egui::Context, ed: &mut Editor) {
    let Some(d) = &mut ed.bitmaps.properties else { return };
    let Some(b) = ed.doc.bitmap(&d.name) else {
        ed.bitmaps.properties = None;
        return;
    };
    let (mut ok, mut cancel, mut update) = (ctx.input(|i| i.key_pressed(Key::Enter)), false, false);
    let modal = egui::Modal::new(Id::new("bitmap-properties")).show(ctx, |ui| {
        ui.set_width(440.0);
        ui.label(RichText::new("Bitmap Properties").heading().color(theme::TEXT));
        ui.add_space(8.0);
        ui.horizontal_top(|ui| {
            let (well, _) = ui.allocate_exact_size(vec2(110.0, 110.0), Sense::hover());
            ui.painter().rect_filled(well, 0.0, Color32::WHITE);
            paint_fitted(ui, b, well.shrink(4.0));
            ui.painter().rect_stroke(well, 0.0, Stroke::new(1.0, theme::BORDER), StrokeKind::Outside);
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 4.0;
                widgets::text_input(ui, &mut d.new_name, "Name", 300.0);
                let dim = |ui: &mut Ui, text: &str| ui.add(egui::Label::new(RichText::new(text).size(11.0).color(theme::TEXT_DIM)).truncate()).on_hover_text(text);
                dim(ui, b.extra.get("sourceExternalFilepath").unwrap_or("Not imported from a file"));
                if let Some((w, h)) = b.size() {
                    dim(ui, &format!("{w} x {h} pixels at 32 bits per pixel"));
                }
                ui.add_space(6.0);
                widgets::checkbox(ui, &mut d.smoothing, "Allow smoothing");
                ui.horizontal(|ui| {
                    widgets::label(ui, "Compression:", 80.0);
                    let items = [(false, "Photo (JPEG)", true), (true, "Lossless (PNG/GIF)", true)];
                    if let Some(lossless) = widgets::pick(ui, "Compression", d.lossless, &items, 160.0) {
                        d.lossless = lossless;
                    }
                });
                ui.add_enabled_ui(!d.lossless, |ui| {
                    ui.horizontal(|ui| {
                        widgets::label(ui, "Quality:", 80.0);
                        if ui.radio(d.quality.is_none(), "Use publish setting").clicked() {
                            d.quality = None;
                        }
                    });
                    ui.horizontal(|ui| {
                        widgets::label(ui, "", 80.0);
                        if ui.radio(d.quality.is_some(), "Custom:").clicked() && d.quality.is_none() {
                            d.quality = Some(80.0);
                        }
                        if let Some(q) = &mut d.quality {
                            ui.add(Number::new("Custom quality", q, 0.0..=100.0).fixed(0).width(40.0));
                        }
                    });
                });
            });
        });
        ui.add_space(12.0);
        ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
            cancel |= ui.button("  Cancel  ").clicked();
            ok |= theme::cta_button(ui, "    OK    ").clicked();
            update |= ui.add_enabled(b.extra.get("sourceExternalFilepath").is_some(), egui::Button::new("  Update  ")).clicked();
        });
    });
    if update {
        let name = d.name.clone();
        return self::update(ed, &name);
    }
    if modal.should_close() || cancel {
        ed.bitmaps.properties = None;
    } else if ok && let Some(d) = ed.bitmaps.properties.take() {
        apply(ed, d);
    }
}

/// Bitmap Properties' OK: a new name through the Library's rename (instances and
/// fills follow), then the settings, as one undo step.
fn apply(ed: &mut Editor, d: Properties) {
    let leaf = d.new_name.trim();
    let target = match d.name.rsplit_once('/') {
        Some((folder, _)) => format!("{folder}/{leaf}"),
        None => leaf.to_string(),
    };
    let renamed = !leaf.is_empty() && target != d.name;
    if renamed {
        crate::library::rename(ed, &d.name, leaf);
        if ed.doc.bitmap(&target).is_none() {
            return;
        }
    }
    let name = if renamed { target } else { d.name };
    let quality = d.quality.map(|q| q.round() as u8);
    let Some(b) = ed.doc.bitmap(&name) else { return };
    if (b.smoothing, b.lossless, b.quality) != (d.smoothing, d.lossless, quality) {
        if !renamed {
            ed.checkpoint();
        }
        if let Some(b) = ed.doc.bitmap_mut(&name) {
            (b.smoothing, b.lossless, b.quality) = (d.smoothing, d.lossless, quality);
        }
    }
}

/// The Library's Update: reads bitmap `name` again from the file it was
/// imported from (a relative path from the document's folder).
pub fn update(ed: &mut Editor, name: &str) {
    let Some(mut b) = ed.doc.bitmap(name).cloned() else { return };
    let base = ed.path.as_deref().and_then(Path::parent);
    match bitmap::update(&mut b, base) {
        Ok(true) => {
            ed.checkpoint();
            if let Some(slot) = ed.doc.bitmap_mut(name) {
                *slot = b;
            }
            ed.log(format!("Updated {name}"));
        }
        Ok(false) => ed.log(format!("{name} is up to date.")),
        Err(e) => ed.error = Some(format!("Couldn't update {name}.\n\n{e}")),
    }
}

/// Renames the library's bitmaps `renamed` gives a new name, and the bitmaps on
/// stage and bitmap fills (the fill default too) that name them.
pub fn rename_all(ed: &mut Editor, renamed: &dyn Fn(&str) -> Option<String>) {
    for m in &mut ed.doc.media {
        if let Media::Bitmap(b) = m
            && let Some(name) = renamed(&b.name)
        {
            b.name = name;
        }
    }
    for_names(ed, &mut |name| {
        if let Some(new) = renamed(name) {
            *name = new;
        }
    });
}

/// Deletes the library's bitmaps that `gone` names and the bitmaps on stage of
/// them, as the Library's trash does; true if there were any.
// ponytail: bitmap fills of them stay, drawing nothing; Animate's way with those is unconfirmed.
pub fn delete_all(ed: &mut Editor, gone: &dyn Fn(&str) -> bool) -> bool {
    let before = ed.doc.media.len();
    ed.doc.media.retain(|m| !matches!(m, Media::Bitmap(b) if gone(&b.name)));
    if ed.bitmaps.fill.as_ref().is_some_and(|f| gone(&f.name)) {
        ed.bitmaps.fill = None;
    }
    fn strip(elements: &mut Vec<Element>, gone: &dyn Fn(&str) -> bool) {
        elements.retain(|el| !matches!(el, Element::Bitmap(b) if gone(&b.name)));
        for el in elements {
            if let Element::Group(g) = el {
                strip(g, gone);
            }
        }
    }
    let doc = &mut ed.doc;
    for key in doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline)).flat_map(|tl| &mut tl.layers).flat_map(|l| &mut l.frames) {
        strip(&mut key.elements, gone);
    }
    ed.doc.media.len() != before
}

/// Runs `f` on the bitmap name of every bitmap on stage and bitmap fill, and the fill default's.
fn for_names(ed: &mut Editor, f: &mut dyn FnMut(&mut String)) {
    fn walk(elements: &mut [Element], f: &mut dyn FnMut(&mut String)) {
        for el in elements {
            match el {
                Element::Bitmap(b) => f(&mut b.name),
                Element::Shape(s) => {
                    for p in s.fills.iter_mut().chain(s.strokes.iter_mut().map(|st| &mut st.paint)) {
                        if let Paint::Bitmap(b) = p {
                            f(&mut b.name);
                        }
                    }
                }
                Element::Group(g) => walk(g, f),
                _ => {}
            }
        }
    }
    let doc = &mut ed.doc;
    for key in doc.scenes.iter_mut().chain(doc.library.values_mut().map(|s| &mut s.timeline)).flat_map(|tl| &mut tl.layers).flat_map(|l| &mut l.frames) {
        walk(&mut key.elements, f);
    }
    if let Some(b) = &mut ed.bitmaps.fill {
        f(&mut b.name);
    }
}

// ---- Properties and the Color panel ------------------------------------------------------

/// A bitmap's Object tab rows under its header: what it's an instance of, with Swap and Edit.
pub fn instance_of(ui: &mut Ui, name: &str) {
    widgets::row(ui, |ui| {
        ui.add_space(16.0);
        widgets::label(ui, "Instance of:", 72.0);
        ui.label(RichText::new(name).color(theme::TEXT_DISABLED));
    });
    widgets::row(ui, |ui| {
        ui.add_space(16.0);
        // ponytail: no Swap Bitmap dialog, and no image editor for Edit to hand the file to.
        ui.add_enabled_ui(false, |ui| {
            widgets::button(ui, "Swap...", 80.0);
            widgets::button(ui, "Edit...", 80.0);
        });
    });
}

/// The bitmap a fill shows, if it's a bitmap fill.
fn fill_name(p: &Paint) -> Option<&str> {
    if let Paint::Bitmap(b) = p { Some(&b.name) } else { None }
}

/// Picking "Bitmap fill" as the Color type: the library's first bitmap, or, with
/// none, the ones File > Import to Library brings.
pub fn choose_fill(ed: &mut Editor) {
    if ed.doc.bitmaps().next().is_none() {
        command(ed, Action::ImportToLibrary);
    }
    let first = ed.doc.bitmaps().next().map(|b| b.name.clone());
    if let Some(name) = first {
        set_fill(ed, &name);
    }
}

/// Makes library bitmap `name` the fill default (tiled from the stage's top left)
/// and the selected shapes' fill: a bitmap fill keeps its place, another fill
/// gets the bitmap tiled from the shape's top left. One undo step.
pub fn set_fill(ed: &mut Editor, name: &str) {
    let fill = BitmapFill { name: name.into(), matrix: Matrix::IDENTITY, clipped: false, extra: Extra::default() };
    ed.bitmaps.fill = Some(fill.clone());
    let shapes: Vec<ElementRef> = ed.selection.iter().copied().filter(|&r| matches!(ed.element(r), Some(Element::Shape(_)))).collect();
    if shapes.is_empty() {
        return;
    }
    ed.checkpoint();
    for r in shapes {
        let Some(Element::Shape(s)) = ed.element_mut(r) else { continue };
        let at = minnamate_core::gradient::edge_box(s).map_or((0.0, 0.0), |b| b.min);
        for p in &mut s.fills {
            *p = match p {
                Paint::Bitmap(b) => Paint::Bitmap(BitmapFill { name: name.into(), ..b.clone() }),
                _ => Paint::Bitmap(BitmapFill { matrix: Matrix { tx: at.0, ty: at.1, ..Matrix::IDENTITY }, ..fill.clone() }),
            };
        }
    }
}

/// The Color panel in Bitmap fill mode (§3.2 item 7): Import... and the
/// library's bitmaps as 48 px swatches; a click fills with one.
pub fn fill_swatches(ui: &mut Ui, ed: &mut Editor, shown: &Paint) {
    widgets::row(ui, |ui| {
        if widgets::button(ui, "Import...", 90.0).clicked() {
            let before = ed.doc.bitmaps().count();
            command(ed, Action::ImportToLibrary);
            let imported = ed.doc.bitmaps().nth(before).map(|b| b.name.clone());
            if let Some(name) = imported {
                set_fill(ed, &name);
            }
        }
    });
    let names: Vec<String> = ed.doc.bitmaps().map(|b| b.name.clone()).collect();
    let columns = ((ui.available_width() + 4.0) / 52.0).floor().max(1.0) as usize;
    let mut picked = None;
    for row in names.chunks(columns) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for name in row {
                let (rect, resp) = ui.allocate_exact_size(vec2(48.0, 48.0), Sense::click());
                ui.painter().rect_filled(rect, 0.0, Color32::WHITE);
                if let Some(b) = ed.doc.bitmap(name) {
                    paint_fitted(ui, b, rect.shrink(2.0));
                }
                let current = fill_name(shown) == Some(name.as_str());
                let ring = if current { Stroke::new(2.0, theme::ACCENT) } else { Stroke::new(1.0, theme::BORDER) };
                ui.painter().rect_stroke(rect, 0.0, ring, StrokeKind::Outside);
                if resp.on_hover_text(name).clicked() {
                    picked = Some(name.clone());
                }
            }
        });
        ui.add_space(4.0);
    }
    if let Some(name) = picked {
        set_fill(ed, &name);
    }
}

#[cfg(test)]
mod tests {
    use eframe::egui::{Key, Modifiers};
    use minnamate_core::doc::{Element, Paint};

    use super::*;
    use crate::app::testkit::*;

    fn dir() -> PathBuf {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/tmp/editor-bitmaps");
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// `pm` as a PNG file named `name` (each test its own).
    fn png_file(name: &str, pm: &sk::Pixmap) -> PathBuf {
        let path = dir().join(name);
        std::fs::write(&path, pm.encode_png().unwrap()).unwrap();
        path
    }

    /// A 2 × 2 PNG, red and green over blue and white, named `name`.
    fn checker(name: &str) -> PathBuf {
        let mut pm = sk::Pixmap::new(2, 2).unwrap();
        let colors = [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255], [255, 255, 255, 255]];
        for (p, c) in pm.data_mut().chunks_exact_mut(4).zip(colors) {
            p.copy_from_slice(&c);
        }
        png_file(name, &pm)
    }

    const AT: ElementRef = ElementRef { layer: 0, keyframe: 0, element: 0 };

    /// A blank document with the checker imported to the stage as `name`, 50× at (100, 100), unsmoothed.
    fn imported(name: &str) -> egui_kittest::Harness<'static, Editor> {
        let mut h = blank();
        import(h.state_mut(), &[checker(name)], true);
        let ed = h.state_mut();
        ed.doc.bitmap_mut(name).unwrap().smoothing = false;
        if let Some(Element::Bitmap(b)) = ed.element_mut(AT) {
            b.matrix = Matrix { a: 50.0, d: 50.0, tx: 100.0, ty: 100.0, ..Matrix::IDENTITY };
        }
        h.run();
        h
    }

    #[test]
    fn import_to_stage_library_and_by_dropping() {
        let mut h = blank();
        let ed = h.state_mut();
        import(ed, &[checker("import.png")], true);
        assert_eq!(ed.doc.bitmaps().map(|b| b.name.as_str()).collect::<Vec<_>>(), ["import.png"]);
        let Some(Element::Bitmap(b)) = ed.element(AT) else { panic!("no bitmap on stage") };
        assert_eq!((b.name.as_str(), b.matrix), ("import.png", Matrix::IDENTITY), "at the stage's top left");
        assert_eq!(ed.selection, [AT], "and selected");
        // To the library alone: no instance, a name of its own.
        import(ed, &[checker("import.png")], false);
        assert_eq!(ed.doc.bitmaps().count(), 2);
        assert_eq!(ed.all_elements().len(), 1);
        assert_eq!(ed.library_selection.as_deref(), Some("import copy.png"));
        // One undo step each.
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        h.state_mut().run(Action::Undo, &ctx);
        assert!(h.state().doc.media.is_empty() && h.state().all_elements().is_empty());
        // A file that isn't an image says so, and changes nothing.
        let text = dir().join("not-an-image.png");
        std::fs::write(&text, "hello").unwrap();
        import(h.state_mut(), &[text], true);
        assert!(h.state().error.is_some() && h.state().doc.media.is_empty());
        h.state_mut().error = None;
        // An image dropped on the window comes in on the stage, with nothing asked.
        drop_file(&mut h, checker("dropped.png"));
        let ed = h.state();
        assert!(ed.doc.bitmap("dropped.png").is_some() && ed.confirm.is_none(), "{:?}", ed.doc.media);
        assert!(matches!(ed.element(AT), Some(Element::Bitmap(_))));
    }

    #[test]
    fn break_apart_then_the_magic_wand() {
        let mut h = imported("wand.png");
        let ctx = h.ctx.clone();
        h.state_mut().selection = vec![AT];
        h.key_press_modifiers(Modifiers::COMMAND, Key::B);
        h.run();
        let ed = h.state();
        let Some(Element::Shape(s)) = ed.element(AT) else { panic!("broken apart into a shape") };
        assert!(matches!(&s.fills[..], [Paint::Bitmap(f)] if f.name == "wand.png"));
        assert_eq!(shape_bounds(ed), minnamate_core::hit::Rect { min: (100.0, 100.0), max: (200.0, 200.0) });

        // The wand on the red block: it becomes a shape of its own, selected.
        h.state_mut().tool = Tool::MagicWand;
        h.run();
        mouse_click(&mut h, (120.0, 130.0));
        let ed = h.state();
        assert_eq!(ed.all_elements().len(), 2);
        let part = ElementRef { element: 1, ..AT };
        assert_eq!(ed.selection, [part]);
        let b = minnamate_core::hit::bounds(&ed.doc, ed.timeline(), 0, &[part]).unwrap();
        assert_eq!((b.min, b.max), ((100.0, 100.0), (150.0, 150.0)));
        // Recoloured, then deleted: the rest of the bitmap stays.
        crate::color::restyle(h.state_mut(), crate::color::Style::Fill(Color32::BLACK));
        assert!(matches!(h.state().element(part), Some(Element::Shape(s)) if s.fills == [Paint::Solid([0.0, 0.0, 0.0, 1.0])]));
        h.key_press(Key::Delete);
        h.run();
        assert_eq!(h.state().all_elements().len(), 1);
        // Each step undoes on its own: the delete, the colour, the cut.
        for _ in 0..3 {
            h.state_mut().run(Action::Undo, &ctx);
        }
        assert!(matches!(h.state().element(AT), Some(Element::Shape(s)) if s.edges.len() == 1) && h.state().all_elements().len() == 1);
        // On a bitmap not broken apart it does nothing.
        h.state_mut().run(Action::Undo, &ctx);
        mouse_click(&mut h, (120.0, 130.0));
        assert!(matches!(h.state().element(AT), Some(Element::Bitmap(_))));
    }

    #[test]
    fn trace_bitmap_replaces_it_with_flat_colours() {
        let mut h = imported("trace.png");
        let ctx = h.ctx.clone();
        h.state_mut().selection.clear();
        assert!(!crate::menu::enabled(h.state(), Action::TraceBitmap), "nothing to trace");
        h.state_mut().selection = vec![AT];
        assert!(crate::menu::enabled(h.state(), Action::TraceBitmap));
        h.state_mut().run(Action::TraceBitmap, &ctx);
        h.run();
        assert_eq!(h.state().bitmaps.trace, Some((100.0, 8.0)), "the dialog, with Animate's defaults");
        // The 2 × 2 checker's areas are a pixel each, under the 8 pixels Animate keeps by default.
        h.state_mut().bitmaps.trace = Some((100.0, 1.0));
        h.key_press(Key::Enter);
        h.run();
        let ed = h.state();
        let Some(Element::Shape(s)) = ed.element(AT) else { panic!("traced into a shape") };
        assert_eq!(s.fills.len(), 4);
        assert_eq!(s.matrix, Matrix { a: 50.0, d: 50.0, tx: 100.0, ty: 100.0, ..Matrix::IDENTITY }, "where the bitmap was");
        assert!(ed.bitmaps.trace.is_none());
    }

    #[test]
    fn bitmap_fills_from_the_color_panel_and_the_bucket() {
        let mut h = imported("fills.png");
        // A rectangle, filled with the bitmap from its top left.
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (300.0, 50.0), (400.0, 150.0));
        let rect = ElementRef { element: 1, ..AT };
        h.state_mut().selection = vec![rect];
        set_fill(h.state_mut(), "fills.png");
        let Some(Element::Shape(s)) = h.state().element(rect) else { panic!() };
        let Paint::Bitmap(f) = &s.fills[0] else { panic!("{:?}", s.fills) };
        assert!((f.matrix.tx - 300.0).abs() < 1.0 && (f.matrix.ty - 50.0).abs() < 1.0, "{:?}", f.matrix);
        assert!(matches!(h.state().fill(), Paint::Bitmap(_)), "and the drawing default");
        // A colour picked after puts the solid fill back.
        h.state_mut().selection.clear();
        crate::color::restyle(h.state_mut(), crate::color::Style::Fill(Color32::RED));
        assert!(matches!(h.state().fill(), Paint::Solid(_)) && h.state().bitmaps.fill.is_none());

        // The Eyedropper takes the bitmap fill up, Lock Fill on; the bucket pours it on where it lies.
        h.state_mut().tool = Tool::Oval;
        mouse_drag(&mut h, (300.0, 200.0), (380.0, 280.0));
        h.state_mut().tool = Tool::Eyedropper;
        h.run();
        mouse_click(&mut h, (350.0, 100.0));
        let ed = h.state();
        assert_eq!((ed.tool, ed.paint.lock_fill), (Tool::PaintBucket, true));
        let picked = ed.bitmaps.fill.clone().expect("picked up");
        mouse_click(&mut h, (340.0, 240.0));
        let Some(Element::Shape(oval)) = h.state().element(ElementRef { element: 2, ..AT }) else { panic!() };
        let Paint::Bitmap(poured) = &oval.fills[0] else { panic!("{:?}", oval.fills) };
        assert_eq!(oval.matrix * poured.matrix, picked.matrix, "it lines up with the rectangle's");
    }

    #[test]
    fn a_bitmap_stroke_shows_no_fill_swatches() {
        use egui_kittest::kittest::Queryable;
        let mut h = imported("stroked.png");
        let ctx = h.ctx.clone();
        h.state_mut().tool = Tool::Rectangle;
        mouse_drag(&mut h, (300.0, 50.0), (400.0, 150.0));
        let rect = ElementRef { element: 1, ..AT };
        // Filled and stroked with the bitmap, as files can have strokes.
        if let Some(Element::Shape(s)) = h.state_mut().element_mut(rect) {
            let bitmap = Paint::Bitmap(BitmapFill { name: "stroked.png".into(), matrix: Matrix::IDENTITY, clipped: false, extra: Extra::default() });
            (s.fills[0], s.strokes[0].paint) = (bitmap.clone(), bitmap);
        }
        h.state_mut().selection = vec![rect];
        h.state_mut().run(Action::Show(crate::app::PanelId::Color), &ctx);
        h.run();
        assert!(h.query_by_label("Import...").is_some(), "the fill's bitmaps");
        // On the stroke, the swatches (which set fills) aren't offered.
        ctx.data_mut(|d| d.insert_temp(Id::new("color-fill-active"), false));
        h.run();
        assert!(h.query_by_label("Import...").is_none());
    }

    #[test]
    fn library_rows_properties_update_and_delete() {
        use egui_kittest::kittest::Queryable;
        let mut h = imported("library.png");
        h.state_mut().right_tab = 1;
        h.run();
        assert!(h.query_by_label("library.png").is_some(), "a row in the Library");
        // Bitmap Properties: smoothing back on and a new name, one undo step; the stage follows.
        open_properties(h.state_mut(), "library.png");
        h.run();
        let d = h.state_mut().bitmaps.properties.as_mut().unwrap();
        (d.smoothing, d.new_name) = (true, "tiles.png".into());
        h.key_press(Key::Enter);
        h.run();
        let ed = h.state();
        assert!(ed.doc.bitmap("tiles.png").is_some_and(|b| b.smoothing) && ed.doc.bitmap("library.png").is_none());
        assert!(matches!(ed.element(AT), Some(Element::Bitmap(b)) if b.name == "tiles.png"));
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert!(h.state().doc.bitmap("library.png").is_some_and(|b| !b.smoothing));
        // Update reads it again from its file.
        let mut wide = sk::Pixmap::new(3, 1).unwrap();
        wide.fill(sk::Color::BLACK);
        png_file("library.png", &wide);
        update(h.state_mut(), "library.png");
        assert_eq!(h.state().doc.bitmap("library.png").and_then(BitmapItem::size), Some((3, 1)));
        // The trash takes its instances with it.
        crate::library::delete(h.state_mut(), "library.png");
        assert!(h.state().doc.media.is_empty() && h.state().all_elements().is_empty());
    }

    /// A 200 × 150 picture: a sky over a hill, and a sun.
    fn photo(name: &str) -> PathBuf {
        let (w, h) = (200, 150);
        let mut pm = sk::Pixmap::new(w, h).unwrap();
        for (i, p) in pm.data_mut().chunks_exact_mut(4).enumerate() {
            let (x, y) = ((i as u32 % w) as f32, (i as u32 / w) as f32);
            let sky = [(90.0 + y * 0.6) as u8, (150.0 + y * 0.4) as u8, 235, 255];
            let sun = ((x - 150.0).powi(2) + (y - 40.0).powi(2)).sqrt() < 18.0;
            let hill = y > 100.0 - 30.0 * (x / 60.0).sin().abs();
            let c = if sun { [255, 210, 60, 255] } else if hill { [60, (120.0 + x * 0.3) as u8, 50, 255] } else { sky };
            p.copy_from_slice(&c);
        }
        png_file(name, &pm)
    }

    fn gpu() -> egui_kittest::Harness<'static, Editor> {
        let mut h = egui_kittest::Harness::builder().with_size(vec2(1600.0, 900.0)).wgpu().build_eframe(|cc| Editor::new(&cc.egui_ctx, None));
        h.state_mut().new_document(550.0, 400.0, 24.0);
        h.run();
        h
    }

    /// Writes target/screenshots/bitmaps.png: an imported image and a star filled
    /// with it, and bitmap_fill.png: the Color panel's Bitmap fill swatches.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn bitmaps_screenshot() {
        let mut h = gpu();
        import(h.state_mut(), &[photo("photo.png")], true);
        let ed = h.state_mut();
        if let Some(Element::Bitmap(b)) = ed.element_mut(AT) {
            b.matrix = Matrix { tx: 30.0, ty: 40.0, ..Matrix::IDENTITY };
        }
        (ed.tool, ed.polystar_star) = (Tool::PolyStar, true);
        h.run();
        mouse_drag(&mut h, (420.0, 220.0), (420.0, 330.0));
        let star = ElementRef { element: 1, ..AT };
        h.state_mut().selection = vec![star];
        set_fill(h.state_mut(), "photo.png");
        h.state_mut().selection = vec![AT];
        h.state_mut().tool = Tool::Selection;
        h.run();
        screenshot(&mut h, "bitmaps");
        let ctx = h.ctx.clone();
        h.state_mut().selection = vec![star];
        h.state_mut().run(Action::Show(crate::app::PanelId::Color), &ctx);
        h.run();
        screenshot(&mut h, "bitmap_fill");

        // magic_wand.png: the photo broken apart, its sky picked with the wand, the Tool tab's options.
        h.state_mut().run(Action::Show(crate::app::PanelId::Color), &ctx);
        h.state_mut().selection = vec![AT];
        h.state_mut().run(Action::BreakApart, &ctx);
        crate::tools::select(h.state_mut(), Tool::MagicWand);
        h.run();
        mouse_click(&mut h, (60.0, 60.0));
        h.state_mut().props_tab = crate::app::PropsTab::Tool;
        h.hover_at(on_stage(&h, (150.0, 170.0)));
        h.run();
        screenshot(&mut h, "magic_wand");
    }

    /// Writes target/screenshots/library_bitmap.png: the Library with a bitmap
    /// selected and its preview, and bitmap_properties.png: its Bitmap Properties.
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn library_bitmap_screenshot() {
        let mut h = gpu();
        import(h.state_mut(), &[photo("sky.png")], false);
        import(h.state_mut(), &[checker("tile.png")], false);
        let ed = h.state_mut();
        (ed.library_selection, ed.right_tab) = (Some("sky.png".into()), 1);
        h.run();
        screenshot(&mut h, "library_bitmap");
        open_properties(h.state_mut(), "sky.png");
        h.run();
        screenshot(&mut h, "bitmap_properties");
    }
}
