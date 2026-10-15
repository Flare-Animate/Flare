//! Properties panel (right by default), Library (below Properties or in its own column) and
//! the Transform panel.

use crate::toolbar::{FtMode, PencilMode, Tool};
use crate::{c32, from32, App, Dialog, PropsTab, ACCENT};
use ac_core::deform::Transform3D;
use ac_core::geom::Decomposed;
use ac_core::text::{Align, FontFace};
use ac_core::{pt, BlendMode, Editor, Element, ElementKind, Filter, Gradient, GradientKind, LoopMode, Matrix, Pt, Stroke, SymbolKind, Tween};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Rect, RichText, Sense, Stroke as EStroke};
use tiny_skia as sk;

/// Library panel state.
pub struct LibraryState {
    pub open: bool,
    pub search: String,
    pub play: bool,
    pub frame: u32,
    accum: f64,
    sort: (u8, bool),
    /// Selected / dragged library bitmap (symbols use `App::lib_selected` / `lib_drag`).
    pub bitmap: Option<u32>,
    pub drag_bitmap: Option<u32>,
    /// Selected / dragged library sound.
    pub sound: Option<u32>,
    pub drag_sound: Option<u32>,
    /// Open folders (paths) in the library tree.
    pub open_folders: std::collections::BTreeSet<String>,
    /// Folder being renamed: (path, text).
    pub rename_folder: Option<(String, String)>,
    /// Library item Properties dialog.
    pub props: Option<ItemProps>,
}

/// Library › Properties… for one item.
#[derive(Clone)]
pub struct ItemProps {
    pub item: ac_core::editor::LibItem,
    pub name: String,
    pub kind: Option<SymbolKind>,
    pub export: bool,
    pub class: String,
    pub folder: String,
}

impl Default for LibraryState {
    fn default() -> Self {
        LibraryState { open: true, search: String::new(), play: false, frame: 0, accum: 0.0, sort: (0, true), bitmap: None, drag_bitmap: None, sound: None, drag_sound: None, open_folders: Default::default(), rename_folder: None, props: None }
    }
}

/// Transform panel (Ctrl+T) state.
pub struct TransformPanel {
    pub skew: bool,
    pub constrain: bool,
    pub step_rotate: f32,
    pub step_scale: f32,
}

impl Default for TransformPanel {
    fn default() -> Self {
        TransformPanel { skew: false, constrain: true, step_rotate: 30.0, step_scale: 100.0 }
    }
}

/// A panel's title bar, with its grip for moving the panel (dock.rs).
fn panel_title(app: &mut App, ui: &mut egui::Ui, module: crate::dock::Module, title: &str) {
    egui::Frame::none().fill(Color32::from_gray(0x2A)).inner_margin(egui::Margin::symmetric(4.0, 2.0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            crate::dock::grip(app, ui, module, Some(title));
        });
    });
}

pub fn right_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::new(app.layout.props_side(), "props").default_width(300.0).min_width(250.0).max_width((ctx.screen_rect().width() * 0.45).max(320.0)).show(ctx, |ui| {
        let lib_here = app.lib.open && app.layout.library == crate::dock::LibraryDock::WithProperties;
        let total = ui.available_height();
        // Properties take what they need (scrolling past a cap); the Library fills the rest.
        let props_max = if lib_here { (total - 220.0).max(160.0) } else { total - 30.0 };
        panel_title(app, ui, crate::dock::Module::Properties, "Properties");
        egui::ScrollArea::vertical().id_salt("props-scroll").max_height(props_max).auto_shrink([false, true]).show(ui, |ui| {
            ui.horizontal(|ui| {
                for (t, name) in [(PropsTab::Tool, "Tool"), (PropsTab::Object, "Object"), (PropsTab::Frame, "Frame"), (PropsTab::Doc, "Doc")] {
                    ui.selectable_value(&mut app.props_tab, t, name);
                }
            });
            ui.separator();
            match app.props_tab {
                PropsTab::Tool => tool_properties(app, ui),
                PropsTab::Object => {
                    if app.ed.selection.is_empty() {
                        ui.label(RichText::new("Nothing selected. Click an object on the stage.").color(Color32::from_gray(150)));
                    } else {
                        object_properties(app, ui);
                    }
                }
                PropsTab::Frame => frame_properties(app, ui),
                PropsTab::Doc => document_properties(app, ui),
            }
            ui.add_space(4.0);
        });
        if lib_here {
            ui.add_space(2.0);
            panel_title(app, ui, crate::dock::Module::Library, "Library");
            library(app, ui);
        }
    });
}

/// The Library in its own column, left or right (Workspace › Library).
pub fn library_column(app: &mut App, ctx: &egui::Context) {
    let side = match app.layout.library {
        crate::dock::LibraryDock::Left => egui::panel::Side::Left,
        crate::dock::LibraryDock::Right => egui::panel::Side::Right,
        crate::dock::LibraryDock::WithProperties => return,
    };
    if !app.lib.open {
        return;
    }
    egui::SidePanel::new(side, "library-left").default_width(270.0).min_width(220.0).show(ctx, |ui| {
        panel_title(app, ui, crate::dock::Module::Library, "Library");
        library(app, ui);
    });
}

/// A real user edit. egui also reports `changed` when a DragValue clamps a
/// value that is outside its range (e.g. set by a script); that must never be
/// written back, or the panel would keep "fixing" the document every frame.
pub(crate) fn edited(r: &egui::Response) -> bool {
    r.changed() && (r.dragged() || r.has_focus() || r.lost_focus() || r.clicked())
}

/// Route a widget change to the editor: drags merge into one undo step,
/// typed/clicked changes are one step each.
fn apply(app: &mut App, resp: &egui::Response, label: &str, f: impl Fn(&mut Element)) {
    if resp.drag_stopped() {
        app.prop_dragging = false;
    }
    if !edited(resp) {
        return;
    }
    if resp.dragged() {
        if !app.prop_dragging {
            app.ed.begin_edit(label);
            app.prop_dragging = true;
        }
        app.ed.modify_selected_continuous(f);
    } else {
        app.ed.modify_selected(label, f);
    }
}

/// Change scale/rotation/skew of each selected object while keeping the
/// transformation point (Free Transform's white dot) where it is.
fn apply_decomp(app: &mut App, resp: &egui::Response, label: &str, f: impl Fn(&mut Decomposed)) {
    let Some(pw) = app.pivot() else { return };
    apply(app, resp, label, move |e| {
        let Some(inv) = e.matrix.invert() else { return };
        let lp = inv.apply(pw);
        let mut dd = e.matrix.decompose();
        f(&mut dd);
        let mut m = Matrix::compose(&dd);
        let q = m.apply(lp);
        m.tx += pw.x - q.x;
        m.ty += pw.y - q.y;
        e.matrix = m;
    });
}

/// Edit 3D properties; objects get a 3D centre first.
fn apply_3d(app: &mut App, resp: &egui::Response, label: &str, f: impl Fn(&mut Transform3D)) {
    if edited(resp) {
        app.ensure_3d();
    }
    apply(app, resp, label, move |e| {
        let mut t = e.three_d.unwrap_or_default();
        f(&mut t);
        e.three_d = Some(t);
    });
}

fn section(ui: &mut egui::Ui, title: &str) {
    ui.add_space(2.0);
    ui.label(RichText::new(title).strong().color(Color32::from_gray(200)));
}

/// `s` cut to `max` characters with an ellipsis in the middle (keeps the extension).
pub(crate) fn ellipsize(s: &str, max: usize) -> String {
    let n = s.chars().count();
    if n <= max {
        return s.to_string();
    }
    let head = max * 2 / 3;
    let tail = max - head - 1;
    let a: String = s.chars().take(head).collect();
    let b: String = s.chars().skip(n - tail).collect();
    format!("{a}…{b}")
}

fn hint(ui: &mut egui::Ui, text: &str) {
    ui.label(RichText::new(text).small().color(Color32::from_gray(150)));
}

