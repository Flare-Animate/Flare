//! Rulers, ruler guides and the grid (View › Rulers / Grid / Guides), plus
//! the snapping they provide.

use crate::stage::Drag;
use crate::App;
use ac_core::{pt, Pt};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, Sense, Stroke};

pub const RULER: f32 = 18.0;

pub struct ViewOpts {
    pub rulers: bool,
    pub grid: bool,
    pub snap_grid: bool,
    pub grid_size: f32,
    pub grid_color: Color32,
    pub guides: bool,
    pub lock_guides: bool,
    pub snap_guides: bool,
    pub edit_grid_open: bool,
    /// View › Guides › Edit Guides…
    pub edit_guides_open: bool,
    pub guide_color: Color32,
    /// Snap accuracy in screen px (Animate: Must be close 3 / Normal 6 / Can be distant 12).
    pub snap_px: f32,
    /// View › Snapping › Snap to Pixels: drawing and moving land on whole pixels.
    pub snap_pixels: bool,
    /// View › Clip Content Outside Stage: hide what lies off the stage.
    pub clip: bool,
}

impl Default for ViewOpts {
    fn default() -> Self {
        ViewOpts {
            rulers: false,
            grid: false,
            snap_grid: false,
            grid_size: 10.0,
            grid_color: Color32::from_rgba_unmultiplied(0x80, 0x80, 0x80, 90),
            guides: true,
            lock_guides: false,
            snap_guides: true,
            edit_grid_open: false,
            edit_guides_open: false,
            guide_color: Color32::from_rgb(0x40, 0xD0, 0xE0),
            snap_px: 6.0,
            snap_pixels: false,
            clip: false,
        }
    }
}

/// Split the stage area into (rulers top, rulers left, stage).
pub fn layout(app: &App, rect: Rect) -> (Option<(Rect, Rect)>, Rect) {
    if !app.view.rulers {
        return (None, rect);
    }
    let top = Rect::from_min_max(pos2(rect.left() + RULER, rect.top()), pos2(rect.right(), rect.top() + RULER));
    let left = Rect::from_min_max(pos2(rect.left(), rect.top() + RULER), pos2(rect.left() + RULER, rect.bottom()));
    (Some((top, left)), Rect::from_min_max(rect.min + vec2(RULER, RULER), rect.max))
}

fn nice_step(zoom: f32) -> f32 {
    // label every ~70 px on screen
    let raw = 70.0 / zoom.max(0.01);
    let p = 10f32.powf(raw.log10().floor());
    for m in [1.0, 2.0, 5.0, 10.0] {
        if p * m >= raw {
            return p * m;
        }
    }
    p * 10.0
}

/// Draw the rulers and handle dragging guides out of them.
pub fn rulers(app: &mut App, ui: &mut egui::Ui, top: Rect, left: Rect, stage: Rect) {
    let p = ui.painter();
    let bg = Color32::from_gray(0x2C);
    p.rect_filled(top, 0.0, bg);
    p.rect_filled(left, 0.0, bg);
    p.rect_filled(Rect::from_min_max(pos2(left.left(), top.top()), pos2(left.right(), top.bottom())), 0.0, Color32::from_gray(0x26));
    let rotated = app.view_rot.abs() > 1e-4;
    let tick = Stroke::new(1.0_f32, Color32::from_gray(120));
    let text = Color32::from_gray(160);
    if !rotated {
        let step = nice_step(app.zoom);
        let minor = step / 5.0;
        let d0 = app.to_doc(stage, stage.min);
        let d1 = app.to_doc(stage, stage.max);
        let mut v = (d0.x / minor).floor() * minor;
        while v <= d1.x {
            let x = app.to_screen(stage, pt(v, 0.0)).x;
            let major = (v / step).round() * step == v || ((v / step) - (v / step).round()).abs() < 1e-3;
            p.line_segment([pos2(x, top.bottom() - if major { 9.0 } else { 4.0 }), pos2(x, top.bottom())], tick);
            if major {
                p.text(pos2(x + 2.0, top.top() + 1.0), Align2::LEFT_TOP, format!("{v:.0}"), FontId::monospace(9.0), text);
            }
            v += minor;
        }
        let mut v = (d0.y / minor).floor() * minor;
        while v <= d1.y {
            let y = app.to_screen(stage, pt(0.0, v)).y;
            let major = ((v / step) - (v / step).round()).abs() < 1e-3;
            p.line_segment([pos2(left.right() - if major { 9.0 } else { 4.0 }, y), pos2(left.right(), y)], tick);
            if major {
                p.text(pos2(left.left() + 1.0, y + 2.0), Align2::LEFT_TOP, format!("{v:.0}"), FontId::monospace(9.0), text);
            }
            v += minor;
        }
        // cursor position markers
        if let Some(h) = ui.input(|i| i.pointer.hover_pos()).filter(|h| stage.contains(*h)) {
            p.line_segment([pos2(h.x, top.top()), pos2(h.x, top.bottom())], Stroke::new(1.0_f32, crate::ACCENT));
            p.line_segment([pos2(left.left(), h.y), pos2(left.right(), h.y)], Stroke::new(1.0_f32, crate::ACCENT));
        }
    } else {
        p.text(top.left_center() + vec2(6.0, 0.0), Align2::LEFT_CENTER, "View rotated: rulers off", FontId::proportional(10.0), text);
    }
    // drag a new guide out of a ruler
    for (r, vertical, id) in [(top, false, "ruler-top"), (left, true, "ruler-left")] {
        let resp = ui.interact(r, ui.id().with(id), Sense::drag()).on_hover_text("Drag onto the stage to add a guide");
        if resp.drag_started() && app.view.guides && !rotated {
            app.drag = Drag::GuideNew { vertical, pos: 0.0 };
        }
        if resp.dragged() {
            if let Some(h) = resp.interact_pointer_pos() {
                let d = app.to_doc(stage, h);
                if let Drag::GuideNew { vertical: v, pos } = &mut app.drag {
                    *pos = if *v { d.x } else { d.y };
                }
            }
        }
        if resp.drag_stopped() {
            if let Drag::GuideNew { vertical: v, pos } = std::mem::replace(&mut app.drag, Drag::None) {
                if resp.interact_pointer_pos().map(|h| stage.contains(h)).unwrap_or(false) {
                    let pos = app.snap_scalar(pos, v, false);
                    app.ed.add_guide(v, pos.round());
                    app.status = format!("Guide at {} = {:.0} (drag it back onto the ruler to remove)", if v { "x" } else { "y" }, pos.round());
                }
            }
        }
    }
}

