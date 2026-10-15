//! Align (Ctrl+K) and Info (Ctrl+I) panels.

use crate::App;
use ac_core::editor::AlignOp;
use ac_core::pt;
use eframe::egui::{self, Color32, RichText};


fn op_button(ui: &mut egui::Ui, label: &str, tip: &str) -> bool {
    ui.add(egui::Button::new(label).min_size(egui::vec2(30.0, 24.0))).on_hover_text(tip).clicked()
}

/// Align / space buttons drawn with lines (the default font lacks these arrows).
fn icon_button(ui: &mut egui::Ui, op: AlignOp, tip: &str) -> bool {
    use egui::{pos2, Rect, Sense, Stroke};
    let (r, resp) = ui.allocate_exact_size(egui::vec2(30.0, 24.0), Sense::click());
    let v = ui.style().interact(&resp);
    ui.painter().rect(r, 3.0, v.weak_bg_fill, v.bg_stroke);
    let c = r.center();
    let col = Color32::from_gray(225);
    let line = Stroke::new(1.5_f32, crate::ACCENT);
    let bar = |x0: f32, y0: f32, x1: f32, y1: f32| Rect::from_min_max(pos2(x0, y0), pos2(x1, y1));
    let p = ui.painter();
    match op {
        AlignOp::Left | AlignOp::HCenter | AlignOp::Right => {
            let x = match op { AlignOp::Left => c.x - 8.0, AlignOp::HCenter => c.x, _ => c.x + 8.0 };
            p.line_segment([pos2(x, c.y - 9.0), pos2(x, c.y + 9.0)], line);
            for (w, y) in [(14.0, c.y - 5.0), (9.0, c.y + 3.0)] {
                let (x0, x1) = match op { AlignOp::Left => (x, x + w), AlignOp::HCenter => (x - w / 2.0, x + w / 2.0), _ => (x - w, x) };
                p.rect_filled(bar(x0, y, x1, y + 4.0), 1.0, col);
            }
        }
        AlignOp::Top | AlignOp::VCenter | AlignOp::Bottom => {
            let y = match op { AlignOp::Top => c.y - 8.0, AlignOp::VCenter => c.y, _ => c.y + 8.0 };
            p.line_segment([pos2(c.x - 10.0, y), pos2(c.x + 10.0, y)], line);
            for (h, x) in [(14.0, c.x - 6.0), (9.0, c.x + 2.0)] {
                let (y0, y1) = match op { AlignOp::Top => (y, y + h), AlignOp::VCenter => (y - h / 2.0, y + h / 2.0), _ => (y - h, y) };
                p.rect_filled(bar(x, y0, x + 4.0, y1), 1.0, col);
            }
        }
        AlignOp::SpaceH => {
            for x in [c.x - 10.0, c.x - 2.0, c.x + 6.0] {
                p.rect_filled(bar(x, c.y - 6.0, x + 4.0, c.y + 6.0), 1.0, col);
            }
        }
        _ => {
            for y in [c.y - 9.0, c.y - 2.0, c.y + 5.0] {
                p.rect_filled(bar(c.x - 7.0, y, c.x + 7.0, y + 4.0), 1.0, col);
            }
        }
    }
    resp.on_hover_text(tip).clicked()
}