fn object_properties(app: &mut App, ui: &mut egui::Ui) {
    let n = app.ed.selection.len();
    let Some(el) = app.ed.selected_element() else {
        ui.label("Selection is not visible on this frame.");
        return;
    };
    let sel0 = app.ed.selection[0];
    let editable = app.ed.selection.iter().all(|s| app.ed.layer_editable(s.layer));
    let title = if n > 1 {
        format!("{n} objects")
    } else {
        match &el.kind {
            ElementKind::Shape(_) => "Shape (drawing object)".to_string(),
            ElementKind::Canvas(c) => match &c.float {
                Some(f) => format!("Shape selection ({} fill(s), {} line(s))", f.fills.len(), f.strokes.len()),
                None => "Shape (merge drawing)".to_string(),
            },
            ElementKind::Text(t) => match t.field.kind {
                ac_core::text::TextKind::Static => "Static text",
                ac_core::text::TextKind::Dynamic => "Dynamic text",
                ac_core::text::TextKind::Input => "Input text",
            }
            .to_string(),
            ElementKind::Instance(i) => {
                let s = app.ed.doc.symbol(i.symbol);
                let kind = s.map(|s| match s.kind { SymbolKind::Graphic => "Graphic", SymbolKind::MovieClip => "Movie Clip", SymbolKind::Button => "Button" }).unwrap_or("?");
                format!("{kind} instance of “{}”", ellipsize(s.map(|s| s.name.as_str()).unwrap_or("?"), 28))
            }
            ElementKind::Group(c) => format!("Group ({} objects)", c.len()),
            ElementKind::Bitmap(b) => app.ed.doc.bitmap(b.bitmap).map(|a| format!("Bitmap “{}” {}×{}", a.name, a.width, a.height)).unwrap_or("Bitmap (missing)".into()),
            ElementKind::Particles(_) => "Particle emitter".to_string(),
        }
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(title).color(Color32::from_gray(200)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.add_enabled(editable, egui::Button::new("🗑 Delete")).on_hover_text("Delete (Del)").clicked() {
                app.delete();
            }
        });
    });
    if !editable {
        ui.label(RichText::new("On a locked or hidden layer: unlock it in the timeline to edit.").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
    }
    ui.add_enabled_ui(editable, |ui| {
        if n == 1 && !matches!(el.kind, ElementKind::Shape(_) | ElementKind::Canvas(_)) {
            let mut name = el.name.clone();
            ui.horizontal(|ui| {
                ui.label("Instance name");
                let r = ui.add(egui::TextEdit::singleline(&mut name).hint_text("<none>").desired_width(f32::INFINITY));
                if r.lost_focus() && name != el.name {
                    let v = name.trim().to_string();
                    app.ed.modify_selected("Instance Name", move |e| e.name = v.clone());
                }
            });
        }
        if let (1, ElementKind::Particles(e)) = (n, &el.kind) {
            particles_properties(app, ui, e);
        }
        section(ui, "Position and size");
        let (lo, hi) = app.ed.selection_bounds().unwrap_or((pt(0.0, 0.0), pt(0.0, 0.0)));
        let (w0, h0) = (hi.x - lo.x, hi.y - lo.y);
        egui::Grid::new("xform").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            let (mut x, mut y) = (lo.x, lo.y);
            ui.label("X");
            let r = ui.add(egui::DragValue::new(&mut x).speed(1.0).max_decimals(1));
            apply(app, &r, "Move", |e| e.matrix.tx += x - lo.x);
            ui.label("Y");
            let r = ui.add(egui::DragValue::new(&mut y).speed(1.0).max_decimals(1));
            apply(app, &r, "Move", |e| e.matrix.ty += y - lo.y);
            ui.end_row();
            let (mut w, mut h) = (w0, h0);
            ui.label("W");
            // No range clamp here: egui reports a clamped value as a change,
            // which would rescale a zero-width object every frame.
            let r = ui.add_enabled(w0 > 0.05, egui::DragValue::new(&mut w).speed(1.0).max_decimals(1));
            if w > 0.05 && w0 > 0.05 && (w - w0).abs() > 1e-3 {
                let m = Matrix::translate(lo.x, lo.y).concat(&Matrix::scale(w / w0, 1.0)).concat(&Matrix::translate(-lo.x, -lo.y));
                apply(app, &r, "Resize", |e| e.matrix = m.concat(&e.matrix));
            }
            ui.label("H");
            let r = ui.add_enabled(h0 > 0.05, egui::DragValue::new(&mut h).speed(1.0).max_decimals(1));
            if h > 0.05 && h0 > 0.05 && (h - h0).abs() > 1e-3 {
                let m = Matrix::translate(lo.x, lo.y).concat(&Matrix::scale(1.0, h / h0)).concat(&Matrix::translate(-lo.x, -lo.y));
                apply(app, &r, "Resize", |e| e.matrix = m.concat(&e.matrix));
            }
            ui.end_row();
        });

        section(ui, "Transform");
        let d = el.matrix.decompose();
        egui::Grid::new("xform2").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            let mut rot = d.skew_y.to_degrees();
            ui.label("Rotate");
            let r = ui.add(egui::DragValue::new(&mut rot).speed(0.5).suffix("°").max_decimals(1));
            let delta = rot.to_radians() - d.skew_y;
            apply_decomp(app, &r, "Rotate", move |dd| {
                dd.skew_y += delta;
                dd.skew_x += delta;
            });
            let mut alpha = el.alpha * 100.0;
            ui.label("Alpha");
            let r = ui.add(egui::DragValue::new(&mut alpha).range(0.0..=100.0).speed(0.5).suffix("%").max_decimals(0));
            apply(app, &r, "Color Effect", |e| e.alpha = alpha / 100.0);
            ui.end_row();
            // Skew: the angle between the object's axes (0° = none).
            let mut sh = (d.skew_x - d.skew_y).to_degrees();
            if sh.abs() < 0.05 {
                sh = 0.0; // no "-0.0°"
            }
            ui.label("Skew");
            let r = ui.add(egui::DragValue::new(&mut sh).speed(0.5).suffix("°").max_decimals(1)).on_hover_text("Horizontal skew. With Free Transform (Q) drag an edge of the box: top/bottom = horizontal, left/right = vertical. Ctrl+T for both axes");
            apply_decomp(app, &r, "Skew", move |dd| dd.skew_x = dd.skew_y + sh.clamp(-89.0, 89.0).to_radians());
            ui.end_row();
        });
        ui.horizontal(|ui| {
            if ui.button("Flip H").clicked() { app.flip(true); }
            if ui.button("Flip V").clicked() { app.flip(false); }
            if ui.button("↻ 90°").clicked() { app.rotate_selection(std::f32::consts::FRAC_PI_2); }
            if ui.button("Reset").on_hover_text("Remove Transform").clicked() {
                app.ed.modify_selected("Remove Transform", |e| { let (x, y) = (e.matrix.tx, e.matrix.ty); e.matrix = Matrix::translate(x, y); e.three_d = None; });
            }
            if ui.button("More…").on_hover_text("Transform panel (Ctrl+T)").clicked() { app.transform_open = true; }
        });

        // 3D
        let td = el.three_d;
        if td.is_some() || matches!(app.tool, Tool::Rotate3D | Tool::Translate3D) {
            section(ui, "3D position and rotation");
            let t = td.unwrap_or_default();
            egui::Grid::new("xform3d").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                let (mut rx, mut ry, mut rz, mut z) = (t.rx, t.ry, t.rz, t.z);
                ui.label("X°");
                let r = ui.add(egui::DragValue::new(&mut rx).speed(0.5).max_decimals(1));
                apply_3d(app, &r, "3D Rotation", |t| t.rx = rx);
                ui.label("Y°");
                let r = ui.add(egui::DragValue::new(&mut ry).speed(0.5).max_decimals(1));
                apply_3d(app, &r, "3D Rotation", |t| t.ry = ry);
                ui.end_row();
                ui.label("Z°");
                let r = ui.add(egui::DragValue::new(&mut rz).speed(0.5).max_decimals(1));
                apply_3d(app, &r, "3D Rotation", |t| t.rz = rz);
                ui.label("Depth");
                let r = ui.add(egui::DragValue::new(&mut z).speed(1.0).max_decimals(0)).on_hover_text("Z position: positive = further away");
                apply_3d(app, &r, "3D Translation", |t| t.z = z);
                ui.end_row();
            });
            ui.horizontal(|ui| {
                if ui.add_enabled(td.is_some(), egui::Button::new("Reset 3D")).clicked() {
                    app.ed.modify_selected("Remove 3D", |e| e.three_d = None);
                }
                if ui.button("Perspective…").on_hover_text("Perspective angle and vanishing point are document settings").clicked() {
                    app.props_tab = PropsTab::Doc;
                }
            });
        }

        match &el.kind {
            ElementKind::Canvas(c) if n == 1 => {
                section(ui, "Fill and stroke");
                let f = c.float.as_deref();
                let fill = f.and_then(|f| f.fills.first()).and_then(|r| r.fill);
                let stroke = f.and_then(|f| f.strokes.first()).map(|s| s.stroke);
                ui.horizontal(|ui| {
                    if f.map(|f| !f.fills.is_empty()).unwrap_or(false) {
                        ui.label("Fill");
                        let mut col = fill.map(c32).unwrap_or(Color32::WHITE);
                        if ui.color_edit_button_srgba(&mut col).changed() {
                            app.ed.merge_set_paint(Some(Some(from32(col))), Some(None), None);
                        }
                    }
                    if let Some(st) = stroke {
                        ui.label("Stroke");
                        let mut col = c32(st.color);
                        if ui.color_edit_button_srgba(&mut col).changed() {
                            app.ed.merge_set_paint(None, None, Some(Some(Stroke { color: from32(col), width: st.width })));
                        }
                        let mut w = st.width;
                        let r = ui.add(egui::DragValue::new(&mut w).range(0.1..=100.0).speed(0.1).suffix(" px"));
                        if r.drag_stopped() || r.lost_focus() || (r.changed() && !r.dragged()) {
                            app.ed.merge_set_paint(None, None, Some(Some(Stroke { color: st.color, width: w })));
                        }
                    }
                });
                if let Some(fl) = f.filter(|f| !f.fills.is_empty() && !app.ed.doc.bitmaps.is_empty()) {
                    let cur_id = fl.fills.first().and_then(|r| r.bitmap).map(|b| b.bitmap);
                    let cur = cur_id.and_then(|id| app.ed.doc.bitmap(id)).map(|a| a.name.clone()).unwrap_or_else(|| "None".into());
                    let mut pick: Option<Option<ac_core::bitmap::BitmapId>> = None;
                    ui.horizontal(|ui| {
                        ui.label("Bitmap fill");
                        egui::ComboBox::from_id_salt("merge-bmpfill").selected_text(cur).show_ui(ui, |ui| {
                            if ui.selectable_label(cur_id.is_none(), "None").clicked() {
                                pick = Some(None);
                            }
                            for b in &app.ed.doc.bitmaps {
                                if ui.selectable_label(cur_id == Some(b.id), &b.name).clicked() {
                                    pick = Some(Some(b.id));
                                }
                            }
                        });
                    });
                    if let Some(choice) = pick {
                        app.ed.merge_bitmap_fill(choice);
                    }
                }
                if f.is_none() {
                    hint(ui, "Click a fill or line with the Selection tool to select it; drag an edge to bend it.");
                }
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(f.is_some(), egui::Button::new("Make Drawing Object")).on_hover_text("Modify › Combine Objects › Union").clicked() {
                        app.ed.merge_to_object();
                    }
                    if ui.add_enabled(f.is_some(), egui::Button::new("Smooth")).clicked() {
                        app.ed.shape_cleanup("smooth");
                    }
                    if ui.add_enabled(f.is_some(), egui::Button::new("Straighten")).clicked() {
                        app.ed.shape_cleanup("straighten");
                    }
                    if ui.add_enabled(f.is_some(), egui::Button::new("Optimize")).clicked() {
                        app.ed.shape_cleanup("optimize");
                    }
                });
                if f.is_some_and(|f| !f.strokes.is_empty()) {
                    width_profile_buttons(app, ui);
                }
            }
            ElementKind::Shape(s) if n == 1 => {
                section(ui, "Fill and stroke");
                let mut kind = s.gradient.as_ref().map(|g| g.kind);
                ui.horizontal(|ui| {
                    ui.label("Fill");
                    let before = kind;
                    egui::ComboBox::from_id_salt("filltype").width(80.0).selected_text(match kind { None => "Solid", Some(GradientKind::Linear) => "Linear", Some(GradientKind::Radial) => "Radial" }).show_ui(ui, |ui| {
                        ui.selectable_value(&mut kind, None, "Solid");
                        ui.selectable_value(&mut kind, Some(GradientKind::Linear), "Linear gradient");
                        ui.selectable_value(&mut kind, Some(GradientKind::Radial), "Radial gradient");
                    });
                    if kind != before {
                        let base = s.fill.unwrap_or(from32(app.fill));
                        let stops = s.gradient.as_ref().map(|g| g.stops.clone()).unwrap_or_else(|| vec![(0.0, base), (1.0, from32(app.topts.fill2))]);
                        app.ed.modify_selected("Fill Type", move |e| {
                            if let ElementKind::Shape(sh) = &mut e.kind {
                                sh.gradient = kind.and_then(|k| sh.path.bounds().map(|(lo, hi)| Gradient::fit(k, stops.clone(), lo, hi)));
                                if sh.fill.is_none() {
                                    sh.fill = Some(stops[0].1);
                                }
                            }
                        });
                    }
                    if kind.is_none() {
                        let mut has_fill = s.fill.is_some();
                        let mut c = s.fill.map(c32).unwrap_or(app.fill);
                        if ui.checkbox(&mut has_fill, "").on_hover_text("Fill on/off").changed() {
                            let col = from32(c);
                            app.ed.modify_selected("Fill", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.fill = has_fill.then_some(col) });
                        }
                        if ui.color_edit_button_srgba(&mut c).changed() {
                            let col = from32(c);
                            app.ed.modify_selected("Fill Color", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.fill = Some(col) });
                        }
                    }
                });
                if let Some(g) = &s.gradient {
                    gradient_editor(app, ui, g);
                }
                ui.horizontal(|ui| {
                    let mut has_stroke = s.stroke.is_some();
                    let mut sc = s.stroke.map(|s| c32(s.color)).unwrap_or(app.stroke);
                    let w = s.stroke.map(|s| s.width).unwrap_or(app.stroke_w);
                    if ui.checkbox(&mut has_stroke, "Stroke").changed() {
                        let col = from32(sc);
                        app.ed.modify_selected("Stroke", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.stroke = has_stroke.then_some(Stroke { color: col, width: w }) });
                    }
                    if ui.color_edit_button_srgba(&mut sc).changed() {
                        let col = from32(sc);
                        app.ed.modify_selected("Stroke Color", move |e| if let ElementKind::Shape(s) = &mut e.kind { s.stroke = Some(Stroke { color: col, width: w }) });
                    }
                    if let Some(st) = s.stroke {
                        let mut w = st.width;
                        let r = ui.add(egui::DragValue::new(&mut w).range(0.1..=100.0).speed(0.1).suffix(" px"));
                        apply(app, &r, "Stroke Size", |e| if let ElementKind::Shape(s) = &mut e.kind { if let Some(st) = &mut s.stroke { st.width = w } });
                    }
                });
                if let Some(prof) = &s.width_profile {
                    ui.horizontal(|ui| {
                        hint(ui, &format!("Variable width: {} point(s)", prof.len().saturating_sub(2)));
                        if ui.small_button("Reset width").clicked() {
                            app.ed.modify_selected("Reset Width", |e| if let ElementKind::Shape(s) = &mut e.kind { s.width_profile = None });
                        }
                    });
                }
                if s.stroke.is_some() {
                    width_profile_buttons(app, ui);
                }
                shape_extras(app, ui, s);
                if let Some(w) = &s.warp {
                    ui.horizontal(|ui| {
                        hint(ui, &format!("Asset Warp: {} pin(s)", w.pins.len()));
                        if ui.small_button("Remove warp").on_hover_text("Restore the shape before warping").clicked() {
                            app.ed.modify_selected("Remove Warp", |e| if let ElementKind::Shape(s) = &mut e.kind { if let Some(w) = s.warp.take() { s.path = w.base } });
                        }
                    });
                }
            }
            ElementKind::Text(t) if n == 1 => {
                section(ui, "Text");
                {
                    use ac_core::text::TextKind;
                    let mut kind = t.field.kind;
                    ui.horizontal(|ui| {
                        ui.label("Type");
                        for (k, l, tip) in [(TextKind::Static, "Static", "Drawn as it is (outlined when published)"), (TextKind::Dynamic, "Dynamic", "Frame scripts can change its text: this.<instance name>.text = …"), (TextKind::Input, "Input", "The viewer types into it in published movies")] {
                            if ui.selectable_value(&mut kind, k, l).on_hover_text(tip).changed() {
                                app.ed.modify_selected("Text Type", move |e| {
                                    if let ElementKind::Text(t) = &mut e.kind {
                                        t.field.kind = kind;
                                        if kind != TextKind::Static {
                                            t.field.selectable = true;
                                        }
                                    }
                                });
                            }
                        }
                    });
                    if kind != TextKind::Static && el.name.trim().is_empty() {
                        hint(ui, "Give it an instance name (above) so scripts can reach it.");
                    }
                }
                let mut text = t.text.clone();
                let r = ui.add(egui::TextEdit::multiline(&mut text).desired_rows(2).desired_width(f32::INFINITY));
                if r.changed() {
                    let v = text.clone();
                    app.ed.modify_selected("Edit Text", move |e| if let ElementKind::Text(t) = &mut e.kind { t.text = v.clone() });
                }
                let mut size = t.size;
                let mut col = c32(t.color);
                let font = t.font;
                let mut align = t.align;
                let mut spacing = t.letter_spacing;
                egui::Grid::new("textprops").num_columns(4).show(ui, |ui| {
                    ui.label("Size");
                    let r = ui.add(egui::DragValue::new(&mut size).range(1.0..=500.0).speed(0.5));
                    apply(app, &r, "Text Size", |e| if let ElementKind::Text(t) = &mut e.kind { t.size = size });
                    ui.label("Colour");
                    if ui.color_edit_button_srgba(&mut col).changed() {
                        let c = from32(col);
                        app.ed.modify_selected("Text Colour", move |e| if let ElementKind::Text(t) = &mut e.kind { t.color = c });
                    }
                    ui.end_row();
                    ui.label("Spacing");
                    let r = ui.add(egui::DragValue::new(&mut spacing).range(-20.0..=100.0).speed(0.1));
                    apply(app, &r, "Letter Spacing", |e| if let ElementKind::Text(t) = &mut e.kind { t.letter_spacing = spacing });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    ui.label("Font");
                    if let Some((fam, face)) = font_picker(ui, &mut app.font_search, t.family.as_deref(), font) {
                        app.ed.modify_selected("Font", move |e| if let ElementKind::Text(t) = &mut e.kind { t.family = fam.clone(); t.font = face });
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Style");
                    if let Some((w, i)) = font_style_picker(ui, t.family.as_deref(), t.weight, t.italic) {
                        app.ed.modify_selected("Font Style", move |e| if let ElementKind::Text(t) = &mut e.kind { t.weight = w; t.italic = i });
                    }
                });
                {
                    use ac_core::text::Baseline;
                    let mut base = t.para.baseline;
                    let mut kern = !t.para.no_kern;
                    ui.horizontal(|ui| {
                        ui.label("Position");
                        for (b, l) in [(Baseline::Normal, "Normal"), (Baseline::Superscript, "Super"), (Baseline::Subscript, "Sub")] {
                            if ui.selectable_value(&mut base, b, l).changed() {
                                app.ed.modify_selected("Character Position", move |e| if let ElementKind::Text(t) = &mut e.kind { t.para.baseline = base });
                            }
                        }
                        if ui.checkbox(&mut kern, "Auto-kern").changed() {
                            app.ed.modify_selected("Auto-kern", move |e| if let ElementKind::Text(t) = &mut e.kind { t.para.no_kern = !kern });
                        }
                    });
                }
                section(ui, "Paragraph");
                ui.horizontal(|ui| {
                    for (a, l, tip) in [(Align::Left, "⬅", "Align left"), (Align::Center, "↔", "Centre"), (Align::Right, "➡", "Align right"), (Align::Justify, "☰", "Justify (fixed-width boxes)")] {
                        if ui.selectable_value(&mut align, a, l).on_hover_text(tip).changed() {
                            app.ed.modify_selected("Align", move |e| if let ElementKind::Text(t) = &mut e.kind { t.align = align });
                        }
                    }
                });
                let mut p = t.para.clone();
                egui::Grid::new("para").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
                    ui.label("Spacing");
                    let r = ui.add(egui::DragValue::new(&mut p.line_spacing).range(-50.0..=200.0).speed(0.2).suffix(" px")).on_hover_text("Extra space between lines");
                    apply(app, &r, "Line Spacing", |e| if let ElementKind::Text(t) = &mut e.kind { t.para.line_spacing = p.line_spacing });
                    ui.label("Indent");
                    let r = ui.add(egui::DragValue::new(&mut p.indent).range(-200.0..=500.0).speed(0.2).suffix(" px"));
                    apply(app, &r, "Indent", |e| if let ElementKind::Text(t) = &mut e.kind { t.para.indent = p.indent });
                    ui.end_row();
                    ui.label("Margin L");
                    let r = ui.add(egui::DragValue::new(&mut p.margin_left).range(0.0..=500.0).speed(0.2).suffix(" px"));
                    apply(app, &r, "Left Margin", |e| if let ElementKind::Text(t) = &mut e.kind { t.para.margin_left = p.margin_left });
                    ui.label("R");
                    let r = ui.add(egui::DragValue::new(&mut p.margin_right).range(0.0..=500.0).speed(0.2).suffix(" px"));
                    apply(app, &r, "Right Margin", |e| if let ElementKind::Text(t) = &mut e.kind { t.para.margin_right = p.margin_right });
                    ui.end_row();
                });
                ui.horizontal(|ui| {
                    let mut fixed = p.width.is_some();
                    if ui.checkbox(&mut fixed, "Fixed width").on_hover_text("Words wrap inside the box (drag with the Text tool to make one)").changed() {
                        let w = if fixed { Some(t.box_size().0.max(40.0)) } else { None };
                        app.ed.modify_selected("Text Box Width", move |e| if let ElementKind::Text(t) = &mut e.kind { t.para.width = w });
                    }
                    if let Some(mut w) = p.width {
                        let r = ui.add(egui::DragValue::new(&mut w).range(10.0..=4000.0).speed(0.5).suffix(" px"));
                        apply(app, &r, "Text Box Width", |e| if let ElementKind::Text(t) = &mut e.kind { t.para.width = Some(w) });
                    }
                });
                {
                    use ac_core::text::TextKind;
                    let mut f = t.field.clone();
                    section(ui, "Options");
                    ui.horizontal_wrapped(|ui| {
                        if f.kind != TextKind::Static && ui.checkbox(&mut f.selectable, "Selectable").changed() {
                            let v = f.selectable;
                            app.ed.modify_selected("Selectable", move |e| if let ElementKind::Text(t) = &mut e.kind { t.field.selectable = v });
                        }
                        if ui.checkbox(&mut f.border, "Border").on_hover_text("Show a border and white background around the box").changed() {
                            let v = f.border;
                            app.ed.modify_selected("Text Border", move |e| if let ElementKind::Text(t) = &mut e.kind { t.field.border = v });
                        }
                        if f.kind == TextKind::Input && ui.checkbox(&mut f.password, "Password").changed() {
                            let v = f.password;
                            app.ed.modify_selected("Password", move |e| if let ElementKind::Text(t) = &mut e.kind { t.field.password = v });
                        }
                    });
                    if f.kind == TextKind::Input {
                        ui.horizontal(|ui| {
                            ui.label("Max characters");
                            let r = ui.add(egui::DragValue::new(&mut f.max_chars).range(0..=100_000)).on_hover_text("0 = no limit");
                            let v = f.max_chars;
                            apply(app, &r, "Max Characters", |e| if let ElementKind::Text(t) = &mut e.kind { t.field.max_chars = v });
                        });
                    }
                    ui.horizontal(|ui| {
                        ui.label("Link");
                        let mut url = f.link.clone().unwrap_or_default();
                        let r = ui.add(egui::TextEdit::singleline(&mut url).hint_text("https://…").desired_width(160.0));
                        if r.lost_focus() && Some(url.trim().to_string()).filter(|u| !u.is_empty()) != f.link {
                            let v = Some(url.trim().to_string()).filter(|u| !u.is_empty());
                            app.ed.modify_selected("Text Link", move |e| if let ElementKind::Text(t) = &mut e.kind { t.field.link = v.clone() });
                        }
                    });
                }
            }
            ElementKind::Instance(inst) if n == 1 => {
                let id = inst.symbol;
                let kind = app.ed.doc.symbol(id).map(|s| s.kind);
                if kind == Some(SymbolKind::Graphic) {
                    section(ui, "Looping");
                    let mut mode = inst.loop_mode;
                    ui.horizontal(|ui| {
                        ui.label("Options");
                        egui::ComboBox::from_id_salt("loop-mode").selected_text(mode.name()).show_ui(ui, |ui| {
                            for m in LoopMode::ALL {
                                if ui.selectable_value(&mut mode, m, m.name()).changed() {
                                    app.ed.modify_selected("Looping", move |e| if let ElementKind::Instance(i) = &mut e.kind { i.loop_mode = mode });
                                }
                            }
                        });
                    });
                    let mut ff = inst.first_frame + 1;
                    ui.horizontal(|ui| {
                        ui.label("First frame");
                        let r = ui.add(egui::DragValue::new(&mut ff).range(1..=999));
                        apply(app, &r, "First Frame", |e| if let ElementKind::Instance(i) = &mut e.kind { i.first_frame = ff - 1 });
                    });
                }
                color_effect(app, ui, &el);
                if kind == Some(SymbolKind::Graphic) && n == 1 {
                    lip_sync_section(app, ui, id);
                }
                if kind == Some(SymbolKind::Button) {
                    ui.label(RichText::new("Button: Up / Over / Down / Hit frames. Control › Enable Simple Buttons to try it on the stage.").small().color(Color32::from_gray(150)));
                }
                ui.horizontal(|ui| {
                    ui.label("Symbol");
                    let cur = app.ed.doc.symbol(id).map(|s| s.name.clone()).unwrap_or_default();
                    let mut pick = None;
                    egui::ComboBox::from_id_salt("swap-symbol").width((ui.available_width() - 80.0).clamp(60.0, 220.0)).selected_text(ellipsize(&cur, 30)).show_ui(ui, |ui| {
                        for sym in &app.ed.doc.library {
                            if Some(sym.id) == app.ed.editing || sym.name == ac_core::editor::GROUP_EDIT_NAME {
                                continue; // a symbol can't contain itself
                            }
                            if ui.selectable_label(sym.id == id, ellipsize(&sym.name, 60)).clicked() && sym.id != id {
                                pick = Some(sym.id);
                            }
                        }
                    }).response.on_hover_text("Swap Symbol: keep this instance's transform and effects, show another symbol");
                    if let Some(new_id) = pick {
                        app.ed.swap_symbol(new_id);
                    }
                    if ui.button("Duplicate").on_hover_text("Duplicate Symbol and swap: copy this symbol (same library folder) and show the copy here, so you can change it without changing the other instances").clicked() {
                        if let Some(nid) = app.ed.duplicate_and_swap("") {
                            app.status = format!("Swapped to {}", app.ed.doc.symbol(nid).map(|s| s.name.as_str()).unwrap_or(""));
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Edit Symbol").clicked() { app.edit_symbol(id); }
                    if ui.button("Break Apart").on_hover_text("Ctrl+B").clicked() { app.ed.break_apart(); }
                });
            }
            ElementKind::Group(_) if n == 1 => {
                section(ui, "Group");
                ui.horizontal(|ui| {
                    if ui.button("Ungroup (Ctrl+Shift+G)").clicked() { app.ed.ungroup_selected(); }
                });
            }
            ElementKind::Bitmap(b) if n == 1 => {
                section(ui, "Bitmap");
                let id = b.bitmap;
                if let Some(a) = app.ed.doc.bitmap(id) {
                    let mut smooth = a.smoothing;
                    ui.horizontal(|ui| {
                        if ui.checkbox(&mut smooth, "Allow smoothing").on_hover_text("Off = sharp pixels when scaled (pixel art)").changed() {
                            app.ed.set_bitmap_smoothing(id, smooth);
                        }
                        hint(ui, &format!("used {}×", app.ed.bitmap_usage(id)));
                    });
                }
            }
            _ => {}
        }

        filters_section(app, ui, &el);
        if let Some(w) = &el.mesh {
            ui.horizontal(|ui| {
                hint(ui, &format!("Asset Warp: {} pin(s)", w.pins.len()));
                if ui.small_button("Remove warp").clicked() {
                    app.ed.modify_selected("Remove Warp", |e| e.mesh = None);
                }
            });
        }

        // Bones (IK)
        let bones = app.ed.bones(sel0.layer).to_vec();
        if n == 1 && bones.iter().any(|b| b.parent == sel0.element || b.child == sel0.element) {
            section(ui, "Bones (inverse kinematics)");
            let name = |i: usize| app.ed.element_at(ac_core::Selection { layer: sel0.layer, element: i }).map(|e| if e.name.is_empty() { format!("object {}", i + 1) } else { e.name }).unwrap_or_default();
            let parent = bones.iter().find(|b| b.child == sel0.element).map(|b| b.parent);
            let kids: Vec<usize> = bones.iter().filter(|b| b.parent == sel0.element).map(|b| b.child).collect();
            match parent {
                Some(p) => hint(ui, &format!("Attached to {}", name(p))),
                None => hint(ui, "Root of the armature (dragging it moves the whole armature)"),
            }
            if !kids.is_empty() {
                hint(ui, &format!("Drives: {}", kids.iter().map(|k| name(*k)).collect::<Vec<_>>().join(", ")));
            }
            // Joint: Rotation (enable / constrain), as in Animate's bone properties
            if let Some(b) = bones.iter().find(|b| b.child == sel0.element).copied() {
                if let Some((rotate, limit, pin)) = joint_ui(ui, ("joint", sel0.layer, sel0.element), b.rotate, b.limit, b.pin.is_some()) {
                    if pin != b.pin.is_some() {
                        app.ed.set_joint_pin(sel0.layer, sel0.element, pin);
                    } else {
                        app.ed.set_joint(sel0.layer, sel0.element, rotate, limit);
                    }
                }
            }
            ui.horizontal(|ui| {
                if ui.add_enabled(parent.is_some(), egui::Button::new("Remove bone")).on_hover_text("Detach this object from its parent").clicked() {
                    app.ed.remove_bone(sel0.layer, sel0.element);
                }
                if ui.button(format!("Remove all bones ({})", bones.len())).clicked() {
                    app.ed.clear_bones(sel0.layer);
                }
            });
            hint(ui, "Drag a bonded object with the Selection tool (V) to pose the chain.");
        }

        ui.horizontal(|ui| {
            if ui.button("Convert to Symbol… (F8)").clicked() { app.open_symbol_dialog(true); }
            if ui.button("Duplicate").clicked() { app.ed.duplicate(); }
        });
    });
}

/// Gradient stops: colours and positions, add/remove, refit.
fn gradient_editor(app: &mut App, ui: &mut egui::Ui, g: &Gradient) {
    let preview_h = 14.0;
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width().min(240.0), preview_h), Sense::hover());
    let mut mesh = egui::Mesh::default();
    let mut stops = g.stops.clone();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut add_col = |x: f32, c: Color32| {
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(pos2(r.left() + x * r.width(), r.top()), c);
        mesh.colored_vertex(pos2(r.left() + x * r.width(), r.bottom()), c);
        if i >= 2 {
            mesh.add_triangle(i - 2, i - 1, i);
            mesh.add_triangle(i - 1, i, i + 1);
        }
    };
    if let Some(f) = stops.first() {
        add_col(0.0, c32(f.1));
    }
    for (t, c) in &stops {
        add_col(t.clamp(0.0, 1.0), c32(*c));
    }
    if let Some(l) = stops.last() {
        add_col(1.0, c32(l.1));
    }
    ui.painter().add(mesh);
    ui.painter().rect_stroke(r, 0.0, EStroke::new(1.0_f32, Color32::from_gray(90)));
    let mut new_stops = g.stops.clone();
    let mut changed: Option<&str> = None;
    let mut remove: Option<usize> = None;
    for (i, (t, c)) in g.stops.iter().enumerate() {
        ui.horizontal(|ui| {
            let mut col = c32(*c);
            if ui.color_edit_button_srgba(&mut col).changed() {
                new_stops[i].1 = from32(col);
                changed = Some("Gradient Colour");
            }
            let mut pos = t * 100.0;
            if edited(&ui.add(egui::DragValue::new(&mut pos).range(0.0..=100.0).speed(0.5).suffix("%"))) {
                new_stops[i].0 = pos / 100.0;
                changed = Some("Gradient Stop");
            }
            if g.stops.len() > 2 && ui.small_button("✕").on_hover_text("Remove colour stop").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        new_stops.remove(i);
        changed = Some("Remove Gradient Stop");
    }
    ui.horizontal(|ui| {
        if ui.small_button("+ Stop").clicked() {
            let c = new_stops.last().map(|s| s.1).unwrap_or(from32(app.fill));
            new_stops.push((0.5, c));
            changed = Some("Add Gradient Stop");
        }
        if ui.small_button("Fit to shape").on_hover_text("Reset the gradient's position and size").clicked() {
            let (kind, stops) = (g.kind, new_stops.clone());
            app.ed.modify_selected("Fit Gradient", move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.gradient = sh.path.bounds().map(|(lo, hi)| Gradient::fit(kind, stops.clone(), lo, hi)) });
        }
        if ui.small_button("Adjust…").on_hover_text("Gradient Transform tool (F)").clicked() {
            app.set_tool(Tool::GradientTransform);
        }
    });
    let mut spread = g.spread;
    ui.horizontal(|ui| {
        ui.label("Flow");
        for s in ac_core::Spread::ALL {
            ui.selectable_value(&mut spread, s, s.name()).on_hover_text("What the gradient does past its ends");
        }
    });
    if g.kind == GradientKind::Radial {
        let mut focal = g.focal;
        let r = ui.add(egui::Slider::new(&mut focal, -1.0..=1.0).text("Focal point").max_decimals(2)).on_hover_text("Moves the bright centre of the radial gradient along its axis (rotate it with the Gradient Transform tool)");
        apply(app, &r, "Gradient Focal Point", move |e| if let ElementKind::Shape(sh) = &mut e.kind { if let Some(g) = &mut sh.gradient { g.focal = focal } });
    }
    if spread != g.spread {
        app.ed.modify_selected("Gradient Flow", move |e| if let ElementKind::Shape(sh) = &mut e.kind { if let Some(g) = &mut sh.gradient { g.spread = spread } });
    }
    if let Some(label) = changed {
        new_stops.sort_by(|a, b| a.0.total_cmp(&b.0));
        app.ed.modify_selected(label, move |e| if let ElementKind::Shape(sh) = &mut e.kind { if let Some(g) = &mut sh.gradient { g.stops = new_stops.clone() } });
    }
}

