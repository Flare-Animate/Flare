//! Navigator (Window › Navigator; after Flare Studio's Navigator, our own
//! code): a small picture of the current frame with the visible part of the
//! stage outlined. Click or drag in it to scroll there; the slider zooms.

use crate::App;
use ac_core::{pt, Matrix};
use eframe::egui::{self, pos2, vec2, Color32, Rect, Sense, Stroke};
use tiny_skia as sk;

#[derive(Default)]
pub(crate) struct Nav {
    pub open: bool,
    tex: Option<egui::TextureHandle>,
    /// What the picture shows: (revision, frame, scene, editing, size).
    key: Option<(u64, u32, usize, bool, u32, u32)>,
}

const WIDTH: f32 = 220.0;

/// Stage box shown in the navigator: the stage plus a margin of pasteboard.
fn area(w: f32, h: f32) -> (ac_core::Pt, ac_core::Pt) {
    let m = 0.15 * w.max(h);
    (pt(-m, -m), pt(w + m, h + m))
}

pub(crate) fn window(app: &mut App, ctx: &egui::Context) {
    if !app.nav.open {
        return;
    }
    let mut open = true;
    let (w, h) = (app.ed.doc.width.max(1.0), app.ed.doc.height.max(1.0));
    let (lo, hi) = area(w, h);
    let s = WIDTH / (hi.x - lo.x);
    let size = vec2(WIDTH, ((hi.y - lo.y) * s).round());
    let key = (app.ed.revision, app.ed.frame, app.ed.scene, app.ed.editing.is_some(), size.x as u32, size.y as u32);
    if app.nav.key != Some(key) || app.nav.tex.is_none() {
        let ppp = ctx.pixels_per_point();
        let (pw, ph) = ((size.x * ppp) as u32, (size.y * ppp) as u32);
        if let Some(mut pix) = sk::Pixmap::new(pw.max(1), ph.max(1)) {
            pix.fill(sk::Color::from_rgba8(0x3C, 0x3C, 0x3C, 255));
            let view = Matrix::scale(s * ppp, s * ppp).concat(&Matrix::translate(-lo.x, -lo.y));
            let bg = app.ed.doc.background;
            if let Some(r) = sk::Rect::from_xywh(0.0, 0.0, w, h) {
                let mut paint = sk::Paint::default();
                paint.set_color(sk::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
                pix.fill_rect(r, &paint, sk::Transform::from_row(view.a, view.b, view.c, view.d, view.tx, view.ty), None);
            }
            let items = app.ed.evaluate_current(app.ed.frame);
            ac_render::draw_items(&mut pix.as_mut(), &items, &ac_render::RenderOpts { view: view.concat(&app.content_matrix()), ..Default::default() });
            let img = egui::ColorImage::from_rgba_premultiplied([pw.max(1) as usize, ph.max(1) as usize], pix.data());
            match &mut app.nav.tex {
                Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                None => app.nav.tex = Some(ctx.load_texture("navigator", img, egui::TextureOptions::LINEAR)),
            }
            app.nav.key = Some(key);
        }
    }
    egui::Window::new("Navigator").open(&mut open).resizable(false).default_pos(app.stage_rect.right_bottom() - vec2(WIDTH + 30.0, size.y + 110.0)).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(size, Sense::click_and_drag());
        if let Some(t) = &app.nav.tex {
            ui.painter().image(t.id(), r, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
        }
        let to_nav = |p: ac_core::Pt| pos2(r.left() + (p.x - lo.x) * s, r.top() + (p.y - lo.y) * s);
        // the part of the stage on screen (a quad when the view is rotated)
        let sr = app.stage_rect;
        let quad: Vec<egui::Pos2> = [sr.left_top(), sr.right_top(), sr.right_bottom(), sr.left_bottom()].iter().map(|c| to_nav(app.screen_to_stage(sr, *c))).collect();
        let red = Color32::from_rgb(0xFF, 0x50, 0x40);
        let painter = ui.painter_at(r);
        painter.add(egui::Shape::closed_line(quad, Stroke::new(1.5_f32, red)));
        if let Some(p) = resp.interact_pointer_pos().filter(|_| resp.clicked() || resp.dragged()) {
            let target = pt(lo.x + (p.x - r.left()) / s, lo.y + (p.y - r.top()) / s);
            app.center_on(sr, target);
        }
        resp.on_hover_cursor(egui::CursorIcon::Crosshair).on_hover_text("Click or drag to scroll the stage there");
        ui.horizontal(|ui| {
            let mut pct = app.zoom * 100.0;
            let r = ui.add(egui::Slider::new(&mut pct, 5.0..=3200.0).logarithmic(true).suffix("%").max_decimals(0));
            if r.changed() {
                // keep the middle of the view where it is
                let c = app.screen_to_stage(sr, sr.center());
                app.zoom = (pct / 100.0).clamp(0.05, 32.0);
                app.center_on(sr, c);
            }
        });
        ui.horizontal(|ui| {
            if ui.button("Fit").clicked() {
                app.fit_pending = true;
            }
            if ui.button("100%").clicked() {
                let c = app.screen_to_stage(sr, sr.center());
                app.zoom = 1.0;
                app.center_on(sr, c);
            }
            if ui.button("Centre").on_hover_text("Scroll to the middle of the stage").clicked() {
                app.center_on(sr, pt(w / 2.0, h / 2.0));
            }
        });
    });
    if !open {
        app.nav.open = false;
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn area_keeps_aspect_and_surrounds_the_stage() {
        let (lo, hi) = super::area(550.0, 400.0);
        assert!(lo.x < 0.0 && lo.y < 0.0 && hi.x > 550.0 && hi.y > 400.0);
        assert!(((hi.x - lo.x) - (hi.y - lo.y) - 150.0).abs() < 1e-3);
    }
}