/// Grid and guides over the stage.
pub fn draw(app: &App, painter: &egui::Painter, stage: Rect) {
    if app.view.grid {
        let g = app.view.grid_size.max(1.0);
        if g * app.zoom >= 4.0 {
            let d0 = app.to_doc(stage, stage.min);
            let d1 = app.to_doc(stage, stage.max);
            let (lo, hi) = (pt(d0.x.min(d1.x), d0.y.min(d1.y)), pt(d0.x.max(d1.x), d0.y.max(d1.y)));
            let st = Stroke::new(1.0_f32, app.view.grid_color);
            let mut x = (lo.x / g).floor() * g;
            while x <= hi.x {
                painter.line_segment([app.to_screen(stage, pt(x, lo.y - g)), app.to_screen(stage, pt(x, hi.y + g))], st);
                x += g;
            }
            let mut y = (lo.y / g).floor() * g;
            while y <= hi.y {
                painter.line_segment([app.to_screen(stage, pt(lo.x - g, y)), app.to_screen(stage, pt(hi.x + g, y))], st);
                y += g;
            }
        }
    }
    if app.view.guides {
        let st = Stroke::new(1.0_f32, app.view.guide_color);
        for x in &app.ed.doc.guides.x {
            painter.line_segment([app.to_screen(stage, pt(*x, -1e5)), app.to_screen(stage, pt(*x, 1e5))], st);
        }
        for y in &app.ed.doc.guides.y {
            painter.line_segment([app.to_screen(stage, pt(-1e5, *y)), app.to_screen(stage, pt(1e5, *y))], st);
        }
        match &app.drag {
            Drag::GuideNew { vertical, pos } | Drag::Guide { vertical, pos, .. } => {
                let st = Stroke::new(1.5_f32, Color32::WHITE);
                let (a, b) = if *vertical { (pt(*pos, -1e5), pt(*pos, 1e5)) } else { (pt(-1e5, *pos), pt(1e5, *pos)) };
                painter.line_segment([app.to_screen(stage, a), app.to_screen(stage, b)], st);
                let lbl = app.to_screen(stage, if *vertical { pt(*pos, 0.0) } else { pt(0.0, *pos) });
                painter.text(pos2(lbl.x.max(stage.left()) + 4.0, lbl.y.max(stage.top()) + 4.0), Align2::LEFT_TOP, format!("{:.0}", pos), FontId::monospace(11.0), Color32::WHITE);
            }
            _ => {}
        }
    }
}

impl App {
    /// A guide under the pointer (screen px), if guides are shown and unlocked.
    pub(crate) fn guide_at(&self, stage: Rect, s: Pos2) -> Option<(bool, usize)> {
        if !self.view.guides || self.view.lock_guides {
            return None;
        }
        let g = &self.ed.doc.guides;
        for (i, x) in g.x.iter().enumerate() {
            if (self.to_screen(stage, pt(*x, 0.0)).x - s.x).abs() < 4.0 {
                return Some((true, i));
            }
        }
        for (i, y) in g.y.iter().enumerate() {
            if (self.to_screen(stage, pt(0.0, *y)).y - s.y).abs() < 4.0 {
                return Some((false, i));
            }
        }
        None
    }