fn frame_properties(app: &mut App, ui: &mut egui::Ui) {
    let target = app.ed.frame_target();
    let tl = app.ed.timeline();
    let Some(layer) = tl.layers.get(target.layer0) else { return };
    let layer_name = layer.name.clone();
    let what = if target.is_single() {
        format!("Frame {} · {}", target.f0 + 1, layer_name)
    } else if target.layer0 == target.layer1 {
        format!("Frames {}–{} · {}", target.f0 + 1, target.f1 + 1, layer_name)
    } else {
        format!("Frames {}–{} · {} layers", target.f0 + 1, target.f1 + 1, target.layer1 - target.layer0 + 1)
    };
    section(ui, if app.ed.frame_sel.is_some() { "Frame selection" } else { "Frame" });
    ui.label(RichText::new(what).color(Color32::from_gray(200)));

    let span = layer.span_at(target.f0).map(|(s, k, _)| (s, k.clone()));
    let Some((start, kf)) = span else {
        ui.label(RichText::new("Empty frame. Press F6/F7 or draw here to add a keyframe.").small().color(Color32::from_gray(150)));
        frame_buttons(app, ui);
        return;
    };
    ui.label(RichText::new(format!("Keyframe at {} · {} object(s)", start + 1, kf.elements.len())).small().color(Color32::from_gray(150)));

    if target.is_single() {
        let mut label = kf.label.clone().unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label("Label");
            let r = ui.text_edit_singleline(&mut label);
            if r.lost_focus() && label != kf.label.clone().unwrap_or_default() {
                app.ed.set_frame_label(&label);
            }
        });
        if kf.label.is_some() {
            let mut lk = kf.label_kind;
            ui.horizontal(|ui| {
                ui.label("Type");
                egui::ComboBox::from_id_salt("label-kind").selected_text(lk.name()).show_ui(ui, |ui| {
                    for k in ac_core::LabelKind::ALL {
                        ui.selectable_value(&mut lk, k, k.name()).on_hover_text(match k {
                            ac_core::LabelKind::Name => "A frame name scripts can jump to (gotoAndPlay)",
                            ac_core::LabelKind::Comment => "A note for authors; not published",
                            ac_core::LabelKind::Anchor => "A named anchor; published like a name",
                        });
                    }
                });
            });
            if lk != kf.label_kind {
                app.ed.set_frame_label_kind(lk);
            }
        }
    }
    // Tween
    let cur = match kf.tween { Tween::None => 0, Tween::Classic { .. } => 1, Tween::Shape { .. } => 2, Tween::Motion { .. } => 3 };
    let mut kind = cur;
    ui.horizontal(|ui| {
        ui.label("Tween");
        egui::ComboBox::from_id_salt("tween").selected_text(["None", "Classic", "Shape", "Motion"][kind]).show_ui(ui, |ui| {
            ui.selectable_value(&mut kind, 0, "None");
            ui.selectable_value(&mut kind, 1, "Classic");
            ui.selectable_value(&mut kind, 2, "Shape");
            ui.selectable_value(&mut kind, 3, "Motion");
        });
    });
    let ease = kf.tween.ease();
    let turns = match kf.tween {
        Tween::Classic { rotate_turns, .. } => rotate_turns,
        Tween::Motion { .. } => kf.motion.as_ref().map(|m| m.rotate_turns).unwrap_or(0),
        _ => 0,
    };
    if kind != cur {
        app.ed.set_tween(match kind {
            1 => Tween::Classic { ease, rotate_turns: turns },
            2 => Tween::Shape { ease },
            3 => Tween::Motion { ease },
            _ => Tween::None,
        });
    } else if kind != 0 {
        let (mut e, mut t) = (ease, turns);
        let mut changed = false;
        if kf.ease_curve.is_none() {
            ui.horizontal(|ui| {
                ui.label("Amount");
                changed |= ui.add(egui::Slider::new(&mut e, -100.0..=100.0).step_by(1.0)).changed();
            });
            ui.label(RichText::new("−100 = ease in (slow start) · +100 = ease out (slow end)").small().color(Color32::from_gray(140)));
        }
        crate::motion_ui::span_ease_ui(app, ui, kf.ease_curve.as_ref(), ease);
        let guided = kind == 1 && ac_core::layer_effect_parent(app.ed.timeline(), target.layer0, ac_core::LayerKind::Guide).is_some();
        if guided || kind == 3 {
            let mut orient = kf.orient_to_path;
            if ui.checkbox(&mut orient, "Orient to path").on_hover_text("Rotate with the path's direction").changed() {
                app.ed.set_orient_to_path(orient);
            }
        }
        if kind == 1 {
            classic_options(app, ui, &kf, guided);
        }
        if kind == 1 || kind == 3 {
            ui.horizontal(|ui| {
                ui.label("Rotate");
                changed |= edited(&ui.add(egui::DragValue::new(&mut t).range(-10..=10).suffix(" × CW")));
            });
        }
        if kind == 3 {
            let n = kf.motion.as_ref().map(|m| m.key_frames().len()).unwrap_or(0);
            ui.label(RichText::new(format!("{n} frame(s) with property keyframes. Move to a frame and change the object to add one.")).small().color(Color32::from_gray(150)));
            if ui.button("Open Motion Editor").clicked() {
                app.motion.open = true;
            }
        }
        if changed {
            app.ed.set_tween_params(e, t);
        }
    }
    frame_buttons(app, ui);
    sound_section(app, ui, &kf);
    if target.is_single() {
        section(ui, "Frame script (JavaScript)");
        let code = kf.script.clone().unwrap_or_default();
        let lines = code.lines().count();
        ui.label(RichText::new(if code.is_empty() { "No script on this keyframe.".to_string() } else { format!("{lines} line(s) on keyframe {}", start + 1) }).small().color(Color32::from_gray(150)));
        if ui.button("Edit in Actions panel (F9)").clicked() {
            app.scripting.actions_open = true;
        }
    }
}

/// Classic tween: Sync, Snap and "Edit each property separately" eases.
fn classic_options(app: &mut App, ui: &mut egui::Ui, kf: &ac_core::Keyframe, guided: bool) {
    use ac_core::motion::{PropEases, PropGroup};
    ui.horizontal(|ui| {
        let mut sync = kf.sync;
        if ui.checkbox(&mut sync, "Sync").on_hover_text("Graphic symbols keep playing across the following keyframes instead of restarting").changed() {
            app.ed.edit_keyframe("Sync", |k| k.sync = sync);
        }
        if guided {
            let mut snap = kf.snap;
            if ui.checkbox(&mut snap, "Snap").on_hover_text("Snap the transformation point to the motion guide").changed() {
                app.ed.edit_keyframe("Snap", |k| k.snap = snap);
            }
        }
    });
    let mut separate = kf.prop_eases.is_some();
    if ui.checkbox(&mut separate, "Ease each property separately").changed() {
        app.ed.edit_keyframe("Property Eases", |k| k.prop_eases = separate.then(Default::default));
    }
    let Some(pe) = kf.prop_eases.as_deref() else { return };
    for g in PropGroup::ALL {
        ui.horizontal(|ui| {
            ui.add_sized([62.0, 18.0], egui::Label::new(g.name()));
            if let Some(choice) = crate::motion_ui::ease_combo(ui, ("prop-ease", g.id()), pe.get(g), "Same as tween", 0.0, 140.0) {
                app.ed.edit_keyframe("Property Ease", |k| {
                    let p = k.prop_eases.get_or_insert_with(|| Box::new(PropEases::default()));
                    *p.get_mut(g) = choice;
                });
            }
        });
    }
}

/// Properties ▸ Sound for the keyframe at the playhead.
fn sound_section(app: &mut App, ui: &mut egui::Ui, kf: &ac_core::Keyframe) {
    use ac_core::sound::{FrameSound, SoundSync};
    section(ui, "Sound");
    let cur = kf.sound.clone();
    let name_of = |id: u32| app.ed.doc.sound(id).map(|s| s.name.clone()).unwrap_or_else(|| "(missing)".into());
    let mut pick: Option<Option<u32>> = None;
    ui.horizontal(|ui| {
        ui.label("Name");
        // long file names (real projects have them) must not widen the panel
        egui::ComboBox::from_id_salt("frame-sound").width(ui.available_width().min(220.0)).selected_text(ellipsize(&cur.as_ref().map(|s| name_of(s.sound)).unwrap_or_else(|| "None".into()), 30)).show_ui(ui, |ui| {
            if ui.selectable_label(cur.is_none(), "None").clicked() {
                pick = Some(None);
            }
            for s in &app.ed.doc.sounds {
                if ui.selectable_label(cur.as_ref().is_some_and(|c| c.sound == s.id), ellipsize(&s.name, 60)).on_hover_text(&s.name).clicked() {
                    pick = Some(Some(s.id));
                }
            }
        });
    });
    if let Some(p) = pick {
        match (p, &cur) {
            (None, _) => app.ed.set_frame_sound(None),
            (Some(id), Some(c)) => app.ed.set_frame_sound(Some(FrameSound { sound: id, ..c.clone() })),
            (Some(id), None) => app.ed.set_frame_sound(Some(FrameSound::new(id))),
        }
        return;
    }
    let Some(fs) = cur else {
        if app.ed.doc.sounds.is_empty() {
            hint(ui, "File › Import › Import Sound… (WAV, MP3, OGG, FLAC), then pick it here or drag it from the Library onto the stage.");
        }
        return;
    };
    let mut sync = fs.sync;
    ui.horizontal(|ui| {
        ui.label("Sync");
        egui::ComboBox::from_id_salt("sound-sync").selected_text(sync.name()).show_ui(ui, |ui| {
            for s in SoundSync::ALL {
                ui.selectable_value(&mut sync, s, s.name()).on_hover_text(match s {
                    SoundSync::Event => "Plays to its end on its own",
                    SoundSync::Start => "Like Event, but not if it's already playing",
                    SoundSync::Stop => "Silences this sound",
                    SoundSync::Stream => "Follows the timeline; stops where the frames end",
                });
            }
        });
    });
    if sync != fs.sync {
        app.ed.edit_frame_sound(|s| s.sync = sync);
    }
    if fs.sync != SoundSync::Stop {
        let mut effect = fs.effect;
        ui.horizontal(|ui| {
            ui.label("Effect");
            egui::ComboBox::from_id_salt("sound-effect").selected_text(effect.name()).show_ui(ui, |ui| {
                for e in ac_core::sound::SoundEffect::ALL {
                    ui.selectable_value(&mut effect, e, e.name());
                }
            });
        });
        if effect != fs.effect {
            app.ed.edit_frame_sound(|s| s.effect = effect);
        }
    }
    if fs.sync != SoundSync::Stop {
        let mut forever = fs.forever;
        let mut rep = fs.repeat;
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("sound-loop").width(80.0).selected_text(if forever { "Loop" } else { "Repeat" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut forever, false, "Repeat");
                ui.selectable_value(&mut forever, true, "Loop");
            });
            if !forever {
                ui.add(egui::DragValue::new(&mut rep).range(1..=999).suffix(" ×"));
            }
        });
        if forever != fs.forever || rep != fs.repeat {
            app.ed.edit_frame_sound(|s| {
                s.forever = forever;
                s.repeat = rep;
            });
        }
        if let Some(p) = app.ed.doc.sound(fs.sound).and_then(ac_audio::pcm) {
            ui.label(RichText::new(format!("{:.2} s · {:.0} frames at {} fps", p.seconds(), p.seconds() * app.ed.doc.fps, app.ed.doc.fps)).small().color(Color32::from_gray(150)));
        }
    }
}

/// Properties ▸ Lip Syncing for a selected graphic instance.
fn lip_sync_section(app: &mut App, ui: &mut egui::Ui, symbol: ac_core::SymbolId) {
    let layers: Vec<(usize, String)> = app.ed.timeline().layers.iter().enumerate().filter(|(_, l)| l.keyframes.values().any(|k| k.sound.is_some())).map(|(i, l)| (i, l.name.clone())).collect();
    section(ui, "Lip Syncing");
    if layers.is_empty() {
        hint(ui, "Put a voice sound on a layer, then match this mouth symbol to it. The symbol's frames 1–10 are the mouth shapes: Neutral, MBP, Ai, E, O, U, FV, L, WQ, etc.");
        return;
    }
    let id = egui::Id::new("lipsync-layer");
    let mut pick: usize = ui.ctx().data(|d| d.get_temp(id)).filter(|v| layers.iter().any(|(i, _)| i == v)).unwrap_or(layers[0].0);
    ui.horizontal(|ui| {
        ui.label("Audio layer");
        egui::ComboBox::from_id_salt("lipsync-layer-combo").selected_text(layers.iter().find(|(i, _)| *i == pick).map(|l| l.1.clone()).unwrap_or_default()).show_ui(ui, |ui| {
            for (i, n) in &layers {
                ui.selectable_value(&mut pick, *i, n);
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(id, pick));
    // which symbol frame shows each mouth shape (Animate's Lip-sync mapping)
    let poses = app.ed.doc.symbol(symbol).map(|s| s.timeline.frame_count()).unwrap_or(1);
    let map_id = egui::Id::new(("lipsync-map", symbol));
    let mut map: Vec<u32> = ui.ctx().data(|d| d.get_temp(map_id)).unwrap_or_else(|| (0..10).map(|i| (i as u32).min(poses - 1) + 1).collect());
    egui::CollapsingHeader::new(format!("Mouth shapes ({poses} frames in the symbol)")).id_salt("lipsync-shapes").show(ui, |ui| {
        egui::Grid::new("visemes").num_columns(4).spacing([8.0, 4.0]).show(ui, |ui| {
            for (i, v) in ac_core::sound::Viseme::ALL.iter().enumerate() {
                ui.label(v.name());
                ui.add(egui::DragValue::new(&mut map[i]).range(1..=poses).prefix("frame "));
                if i % 2 == 1 {
                    ui.end_row();
                }
            }
        });
    });
    ui.ctx().data_mut(|d| d.insert_temp(map_id, map.clone()));
    // speech-based analysis when Rhubarb Lip Sync is installed (separate program, MIT)
    let rhubarb = ac_audio::rhubarb::find();
    let mut use_rh = ac_audio::rhubarb::preferred();
    ui.horizontal(|ui| {
        ui.add_enabled_ui(rhubarb.is_some(), |ui| {
            if ui.checkbox(&mut use_rh, "Use Rhubarb Lip Sync").changed() {
                ac_audio::rhubarb::set_preferred(use_rh);
            }
        });
        let note = match &rhubarb {
            Some(p) => format!("found: {}", p.display()),
            None => "not installed — using the built-in analysis (put rhubarb on PATH or next to Flare, or set AC_RHUBARB)".into(),
        };
        ui.add(egui::Label::new(RichText::new(note).small().color(Color32::from_gray(140))).wrap());
    });
    if ui.button("Lip Sync").on_hover_text("Keyframe this instance to the voice: each frame shows the mouth shape heard, using the frames chosen under Mouth shapes").clicked() {
        let pairs: Vec<(ac_core::sound::Viseme, u32)> = ac_core::sound::Viseme::ALL.iter().zip(&map).map(|(v, f)| (*v, f.saturating_sub(1))).collect();
        app.status = match app.ed.lip_sync(pick, &pairs) {
            Ok(n) => format!("Lip sync: {n} mouth keyframe{}", if n == 1 { "" } else { "s" }),
            Err(e) => format!("Lip sync: {e}"),
        };
    }
}

fn frame_buttons(app: &mut App, ui: &mut egui::Ui) {
    ui.horizontal_wrapped(|ui| {
        if ui.button("Keyframe (F6)").clicked() { app.ed.insert_keyframe(); }
        if ui.button("Blank (F7)").clicked() { app.ed.insert_blank_keyframe(); }
        if ui.button("+ Frame (F5)").clicked() { app.ed.insert_frame(); }
        if ui.button("− Frames").on_hover_text("Remove Frames (Shift+F5)").clicked() { app.ed.remove_frame(); }
        if ui.button("Clear").on_hover_text("Clear Frames (Delete)").clicked() { app.ed.clear_frames(); }
    });
}

fn document_properties(app: &mut App, ui: &mut egui::Ui) {
    let d = &app.ed.doc;
    let (mut w, mut h, mut fps, mut bg) = (d.width, d.height, d.fps, c32(d.background));
    section(ui, "Document");
    let mut changed = false;
    egui::Grid::new("docprops").num_columns(4).show(ui, |ui| {
        ui.label("W");
        changed |= edited(&ui.add(egui::DragValue::new(&mut w).range(16.0..=8000.0)));
        ui.label("H");
        changed |= edited(&ui.add(egui::DragValue::new(&mut h).range(16.0..=8000.0)));
        ui.end_row();
        ui.label("FPS");
        changed |= edited(&ui.add(egui::DragValue::new(&mut fps).range(0.1..=240.0).speed(0.1)));
        ui.label("Stage");
        changed |= ui.color_edit_button_srgba(&mut bg).changed();
        ui.end_row();
    });
    if changed {
        app.ed.set_doc_props(w, h, fps, from32(bg));
        app.fit_pending = true;
    }
    if ui.button("Document Settings…").on_hover_text("Units, anchor, scale content, scale spans (Ctrl+J)").clicked() {
        app.doc_dlg = Some(crate::windows::DocDlg::of(app));
    }
    section(ui, "3D perspective");
    let d = &app.ed.doc;
    let (mut angle, vp) = (d.perspective_angle, d.vanishing_point);
    let cur = d.perspective().vanishing;
    let (mut vx, mut vy) = (cur.x, cur.y);
    let mut pchanged = None;
    egui::Grid::new("persp").num_columns(4).show(ui, |ui| {
        ui.label("Angle");
        if edited(&ui.add(egui::DragValue::new(&mut angle).range(1.0..=179.0).speed(0.5).suffix("°")).on_hover_text("Perspective angle (field of view). Larger = stronger perspective")) {
            pchanged = Some(vp);
        }
        ui.end_row();
        ui.label("Vanishing X");
        if edited(&ui.add(egui::DragValue::new(&mut vx).speed(1.0))) {
            pchanged = Some(Some(pt(vx, vy)));
        }
        ui.label("Y");
        if edited(&ui.add(egui::DragValue::new(&mut vy).speed(1.0))) {
            pchanged = Some(Some(pt(vx, vy)));
        }
        ui.end_row();
    });
    if ui.add_enabled(vp.is_some() || (angle - 55.0).abs() > 1e-3, egui::Button::new("Reset perspective")).clicked() {
        app.ed.set_perspective(55.0, None);
    } else if let Some(v) = pchanged {
        app.ed.set_perspective(angle, v);
    }
    hint(ui, "Applies to objects edited with the 3D Rotation (W) and 3D Translation (G) tools.");
    section(ui, "Camera");
    ui.horizontal(|ui| {
        if app.ed.camera_layer().is_none() {
            if ui.add_enabled(app.ed.editing.is_none(), egui::Button::new("Add camera layer")).clicked() {
                app.ed.add_camera();
                app.set_tool(Tool::Camera);
            }
        } else if ui.button("Remove camera layer").clicked() {
            app.ed.remove_camera();
        }
    });
}

/// Render the selected library item at `frame` into the preview texture.
fn library_preview(app: &mut App, ui: &mut egui::Ui, prev_rect: Rect) {
    ui.painter().rect_filled(prev_rect, 3.0, Color32::from_gray(0x26));
    if let Some(asset) = app.lib.sound.and_then(|id| app.ed.doc.sound(id)) {
        let r = prev_rect.shrink(8.0);
        match ac_audio::pcm(asset) {
            Some(p) => {
                let n = r.width() as usize;
                let pk = ac_audio::peaks(&p, 0.0, p.seconds(), n.max(1));
                let mid = r.center().y;
                let h = r.height() / 2.0;
                for (i, (lo, hi)) in pk.iter().enumerate() {
                    let x = r.left() + i as f32 + 0.5;
                    ui.painter().line_segment([pos2(x, mid - hi * h), pos2(x, mid - lo * h)], EStroke::new(1.0_f32, Color32::from_rgb(0x6C, 0xC6, 0xFF)));
                }
                ui.painter().line_segment([pos2(r.left(), mid), pos2(r.right(), mid)], EStroke::new(1.0_f32, Color32::from_gray(80)));
                ui.painter().text(r.left_top(), Align2::LEFT_TOP, format!("{} · {:.2} s · {} Hz", asset.name, p.seconds(), p.rate), FontId::proportional(11.0), Color32::from_gray(190));
            }
            None => {
                ui.painter().text(prev_rect.center(), Align2::CENTER_CENTER, "This build can't decode the sound", FontId::proportional(12.0), Color32::from_gray(160));
            }
        }
        return;
    }
    if let Some(asset) = app.lib.bitmap.and_then(|id| app.ed.doc.bitmap(id)) {
        // bitmap preview: fit inside the box, checkerboard behind transparency
        let (w, h) = (asset.width as f32, asset.height as f32);
        let k = ((prev_rect.width() - 16.0) / w).min((prev_rect.height() - 16.0) / h).min(4.0);
        let r = Rect::from_center_size(prev_rect.center(), vec2(w * k, h * k));
        let ppp = ui.ctx().pixels_per_point();
        let (pw, ph) = ((r.width() * ppp).max(1.0) as u32, (r.height() * ppp).max(1.0) as u32);
        if let Some(mut pix) = sk::Pixmap::new(pw, ph) {
            for y in (0..ph).step_by(8) {
                for x in (0..pw).step_by(8) {
                    if (x / 8 + y / 8) % 2 == 0 {
                        if let Some(rr) = sk::Rect::from_xywh(x as f32, y as f32, 8.0, 8.0) {
                            let mut paint = sk::Paint::default();
                            paint.set_color_rgba8(0x3A, 0x3A, 0x3A, 255);
                            pix.fill_rect(rr, &paint, sk::Transform::identity(), None);
                        }
                    }
                }
            }
            let el = Element::bitmap("", asset.id, Matrix::IDENTITY);
            let mut items = Vec::new();
            ac_core::eval_element(&ac_core::EvalCtx::new(&app.ed.doc, 0), &el, &Matrix::IDENTITY, 1.0, 0, None, 0, &mut items);
            ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: Matrix::scale(pw as f32 / w, ph as f32 / h), ..Default::default() });
            let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
            let tex = ui.ctx().load_texture("lib-bitmap", img, egui::TextureOptions::LINEAR);
            ui.painter().image(tex.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
            app.lib_tex = Some(tex);
        }
        return;
    }
    let Some(sym) = app.lib.selected_symbol(app) else {
        ui.painter().text(prev_rect.center(), Align2::CENTER_CENTER, "Select an item to preview", FontId::proportional(12.0), Color32::from_gray(120));
        return;
    };
    let ppp = ui.ctx().pixels_per_point();
    let (pw, ph) = ((prev_rect.width() * ppp) as u32, (prev_rect.height() * ppp) as u32);
    let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) else { return };
    let frames = sym.timeline.frame_count().max(1);
    let f = app.lib.frame % frames;
    // Fit using the union of all frames so the preview doesn't jump while playing.
    let ctx = ac_core::EvalCtx::new(&app.ed.doc, f);
    let mut bounds: Option<(Pt, Pt)> = None;
    for k in (0..frames).step_by(((frames / 12).max(1)) as usize) {
        let mut items = Vec::new();
        ac_core::evaluate(&ac_core::EvalCtx::new(&app.ed.doc, k), &sym.timeline, k, &Matrix::IDENTITY, 1.0, 1, &mut items);
        for b in items.iter().filter_map(|it| it.path.transformed(&it.matrix).bounds()) {
            bounds = Some(match bounds {
                None => b,
                Some(a) => (pt(a.0.x.min(b.0.x), a.0.y.min(b.0.y)), pt(a.1.x.max(b.1.x), a.1.y.max(b.1.y))),
            });
        }
    }
    let mut items = Vec::new();
    ac_core::evaluate(&ctx, &sym.timeline, f, &Matrix::IDENTITY, 1.0, 1, &mut items);
    if let Some((lo, hi)) = bounds {
        let s = ((pw as f32 - 20.0) / (hi.x - lo.x).max(1.0)).min((ph as f32 - 20.0) / (hi.y - lo.y).max(1.0)).min(6.0 * ppp);
        let view = Matrix::translate(pw as f32 / 2.0, ph as f32 / 2.0).concat(&Matrix::scale(s, s)).concat(&Matrix::translate(-(lo.x + hi.x) / 2.0, -(lo.y + hi.y) / 2.0));
        ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view, ..Default::default() });
    }
    let img = egui::ColorImage::from_rgba_premultiplied([pw as usize, ph as usize], pix.data());
    let tex = match &mut app.lib_tex {
        Some(t) => {
            t.set(img, egui::TextureOptions::LINEAR);
            t.clone()
        }
        None => {
            let t = ui.ctx().load_texture("lib", img, egui::TextureOptions::LINEAR);
            app.lib_tex = Some(t.clone());
            t
        }
    };
    ui.painter().image(tex.id(), prev_rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
}

impl LibraryState {
    fn selected_symbol<'a>(&self, app: &'a App) -> Option<&'a ac_core::Symbol> {
        app.lib_selected.and_then(|id| app.ed.doc.symbol(id))
    }
}