pub fn align_window(app: &mut App, ctx: &egui::Context) {
    if !app.align_open {
        return;
    }
    let mut open = true;
    let mut op = None;
    egui::Window::new("Align").open(&mut open).resizable(false).default_pos(app.stage_rect.right_top() + egui::vec2(-260.0, 60.0)).show(ctx, |ui| {
        let n = app.ed.selection.len();
        ui.checkbox(&mut app.align_to_stage, "Align to stage").on_hover_text("Off: align to each other (needs 2+ objects)");
        ui.label(RichText::new("Align").small().color(Color32::from_gray(170)));
        ui.horizontal(|ui| {
            for (tip, o) in [("Left edges", AlignOp::Left), ("Horizontal centres", AlignOp::HCenter), ("Right edges", AlignOp::Right)] {
                if icon_button(ui, o, tip) {
                    op = Some(o);
                }
            }
            ui.separator();
            for (tip, o) in [("Top edges", AlignOp::Top), ("Vertical centres", AlignOp::VCenter), ("Bottom edges", AlignOp::Bottom)] {
                if icon_button(ui, o, tip) {
                    op = Some(o);
                }
            }
        });
        ui.label(RichText::new("Distribute").small().color(Color32::from_gray(170)));
        ui.horizontal(|ui| {
            for (l, tip, o) in [("L", "Distribute left edges", AlignOp::DistLeft), ("C", "Distribute horizontal centres", AlignOp::DistHCenter), ("R", "Distribute right edges", AlignOp::DistRight)] {
                if op_button(ui, l, tip) {
                    op = Some(o);
                }
            }
            ui.separator();
            for (l, tip, o) in [("T", "Distribute top edges", AlignOp::DistTop), ("M", "Distribute vertical centres", AlignOp::DistVCenter), ("B", "Distribute bottom edges", AlignOp::DistBottom)] {
                if op_button(ui, l, tip) {
                    op = Some(o);
                }
            }
        });
        ui.label(RichText::new("Match size · Space evenly").small().color(Color32::from_gray(170)));
        ui.horizontal(|ui| {
            for (l, tip, o) in [("W", "Match width (widest, or the stage)", AlignOp::MatchWidth), ("H", "Match height", AlignOp::MatchHeight), ("W+H", "Match width and height", AlignOp::MatchSize)] {
                if op_button(ui, l, tip) {
                    op = Some(o);
                }
            }
            ui.separator();
            for (tip, o) in [("Space evenly horizontally", AlignOp::SpaceH), ("Space evenly vertically", AlignOp::SpaceV)] {
                if icon_button(ui, o, tip) {
                    op = Some(o);
                }
            }
        });
        if n == 0 {
            ui.label(RichText::new("Select objects on the stage.").small().color(Color32::from_rgb(0xFF, 0xB0, 0x40)));
        } else if n == 1 && !app.align_to_stage {
            ui.label(RichText::new("One object: tick “Align to stage”.").small().color(Color32::from_gray(150)));
        }
    });
    if let Some(o) = op {
        app.ed.align(o, app.align_to_stage);
    }
    if !open {
        app.align_open = false;
    }
}

pub fn info_window(app: &mut App, ctx: &egui::Context) {
    if !app.info_open {
        return;
    }
    let mut open = true;
    let pointer = ctx.input(|i| i.pointer.hover_pos()).filter(|p| app.stage_rect.contains(*p));
    egui::Window::new("Info").open(&mut open).resizable(false).default_pos(app.stage_rect.right_top() + egui::vec2(-260.0, 320.0)).show(ctx, |ui| {
        egui::Grid::new("info").num_columns(4).spacing([10.0, 4.0]).show(ui, |ui| {
            match app.ed.selection_bounds() {
                Some((lo, hi)) => {
                    ui.label("X");
                    ui.monospace(format!("{:.1}", lo.x));
                    ui.label("W");
                    ui.monospace(format!("{:.1}", hi.x - lo.x));
                    ui.end_row();
                    ui.label("Y");
                    ui.monospace(format!("{:.1}", lo.y));
                    ui.label("H");
                    ui.monospace(format!("{:.1}", hi.y - lo.y));
                    ui.end_row();
                }
                None => {
                    ui.label(RichText::new("No selection").color(Color32::from_gray(150)));
                    ui.end_row();
                }
            }
            if let Some(p) = pointer {
                let d = app.to_doc(app.stage_rect, p);
                ui.label("Cursor");
                ui.monospace(format!("{:.0}, {:.0}", d.x, d.y));
                // colour under the cursor, from the rendered document
                let doc = &app.ed.doc;
                if app.ed.editing.is_none() && d.x >= 0.0 && d.y >= 0.0 && d.x < doc.width && d.y < doc.height {
                    let key = (app.ed.revision, app.ed.frame);
                    if app.info_cache.as_ref().map(|c| c.0 != key).unwrap_or(true) {
                        app.info_cache = Some((key, ac_render::render_frame(doc, app.ed.frame, 1.0)));
                    }
                    let pix = &app.info_cache.as_ref().unwrap().1;
                    if let Some(c) = pix.pixel(d.x as u32, d.y as u32) {
                        let c = c.demultiply();
                        let col = Color32::from_rgb(c.red(), c.green(), c.blue());
                        ui.label("RGBA");
                        ui.horizontal(|ui| {
                            let (r, _) = ui.allocate_exact_size(egui::vec2(12.0, 12.0), egui::Sense::hover());
                            ui.painter().rect_filled(r, 2.0, col);
                            ui.monospace(format!("{} {} {} {}%", c.red(), c.green(), c.blue(), c.alpha() as u32 * 100 / 255));
                        });
                    }
                }
                ui.end_row();
                let _ = pt(0.0, 0.0);
            }
        });
        let n = app.ed.selection.len();
        ui.label(RichText::new(format!("{n} selected · frame {} · layer “{}”", app.ed.frame + 1, app.ed.timeline().layers.get(app.ed.layer).map(|l| l.name.as_str()).unwrap_or(""))).small().color(Color32::from_gray(150)));
    });
    if pointer.is_some() {
        ctx.request_repaint();
    }
    if !open {
        app.info_open = false;
    }
}