    /// Snap one coordinate to guides / grid (within 6 screen px).
    pub(crate) fn snap_scalar(&self, v: f32, vertical: bool, objects_too: bool) -> f32 {
        let thr = self.view.snap_px / self.zoom;
        let mut best: Option<f32> = None;
        let mut consider = |t: f32| {
            if (t - v).abs() <= thr && best.map(|b| (t - v).abs() < (b - v).abs()).unwrap_or(true) {
                best = Some(t);
            }
        };
        if self.view.snap_guides && self.view.guides {
            for t in if vertical { &self.ed.doc.guides.x } else { &self.ed.doc.guides.y } {
                consider(*t);
            }
        }
        if self.view.snap_grid {
            let g = self.view.grid_size.max(1.0);
            consider((v / g).round() * g);
        }
        let _ = objects_too;
        best.unwrap_or(if self.view.snap_pixels { v.round() } else { v })
    }

    /// Snap a drawing point to guides and the grid.
    pub(crate) fn snap_pt(&self, p: Pt) -> Pt {
        if self.ed.editing.is_some() && !self.view.snap_grid {
            return p;
        }
        pt(self.snap_scalar(p.x, true, false), self.snap_scalar(p.y, false, false))
    }

    /// Extra snap targets for moving objects: guides and nearby grid lines.
    pub(crate) fn guide_grid_targets(&self, xs: &mut Vec<f32>, ys: &mut Vec<f32>, around: Option<(Pt, Pt)>) {
        if self.view.snap_guides && self.view.guides {
            xs.extend(self.ed.doc.guides.x.iter().copied());
            ys.extend(self.ed.doc.guides.y.iter().copied());
        }
        if let (true, Some((lo, hi))) = (self.view.snap_grid, around) {
            let g = self.view.grid_size.max(1.0);
            // grid lines in a window around the object (it can be dragged far, so be generous)
            let span = 4000.0;
            let mut x = ((lo.x - span) / g).floor() * g;
            while x <= hi.x + span {
                xs.push(x);
                x += g;
            }
            let mut y = ((lo.y - span) / g).floor() * g;
            while y <= hi.y + span {
                ys.push(y);
                y += g;
            }
        }
    }
}

/// View › Guides › Edit Guides…
pub fn edit_guides_window(app: &mut App, ctx: &egui::Context) {
    if !app.view.edit_guides_open {
        return;
    }
    let mut open = true;
    let mut clear = false;
    egui::Window::new("Guides").open(&mut open).resizable(false).show(ctx, |ui| {
        let v = &mut app.view;
        ui.horizontal(|ui| {
            ui.label("Colour");
            ui.color_edit_button_srgba(&mut v.guide_color);
        });
        ui.checkbox(&mut v.guides, "Show guides");
        ui.checkbox(&mut v.snap_guides, "Snap to guides");
        ui.checkbox(&mut v.lock_guides, "Lock guides");
        ui.horizontal(|ui| {
            ui.label("Snap accuracy");
            let name = |px: f32| if px <= 3.0 { "Must be close" } else if px >= 12.0 { "Can be distant" } else { "Normal" };
            egui::ComboBox::from_id_salt("snap-acc").selected_text(name(v.snap_px)).show_ui(ui, |ui| {
                for px in [3.0, 6.0, 12.0] {
                    ui.selectable_value(&mut v.snap_px, px, name(px));
                }
            });
        });
        ui.horizontal(|ui| {
            if ui.button("Clear All").clicked() {
                clear = true;
            }
            if ui.button("Restore defaults").clicked() {
                let d = ViewOpts::default();
                (v.guide_color, v.guides, v.snap_guides, v.lock_guides, v.snap_px) = (d.guide_color, d.guides, d.snap_guides, d.lock_guides, d.snap_px);
            }
        });
        ui.label(egui::RichText::new(format!("{} vertical, {} horizontal guide(s). Drag from a ruler to add one, drag it back to remove it.", app.ed.doc.guides.x.len(), app.ed.doc.guides.y.len())).small().color(Color32::from_gray(150)));
    });
    if clear {
        app.ed.clear_guides();
    }
    if !open {
        app.view.edit_guides_open = false;
    }
}

/// View › Grid › Edit Grid…
pub fn edit_grid_window(app: &mut App, ctx: &egui::Context) {
    if !app.view.edit_grid_open {
        return;
    }
    let mut open = true;
    egui::Window::new("Grid").open(&mut open).resizable(false).show(ctx, |ui| {
        let v = &mut app.view;
        ui.checkbox(&mut v.grid, "Show grid");
        ui.checkbox(&mut v.snap_grid, "Snap to grid");
        ui.horizontal(|ui| {
            ui.label("Spacing");
            ui.add(egui::DragValue::new(&mut v.grid_size).range(1.0..=1000.0).suffix(" px"));
        });
        ui.horizontal(|ui| {
            ui.label("Colour");
            ui.color_edit_button_srgba(&mut v.grid_color);
        });
    });
    if !open {
        app.view.edit_grid_open = false;
    }
}