/// The Library: search, animated preview, sortable item list, actions. It
/// takes all the height it is given.
fn library(app: &mut App, ui: &mut egui::Ui) {
    let total_h = ui.available_height();
    ui.horizontal(|ui| {
        ui.add(egui::TextEdit::singleline(&mut app.lib.search).hint_text("🔍 Search library").desired_width(ui.available_width() - 70.0));
        ui.label(RichText::new(format!("{} items", app.ed.doc.library.iter().filter(|s| s.name != ac_core::editor::GROUP_EDIT_NAME).count() + app.ed.doc.bitmaps.len() + app.ed.doc.sounds.len())).small().color(Color32::from_gray(150)));
    });
    // Preview with play controls
    let prev_h = (total_h * 0.32).clamp(90.0, 260.0);
    let (prev_rect, prev_resp) = ui.allocate_exact_size(vec2(ui.available_width(), prev_h), Sense::click());
    if app.lib.play {
        let dt = ui.input(|i| i.stable_dt) as f64;
        app.lib.accum += dt;
        let spf = 1.0 / app.ed.doc.fps.max(1.0) as f64;
        while app.lib.accum >= spf {
            app.lib.accum -= spf;
            app.lib.frame = app.lib.frame.wrapping_add(1);
        }
        ui.ctx().request_repaint();
    }
    library_preview(app, ui, prev_rect);
    let frames = app.lib.selected_symbol(app).map(|s| s.timeline.frame_count().max(1));
    if let Some(n) = frames {
        let br = Rect::from_min_size(prev_rect.left_bottom() + vec2(4.0, -24.0), vec2(prev_rect.width() - 8.0, 20.0));
        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(br), |ui| {
            ui.horizontal(|ui| {
                if ui.small_button(if app.lib.play { "⏸" } else { "▶" }).on_hover_text("Play the symbol's timeline").clicked() {
                    app.lib.play = !app.lib.play;
                }
                if ui.small_button("⏮").clicked() {
                    app.lib.frame = 0;
                    app.lib.play = false;
                }
                ui.label(RichText::new(format!("{}/{}", app.lib.frame % n + 1, n)).small().color(Color32::from_gray(170)));
            });
        });
    }
    if prev_resp.double_clicked() {
        if let Some(id) = app.lib_selected {
            app.edit_symbol(id);
        }
    }

    // Column header (click to sort)
    let stage_center = if app.ed.editing.is_some() { pt(0.0, 0.0) } else { pt(app.ed.doc.width / 2.0, app.ed.doc.height / 2.0) };
    let w = ui.available_width();
    let cols = [("Name", 6.0), ("Type", w - 168.0), ("Uses", w - 84.0), ("Frames", w - 44.0)];
    let (hr, _) = ui.allocate_exact_size(vec2(w, 20.0), Sense::hover());
    ui.painter().rect_filled(hr, 0.0, Color32::from_gray(0x2A));
    for (i, (label, x)) in cols.iter().enumerate() {
        let r = Rect::from_min_size(pos2(hr.left() + x - 4.0, hr.top()), vec2(if i == 0 { (w - 172.0).max(40.0) } else { 42.0 }, 20.0));
        let resp = ui.interact(r, ui.id().with(("libcol", i)), Sense::click());
        let arrow = if app.lib.sort.0 as usize == i { if app.lib.sort.1 { " ⬆" } else { " ⬇" } } else { "" };
        let col = if resp.hovered() { Color32::WHITE } else { Color32::from_gray(170) };
        ui.painter().text(pos2(hr.left() + x, hr.center().y), Align2::LEFT_CENTER, format!("{label}{arrow}"), FontId::proportional(11.5), col);
        if resp.clicked() {
            app.lib.sort = if app.lib.sort.0 as usize == i { (i as u8, !app.lib.sort.1) } else { (i as u8, true) };
        }
    }
    let q = app.lib.search.to_lowercase();
    let mut syms: Vec<(u32, String, SymbolKind, u32, usize)> = app
        .ed
        .doc
        .library
        .iter()
        .filter(|s| s.name != ac_core::editor::GROUP_EDIT_NAME && (q.is_empty() || s.name.to_lowercase().contains(&q)))
        .map(|s| (s.id, s.name.clone(), s.kind, s.timeline.frame_count(), app.ed.symbol_usage(s.id)))
        .collect();
    let kind_name = |k: SymbolKind| match k { SymbolKind::Graphic => "Graphic", SymbolKind::MovieClip => "Movie Clip", SymbolKind::Button => "Button" };
    match app.lib.sort.0 {
        1 => syms.sort_by(|a, b| kind_name(a.2).cmp(kind_name(b.2))),
        2 => syms.sort_by_key(|s| s.4),
        3 => syms.sort_by_key(|s| s.3),
        _ => syms.sort_by_key(|s| s.1.to_lowercase()),
    }
    if !app.lib.sort.1 {
        syms.reverse();
    }
    let list_h = (ui.available_height() - 34.0).max(60.0);
    use ac_core::editor::LibItem;
    // every item with its full name; the tree below lists folders, then items, per folder
    let mut items: Vec<(LibItem, String)> = vec![];
    for s in &syms {
        items.push((LibItem::Symbol(s.0), s.1.clone()));
    }
    for b in app.ed.doc.bitmaps.iter().filter(|b| q.is_empty() || b.name.to_lowercase().contains(&q)) {
        items.push((LibItem::Bitmap(b.id), b.name.clone()));
    }
    for so in app.ed.doc.sounds.iter().filter(|s| q.is_empty() || s.name.to_lowercase().contains(&q)) {
        items.push((LibItem::Sound(so.id), so.name.clone()));
    }
    let folders = app.ed.library_folders();
    let searching = !q.is_empty();
    // rows: (depth, Some(folder path) | None, item)
    let mut rows: Vec<(usize, Option<String>, Option<LibItem>)> = vec![];
    fn walk(path: &str, depth: usize, folders: &[String], items: &[(LibItem, String)], open: &std::collections::BTreeSet<String>, rows: &mut Vec<(usize, Option<String>, Option<LibItem>)>) {
        for f in folders.iter().filter(|f| ac_core::split_item_path(f).0 == path) {
            rows.push((depth, Some(f.clone()), None));
            if open.contains(f) {
                walk(f, depth + 1, folders, items, open, rows);
            }
        }
        for (it, n) in items.iter().filter(|(_, n)| ac_core::split_item_path(n).0 == path) {
            let _ = n;
            rows.push((depth, None, Some(*it)));
        }
    }
    if searching {
        rows.extend(items.iter().map(|(it, _)| (0, None, Some(*it))));
    } else {
        walk("", 0, &folders, &items, &app.lib.open_folders, &mut rows);
    }
    let dragging = app.lib_drag.map(LibItem::Symbol).or(app.lib.drag_bitmap.map(LibItem::Bitmap)).or(app.lib.drag_sound.map(LibItem::Sound));
    let released = ui.input(|i| i.pointer.any_released());
    let mut drop_into: Option<String> = None;
    egui::ScrollArea::vertical().id_salt("lib-list").max_height(list_h).auto_shrink([false, false]).show(ui, |ui| {
        if rows.is_empty() {
            hint(ui, if app.ed.doc.library.iter().all(|s| s.name == ac_core::editor::GROUP_EDIT_NAME) && app.ed.doc.bitmaps.is_empty() && app.ed.doc.sounds.is_empty() { "Empty. Select objects and press F8 to make a symbol." } else { "No items match the search." });
        }
        if dragging.is_some() && !folders.is_empty() {
            let (r, resp) = ui.allocate_exact_size(vec2(w, 18.0), Sense::hover());
            let hot = resp.contains_pointer();
            ui.painter().rect_stroke(r.shrink(1.0), 3.0, EStroke::new(1.0_f32, if hot { ACCENT } else { Color32::from_gray(90) }));
            ui.painter().text(r.center(), Align2::CENTER_CENTER, "Drop here: library root", FontId::proportional(11.0), Color32::from_gray(190));
            if hot && released {
                drop_into = Some(String::new());
            }
        }
        for (depth, folder, item) in rows {
            let indent = 14.0 * depth as f32;
            if let Some(path) = folder {
                let leaf = ac_core::split_item_path(&path).1.to_string();
                let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click());
                let hot_drop = dragging.is_some() && resp.contains_pointer();
                if hot_drop {
                    ui.painter().rect_stroke(r.shrink(1.0), 2.0, EStroke::new(1.5_f32, ACCENT));
                    if released {
                        drop_into = Some(path.clone());
                    }
                } else if resp.hovered() {
                    ui.painter().rect_filled(r, 2.0, Color32::from_gray(0x3A));
                }
                let open = app.lib.open_folders.contains(&path);
                let renaming = app.lib.rename_folder.as_ref().is_some_and(|(p, _)| *p == path);
                ui.painter().text(pos2(r.left() + 6.0 + indent, r.center().y), Align2::LEFT_CENTER, if open { "▼" } else { "▶" }, FontId::proportional(9.0), Color32::from_gray(170));
                if renaming {
                    let tr = Rect::from_min_max(pos2(r.left() + 20.0 + indent, r.top() + 1.0), pos2(r.right() - 4.0, r.bottom() - 1.0));
                    let mut done = false;
                    ui.allocate_new_ui(egui::UiBuilder::new().max_rect(tr), |ui| {
                        if let Some((_, text)) = app.lib.rename_folder.as_mut() {
                            let te = ui.add(egui::TextEdit::singleline(text).desired_width(tr.width()));
                            te.request_focus();
                            if te.lost_focus() {
                                done = true;
                            }
                        }
                    });
                    if done {
                        if let Some((p, t)) = app.lib.rename_folder.take() {
                            if !ui.input(|i| i.key_pressed(egui::Key::Escape)) && app.ed.rename_folder(&p, &t) {
                                let new_path = ac_core::join_item_path(ac_core::split_item_path(&p).0, t.trim());
                                if app.lib.open_folders.remove(&p) {
                                    app.lib.open_folders.insert(new_path);
                                }
                            }
                        }
                    }
                } else {
                    ui.painter().text(pos2(r.left() + 20.0 + indent, r.center().y), Align2::LEFT_CENTER, format!("📁 {leaf}"), FontId::proportional(12.5), Color32::from_gray(225));
                    ui.painter().text(pos2(r.left() + cols[1].1, r.center().y), Align2::LEFT_CENTER, "Folder", FontId::proportional(11.5), Color32::from_gray(150));
                }
                if resp.clicked() {
                    if open {
                        app.lib.open_folders.remove(&path);
                    } else {
                        app.lib.open_folders.insert(path.clone());
                    }
                }
                if resp.double_clicked() {
                    app.lib.rename_folder = Some((path.clone(), leaf.clone()));
                }
                resp.on_hover_text(format!("{path}\nClick to open or close · double-click to rename · drag items onto it")).context_menu(|ui| {
                    if ui.button("New Folder Inside").clicked() {
                        let p = app.ed.new_folder(&path);
                        app.lib.open_folders.insert(path.clone());
                        app.lib.rename_folder = Some((p.clone(), ac_core::split_item_path(&p).1.to_string()));
                        ui.close_menu();
                    }
                    if ui.button("Rename…").clicked() {
                        app.lib.rename_folder = Some((path.clone(), leaf.clone()));
                        ui.close_menu();
                    }
                    if ui.button("Delete Folder").on_hover_text("Its contents move up a level").clicked() {
                        app.ed.delete_folder(&path);
                        ui.close_menu();
                    }
                });
                continue;
            }
            let Some(item) = item else { continue };
            match item {
                LibItem::Sound(id) => {
                    let Some(so) = app.ed.doc.sound(id) else { continue };
                    let name = so.name.clone();
                    let leaf = ac_core::split_item_path(&name).1.to_string();
                    let uses = app.ed.sound_usage(id);
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click_and_drag());
                    if app.lib.sound == Some(id) {
                        ui.painter().rect_filled(r, 2.0, ACCENT.gamma_multiply(0.55));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 2.0, Color32::from_gray(0x3A));
                    }
                    let clip = ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.left() + cols[1].1 - 6.0, r.bottom())));
                    clip.text(pos2(r.left() + 6.0 + indent, r.center().y), Align2::LEFT_CENTER, format!("🔊 {leaf}"), FontId::proportional(12.5), Color32::from_gray(230));
                    ui.painter().text(pos2(r.left() + cols[1].1, r.center().y), Align2::LEFT_CENTER, "Sound", FontId::proportional(11.5), Color32::from_gray(180));
                    ui.painter().text(pos2(r.left() + cols[2].1 + 24.0, r.center().y), Align2::RIGHT_CENTER, uses.to_string(), FontId::proportional(11.5), Color32::from_gray(180));
                    ui.painter().text(pos2(r.left() + cols[3].1 + 36.0, r.center().y), Align2::RIGHT_CENTER, "–", FontId::proportional(11.5), Color32::from_gray(180));
                    if resp.clicked() || resp.drag_started() {
                        app.lib.sound = Some(id);
                        app.lib.bitmap = None;
                        app.lib_selected = None;
                    }
                    if resp.drag_started() {
                        app.lib.drag_sound = Some(id);
                    }
                    if resp.double_clicked() {
                        app.ed.set_frame_sound(Some(ac_core::sound::FrameSound::new(id)));
                    }
                    let resp = resp.on_hover_text(format!("{name}\nSound · used on {uses} keyframe(s)\nDrag onto the stage to start it on the current keyframe, or onto a folder"));
                    resp.context_menu(|ui| {
                        if ui.button("Use on Current Keyframe").clicked() { app.ed.set_frame_sound(Some(ac_core::sound::FrameSound::new(id))); ui.close_menu(); }
                        item_menu(app, ui, item, &folders);
                        ui.separator();
                        if ui.button("Delete").clicked() {
                            app.ed.delete_sound(id);
                            app.lib.sound = None;
                            ui.close_menu();
                        }
                    });
                }
                LibItem::Bitmap(id) => {
                    let Some(b) = app.ed.doc.bitmap(id) else { continue };
                    let (name, bw, bh) = (b.name.clone(), b.width, b.height);
                    let leaf = ac_core::split_item_path(&name).1.to_string();
                    let uses = app.ed.bitmap_usage(id);
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click_and_drag());
                    if app.lib.bitmap == Some(id) {
                        ui.painter().rect_filled(r, 2.0, ACCENT.gamma_multiply(0.55));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 2.0, Color32::from_gray(0x3A));
                    }
                    let clip = ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.left() + cols[1].1 - 6.0, r.bottom())));
                    clip.text(pos2(r.left() + 6.0 + indent, r.center().y), Align2::LEFT_CENTER, format!("🖼 {leaf}"), FontId::proportional(12.5), Color32::from_gray(230));
                    ui.painter().text(pos2(r.left() + cols[1].1, r.center().y), Align2::LEFT_CENTER, "Bitmap", FontId::proportional(11.5), Color32::from_gray(180));
                    ui.painter().text(pos2(r.left() + cols[2].1 + 24.0, r.center().y), Align2::RIGHT_CENTER, uses.to_string(), FontId::proportional(11.5), Color32::from_gray(180));
                    ui.painter().text(pos2(r.left() + cols[3].1 + 36.0, r.center().y), Align2::RIGHT_CENTER, "–", FontId::proportional(11.5), Color32::from_gray(180));
                    if resp.clicked() || resp.drag_started() {
                        app.lib.bitmap = Some(id);
                        app.lib.sound = None;
                        app.lib_selected = None;
                    }
                    if resp.drag_started() {
                        app.lib.drag_bitmap = Some(id);
                    }
                    let center_place = pt((stage_center.x - bw as f32 / 2.0).max(0.0), (stage_center.y - bh as f32 / 2.0).max(0.0));
                    if resp.double_clicked() {
                        app.ed.place_bitmap(id, center_place);
                    }
                    let resp = resp.on_hover_text(format!("{name}\nBitmap {bw}×{bh} · used {uses}×\nDrag onto the stage to place, or onto a folder"));
                    resp.context_menu(|ui| {
                        if ui.button("Place on Stage").clicked() { app.ed.place_bitmap(id, center_place); ui.close_menu(); }
                        if ui.button("Rename…").clicked() { app.dialog = Dialog::RenameBitmap { id, name: leaf.clone() }; ui.close_menu(); }
                        item_menu(app, ui, item, &folders);
                        ui.separator();
                        if ui.button("Delete").clicked() { app.dialog = Dialog::DeleteBitmap { id, uses }; ui.close_menu(); }
                    });
                }
                LibItem::Symbol(id) => {
                    let Some(s) = app.ed.doc.symbol(id) else { continue };
                    let (name, kind, frames) = (s.name.clone(), s.kind, s.timeline.frame_count());
                    let leaf = ac_core::split_item_path(&name).1.to_string();
                    let uses = app.ed.symbol_usage(id);
                    let linked = app.ed.doc.linkage.get(&id).cloned();
                    let (r, resp) = ui.allocate_exact_size(vec2(w, 22.0), Sense::click_and_drag());
                    let selected = app.lib_selected == Some(id);
                    if selected {
                        ui.painter().rect_filled(r, 2.0, ACCENT.gamma_multiply(0.55));
                    } else if resp.hovered() {
                        ui.painter().rect_filled(r, 2.0, Color32::from_gray(0x3A));
                    }
                    let icon = match kind { SymbolKind::Graphic => "🔷", SymbolKind::MovieClip => "🎞", SymbolKind::Button => "🔘" };
                    let clip = ui.painter().with_clip_rect(Rect::from_min_max(r.min, pos2(r.left() + cols[1].1 - 6.0, r.bottom())));
                    clip.text(pos2(r.left() + 6.0 + indent, r.center().y), Align2::LEFT_CENTER, format!("{icon} {leaf}"), FontId::proportional(12.5), Color32::from_gray(230));
                    ui.painter().text(pos2(r.left() + cols[1].1, r.center().y), Align2::LEFT_CENTER, kind_name(kind), FontId::proportional(11.5), Color32::from_gray(180));
                    ui.painter().text(pos2(r.left() + cols[2].1 + 24.0, r.center().y), Align2::RIGHT_CENTER, uses.to_string(), FontId::proportional(11.5), Color32::from_gray(180));
                    ui.painter().text(pos2(r.left() + cols[3].1 + 36.0, r.center().y), Align2::RIGHT_CENTER, frames.to_string(), FontId::proportional(11.5), Color32::from_gray(180));
                    if resp.clicked() || resp.drag_started() {
                        if app.lib_selected != Some(id) {
                            app.lib.frame = 0;
                        }
                        app.lib_selected = Some(id);
                        app.lib.bitmap = None;
                    }
                    if resp.drag_started() {
                        app.lib_drag = Some(id);
                    }
                    if resp.double_clicked() {
                        app.edit_symbol(id);
                    }
                    let link = linked.map(|c| format!("\nLinkage: {c}")).unwrap_or_default();
                    let resp = resp.on_hover_text(format!("{name}\n{} · {frames} frame(s) · used {uses}×{link}\nDrag onto the stage to place, or onto a folder · double-click to edit", kind_name(kind)));
                    resp.context_menu(|ui| {
                        if ui.button("Edit").clicked() { app.edit_symbol(id); ui.close_menu(); }
                        if ui.button("Place on Stage").clicked() { app.ed.place_symbol(id, stage_center); ui.close_menu(); }
                        if ui.button("Rename…").clicked() { app.dialog = Dialog::RenameSymbol { id, name: leaf.clone() }; ui.close_menu(); }
                        if ui.button("Duplicate").clicked() { app.lib_selected = app.ed.duplicate_symbol(id); ui.close_menu(); }
                        ui.menu_button("Type", |ui| {
                            for (k, l) in [(SymbolKind::Graphic, "Graphic"), (SymbolKind::MovieClip, "Movie Clip"), (SymbolKind::Button, "Button")] {
                                if ui.radio(kind == k, l).clicked() { app.ed.set_symbol_kind(id, k); ui.close_menu(); }
                            }
                        });
                        item_menu(app, ui, item, &folders);
                        ui.separator();
                        if ui.button("Delete").clicked() { app.dialog = Dialog::DeleteSymbol { id, uses: app.ed.symbol_usage(id) }; ui.close_menu(); }
                    });
                }
            }
        }
    });
    if let (Some(it), Some(folder)) = (dragging, drop_into) {
        if app.ed.move_to_folder(it, &folder) {
            if !folder.is_empty() {
                app.lib.open_folders.insert(folder.clone());
            }
            app.status = if folder.is_empty() { "Moved to the library root".into() } else { format!("Moved into {folder}") };
        }
        app.lib_drag = None;
        app.lib.drag_bitmap = None;
        app.lib.drag_sound = None;
    }
    ui.horizontal(|ui| {
        let sel = app.lib_selected.filter(|id| app.ed.doc.symbol(*id).is_some());
        if ui.button("➕").on_hover_text("New Symbol (Ctrl+F8)").clicked() {
            app.open_symbol_dialog(false);
        }
        if ui.button("📁").on_hover_text("New Folder (in the selected item's folder)").clicked() {
            let parent = selected_item(app).and_then(|it| app.ed.item_name(it)).map(|n| ac_core::split_item_path(&n).0.to_string()).unwrap_or_default();
            let p = app.ed.new_folder(&parent);
            if !parent.is_empty() {
                app.lib.open_folders.insert(parent);
            }
            app.lib.rename_folder = Some((p.clone(), ac_core::split_item_path(&p).1.to_string()));
        }
        if ui.add_enabled(selected_item(app).is_some(), egui::Button::new("ⓘ")).on_hover_text("Properties of the selected item").clicked() {
            if let Some(it) = selected_item(app) {
                open_item_props(app, it);
            }
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Place")).on_hover_text("Place an instance at the centre of the stage").clicked() {
            app.ed.place_symbol(sel.unwrap(), stage_center);
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Edit")).clicked() {
            app.edit_symbol(sel.unwrap());
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Rename")).clicked() {
            let id = sel.unwrap();
            app.dialog = Dialog::RenameSymbol { id, name: ac_core::split_item_path(&app.ed.doc.symbol(id).unwrap().name).1.to_string() };
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("Dup")).on_hover_text("Duplicate item").clicked() {
            app.lib_selected = app.ed.duplicate_symbol(sel.unwrap());
        }
        if ui.add_enabled(sel.is_some(), egui::Button::new("🗑")).on_hover_text("Delete library item").clicked() {
            let id = sel.unwrap();
            app.dialog = Dialog::DeleteSymbol { id, uses: app.ed.symbol_usage(id) };
        }
    });
}
/// The library item selected in the panel (symbol, bitmap or sound).
fn selected_item(app: &App) -> Option<ac_core::editor::LibItem> {
    use ac_core::editor::LibItem;
    app.lib_selected.filter(|id| app.ed.doc.symbol(*id).is_some()).map(LibItem::Symbol).or(app.lib.bitmap.map(LibItem::Bitmap)).or(app.lib.sound.map(LibItem::Sound))
}