type SceneAction = Box<dyn FnOnce(&mut App)>;

/// Window ▸ Scene (Shift+F2): list, switch, add, duplicate, delete, rename, reorder.
pub fn scene_window(app: &mut App, ctx: &egui::Context) {
    if !app.scene_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Scene").open(&mut open).resizable(false).default_width(240.0).default_pos(app.stage_rect.right_top() + egui::vec2(-280.0, 80.0)).show(ctx, |ui| {
        let n = app.ed.doc.scene_count();
        let names: Vec<(String, u32)> = app.ed.doc.scenes().map(|t| (t.name.clone(), t.frame_count())).collect();
        let mut action: Option<SceneAction> = None;
        for (i, (name, frames)) in names.iter().enumerate() {
            ui.horizontal(|ui| {
                let renaming = app.scene_rename.as_ref().map(|r| r.0 == i).unwrap_or(false);
                if renaming {
                    let (_, buf) = app.scene_rename.as_mut().unwrap();
                    let r = ui.add(egui::TextEdit::singleline(buf).desired_width(130.0));
                    r.request_focus();
                    if r.lost_focus() {
                        let (_, nm) = app.scene_rename.take().unwrap();
                        action = Some(Box::new(move |app: &mut App| app.ed.rename_scene(i, &nm)));
                    }
                } else {
                    let cur = i == app.ed.scene && app.ed.editing.is_none();
                    let r = ui.selectable_label(cur, RichText::new(format!("{}. {name}", i + 1)).strong()).on_hover_text("Click to edit this scene · double-click to rename");
                    if r.double_clicked() {
                        app.scene_rename = Some((i, name.clone()));
                    } else if r.clicked() {
                        action = Some(Box::new(move |app: &mut App| {
                            app.ed.goto_scene(i);
                            app.fit_pending = true;
                        }));
                    }
                    ui.label(RichText::new(format!("{frames} f")).small().color(Color32::from_gray(140)));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(i + 1 < n, egui::Button::new("v").small()).on_hover_text("Move down (plays later)").clicked() {
                        action = Some(Box::new(move |app: &mut App| app.ed.move_scene(i, i + 1)));
                    }
                    if ui.add_enabled(i > 0, egui::Button::new("^").small()).on_hover_text("Move up (plays earlier)").clicked() {
                        action = Some(Box::new(move |app: &mut App| app.ed.move_scene(i, i - 1)));
                    }
                });
            });
        }
        ui.separator();
        ui.horizontal(|ui| {
            if ui.button("Add").on_hover_text("Insert › Scene").clicked() {
                action = Some(Box::new(|app: &mut App| {
                    app.ed.add_scene();
                    app.fit_pending = true;
                }));
            }
            if ui.button("Duplicate").clicked() {
                let i = app.ed.scene;
                action = Some(Box::new(move |app: &mut App| app.ed.duplicate_scene(i)));
            }
            if ui.add_enabled(n > 1, egui::Button::new("Delete")).clicked() {
                let i = app.ed.scene;
                action = Some(Box::new(move |app: &mut App| app.ed.delete_scene(i)));
            }
        });
        ui.label(RichText::new(format!("The movie plays the scenes in this order: {} frames in all.", app.ed.doc.total_frames())).small().color(Color32::from_gray(140)));
        if let Some(a) = action {
            a(app);
        }
    });
    app.scene_open = open;
}

/// Modify › Shape › Expand Fill… / Soften Fill Edges… settings.
#[derive(Clone, Copy)]
pub(crate) struct ShapeDlg {
    pub soften: bool,
    pub dist: f32,
    pub steps: u32,
    pub inset: bool,
}

pub fn shape_window(app: &mut App, ctx: &egui::Context) {
    let Some(mut d) = app.shape_dlg else { return };
    let mut open = true;
    let mut ok = false;
    let mut cancel = false;
    egui::Window::new(if d.soften { "Soften Fill Edges" } else { "Expand Fill" }).open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        egui::Grid::new("shape-dlg").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Distance");
            ui.add(egui::DragValue::new(&mut d.dist).range(0.1..=200.0).speed(0.2).suffix(" px"));
            ui.end_row();
            if d.soften {
                ui.label("Number of steps");
                ui.add(egui::DragValue::new(&mut d.steps).range(1..=50));
                ui.end_row();
            }
            ui.label("Direction");
            ui.horizontal(|ui| {
                ui.radio_value(&mut d.inset, false, "Expand");
                ui.radio_value(&mut d.inset, true, "Inset");
            });
            ui.end_row();
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                ok = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    });
    open &= !cancel;
    app.shape_dlg = Some(d);
    if ok {
        if d.soften {
            app.ed.shape_fill_op("soften", d.dist, d.steps, d.inset);
        } else {
            app.ed.shape_fill_op("expand", if d.inset { -d.dist } else { d.dist }, 0, false);
        }
        app.shape_dlg = None;
    } else if !open {
        app.shape_dlg = None;
    }
}