/// Context-menu entries every library item has: Move to Folder, Properties.
fn item_menu(app: &mut App, ui: &mut egui::Ui, it: ac_core::editor::LibItem, folders: &[String]) {
    ui.menu_button("Move to Folder", |ui| {
        if ui.button("(Library root)").clicked() {
            app.ed.move_to_folder(it, "");
            ui.close_menu();
        }
        for f in folders {
            if ui.button(f).clicked() {
                app.ed.move_to_folder(it, f);
                app.lib.open_folders.insert(f.clone());
                ui.close_menu();
            }
        }
        ui.separator();
        if ui.button("New Folder…").clicked() {
            let p = app.ed.new_folder("");
            app.ed.move_to_folder(it, &p);
            app.lib.open_folders.insert(p.clone());
            app.lib.rename_folder = Some((p.clone(), ac_core::split_item_path(&p).1.to_string()));
            ui.close_menu();
        }
    });
    if ui.button("Properties…").clicked() {
        open_item_props(app, it);
        ui.close_menu();
    }
}

pub(crate) fn open_item_props(app: &mut App, it: ac_core::editor::LibItem) {
    let Some(name) = app.ed.item_name(it) else { return };
    let (folder, leaf) = ac_core::split_item_path(&name);
    let (kind, class) = match it {
        ac_core::editor::LibItem::Symbol(id) => (app.ed.doc.symbol(id).map(|s| s.kind), app.ed.doc.linkage.get(&id).cloned()),
        _ => (None, None),
    };
    app.lib.props = Some(ItemProps { item: it, name: leaf.to_string(), kind, export: class.is_some(), class: class.unwrap_or_else(|| leaf.replace(|c: char| !c.is_alphanumeric() && c != '_', "_")), folder: folder.to_string() });
}

/// Library item Properties: name, type and linkage (symbols), folder.
pub fn item_props_window(app: &mut App, ctx: &egui::Context) {
    let Some(mut p) = app.lib.props.clone() else { return };
    let mut open = true;
    let (mut ok, mut cancel) = (false, false);
    egui::Window::new("Library Item Properties").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        egui::Grid::new("itemprops").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Name");
            ui.text_edit_singleline(&mut p.name);
            ui.end_row();
            ui.label("Folder");
            ui.label(if p.folder.is_empty() { "Library root".to_string() } else { p.folder.clone() });
            ui.end_row();
            if let Some(k) = p.kind.as_mut() {
                ui.label("Type");
                ui.horizontal(|ui| {
                    ui.selectable_value(k, SymbolKind::Graphic, "Graphic");
                    ui.selectable_value(k, SymbolKind::MovieClip, "Movie Clip");
                    ui.selectable_value(k, SymbolKind::Button, "Button");
                });
                ui.end_row();
                ui.label("Linkage");
                ui.checkbox(&mut p.export, "Export for runtime").on_hover_text("Gives the symbol a class name for code (kept in .fla files)");
                ui.end_row();
                ui.label("Class");
                ui.add_enabled(p.export, egui::TextEdit::singleline(&mut p.class));
                ui.end_row();
            }
            if let ac_core::editor::LibItem::Bitmap(id) = p.item {
                if let Some(b) = app.ed.doc.bitmap(id) {
                    ui.label("Size");
                    ui.label(format!("{} × {} px", b.width, b.height));
                    ui.end_row();
                }
            }
        });
        ui.horizontal(|ui| {
            ok = ui.button("OK").clicked();
            cancel = ui.button("Cancel").clicked();
        });
    });
    app.lib.props = Some(p.clone());
    if ok {
        if app.ed.set_item_props(p.item, &p.name, p.kind, p.export.then(|| p.class.clone())) {
            app.lib.props = None;
        } else {
            app.status = "A name can't be empty or contain '/'".into();
        }
    } else if cancel || !open {
        app.lib.props = None;
    }
}

/// Ghost label under the cursor while a library item is dragged, and the drop.
pub fn library_drag_ghost(app: &mut App, ctx: &egui::Context) {
    if let Some(id) = app.lib.drag_sound {
        let (pos, released) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.any_released()));
        if let Some(p) = pos {
            let name = app.ed.doc.sound(id).map(|b| b.name.clone()).unwrap_or_default();
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("libdrag")));
            let r = painter.text(p + vec2(14.0, 8.0), Align2::LEFT_TOP, format!("🔊 {name}"), FontId::proportional(13.0), Color32::WHITE);
            painter.rect_stroke(r.expand(4.0), 3.0, EStroke::new(1.0_f32, ACCENT));
            if released {
                if app.stage_rect.contains(p) {
                    if app.ed.current_keyframe().is_some() {
                        app.ed.set_frame_sound(Some(ac_core::sound::FrameSound::new(id)));
                        app.status = format!("{name} starts on frame {}", app.ed.current_keyframe().map(|k| k.0 + 1).unwrap_or(1));
                    } else {
                        app.status = "No keyframe here for the sound: add one with F7.".into();
                    }
                }
                app.lib.drag_sound = None;
            }
        } else if released {
            app.lib.drag_sound = None;
        }
        ctx.request_repaint();
        return;
    }
    if let Some(id) = app.lib.drag_bitmap {
        let (pos, released) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.any_released()));
        if let Some(p) = pos {
            let name = app.ed.doc.bitmap(id).map(|b| b.name.clone()).unwrap_or_default();
            let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("libdrag")));
            let r = painter.text(p + vec2(14.0, 8.0), Align2::LEFT_TOP, format!("🖼 {name}"), FontId::proportional(13.0), Color32::WHITE);
            painter.rect_stroke(r.expand(4.0), 3.0, EStroke::new(1.0_f32, ACCENT));
            if released {
                if app.stage_rect.contains(p) {
                    let at = app.to_doc(app.stage_rect, p);
                    if app.ed.place_bitmap(id, at).is_none() {
                        app.status = "The current layer is locked or hidden.".into();
                    }
                }
                app.lib.drag_bitmap = None;
            }
        } else if released {
            app.lib.drag_bitmap = None;
        }
        ctx.request_repaint();
        return;
    }
    let Some(id) = app.lib_drag else { return };
    let (pos, released) = ctx.input(|i| (i.pointer.latest_pos(), i.pointer.any_released()));
    if let Some(p) = pos {
        let name = app.ed.doc.symbol(id).map(|s| s.name.clone()).unwrap_or_default();
        let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Tooltip, egui::Id::new("libdrag")));
        let r = painter.text(p + vec2(14.0, 8.0), Align2::LEFT_TOP, format!("🔷 {name}"), FontId::proportional(13.0), Color32::WHITE);
        painter.rect_stroke(r.expand(4.0), 3.0, EStroke::new(1.0_f32, ACCENT));
        if released {
            if app.stage_rect.contains(p) {
                let at = app.to_doc(app.stage_rect, p);
                if app.ed.layer_locked() {
                    app.status = "The current layer is locked or hidden.".into();
                } else {
                    app.ed.place_symbol(id, at);
                    app.status = format!("Placed “{name}”");
                }
            }
            app.lib_drag = None;
        }
    } else if released {
        app.lib_drag = None;
    }
    ctx.request_repaint();
}

/// Copy the selected shape's variable-width profile / paste it onto the selected shapes.
fn width_profile_buttons(app: &mut App, ui: &mut egui::Ui) {
    // a drawing object's stroke, or the lines of a floating merge selection
    let merge_lines = app.ed.merge_float().and_then(|(_, f)| f.strokes.first().map(|s| s.width.clone()));
    let current = app
        .ed
        .selection
        .iter()
        .find_map(|s| match app.ed.element_at(*s).map(|e| e.kind) {
            Some(ElementKind::Shape(sh)) if sh.stroke.is_some() => Some(sh.width_profile.clone()),
            _ => None,
        })
        .or(merge_lines.clone());
    ui.horizontal(|ui| {
        if ui.add_enabled(matches!(current, Some(Some(_))), egui::Button::new("Copy width profile").small()).clicked() {
            app.width_clip = current.clone().flatten();
            app.status = "Width profile copied".into();
        }
        if ui.add_enabled(app.width_clip.is_some() && current.is_some(), egui::Button::new("Paste width profile").small()).clicked() {
            let prof = app.width_clip.clone();
            if !(merge_lines.is_some() && app.ed.merge_paste_width(prof.clone())) {
                app.ed.modify_selected("Paste Width Profile", move |e| if let ElementKind::Shape(s) = &mut e.kind { if s.stroke.is_some() { s.width_profile = prof.clone() } });
            }
        }
    });
}

fn tool_properties(app: &mut App, ui: &mut egui::Ui) {
    let tool = app.tool;
    let info = tool.info();
    ui.label(RichText::new(format!("{}  {}", info.name, tool.shortcut_label())).strong());
    ui.label(RichText::new(info.tip).small().color(Color32::from_gray(160)));
    ui.add_space(4.0);
    match tool {
        Tool::Rect => {
            section(ui, "Rectangle options");
            ui.add(egui::Slider::new(&mut app.topts.rect_radius, 0.0..=100.0).text("Corner radius"));
        }
        Tool::Oval => {
            section(ui, "Oval options");
            let o = &mut app.topts;
            ui.add(egui::Slider::new(&mut o.oval_start, 0.0..=360.0).text("Start angle"));
            ui.add(egui::Slider::new(&mut o.oval_end, 0.0..=360.0).text("End angle"));
            ui.add(egui::Slider::new(&mut o.oval_inner, 0.0..=99.0).text("Inner radius %"));
            if ui.small_button("Reset").clicked() {
                o.oval_start = 0.0;
                o.oval_end = 0.0;
                o.oval_inner = 0.0;
            }
            hint(ui, "Start = end gives a full oval. Angles run clockwise from 3 o'clock; an inner radius makes rings and arcs.");
        }
        Tool::PolyStar => {
            section(ui, "PolyStar options");
            let o = &mut app.topts;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut o.poly_star, false, "Polygon");
                ui.selectable_value(&mut o.poly_star, true, "Star");
            });
            ui.add(egui::Slider::new(&mut o.poly_sides, 3..=32).text("Sides"));
            ui.add_enabled(o.poly_star, egui::Slider::new(&mut o.poly_inner, 0.05..=1.0).text("Star point size"));
        }
        Tool::Pencil => {
            section(ui, "Pencil mode");
            let o = &mut app.topts;
            ui.horizontal(|ui| {
                ui.selectable_value(&mut o.pencil_mode, PencilMode::Straighten, "Straighten");
                ui.selectable_value(&mut o.pencil_mode, PencilMode::Smooth, "Smooth");
                ui.selectable_value(&mut o.pencil_mode, PencilMode::Ink, "Ink");
            });
        }
        Tool::Brush => {
            section(ui, "Paint Brush");
            ui.add(egui::Slider::new(&mut app.topts.brush_size, 1.0..=200.0).text("Size"));
            ui.add(egui::Slider::new(&mut app.topts.brush_smooth, 0.0..=100.0).text("Smoothing"));
            ui.horizontal(|ui| {
                ui.label("Brush shape");
                let b = app.topts.brush_shape;
                egui::ComboBox::from_id_salt("brush-shape").selected_text(b.name()).show_ui(ui, |ui| {
                    for x in ac_core::merge::BrushShape::ALL {
                        ui.selectable_value(&mut app.topts.brush_shape, x, x.name());
                    }
                });
            });
            ui.horizontal(|ui| {
                ui.label("Brush mode");
                let m = app.topts.brush_mode;
                egui::ComboBox::from_id_salt("brush-mode").selected_text(m.name(false)).show_ui(ui, |ui| {
                    for x in ac_core::merge::PaintMode::BRUSH {
                        ui.selectable_value(&mut app.topts.brush_mode, x, x.name(false));
                    }
                });
            });
            hint(ui, "Brush modes work on merge shapes (Object Drawing off): Fills leaves lines alone, Behind paints only empty areas, Selection only inside the selected fill, Inside only inside the fill where you start.");
        }
        Tool::FluidBrush => {
            section(ui, "Fluid Brush");
            ui.add(egui::Slider::new(&mut app.topts.fluid_size, 1.0..=200.0).text("Size"));
            ui.add(egui::Slider::new(&mut app.topts.fluid_sensitivity, 0.0..=100.0).text("Velocity"));
            hint(ui, "Velocity: how much speed thins the stroke. Strokes taper at both ends.");
        }
        Tool::Eraser => {
            section(ui, "Eraser");
            ui.add(egui::Slider::new(&mut app.topts.eraser_size, 2.0..=100.0).text("Size"));
            ui.horizontal(|ui| {
                ui.label("Mode");
                let m = app.topts.eraser_mode;
                egui::ComboBox::from_id_salt("eraser-mode").selected_text(m.name(true)).show_ui(ui, |ui| {
                    for x in ac_core::merge::PaintMode::ERASER {
                        ui.selectable_value(&mut app.topts.eraser_mode, x, x.name(true));
                    }
                });
            });
            ui.checkbox(&mut app.topts.faucet, "Faucet").on_hover_text("Click a fill or line segment to delete all of it");
            hint(ui, "On merge shapes the eraser cuts into fills and lines; on drawing objects and symbols it deletes the whole object (Break Apart first to erase into it).");
        }
        Tool::MagicWand => {
            section(ui, "Magic Wand");
            ui.add(egui::Slider::new(&mut app.topts.wand_threshold, 0.0..=200.0).text("Threshold"));
            hint(ui, "Break Apart a bitmap (Ctrl+B) first. The wand selects the connected area whose colour is within the threshold of the pixel you click; press Delete to cut it out.");
        }
        Tool::Text => {
            section(ui, "Text type");
            ui.horizontal(|ui| {
                use ac_core::text::TextKind;
                ui.selectable_value(&mut app.topts.text_kind, TextKind::Static, "Static");
                ui.selectable_value(&mut app.topts.text_kind, TextKind::Dynamic, "Dynamic");
                ui.selectable_value(&mut app.topts.text_kind, TextKind::Input, "Input");
            });
            hint(ui, "Click to type text that grows with what you type; drag to make a fixed-width box where words wrap.");
            section(ui, "Character");
            let o = &mut app.topts;
            ui.add(egui::Slider::new(&mut o.text_size, 4.0..=200.0).text("Size"));
            ui.horizontal(|ui| {
                ui.selectable_value(&mut o.text_font, FontFace::Sans, "Sans");
                ui.selectable_value(&mut o.text_font, FontFace::Mono, "Mono");
                ui.separator();
                ui.selectable_value(&mut o.text_align, Align::Left, "⬅");
                ui.selectable_value(&mut o.text_align, Align::Center, "↔");
                ui.selectable_value(&mut o.text_align, Align::Right, "➡");
            });
            ui.horizontal(|ui| {
                ui.toggle_value(&mut o.text_bold, RichText::new("B").strong());
                ui.toggle_value(&mut o.text_italic, RichText::new("I").italics());
            });
            hint(ui, "Text uses the fill colour. Bold and italic use the family's own faces when it has them (Properties › Object › Style lists them).");
        }
        Tool::Select | Tool::Subselect => {
            section(ui, "Snapping");
            ui.checkbox(&mut app.topts.snap, "Snap to objects and stage (hold Alt to bypass)");
            if tool == Tool::Select {
                ui.checkbox(&mut app.topts.show_bones, "Show bones");
                hint(ui, "Dragging an object joined by bones poses the chain (inverse kinematics).");
            }
        }
        Tool::FreeTransform => {
            section(ui, "Mode");
            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.topts.ft_mode, FtMode::Normal, "Scale / Rotate / Skew");
                ui.selectable_value(&mut app.topts.ft_mode, FtMode::Distort, "Distort");
                ui.selectable_value(&mut app.topts.ft_mode, FtMode::Envelope, "Envelope");
            });
            hint(ui, match app.topts.ft_mode {
                FtMode::Normal => "Corners scale (Shift = keep proportions, Alt = from the transformation point). Side handles stretch one way. Drag an edge to skew. Just outside a corner: rotate. Drag the white dot to move the transformation point.",
                FtMode::Distort => "Drag a corner on its own to distort shapes (perspective-like). Symbols and text: Break Apart first.",
                FtMode::Envelope => "Drag any of the 9 handles to bend shapes smoothly. Symbols and text: Break Apart first.",
            });
            ui.checkbox(&mut app.topts.snap, "Snap (hold Alt to bypass)");
            if ui.button("Transform panel… (Ctrl+T)").clicked() {
                app.transform_open = true;
            }
        }
        Tool::GradientTransform => {
            section(ui, "Gradient Transform");
            hint(ui, "Click a shape with a gradient fill. Centre dot = move; square = rotate and stretch; round handle (radial) = height. Set a gradient with Properties › Object › Fill.");
        }
        Tool::Rotate3D | Tool::Translate3D => {
            section(ui, "3D");
            hint(ui, if tool == Tool::Rotate3D { "Red line = rotate around X, green = around Y, blue ring = around Z, orange ring = free. Drag the centre dot to move the 3D centre." } else { "Red arrow = move in X, green = move in Y, centre dot (drag up/down) = depth." });
            let mut angle = app.ed.doc.perspective_angle;
            ui.horizontal(|ui| {
                ui.label("Perspective angle");
                if edited(&ui.add(egui::DragValue::new(&mut angle).range(1.0..=179.0).speed(0.5).suffix("°"))) {
                    let vp = app.ed.doc.vanishing_point;
                    app.ed.set_perspective(angle, vp);
                }
            });
        }
        Tool::Bone => {
            section(ui, "Bones");
            ui.checkbox(&mut app.topts.show_bones, "Show bones");
            let l = app.ed.layer;
            let n = app.ed.bones(l).len();
            hint(ui, &format!("{n} bone(s) on the current layer. Drag from an object to another to link them; start on a joint to extend a chain."));
            if ui.add_enabled(n > 0, egui::Button::new("Remove all bones on this layer")).clicked() {
                app.ed.clear_bones(l);
            }
        }
        Tool::Width => {
            section(ui, "Width");
            hint(ui, "Press on a shape's stroke and drag away from it (thicker) or towards it (thinner). Each drag adds a width point; Alt+click a point to delete it. Properties › Object › Reset width removes them all.");
            width_profile_buttons(app, ui);
        }
        Tool::Magnet | Tool::Pinch | Tool::Pump | Tool::Bender | Tool::Iron => {
            section(ui, app.tool.info().name);
            if app.tool != Tool::Bender {
                let label = if matches!(app.tool, Tool::Magnet | Tool::Iron) { "Size (radius)" } else { "Size (reach)" };
                ui.add(egui::Slider::new(&mut app.topts.sculpt_size, 2.0..=400.0).logarithmic(true).text(label));
            }
            if app.tool == Tool::Pinch {
                ui.checkbox(&mut app.topts.sculpt_sharp, "Corner").on_hover_text("Pull the grabbed point into a corner instead of a smooth bump");
            }
            if app.tool == Tool::Iron {
                ui.add(egui::Slider::new(&mut app.topts.sculpt_strength, 0.05..=1.0).text("Strength"));
            }
            hint(ui, match app.tool {
                Tool::Magnet => "Press near lines or shapes and drag: everything within the radius follows the pointer, more so near the centre.",
                Tool::Pinch => "Press on a line or a shape's edge and drag: the line follows over Size units of its length on each side.",
                Tool::Pump => "Press on a line and drag up to make it thicker there, down to make it thinner (adds width points, like the Width tool).",
                Tool::Bender => "1. Drag a segment across the lines to bend. 2. Press on one side of it and drag around its start: that side of the lines turns, bending where they cross the segment.",
                _ => "Brush back and forth over wobbly lines to smooth them; Strength sets how much each pass smooths.",
            });
            hint(ui, "Works on drawing objects and merge shapes (fills follow their lines). From OpenToonz.");
        }
        Tool::Assistant => assistant_properties(app, ui),
        Tool::RasterBrush => {
            use ac_core::paint::PRESETS;
            section(ui, "Raster Brush (MyPaint)");
            let cur = PRESETS.get(app.topts.raster_brush).map(|p| format!("{} · {}", p.name, p.group)).unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label("Brush");
                egui::ComboBox::from_id_salt("raster-brush").width(170.0).selected_text(cur).height(420.0).show_ui(ui, |ui| {
                    let mut group = "";
                    for (i, p) in PRESETS.iter().enumerate() {
                        if p.group != group {
                            group = p.group;
                            ui.label(egui::RichText::new(group).small().color(Color32::from_gray(140)));
                        }
                        ui.selectable_value(&mut app.topts.raster_brush, i, p.name);
                    }
                });
            });
            let own = PRESETS.get(app.topts.raster_brush).map(|p| p.base_radius()).unwrap_or(2.0);
            let mut size = if app.topts.raster_size > 0.0 { app.topts.raster_size } else { own };
            if ui.add(egui::Slider::new(&mut size, 0.5..=200.0).logarithmic(true).max_decimals(1).text("Size (radius)")).changed() {
                app.topts.raster_size = size;
            }
            if app.topts.raster_size > 0.0 && ui.small_button("Brush's own size").clicked() {
                app.topts.raster_size = 0.0;
            }
            ui.add(egui::Slider::new(&mut app.topts.raster_opacity, 0.0..=1.0).text("Opacity"));
            ui.add(egui::Slider::new(&mut app.topts.raster_pressure, 0.05..=1.0).text("Pressure")).on_hover_text("The pen pressure brushes respond to (a mouse reports none)");
            ui.checkbox(&mut app.topts.raster_eraser, "Erase (paint transparency)");
            hint(ui, "Paints with the fill colour into the bitmap under the pointer, or into a new stage-sized bitmap on this keyframe. For frame-by-frame animation press F6 (or F7) and paint the next drawing; a bitmap shared with other frames is copied first. Brushes: MyPaint pack (CC0), engine: hokusai.");
        }
        Tool::AssetWarp => {
            section(ui, "Asset Warp");
            hint(ui, "Click inside a shape, bitmap, symbol instance or group to add pins; drag a pin to bend it around the others. Alt+click removes a pin. Properties › Object › Remove warp restores it. Warped bitmaps and symbols show in every export except .fla (exported unwarped).");
        }
        Tool::Camera => camera_properties(app, ui),
        Tool::RotateView => {
            section(ui, "View rotation");
            let mut deg = app.view_rot.to_degrees();
            ui.horizontal(|ui| {
                if edited(&ui.add(egui::DragValue::new(&mut deg).range(-180.0..=180.0).speed(1.0).suffix("°"))) {
                    let r = app.stage_rect;
                    app.set_view_rot(r, deg.to_radians());
                }
                if ui.button("Reset").clicked() {
                    let r = app.stage_rect;
                    app.set_view_rot(r, 0.0);
                }
            });
            hint(ui, "Only the view turns: the document is unchanged. Double-click the stage to reset.");
        }
        _ => {}
    }
    if tool == Tool::Bucket {
        section(ui, "Paint Bucket");
        ui.horizontal(|ui| {
            ui.label("Gap size");
            let names = ["Don't Close Gaps", "Close Small Gaps", "Close Medium Gaps", "Close Large Gaps"];
            egui::ComboBox::from_id_salt("gap").selected_text(names[app.topts.bucket_gap as usize]).show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut app.topts.bucket_gap, i as u8, *n);
                }
            });
        });
        ui.checkbox(&mut app.topts.lock_fill, "Lock Fill").on_hover_text("Gradients span the whole stage, so shapes filled one by one look like one fill");
        hint(ui, "On merge shapes, clicking inside lines fills the area they enclose.");
    }
    if tool.is_drawing() || matches!(tool, Tool::FluidBrush) {
        section(ui, "Drawing model");
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.topts.object_draw, false, "Merge Drawing");
            ui.selectable_value(&mut app.topts.object_draw, true, "Object Drawing");
            ui.label(RichText::new("(J)").small());
        });
    }
    if tool.is_drawing() || matches!(tool, Tool::InkBottle | Tool::Bucket | Tool::FluidBrush) {
        section(ui, "Fill and stroke");
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.fill_on, "Fill");
            ui.color_edit_button_srgba(&mut app.fill);
            ui.checkbox(&mut app.stroke_on, "Stroke");
            ui.color_edit_button_srgba(&mut app.stroke);
        });
        ui.add(egui::Slider::new(&mut app.stroke_w, 0.1..=50.0).text("Stroke size"));
        ui.horizontal(|ui| {
            ui.label("Fill type");
            let o = &mut app.topts;
            ui.selectable_value(&mut o.fill_gradient, None, "Solid");
            ui.selectable_value(&mut o.fill_gradient, Some(GradientKind::Linear), "Linear");
            ui.selectable_value(&mut o.fill_gradient, Some(GradientKind::Radial), "Radial");
        });
        if app.topts.fill_gradient.is_some() {
            ui.horizontal(|ui| {
                ui.label("Gradient");
                ui.color_edit_button_srgba(&mut app.fill);
                ui.label("→");
                ui.color_edit_button_srgba(&mut app.topts.fill2);
            });
        }
    }
}

fn camera_properties(app: &mut App, ui: &mut egui::Ui) {
    section(ui, "Camera");
    if app.ed.editing.is_some() {
        hint(ui, "The camera belongs to the main scene. Leave symbol editing (Esc) to use it.");
        return;
    }
    if app.ed.camera_layer().is_none() {
        hint(ui, "No camera yet. Click the stage with this tool, or:");
        if ui.button("Add camera layer").clicked() {
            app.ed.add_camera();
        }
        return;
    }
    // Mode: Animate's stage camera, or a free camera (VCam) you move around the scene
    let mut mode = app.ed.doc.camera_mode;
    ui.horizontal(|ui| {
        ui.label("Mode");
        ui.selectable_value(&mut mode, ac_core::CameraMode::Stage, "Stage").on_hover_text("The stage is the camera frame: drag to pan the scene under it (Animate's camera)");
        ui.selectable_value(&mut mode, ac_core::CameraMode::Free, "Free (VCam)").on_hover_text("The camera is a frame on the scene you move, zoom and rotate like an object; output renders through it");
    });
    if mode != app.ed.doc.camera_mode {
        app.ed.set_camera_mode(mode);
    }
    if mode == ac_core::CameraMode::Free {
        ui.horizontal(|ui| {
            ui.checkbox(&mut app.vcam_preview, "Preview through camera");
            ui.checkbox(&mut app.vcam_dim, "Dim outside");
        });
    }
    let cam = app.ed.camera_at(app.ed.frame).unwrap_or_else(|| ac_core::default_camera(&app.ed.doc).matrix);
    let (c, rot, zoom) = Editor::camera_params(&cam);
    let (mut x, mut y, mut r, mut z) = (c.x, c.y, rot, zoom * 100.0);
    let mut changed = false;
    egui::Grid::new("camprops").num_columns(4).show(ui, |ui| {
        ui.label("X");
        changed |= edited(&ui.add(egui::DragValue::new(&mut x).speed(1.0)));
        ui.label("Y");
        changed |= edited(&ui.add(egui::DragValue::new(&mut y).speed(1.0)));
        ui.end_row();
        ui.label("Zoom");
        changed |= edited(&ui.add(egui::DragValue::new(&mut z).range(5.0..=5000.0).speed(1.0).suffix("%")));
        ui.label("Rotate");
        changed |= edited(&ui.add(egui::DragValue::new(&mut r).speed(0.5).suffix("°")));
        ui.end_row();
    });
    if changed {
        app.ed.begin_edit("Camera");
        app.ed.set_camera(Editor::camera_from(pt(x, y), r, z / 100.0));
    }
    ui.horizontal(|ui| {
        if ui.button("Reset").on_hover_text("Centre, 100%, no rotation (at this frame)").clicked() {
            app.ed.begin_edit("Camera");
            let d = ac_core::default_camera(&app.ed.doc).matrix;
            app.ed.set_camera(d);
        }
        if ui.button("Remove camera").clicked() {
            app.ed.remove_camera();
        }
    });
    camera_effects_ui(app, ui);
    camera_options_ui(app, ui);
    hint(ui, if mode == ac_core::CameraMode::Free {
        "Free camera: drag inside the orange frame to move it, a corner to zoom, the knob to rotate (Shift: 15° steps). Changes make a camera keyframe; a Classic tween on the Camera layer animates it."
    } else {
        "Changes make a camera keyframe at the playhead; add a Classic tween on the Camera layer to animate it. The stage shows the camera view while this tool is active and during playback."
    });
}

/// Camera colour effect and blur: they apply to everything the camera sees.
fn camera_effects_ui(app: &mut App, ui: &mut egui::Ui) {
    let Some(cam) = app.ed.camera_object(app.ed.frame) else { return };
    section(ui, "Camera effects");
    let bright = ac_core::motion::brightness_of(cam.tint);
    let tint_c = cam.tint.filter(|(c, _)| !((c.r, c.g, c.b) == (255, 255, 255) || (c.r, c.g, c.b) == (0, 0, 0)));
    let blur = cam.filters.iter().find_map(|f| if let ac_core::Filter::Blur { blur_x, .. } = f { Some(*blur_x) } else { None }).unwrap_or(0.0);
    let (mut b, mut al, mut bl) = (bright * 100.0, cam.alpha * 100.0, blur);
    let (mut tc, mut ta) = (tint_c.map(|t| c32(t.0)).unwrap_or(Color32::from_rgb(0xFF, 0x80, 0x20)), tint_c.map(|t| t.1 * 100.0).unwrap_or(0.0));
    let mut changed: Option<&'static str> = None;
    egui::Grid::new("cam-fx").num_columns(2).show(ui, |ui| {
        ui.label("Brightness");
        if edited(&ui.add(egui::Slider::new(&mut b, -100.0..=100.0).suffix("%"))) {
            changed = Some("b");
        }
        ui.end_row();
        ui.label("Tint");
        ui.horizontal(|ui| {
            if ui.color_edit_button_srgba(&mut tc).changed() {
                changed = Some("t");
            }
            if edited(&ui.add(egui::Slider::new(&mut ta, 0.0..=100.0).suffix("%"))) {
                changed = Some("t");
            }
        });
        ui.end_row();
        ui.label("Alpha");
        if edited(&ui.add(egui::Slider::new(&mut al, 0.0..=100.0).suffix("%"))) {
            changed = Some("a");
        }
        ui.end_row();
        ui.label("Blur");
        if edited(&ui.add(egui::Slider::new(&mut bl, 0.0..=60.0).suffix(" px"))) {
            changed = Some("blur");
        }
        ui.end_row();
    });
    if let Some(which) = changed {
        if !app.prop_dragging {
            app.ed.begin_edit("Camera Effect");
            app.prop_dragging = true;
        }
        let tcol = from32(tc);
        app.ed.edit_camera(|e| match which {
            "b" => e.tint = ac_core::motion::brightness_tint(b / 100.0, None),
            "t" => e.tint = (ta > 0.0).then_some((tcol, ta / 100.0)),
            "a" => e.alpha = al / 100.0,
            _ => {
                e.filters.retain(|f| !matches!(f, ac_core::Filter::Blur { .. }));
                if bl > 0.0 {
                    e.filters.push(ac_core::Filter::Blur { blur_x: bl, blur_y: bl, quality: 2 });
                }
            }
        });
    }
    if !ui.input(|i| i.pointer.any_down()) {
        app.prop_dragging = false;
    }
}

/// Shake and follow for the camera span at the playhead.
fn camera_options_ui(app: &mut App, ui: &mut egui::Ui) {
    let Some(o) = app.ed.camera_opts() else { return };
    section(ui, "Camera motion (this span)");
    let mut n = o.clone();
    // named objects on the scene at this frame
    let names: Vec<String> = {
        let tl = app.ed.scene_tl();
        let mut v: Vec<String> = tl
            .layers
            .iter()
            .filter(|l| l.kind != ac_core::LayerKind::Camera)
            .filter_map(|l| ac_core::model::layer_elements_at(l, app.ed.frame).map(|(e, _)| e))
            .flatten()
            .map(|e| e.name)
            .filter(|n| !n.is_empty())
            .collect();
        v.sort();
        v.dedup();
        v
    };
    ui.horizontal(|ui| {
        ui.label("Follow");
        egui::ComboBox::from_id_salt("cam-follow").selected_text(n.follow.clone().unwrap_or_else(|| "Nothing".into())).show_ui(ui, |ui| {
            if ui.selectable_label(n.follow.is_none(), "Nothing").clicked() {
                n.follow = None;
            }
            for name in &names {
                if ui.selectable_label(n.follow.as_deref() == Some(name), name).clicked() {
                    n.follow = Some(name.clone());
                }
            }
        });
    });
    if names.is_empty() {
        hint(ui, "Give an object an instance name (Properties › Object) to follow it.");
    }
    if n.follow.is_some() {
        ui.horizontal(|ui| {
            ui.label("Lag");
            ui.add(egui::DragValue::new(&mut n.follow_lag).range(0..=24).suffix(" frames"));
            ui.label("Offset");
            ui.add(egui::DragValue::new(&mut n.follow_offset.x).speed(1.0).prefix("x "));
            ui.add(egui::DragValue::new(&mut n.follow_offset.y).speed(1.0).prefix("y "));
        });
    }
    ui.horizontal(|ui| {
        ui.label("Shake");
        ui.add(egui::DragValue::new(&mut n.shake).range(0.0..=200.0).speed(0.2).suffix(" px"));
        ui.label("Speed");
        ui.add(egui::DragValue::new(&mut n.shake_speed).range(0.1..=60.0).speed(0.1).suffix("/s"));
    });
    if n != o {
        // one undo step per change; a value being dragged merges into it
        if app.prop_dragging {
            app.ed.set_camera_opts_live(n);
        } else {
            app.ed.set_camera_opts(n);
            app.prop_dragging = ui.input(|i| i.pointer.any_down());
        }
    }
}

/// Window › Transform (Ctrl+T): exact scale, rotation, skew, 3D, and
/// copy-and-transform for building repeated patterns.
pub fn transform_window(app: &mut App, ctx: &egui::Context) {
    if !app.transform_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Transform").open(&mut open).resizable(false).default_pos(app.stage_rect.right_top() + vec2(-300.0, 40.0)).show(ctx, |ui| {
        let Some(el) = app.ed.selected_element() else {
            hint(ui, "Select an object on the stage.");
            return;
        };
        let editable = app.ed.selection.iter().all(|s| app.ed.layer_editable(s.layer));
        ui.add_enabled_ui(editable, |ui| {
            let d = el.matrix.decompose();
            let (mut w, mut h) = (d.scale_x * 100.0, d.scale_y * 100.0);
            ui.horizontal(|ui| {
                ui.label("↔");
                let r = ui.add(egui::DragValue::new(&mut w).speed(0.5).suffix(" %").max_decimals(1));
                let (w0, h0, c) = (d.scale_x * 100.0, d.scale_y * 100.0, app.xf.constrain);
                if w.abs() > 0.1 {
                    apply_decomp(app, &r, "Scale", move |dd| {
                        dd.scale_x = w / 100.0;
                        if c && w0.abs() > 1e-3 {
                            dd.scale_y = h0 / 100.0 * (w / w0);
                        }
                    });
                }
                ui.label("↕");
                let r = ui.add(egui::DragValue::new(&mut h).speed(0.5).suffix(" %").max_decimals(1));
                if h.abs() > 0.1 {
                    apply_decomp(app, &r, "Scale", move |dd| {
                        dd.scale_y = h / 100.0;
                        if c && h0.abs() > 1e-3 {
                            dd.scale_x = w0 / 100.0 * (h / h0);
                        }
                    });
                }
                ui.toggle_value(&mut app.xf.constrain, "🔗").on_hover_text("Constrain proportions");
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut app.xf.skew, false, "Rotate");
                let mut rot = d.skew_y.to_degrees();
                let r = ui.add_enabled(!app.xf.skew, egui::DragValue::new(&mut rot).speed(0.5).suffix("°").max_decimals(1));
                let delta = rot.to_radians() - d.skew_y;
                apply_decomp(app, &r, "Rotate", move |dd| {
                    dd.skew_x += delta;
                    dd.skew_y += delta;
                });
            });
            ui.horizontal(|ui| {
                ui.radio_value(&mut app.xf.skew, true, "Skew");
                let (mut sh, mut sv) = (d.skew_x.to_degrees(), d.skew_y.to_degrees());
                let r = ui.add_enabled(app.xf.skew, egui::DragValue::new(&mut sh).speed(0.5).suffix("°").max_decimals(1)).on_hover_text("Skew H: angle of the vertical axis");
                apply_decomp(app, &r, "Skew", move |dd| dd.skew_x = sh.to_radians());
                let r = ui.add_enabled(app.xf.skew, egui::DragValue::new(&mut sv).speed(0.5).suffix("°").max_decimals(1)).on_hover_text("Skew V: angle of the horizontal axis");
                apply_decomp(app, &r, "Skew", move |dd| dd.skew_y = sv.to_radians());
            });
            ui.separator();
            ui.label(RichText::new("3D rotation").small().color(Color32::from_gray(170)));
            let t = el.three_d.unwrap_or_default();
            ui.horizontal(|ui| {
                let (mut rx, mut ry, mut rz) = (t.rx, t.ry, t.rz);
                ui.label("X");
                let r = ui.add(egui::DragValue::new(&mut rx).speed(0.5).suffix("°").max_decimals(1));
                apply_3d(app, &r, "3D Rotation", |t| t.rx = rx);
                ui.label("Y");
                let r = ui.add(egui::DragValue::new(&mut ry).speed(0.5).suffix("°").max_decimals(1));
                apply_3d(app, &r, "3D Rotation", |t| t.ry = ry);
                ui.label("Z");
                let r = ui.add(egui::DragValue::new(&mut rz).speed(0.5).suffix("°").max_decimals(1));
                apply_3d(app, &r, "3D Rotation", |t| t.rz = rz);
            });
            ui.label(RichText::new("3D centre point (object coordinates)").small().color(Color32::from_gray(170)));
            ui.horizontal(|ui| {
                let (mut cx, mut cy, mut z) = (t.center.x, t.center.y, t.z);
                ui.label("X");
                let r = ui.add_enabled(el.three_d.is_some(), egui::DragValue::new(&mut cx).speed(1.0).max_decimals(1));
                apply_3d(app, &r, "3D Center", |t| t.center.x = cx);
                ui.label("Y");
                let r = ui.add_enabled(el.three_d.is_some(), egui::DragValue::new(&mut cy).speed(1.0).max_decimals(1));
                apply_3d(app, &r, "3D Center", |t| t.center.y = cy);
                ui.label("Depth");
                let r = ui.add(egui::DragValue::new(&mut z).speed(1.0).max_decimals(0));
                apply_3d(app, &r, "3D Translation", |t| t.z = z);
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Flip H").clicked() { app.flip(true); }
                if ui.button("Flip V").clicked() { app.flip(false); }
                if ui.button("Remove Transform").on_hover_text("Reset scale, rotation, skew and 3D (keeps position)").clicked() {
                    app.ed.modify_selected("Remove Transform", |e| { let (x, y) = (e.matrix.tx, e.matrix.ty); e.matrix = Matrix::translate(x, y); e.three_d = None; });
                }
            });
            ui.separator();
            ui.label(RichText::new("Copy and transform (repeat patterns)").small().color(Color32::from_gray(170)));
            ui.horizontal(|ui| {
                ui.label("Rotate");
                ui.add(egui::DragValue::new(&mut app.xf.step_rotate).range(-360.0..=360.0).speed(0.5).suffix("°"));
                ui.label("Scale");
                ui.add(egui::DragValue::new(&mut app.xf.step_scale).range(1.0..=1000.0).speed(0.5).suffix("%"));
            });
            if ui.button("Copy selection and apply transform").on_hover_text("Duplicates the selection and rotates/scales the copy about the transformation point").clicked() {
                if let Some(c) = app.pivot() {
                    let mark = app.ed.undo_depth();
                    app.ed.duplicate_in_place();
                    let k = app.xf.step_scale / 100.0;
                    let m = Matrix::translate(c.x, c.y).concat(&Matrix::rotate(app.xf.step_rotate.to_radians())).concat(&Matrix::scale(k, k)).concat(&Matrix::translate(-c.x, -c.y));
                    app.ed.modify_selected("Copy and Transform", |e| e.matrix = m.concat(&e.matrix));
                    app.ed.squash_undo(mark, "Copy and Transform");
                    // keep the same transformation point for the next copy
                    app.ft_pivot = Some((app.ed.selection.clone(), c));
                }
            }
        });
    });
    if !open {
        app.transform_open = false;
    }
}

/// Font family picker: built-in faces plus installed system fonts (searchable).
/// Returns the new (family, built-in fallback face) when the user picks one.
fn font_picker(ui: &mut egui::Ui, search: &mut String, family: Option<&str>, face: FontFace) -> Option<(Option<String>, FontFace)> {
    let current = family.map(|f| f.to_string()).unwrap_or_else(|| match face { FontFace::Sans => "Sans (built-in)".into(), FontFace::Mono => "Mono (built-in)".into() });
    let mut picked = None;
    egui::ComboBox::from_id_salt("font-family").width(180.0).selected_text(current).height(320.0).show_ui(ui, |ui| {
        let r = ui.add(egui::TextEdit::singleline(search).hint_text("🔍 Search fonts").desired_width(170.0));
        if !r.has_focus() {
            r.request_focus(); // type straight away to filter
        }
        if ui.selectable_label(family.is_none() && face == FontFace::Sans, "Sans (built-in)").clicked() {
            picked = Some((None, FontFace::Sans));
        }
        if ui.selectable_label(family.is_none() && face == FontFace::Mono, "Mono (built-in)").clicked() {
            picked = Some((None, FontFace::Mono));
        }
        ui.separator();
        let q = search.to_lowercase();
        let mut shown = 0;
        for f in ac_render::fonts::families() {
            if !q.is_empty() && !f.to_lowercase().contains(&q) {
                continue;
            }
            shown += 1;
            if shown > 300 {
                ui.label(RichText::new("… type to narrow the list").small());
                break;
            }
            if ui.selectable_label(family == Some(f.as_str()), f).clicked() {
                picked = Some((Some(f.clone()), face));
            }
        }
        if shown == 0 {
            ui.label(RichText::new("No installed font matches").small().color(Color32::from_gray(150)));
        }
    });
    picked
}