/// Modify › Document… (Ctrl+J): Document Settings.
#[derive(Clone, Copy)]
pub(crate) struct DocDlg {
    pub w: f32,
    pub h: f32,
    pub anchor: [u8; 2],
    pub scale_content: bool,
    pub fps: f32,
    pub scale_spans: bool,
    pub bg: Color32,
}

impl DocDlg {
    pub fn of(app: &App) -> DocDlg {
        let d = &app.ed.doc;
        DocDlg { w: d.width, h: d.height, anchor: [1, 1], scale_content: false, fps: d.fps, scale_spans: false, bg: crate::c32(d.background) }
    }
}

/// Units for the size fields: (name, pixels per unit) at 72 pixels per inch.
pub(crate) const UNITS: [(&str, f32); 5] = [("pixels", 1.0), ("inches", 72.0), ("points", 1.0), ("centimetres", 72.0 / 2.54), ("millimetres", 72.0 / 25.4)];

pub fn doc_settings_window(app: &mut App, ctx: &egui::Context) {
    let Some(mut d) = app.doc_dlg else { return };
    let mut open = true;
    let (mut ok, mut cancel, mut matched) = (false, false, false);
    let unit = (app.settings.doc_units as usize).min(UNITS.len() - 1);
    egui::Window::new("Document Settings").open(&mut open).resizable(false).collapsible(false).show(ctx, |ui| {
        egui::Grid::new("docset").num_columns(2).spacing([12.0, 6.0]).show(ui, |ui| {
            ui.label("Units");
            let mut u = unit;
            egui::ComboBox::from_id_salt("docset-units").selected_text(UNITS[u].0).show_ui(ui, |ui| {
                for (i, (n, _)) in UNITS.iter().enumerate() {
                    ui.selectable_value(&mut u, i, *n);
                }
            });
            if u != unit {
                app.settings.doc_units = u as u8;
                app.settings.save();
            }
            ui.end_row();
            let k = UNITS[u].1;
            ui.label("Stage size");
            ui.horizontal(|ui| {
                let (mut w, mut h) = (d.w / k, d.h / k);
                let dec = if u == 0 || u == 2 { 0 } else { 2 };
                if ui.add(egui::DragValue::new(&mut w).range(0.01..=16000.0).max_decimals(dec).speed(0.5)).changed() {
                    d.w = (w * k).max(1.0);
                }
                ui.label("×");
                if ui.add(egui::DragValue::new(&mut h).range(0.01..=16000.0).max_decimals(dec).speed(0.5)).changed() {
                    d.h = (h * k).max(1.0);
                }
                if ui.small_button("Match Contents").on_hover_text("Fit the stage to everything in the scene").clicked() {
                    matched = true;
                }
            });
            ui.end_row();
            ui.label("");
            ui.checkbox(&mut d.scale_content, "Scale content with stage");
            ui.end_row();
            ui.label("Anchor");
            ui.add_enabled_ui(!d.scale_content, |ui| crate::registration_grid(ui, &mut d.anchor)).response.on_hover_text("When the size changes, this part of the stage keeps its content in place");
            ui.end_row();
            ui.label("Stage colour");
            ui.color_edit_button_srgba(&mut d.bg);
            ui.end_row();
            ui.label("Frame rate");
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(&mut d.fps).range(0.01..=240.0).speed(0.1).max_decimals(2));
                ui.checkbox(&mut d.scale_spans, "Scale spans").on_hover_text("Stretch every timeline so the movie keeps its length in seconds");
            });
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("OK").clicked() {
                ok = true;
            }
            if ui.button("Cancel").clicked() {
                cancel = true;
            }
        });
    });
    if matched {
        if app.ed.match_contents() {
            app.fit_pending = true;
            d.w = app.ed.doc.width;
            d.h = app.ed.doc.height;
            app.status = format!("Stage matched to the content: {} × {}", d.w, d.h);
        } else {
            app.status = "Nothing in the scene to match".into();
        }
    }
    app.doc_dlg = Some(d);
    if ok {
        app.ed.document_settings(d.w, d.h, d.anchor, d.scale_content, d.fps, d.scale_spans, crate::from32(d.bg));
        app.fit_pending = true;
        app.doc_dlg = None;
    } else if cancel || !open {
        app.doc_dlg = None;
    }
}