/// Style dropdown for a text family: the faces it has, plus simulated Bold /
/// Italic when it lacks them. Returns the picked (weight, italic).
fn font_style_picker(ui: &mut egui::Ui, family: Option<&str>, weight: u16, italic: bool) -> Option<(u16, bool)> {
    let mut opts: Vec<(String, u16, bool)> = family.map(ac_render::fonts::styles).unwrap_or_default();
    if opts.is_empty() {
        opts.push(("Regular".into(), 400, false));
    }
    for (w, i) in [(400, false), (700, false), (400, true), (700, true)] {
        if !opts.iter().any(|o| o.1 == w && o.2 == i) && !opts.iter().any(|o| (o.1 as i32 - w as i32).abs() < 100 && o.2 == i) {
            opts.push((format!("{} (simulated)", ac_render::fonts::style_name(w, i)), w, i));
        }
    }
    let cur = opts.iter().find(|o| o.1 == weight && o.2 == italic).map(|o| o.0.clone()).unwrap_or_else(|| ac_render::fonts::style_name(weight, italic));
    let mut pick = None;
    egui::ComboBox::from_id_salt("font-style").selected_text(cur).show_ui(ui, |ui| {
        for (name, w, i) in &opts {
            if ui.selectable_label(*w == weight && *i == italic, name).clicked() {
                pick = Some((*w, *i));
            }
        }
    });
    pick
}

/// One EffectCraft parameter editor. Returns the widget's response for
/// drag/undo tracking (None for widgets that only change on click).
fn effect_param_ui(ui: &mut egui::Ui, kind: &ac_render::effectcraft::ParamKind, v: &mut ac_core::EffectValue, salt: (usize, &str)) -> Option<egui::Response> {
    use ac_core::EffectValue as V;
    use ac_render::effectcraft::ParamKind as K;
    match (kind, v) {
        (K::Number { min, max, decimals }, V::Num(n)) => Some(ui.add(egui::DragValue::new(n).speed(((max - min) / 300.0).max(0.01)).max_decimals(*decimals as usize))),
        (K::Pixels, V::Num(n)) => Some(ui.add(egui::DragValue::new(n).speed(0.5).suffix(" px").max_decimals(1))),
        (K::Angle, V::Num(n)) => Some(ui.add(egui::DragValue::new(n).speed(1.0).suffix("°").max_decimals(1))),
        (K::Percent, V::Num(n)) => Some(ui.add(egui::DragValue::new(n).speed(0.5).suffix("%").max_decimals(1))),
        (K::Checkbox, V::Bool(b)) => {
            ui.checkbox(b, "");
            None
        }
        (K::Popup(opts), V::Num(n)) => {
            let i = (*n as usize).min(opts.len().saturating_sub(1));
            egui::ComboBox::from_id_salt(("fxpop", salt.0, salt.1)).selected_text(opts.get(i).cloned().unwrap_or_default()).show_ui(ui, |ui| {
                for (j, o) in opts.iter().enumerate() {
                    if ui.selectable_label(j == i, o).clicked() {
                        *n = j as f64;
                    }
                }
            });
            None
        }
        (K::Color, V::Color(c)) => {
            let mut c32v = c32(*c);
            if ui.color_edit_button_srgba(&mut c32v).changed() {
                *c = from32(c32v);
            }
            None
        }
        (K::Point, V::Vec2(p)) => {
            let r = ui
                .horizontal(|ui| {
                    let a = ui.add(egui::DragValue::new(&mut p[0]).speed(0.005).range(-2.0..=3.0).prefix("x ").max_decimals(3));
                    let b = ui.add(egui::DragValue::new(&mut p[1]).speed(0.005).range(-2.0..=3.0).prefix("y ").max_decimals(3));
                    a | b
                })
                .inner;
            Some(r)
        }
        _ => {
            ui.label(RichText::new("—").color(Color32::from_gray(140)));
            None
        }
    }
}

/// Properties › Filters and Blending (applies to all selected objects).
fn filters_section(app: &mut App, ui: &mut egui::Ui, el: &Element) {
    section(ui, "Filters and blending");
    let mut list = el.filters.clone();
    let mut changed = false;
    let mut dragging = false;
    let mut stopped = false;
    let mut remove = None;
    let mut track = |r: egui::Response, changed: &mut bool| {
        if r.drag_stopped() {
            stopped = true;
        }
        if edited(&r) || (r.changed() && r.clicked()) {
            *changed = true;
            dragging |= r.dragged();
        }
    };
    for (k, f) in list.iter_mut().enumerate() {
        let title = match &*f {
            Filter::Effect { id, .. } => ac_render::effectcraft::info(id).map_or_else(|| format!("Effect: {id} (not available)"), |e| e.name.to_string()),
            other => other.name().to_string(),
        };
        egui::CollapsingHeader::new(RichText::new(title).color(Color32::from_gray(220))).id_salt(("filter", k)).default_open(true).show(ui, |ui| {
            let dv = |ui: &mut egui::Ui, label: &str, v: &mut f32, lo: f32, hi: f32, suffix: &str| {
                ui.label(label);
                let r = ui.add(egui::DragValue::new(v).range(lo..=hi).speed(0.5).suffix(suffix).max_decimals(1));
                ui.end_row();
                r
            };
            let col = |ui: &mut egui::Ui, c: &mut ac_core::Color| -> bool {
                let mut c32v = c32(*c);
                let ch = ui.color_edit_button_srgba(&mut c32v).changed();
                if ch {
                    *c = from32(c32v);
                }
                ch
            };
            let mut footer_slot: Option<String> = None;
            let footer = &mut footer_slot;
            egui::Grid::new(("fgrid", k)).num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| match f {
                Filter::Blur { blur_x, blur_y, quality } => {
                    track(dv(ui, "Blur X", blur_x, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Blur Y", blur_y, 0.0, 255.0, " px"), &mut changed);
                    quality_row(ui, quality, &mut changed);
                }
                Filter::DropShadow { blur_x, blur_y, strength, quality, angle, distance, color, knockout, inner, hide_object } => {
                    track(dv(ui, "Blur X", blur_x, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Blur Y", blur_y, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Strength", strength, 0.0, 1000.0, "%"), &mut changed);
                    track(dv(ui, "Angle", angle, -360.0, 360.0, "°"), &mut changed);
                    track(dv(ui, "Distance", distance, -255.0, 255.0, " px"), &mut changed);
                    ui.label("Colour");
                    changed |= col(ui, color);
                    ui.end_row();
                    ui.label("");
                    ui.horizontal_wrapped(|ui| {
                        changed |= ui.checkbox(knockout, "Knockout").changed();
                        changed |= ui.checkbox(inner, "Inner").changed();
                        changed |= ui.checkbox(hide_object, "Hide object").changed();
                    });
                    ui.end_row();
                    quality_row(ui, quality, &mut changed);
                }
                Filter::Glow { blur_x, blur_y, strength, quality, color, knockout, inner } => {
                    track(dv(ui, "Blur X", blur_x, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Blur Y", blur_y, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Strength", strength, 0.0, 1000.0, "%"), &mut changed);
                    ui.label("Colour");
                    changed |= col(ui, color);
                    ui.end_row();
                    ui.label("");
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(knockout, "Knockout").changed();
                        changed |= ui.checkbox(inner, "Inner glow").changed();
                    });
                    ui.end_row();
                    quality_row(ui, quality, &mut changed);
                }
                Filter::Bevel { blur_x, blur_y, strength, quality, shadow, highlight, angle, distance, knockout, kind } => {
                    track(dv(ui, "Blur X", blur_x, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Blur Y", blur_y, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Strength", strength, 0.0, 1000.0, "%"), &mut changed);
                    track(dv(ui, "Angle", angle, -360.0, 360.0, "°"), &mut changed);
                    track(dv(ui, "Distance", distance, -255.0, 255.0, " px"), &mut changed);
                    ui.label("");
                    changed |= ui.checkbox(knockout, "Knockout").changed();
                    ui.end_row();
                    ui.label("Shadow / light");
                    ui.horizontal(|ui| {
                        changed |= col(ui, shadow);
                        changed |= col(ui, highlight);
                    });
                    ui.end_row();
                    ui.label("Type");
                    ui.horizontal(|ui| {
                        for (b, l) in [(ac_core::BevelType::Inner, "Inner"), (ac_core::BevelType::Outer, "Outer"), (ac_core::BevelType::Full, "Full")] {
                            changed |= ui.selectable_value(kind, b, l).changed();
                        }
                    });
                    ui.end_row();
                    quality_row(ui, quality, &mut changed);
                }
                Filter::GradientGlow { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops } | Filter::GradientBevel { blur_x, blur_y, strength, quality, angle, distance, knockout, kind, stops } => {
                    track(dv(ui, "Blur X", blur_x, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Blur Y", blur_y, 0.0, 255.0, " px"), &mut changed);
                    track(dv(ui, "Strength", strength, 0.0, 1000.0, "%"), &mut changed);
                    track(dv(ui, "Angle", angle, -360.0, 360.0, "°"), &mut changed);
                    track(dv(ui, "Distance", distance, -255.0, 255.0, " px"), &mut changed);
                    ui.label("");
                    changed |= ui.checkbox(knockout, "Knockout").changed();
                    ui.end_row();
                    ui.label("Type");
                    ui.horizontal(|ui| {
                        for (b, l) in [(ac_core::BevelType::Inner, "Inner"), (ac_core::BevelType::Outer, "Outer"), (ac_core::BevelType::Full, "Full")] {
                            changed |= ui.selectable_value(kind, b, l).changed();
                        }
                    });
                    ui.end_row();
                    ui.label("Gradient");
                    ui.vertical(|ui| {
                        // the ramp, then one row per stop (position, colour with alpha)
                        let (r, _) = ui.allocate_exact_size(egui::vec2(150.0, 12.0), egui::Sense::hover());
                        let p = ui.painter_at(r);
                        for i in 0..50 {
                            let t = i as f32 / 49.0;
                            let c = ac_core::fx::ramp(stops, t);
                            let x0 = r.left() + r.width() * i as f32 / 50.0;
                            let cell = egui::Rect::from_min_max(egui::pos2(x0, r.top()), egui::pos2(x0 + r.width() / 50.0 + 0.5, r.bottom()));
                            let checker = if (i / 3) % 2 == 0 { Color32::from_gray(90) } else { Color32::from_gray(140) };
                            p.rect_filled(cell, 0.0, checker);
                            p.rect_filled(cell, 0.0, Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a));
                        }
                        let mut del = None;
                        let n = stops.len();
                        for (i, (pos, c)) in stops.iter_mut().enumerate() {
                            ui.horizontal(|ui| {
                                let mut pct = *pos * 100.0;
                                let r = ui.add(egui::DragValue::new(&mut pct).range(0.0..=100.0).speed(0.5).suffix("%").max_decimals(0));
                                if r.changed() {
                                    *pos = pct / 100.0;
                                }
                                track(r, &mut changed);
                                let mut c32v = Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a);
                                if egui::color_picker::color_edit_button_srgba(ui, &mut c32v, egui::color_picker::Alpha::OnlyBlend).changed() {
                                    let [r, g, b, a] = c32v.to_srgba_unmultiplied();
                                    *c = ac_core::Color { r, g, b, a };
                                    changed = true;
                                }
                                if n > 2 && ui.small_button("✕").clicked() {
                                    del = Some(i);
                                }
                            });
                        }
                        if let Some(i) = del {
                            stops.remove(i);
                            changed = true;
                        }
                        if ui.small_button("Add colour").clicked() {
                            stops.push((0.5, ac_core::fx::ramp(stops, 0.5)));
                            changed = true;
                        }
                        stops.sort_by(|a, b| a.0.total_cmp(&b.0));
                    });
                    ui.end_row();
                    quality_row(ui, quality, &mut changed);
                }
                Filter::AdjustColor { brightness, contrast, saturation, hue } => {
                    track(dv(ui, "Brightness", brightness, -100.0, 100.0, ""), &mut changed);
                    track(dv(ui, "Contrast", contrast, -100.0, 100.0, ""), &mut changed);
                    track(dv(ui, "Saturation", saturation, -100.0, 100.0, ""), &mut changed);
                    track(dv(ui, "Hue", hue, -180.0, 180.0, "°"), &mut changed);
                }
                Filter::MotionBlur { shutter, samples } => {
                    track(dv(ui, "Shutter angle", shutter, 0.0, 720.0, "°"), &mut changed);
                    ui.label("Samples");
                    let mut n = *samples as i32;
                    let r = ui.add(egui::DragValue::new(&mut n).range(2..=64).speed(0.2));
                    *samples = n as u8;
                    track(r, &mut changed);
                    ui.end_row();
                    *footer = Some("Blurs along the object's movement since the previous frame; 180° = half a frame.".into());
                }
                Filter::Effect { id, params } => match ac_render::effectcraft::info(id) {
                    None => {
                        ui.label(RichText::new("This EffectCraft effect isn't in this build.").small().color(Color32::from_gray(160)));
                        ui.end_row();
                    }
                    Some(info) => {
                        for p in &info.params {
                            let cur = params.iter().find(|(k, _)| k == p.id).map(|(_, v)| v.clone()).unwrap_or_else(|| p.default.clone());
                            let mut v = cur.clone();
                            ui.label(p.name);
                            let r = effect_param_ui(ui, &p.kind, &mut v, (k, p.id));
                            ui.end_row();
                            if v != cur {
                                match params.iter_mut().find(|(k, _)| k == p.id) {
                                    Some(slot) => slot.1 = v,
                                    None => params.push((p.id.to_string(), v)),
                                }
                            }
                            if let Some(r) = r {
                                track(r, &mut changed);
                            } else if params.iter().any(|(k, w)| k == p.id && *w != cur) {
                                changed = true;
                            }
                        }
                        *footer = Some(format!("EffectCraft · {} · {}", info.category, info.id));
                    }
                },
            });
            if let Some(t) = footer.take() {
                ui.add(egui::Label::new(RichText::new(t).small().color(Color32::from_gray(145))).wrap());
            }
            if ui.small_button("Remove filter").clicked() {
                remove = Some(k);
            }
        });
    }
    let mut label = "Filter";
    if let Some(k) = remove {
        list.remove(k);
        changed = true;
        label = "Remove Filter";
    }
    ui.horizontal(|ui| {
        ui.menu_button("➕ Add filter", |ui| {
            for name in Filter::NAMES {
                if ui.button(name).clicked() {
                    if let Some(f) = Filter::default_of(name) {
                        list.push(f);
                        changed = true;
                        label = "Add Filter";
                    }
                    ui.close_menu();
                }
            }
            ui.separator();
            ui.menu_button("EffectCraft effects", |ui| {
                ui.add(egui::TextEdit::singleline(&mut app.effect_search).hint_text("Search effects").desired_width(180.0));
                let q = app.effect_search.trim().to_lowercase();
                let mut pick = None;
                let cat = ac_render::effectcraft::catalog();
                if q.is_empty() {
                    let mut cats: Vec<&str> = cat.iter().map(|e| e.category).collect();
                    cats.dedup();
                    for c in cats {
                        ui.menu_button(c, |ui| {
                            egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                                for e in cat.iter().filter(|e| e.category == c) {
                                    if ui.button(e.name).clicked() {
                                        pick = Some(e.id);
                                    }
                                }
                            });
                        });
                    }
                } else {
                    egui::ScrollArea::vertical().max_height(420.0).show(ui, |ui| {
                        for e in cat.iter().filter(|e| e.name.to_lowercase().contains(&q) || e.category.to_lowercase().contains(&q)) {
                            if ui.button(format!("{}  ·  {}", e.name, e.category)).clicked() {
                                pick = Some(e.id);
                            }
                        }
                    });
                }
                if let Some(id) = pick {
                    list.push(Filter::Effect { id: id.to_string(), params: vec![] });
                    changed = true;
                    label = "Add Effect";
                    ui.close_menu();
                }
            });
        });
        if ui.add_enabled(!list.is_empty(), egui::Button::new("Remove all")).clicked() {
            list.clear();
            changed = true;
            label = "Remove Filters";
        }
        ui.menu_button("Presets", |ui| {
            let presets = app.settings.filter_presets.clone();
            if presets.is_empty() {
                ui.label(RichText::new("No presets yet").small().color(Color32::from_gray(150)));
            }
            for (name, fs) in &presets {
                ui.horizontal(|ui| {
                    if ui.button(name).on_hover_text("Apply: adds these filters").clicked() {
                        list.extend(fs.iter().cloned());
                        changed = true;
                        label = "Apply Filter Preset";
                        ui.close_menu();
                    }
                    if ui.small_button("✕").on_hover_text("Delete preset").clicked() {
                        app.settings.filter_presets.retain(|(n, _)| n != name);
                        app.settings.save();
                    }
                });
            }
            ui.separator();
            ui.add_enabled_ui(!list.is_empty(), |ui| {
                ui.horizontal(|ui| {
                    ui.add(egui::TextEdit::singleline(&mut app.preset_name).hint_text("Preset name").desired_width(110.0));
                    if ui.add_enabled(!app.preset_name.trim().is_empty(), egui::Button::new("Save As")).clicked() {
                        let name = app.preset_name.trim().to_string();
                        app.settings.filter_presets.retain(|(n, _)| *n != name);
                        app.settings.filter_presets.push((name.clone(), list.clone()));
                        app.settings.save();
                        app.status = format!("Saved filter preset '{name}'");
                        app.preset_name.clear();
                        ui.close_menu();
                    }
                });
            });
        });
    });
    if stopped {
        app.prop_dragging = false;
    }
    if changed {
        if dragging {
            if !app.prop_dragging {
                app.ed.begin_edit("Filter");
                app.prop_dragging = true;
            }
            app.ed.modify_selected_continuous(|e| e.filters = list.clone());
        } else {
            app.ed.modify_selected(label, |e| e.filters = list.clone());
        }
    }
    let mut blend = el.blend;
    ui.horizontal(|ui| {
        ui.label("Blending");
        egui::ComboBox::from_id_salt("blend").selected_text(blend.name()).show_ui(ui, |ui| {
            for b in BlendMode::ALL {
                ui.selectable_value(&mut blend, b, b.name());
            }
        });
    });
    if blend != el.blend {
        app.ed.modify_selected("Blending", move |e| e.blend = blend);
    }
}

fn quality_row(ui: &mut egui::Ui, q: &mut u8, changed: &mut bool) {
    ui.label("Quality");
    ui.horizontal(|ui| {
        for (v, l) in [(1u8, "Low"), (2, "Medium"), (3, "High")] {
            *changed |= ui.selectable_value(q, v, l).changed();
        }
    });
    ui.end_row();
}

/// Properties ▸ Color Effect: None / Brightness / Tint / Advanced / Alpha.
fn color_effect(app: &mut App, ui: &mut egui::Ui, el: &Element) {
    use ac_core::ColorXform;
    #[derive(PartialEq, Clone, Copy)]
    enum Style {
        None,
        Brightness,
        Tint,
        Advanced,
        Alpha,
    }
    let bright = ac_core::motion::brightness_of(el.tint);
    let style = if el.color_fx.is_some() {
        Style::Advanced
    } else if bright != 0.0 {
        Style::Brightness
    } else if el.tint.is_some() {
        Style::Tint
    } else if el.alpha < 1.0 {
        Style::Alpha
    } else {
        Style::None
    };
    section(ui, "Color effect");
    let mut pick = style;
    let names = [(Style::None, "None"), (Style::Brightness, "Brightness"), (Style::Tint, "Tint"), (Style::Advanced, "Advanced"), (Style::Alpha, "Alpha")];
    ui.horizontal(|ui| {
        ui.label("Style");
        egui::ComboBox::from_id_salt("color-style").selected_text(names.iter().find(|n| n.0 == style).unwrap().1).show_ui(ui, |ui| {
            for (st, n) in names {
                ui.selectable_value(&mut pick, st, n);
            }
        });
    });
    if pick != style {
        let alpha_keep = el.alpha;
        app.ed.modify_selected("Color Effect", move |e| {
            e.tint = None;
            e.color_fx = None;
            match pick {
                Style::None => e.alpha = 1.0,
                Style::Brightness => e.tint = Some((ac_core::Color::rgb(255, 255, 255), 0.0001)),
                Style::Tint => e.tint = Some((ac_core::Color::rgb(255, 0, 0), 0.5)),
                Style::Advanced => {
                    e.color_fx = Some(ColorXform { mul: [1.0, 1.0, 1.0, alpha_keep], add: [0.0; 4] });
                    e.alpha = 1.0;
                }
                Style::Alpha => e.alpha = alpha_keep.min(0.5),
            }
        });
        return;
    }
    match style {
        Style::None => {}
        Style::Brightness => {
            let mut b = bright * 100.0;
            ui.horizontal(|ui| {
                ui.label("Bright");
                let r = ui.add(egui::Slider::new(&mut b, -100.0..=100.0).suffix("%"));
                apply_slider(app, &r, "Brightness", move |e| e.tint = ac_core::motion::brightness_tint(b / 100.0, None).or(Some((ac_core::Color::rgb(255, 255, 255), 0.0001))));
            });
        }
        Style::Tint => {
            let (c, a) = el.tint.unwrap();
            let mut col = c32(c);
            let mut amt = a * 100.0;
            ui.horizontal(|ui| {
                if ui.color_edit_button_srgba(&mut col).changed() {
                    let c = from32(col);
                    app.ed.modify_selected("Tint", move |e| e.tint = Some((c, a)));
                }
                let r = ui.add(egui::Slider::new(&mut amt, 0.0..=100.0).suffix("%"));
                apply_slider(app, &r, "Tint", move |e| e.tint = Some((c, (amt / 100.0).max(0.0001))));
            });
        }
        Style::Alpha => {
            let mut a = el.alpha * 100.0;
            ui.horizontal(|ui| {
                ui.label("Alpha");
                let r = ui.add(egui::Slider::new(&mut a, 0.0..=100.0).suffix("%"));
                apply_slider(app, &r, "Alpha", move |e| e.alpha = a / 100.0);
            });
        }
        Style::Advanced => {
            let cx = el.color_fx.unwrap();
            egui::Grid::new("adv-color").num_columns(3).show(ui, |ui| {
                for (i, ch) in ["Alpha", "Red", "Green", "Blue"].iter().enumerate() {
                    let k = [3, 0, 1, 2][i];
                    ui.label(*ch);
                    let mut m = cx.mul[k] * 100.0;
                    let mut ad = cx.add[k];
                    let r = ui.add(egui::DragValue::new(&mut m).range(-100.0..=100.0).speed(0.5).suffix("% ×"));
                    if edited(&r) || r.drag_stopped() {
                        apply_slider(app, &r, "Advanced Color", move |e| if let Some(c) = &mut e.color_fx { c.mul[k] = m / 100.0 });
                    }
                    let r = ui.add(egui::DragValue::new(&mut ad).range(-255.0..=255.0).speed(1.0).prefix("+ "));
                    if edited(&r) || r.drag_stopped() {
                        apply_slider(app, &r, "Advanced Color", move |e| if let Some(c) = &mut e.color_fx { c.add[k] = ad });
                    }
                    ui.end_row();
                }
            });
        }
    }
}

/// Properties ▸ Tool for the Assistants tool: add, list, edit.
/// A joint's (rotate, limit, pinned) after an edit in Properties.
type JointEdit = (bool, Option<(f32, f32)>, bool);

fn assistant_properties(app: &mut App, ui: &mut egui::Ui) {
    use ac_core::assist::{Assistant, AssistantKind as K};
    section(ui, "Drawing assistants");
    ui.checkbox(&mut app.topts.assist_on, "Use with Pencil, Brush and Fluid Brush");
    ui.horizontal_wrapped(|ui| {
        ui.label("Add");
        for name in K::NAMES {
            if ui.small_button(name).clicked() {
                let r = app.stage_rect;
                let c = app.to_doc(r, r.center());
                if let Some(kind) = K::create(name, c, 100.0 / app.zoom.max(0.01)) {
                    let i = app.ed.add_assistant(Assistant::new(kind));
                    app.assist_sel = Some(i);
                }
            }
        }
    });
    let n = app.ed.doc.assistants.len();
    if n == 0 {
        hint(ui, "Guides — Line, Vanishing point, Ellipse, Perspective, Fisheye — pull freehand strokes onto them (the start of a stroke picks the guide that fits). Replicators — Mirror, Star, Grid, Affine — repeat every stroke; Jitter shakes it. Drag handles on the stage; Alt+click a handle deletes.");
        return;
    }
    let mut remove = None;
    for i in 0..n {
        let a = app.ed.doc.assistants[i].clone();
        ui.horizontal(|ui| {
            let mut on = a.enabled;
            if ui.checkbox(&mut on, "").on_hover_text("Enabled").changed() {
                app.ed.set_assistant(i, Assistant { enabled: on, ..a.clone() }, true);
            }
            if ui.selectable_label(app.assist_sel == Some(i), format!("{}. {}", i + 1, a.name())).clicked() {
                app.assist_sel = Some(i);
            }
            if ui.small_button("Delete").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        app.ed.remove_assistant(i);
        app.assist_sel = None;
        return;
    }
    let Some(i) = app.assist_sel.filter(|i| *i < n) else { return };
    let a = app.ed.doc.assistants[i].clone();
    let mut b = a.clone();
    if a.is_guide() {
        ui.add(egui::Slider::new(&mut b.magnetism, 0.0..=1.0).text("Magnetism"));
    }
    match &mut b.kind {
        K::Line { parallel, .. } => {
            ui.checkbox(parallel, "Parallel (any line in this direction)");
        }
        K::Ellipse { concentric, .. } => {
            ui.checkbox(concentric, "Concentric (any size)");
        }
        K::Star { count, mirror, .. } => {
            ui.add(egui::Slider::new(count, 1..=32).text("Copies"));
            ui.checkbox(mirror, "Mirror (kaleidoscope)");
        }
        K::Grid { count_a, count_a_inv, count_b, count_b_inv, .. } => {
            ui.horizontal(|ui| {
                ui.label("Across");
                ui.add(egui::DragValue::new(count_a_inv).range(0..=20)).on_hover_text("Copies backward");
                ui.add(egui::DragValue::new(count_a).range(1..=20)).on_hover_text("Copies forward (incl. the original)");
                ui.label("Down");
                ui.add(egui::DragValue::new(count_b_inv).range(0..=20));
                ui.add(egui::DragValue::new(count_b).range(1..=20));
            });
        }
        K::Perspective { three_point, .. } => {
            ui.checkbox(three_point, "Three-point (verticals converge)");
            hint(ui, "Handles: left and right vanishing points on the horizon, and the third point below or above.");
        }
        K::Fisheye { .. } => hint(ui, "Strokes follow the radius or the curved meridians through the poles; outside the circle they follow its edge."),
        K::Affine { count, count_inv, .. } => {
            ui.horizontal(|ui| {
                ui.label("Copies");
                ui.add(egui::DragValue::new(count).range(0..=100)).on_hover_text("Forward through the transform");
                ui.add(egui::DragValue::new(count_inv).range(0..=100)).on_hover_text("Backward (the inverse transform)");
            });
            hint(ui, "The blue frame maps onto the orange one; each copy repeats that move. Drag the orange centre to move the second frame.");
        }
        K::Jitter { period, amplitude, keep_first, keep_last, .. } => {
            ui.add(egui::Slider::new(period, 1.0..=300.0).logarithmic(true).text("Period"));
            ui.add(egui::Slider::new(amplitude, 0.0..=100.0).text("Amplitude"));
            ui.horizontal(|ui| {
                ui.checkbox(keep_first, "Fix first point");
                ui.checkbox(keep_last, "Fix last point");
            });
        }
        _ => {}
    }
    if b != a {
        // sliders keep one undo step per drag
        let first = !app.prop_dragging;
        app.prop_dragging = ui.input(|inp| inp.pointer.any_down());
        app.ed.set_assistant(i, b, first);
    } else if !ui.input(|inp| inp.pointer.any_down()) {
        app.prop_dragging = false;
    }
}

/// Joint ▸ Rotation: Enable and Constrain (min/max degrees). Returns a change.
fn joint_ui(ui: &mut egui::Ui, id: impl std::hash::Hash, rotate: bool, limit: Option<(f32, f32)>, pinned: bool) -> Option<JointEdit> {
    let mut out = None;
    ui.push_id(id, |ui| {
        ui.horizontal(|ui| {
            let mut rot = rotate;
            if ui.checkbox(&mut rot, "Joint rotates").on_hover_text("Off: this joint stays as it is while you pose the chain").changed() {
                out = Some((rot, limit, pinned));
            }
            let mut con = limit.is_some();
            if ui.add_enabled(rotate, egui::Checkbox::new(&mut con, "Constrain")).on_hover_text("Limit how far the joint turns from the pose it had when the bone was made").changed() {
                out = Some((rotate, if con { Some((-45.0, 45.0)) } else { None }, pinned));
            }
            let mut pin = pinned;
            if ui.checkbox(&mut pin, "Pin").on_hover_text("The joint stays where it is now while you pose the rest").changed() {
                out = Some((rotate, limit, pin));
            }
        });
        if let (true, Some((mut lo, mut hi))) = (rotate, limit) {
            ui.horizontal(|ui| {
                ui.label("Min");
                let a = ui.add(egui::DragValue::new(&mut lo).range(-180.0..=0.0).speed(1.0).suffix("°"));
                ui.label("Max");
                let b = ui.add(egui::DragValue::new(&mut hi).range(0.0..=180.0).speed(1.0).suffix("°"));
                if a.changed() || b.changed() {
                    out = Some((rotate, Some((lo, hi)), pinned));
                }
            });
        }
    });
    out
}

/// Like `apply` for sliders (which report `changed` while dragging only).
fn apply_slider(app: &mut App, resp: &egui::Response, label: &str, f: impl Fn(&mut Element)) {
    if resp.drag_stopped() {
        app.prop_dragging = false;
    }
    if !resp.changed() {
        return;
    }
    if resp.dragged() {
        if !app.prop_dragging {
            app.ed.begin_edit(label);
            app.prop_dragging = true;
        }
        app.ed.modify_selected_continuous(f);
    } else {
        app.ed.modify_selected(label, f);
    }
}

/// Stroke style, bitmap fill, primitive parameters and shape bones of a drawing object.
fn shape_extras(app: &mut App, ui: &mut egui::Ui, s: &ac_core::Shape) {
    use ac_core::{BitmapFill, Cap, Dash, Join, Primitive};
    if s.stroke.is_some() {
        let st = s.stroke_style;
        ui.horizontal(|ui| {
            let mut v = st;
            egui::ComboBox::from_id_salt("dash").width(70.0).selected_text(v.dash.name()).show_ui(ui, |ui| {
                for d in Dash::ALL {
                    ui.selectable_value(&mut v.dash, d, d.name());
                }
            }).response.on_hover_text("Stroke style");
            egui::ComboBox::from_id_salt("cap").width(64.0).selected_text(match v.cap { Cap::None => "No cap", Cap::Round => "Round", Cap::Square => "Square" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut v.cap, Cap::None, "None");
                ui.selectable_value(&mut v.cap, Cap::Round, "Round");
                ui.selectable_value(&mut v.cap, Cap::Square, "Square");
            }).response.on_hover_text("Cap");
            egui::ComboBox::from_id_salt("join").width(64.0).selected_text(match v.join { Join::Miter => "Miter", Join::Round => "Round", Join::Bevel => "Bevel" }).show_ui(ui, |ui| {
                ui.selectable_value(&mut v.join, Join::Miter, "Miter");
                ui.selectable_value(&mut v.join, Join::Round, "Round");
                ui.selectable_value(&mut v.join, Join::Bevel, "Bevel");
            }).response.on_hover_text("Join");
            if v.dash != st.dash {
                v.art = Default::default(); // parameters belong to the old style
            }
            if v != st {
                app.ed.modify_selected("Stroke Style", move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.stroke_style = v });
            }
        });
        // OpenToonz styles: their parameters and extra colours
        let ps = ac_core::strokes::params(st.dash);
        let cs = ac_core::strokes::color_params(st.dash);
        if !ps.is_empty() || !cs.is_empty() {
            egui::Grid::new("art_params").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
                for (i, p) in ps.iter().enumerate() {
                    ui.label(p.name);
                    let mut x = ac_core::strokes::param(&st, i);
                    let speed = ((p.max - p.min) / 200.0).max(0.005);
                    let r = ui.add(egui::DragValue::new(&mut x).range(p.min..=p.max).speed(speed).max_decimals(2));
                    apply(app, &r, p.name, move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.stroke_style.art.p[i] = Some(x) });
                    ui.end_row();
                }
                for (i, name) in cs.iter().enumerate() {
                    ui.label(*name);
                    ui.horizontal(|ui| {
                        let base = s.stroke.map(|k| k.color).unwrap_or(ac_core::Color::rgb(0, 0, 0));
                        let mut col = c32(ac_core::strokes::color_param(&st, base, i));
                        if ui.color_edit_button_srgba(&mut col).changed() {
                            let c = from32(col);
                            app.ed.modify_selected(name, move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.stroke_style.art.colors[i] = Some(c) });
                        }
                        if st.art.colors[i].is_some() && ui.small_button("Auto").on_hover_text("Derive from the stroke colour").clicked() {
                            app.ed.modify_selected(name, move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.stroke_style.art.colors[i] = None });
                        }
                    });
                    ui.end_row();
                }
            });
        }
        if st.dash == Dash::Dashed {
            ui.horizontal(|ui| {
                let (mut d, mut g) = (st.dash_len, st.gap);
                ui.label("Dash");
                let r = ui.add(egui::DragValue::new(&mut d).range(0.2..=50.0).speed(0.1).suffix(" × width"));
                apply(app, &r, "Dash", move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.stroke_style.dash_len = d });
                ui.label("Gap");
                let r = ui.add(egui::DragValue::new(&mut g).range(0.2..=50.0).speed(0.1));
                apply(app, &r, "Dash Gap", move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.stroke_style.gap = g });
            });
        }
    }
    // bitmap fill
    if !app.ed.doc.bitmaps.is_empty() {
        ui.horizontal(|ui| {
            ui.label("Bitmap fill");
            let cur = s.bitmap_fill.and_then(|b| app.ed.doc.bitmap(b.bitmap)).map(|a| a.name.clone()).unwrap_or_else(|| "None".into());
            let mut pick: Option<Option<ac_core::bitmap::BitmapId>> = None;
            egui::ComboBox::from_id_salt("bmpfill").selected_text(cur).show_ui(ui, |ui| {
                if ui.selectable_label(s.bitmap_fill.is_none(), "None").clicked() {
                    pick = Some(None);
                }
                for b in &app.ed.doc.bitmaps {
                    if ui.selectable_label(s.bitmap_fill.map(|f| f.bitmap) == Some(b.id), &b.name).clicked() {
                        pick = Some(Some(b.id));
                    }
                }
            });
            if let Some(choice) = pick {
                app.ed.modify_selected("Bitmap Fill", move |e| {
                    if let ElementKind::Shape(sh) = &mut e.kind {
                        let origin = sh.path.bounds().map(|(lo, _)| lo).unwrap_or_default();
                        sh.bitmap_fill = choice.map(|bitmap| BitmapFill { bitmap, matrix: Matrix::translate(origin.x, origin.y) });
                    }
                });
            }
        });
    }
    // primitives
    match s.primitive {
        Some(Primitive::Rect { x, y, w, h, radius }) => {
            section(ui, "Rectangle primitive");
            let mut r = radius;
            let mut changed = false;
            egui::Grid::new("prim-r").num_columns(4).show(ui, |ui| {
                for (i, l) in ["TL", "TR", "BR", "BL"].iter().enumerate() {
                    ui.label(*l);
                    let resp = ui.add(egui::DragValue::new(&mut r[i]).range(-100.0..=100.0).speed(0.5));
                    changed |= edited(&resp);
                    if i == 1 {
                        ui.end_row();
                    }
                }
            });
            if ui.small_button("Same radius for all corners").clicked() {
                r = [r[0]; 4];
                changed = true;
            }
            if changed {
                let p = Primitive::Rect { x, y, w, h, radius: r };
                app.ed.modify_selected("Corner Radius", move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.primitive = Some(p); sh.path = p.path(); });
            }
            hint(ui, "Negative radii cut the corners inwards.");
        }
        Some(Primitive::Oval { cx, cy, rx, ry, start, end, inner, close }) => {
            section(ui, "Oval primitive");
            let (mut a, mut b, mut i, mut c) = (start, end, inner, close);
            let mut changed = false;
            changed |= ui.add(egui::Slider::new(&mut a, 0.0..=360.0).text("Start angle")).changed();
            changed |= ui.add(egui::Slider::new(&mut b, 0.0..=360.0).text("End angle")).changed();
            changed |= ui.add(egui::Slider::new(&mut i, 0.0..=99.0).text("Inner radius %")).changed();
            changed |= ui.checkbox(&mut c, "Close path").changed();
            if changed {
                let p = Primitive::Oval { cx, cy, rx, ry, start: a, end: b, inner: i, close: c };
                app.ed.modify_selected("Oval Primitive", move |e| if let ElementKind::Shape(sh) = &mut e.kind { sh.primitive = Some(p); sh.path = p.path(); });
            }
        }
        None => {}
    }
    // shape bones
    if let Some(r) = &s.rig {
        section(ui, "Shape bones");
        ui.horizontal(|ui| {
            hint(ui, &format!("{} bone(s), {} bound point(s)", r.bones.len(), r.bind.len()));
            if ui.small_button("Reset pose").clicked() {
                app.ed.modify_selected("Reset Pose", |e| {
                    if let ElementKind::Shape(ac_core::Shape { rig: Some(r), .. }) = &mut e.kind {
                        for b in &mut r.bones {
                            b.angle = 0.0;
                        }
                    }
                });
            }
            if ui.small_button("Remove bones").on_hover_text("Keeps the posed shape").clicked() {
                if let Some(&sel) = app.ed.selection.first() {
                    app.ed.rig_clear(sel);
                }
            }
        });
        // joint constraints of one bone
        let id = ui.id().with("rig-joint-bone");
        let mut bi: usize = ui.data_mut(|d| d.get_temp(id)).unwrap_or(0).min(r.bones.len().saturating_sub(1));
        if !r.bones.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Joint of");
                egui::ComboBox::from_id_salt("rig-joint").width(70.0).selected_text(format!("bone {}", bi + 1)).show_ui(ui, |ui| {
                    for i in 0..r.bones.len() {
                        ui.selectable_value(&mut bi, i, format!("bone {}", i + 1));
                    }
                });
            });
            ui.data_mut(|d| d.insert_temp(id, bi));
            let b = r.bones[bi];
            let tail = r.posed()[bi].1;
            if let Some((rotate, limit, pin)) = joint_ui(ui, ("rig-joint", bi), b.rotate, b.limit, b.pin.is_some()) {
                app.ed.modify_selected("Joint", move |e| {
                    if let ElementKind::Shape(ac_core::Shape { rig: Some(r), .. }) = &mut e.kind {
                        if let Some(b) = r.bones.get_mut(bi) {
                            b.rotate = rotate;
                            b.limit = limit;
                            b.pin = pin.then_some(b.pin.unwrap_or(tail));
                        }
                    }
                });
            }
        }
    }
}

/// Properties of a particle emitter (one undo step per slider drag).
fn particles_properties(app: &mut App, ui: &mut egui::Ui, e0: &ac_core::particles::Emitter) {
    use ac_core::particles::ParticleSource as Src;
    section(ui, "Particles");
    let mut e = e0.clone();
    let src_name = |s: &Src, app: &App| match s {
        Src::Dot => "Dot".to_string(),
        Src::Square => "Square".to_string(),
        Src::Star => "Star".to_string(),
        Src::Symbol { symbol } => format!("Symbol: {}", app.ed.doc.symbol(*symbol).map(|s| s.name.as_str()).unwrap_or("?")),
        Src::Bitmap { bitmap } => format!("Bitmap: {}", app.ed.doc.bitmap(*bitmap).map(|b| b.name.as_str()).unwrap_or("?")),
    };
    ui.horizontal(|ui| {
        ui.label("Particle");
        egui::ComboBox::from_id_salt("particle_src").selected_text(src_name(&e.source, app)).width(170.0).show_ui(ui, |ui| {
            for s in [Src::Dot, Src::Square, Src::Star] {
                let t = src_name(&s, app);
                ui.selectable_value(&mut e.source, s, t);
            }
            for sym in &app.ed.doc.library {
                ui.selectable_value(&mut e.source, Src::Symbol { symbol: sym.id }, format!("Symbol: {}", sym.name));
            }
            for b in &app.ed.doc.bitmaps {
                ui.selectable_value(&mut e.source, Src::Bitmap { bitmap: b.id }, format!("Bitmap: {}", b.name));
            }
        });
    });
    egui::Grid::new("particles").num_columns(4).spacing([6.0, 3.0]).show(ui, |ui| {
        let dv = |ui: &mut egui::Ui, v: &mut f32, lo: f32, hi: f32, speed: f64, suffix: &str| {
            ui.add(egui::DragValue::new(v).range(lo..=hi).speed(speed).max_decimals(2).suffix(suffix.to_string()));
        };
        ui.label("Rate");
        dv(ui, &mut e.rate, 0.0, 2000.0, 0.5, "/s");
        ui.label("Max");
        ui.add(egui::DragValue::new(&mut e.max).range(1..=20000));
        ui.end_row();
        ui.label("Life");
        dv(ui, &mut e.life, 0.05, 60.0, 0.02, " s");
        ui.label("±");
        dv(ui, &mut e.life_var, 0.0, 1.0, 0.01, "");
        ui.end_row();
        ui.label("Speed");
        dv(ui, &mut e.speed, 0.0, 5000.0, 1.0, "");
        ui.label("±");
        dv(ui, &mut e.speed_var, 0.0, 1.0, 0.01, "");
        ui.end_row();
        ui.label("Direction");
        dv(ui, &mut e.direction, -360.0, 360.0, 1.0, "°");
        ui.label("Spread");
        dv(ui, &mut e.spread, 0.0, 360.0, 1.0, "°");
        ui.end_row();
        ui.label("Gravity X");
        dv(ui, &mut e.gravity.x, -5000.0, 5000.0, 1.0, "");
        ui.label("Y");
        dv(ui, &mut e.gravity.y, -5000.0, 5000.0, 1.0, "");
        ui.end_row();
        ui.label("Drag");
        dv(ui, &mut e.drag, 0.0, 0.99, 0.01, "");
        ui.label("Spin");
        dv(ui, &mut e.spin, -3600.0, 3600.0, 1.0, "°/s");
        ui.end_row();
        ui.label("Spin ±");
        dv(ui, &mut e.spin_var, 0.0, 3600.0, 1.0, "°/s");
        ui.checkbox(&mut e.random_angle, "Random angle");
        ui.end_row();
        ui.label("Scale");
        dv(ui, &mut e.scale_start, 0.0, 2000.0, 1.0, "%");
        ui.label("to");
        dv(ui, &mut e.scale_end, 0.0, 2000.0, 1.0, "%");
        ui.end_row();
        ui.label("Scale ±");
        dv(ui, &mut e.scale_var, 0.0, 1.0, 0.01, "");
        ui.label("");
        ui.end_row();
        ui.label("Fade in");
        dv(ui, &mut e.fade_in, 0.0, 1.0, 0.01, "");
        ui.label("out");
        dv(ui, &mut e.fade_out, 0.0, 1.0, 0.01, "");
        ui.end_row();
        ui.label("Area W");
        dv(ui, &mut e.area.x, 0.0, 10000.0, 1.0, "");
        ui.label("H");
        dv(ui, &mut e.area.y, 0.0, 10000.0, 1.0, "");
        ui.end_row();
        ui.label("Stop after");
        dv(ui, &mut e.duration, 0.0, 3600.0, 0.05, " s");
        ui.label("Seed");
        ui.add(egui::DragValue::new(&mut e.seed));
        ui.end_row();
    });
    ui.horizontal(|ui| {
        ui.label("Colour");
        let mut a = c32(e.color_start);
        if ui.color_edit_button_srgba(&mut a).changed() {
            e.color_start = from32(a);
        }
        ui.label("to");
        let mut b = c32(e.color_end);
        if ui.color_edit_button_srgba(&mut b).changed() {
            e.color_end = from32(b);
        }
        if matches!(e.source, Src::Symbol { .. } | Src::Bitmap { .. }) {
            ui.checkbox(&mut e.tint, "Tint").on_hover_text("Tint the symbol / bitmap by the colour (its alpha = amount)");
        }
    });
    ui.checkbox(&mut e.prewarm, "Pre-warm (start full)");
    hint(ui, "Particles start at the emitter's keyframe and play like a graphic symbol: scrub freely, every export draws them.");
    if e != *e0 {
        let first = !app.prop_dragging;
        app.prop_dragging = ui.input(|inp| inp.pointer.any_down());
        let set = move |el: &mut Element| {
            if let ElementKind::Particles(p) = &mut el.kind {
                **p = e.clone();
            }
        };
        if first {
            app.ed.modify_selected("Particles", set);
        } else {
            app.ed.modify_selected_continuous(set);
        }
    } else if !ui.input(|inp| inp.pointer.any_down()) {
        app.prop_dragging = false;
    }
}
