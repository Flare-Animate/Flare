//! Eases and motion tweens in the UI: the ease picker + curve editor
//! (Properties ▸ Frame), the Motion Editor window, and the editable motion
//! path drawn on the stage.

use crate::{App, ACCENT};
use ac_core::motion::{CurvePt, Ease, EaseDir, EaseFamily, Motion, Pose, Prop};
use ac_core::{pt, Pt};
use eframe::egui::{self, pos2, vec2, Align2, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke};

#[derive(Default)]
pub struct MotionUi {
    pub open: bool,
    /// Key being dragged in the Motion Editor: (property, its current frame).
    drag: Option<(Prop, u32)>,
    /// Graph value range frozen while a key is dragged (no jumping axes).
    range_lock: Option<(Prop, f32, f32)>,
    /// Properties folded away in the Motion Editor.
    folded: Vec<Prop>,
    /// Key whose Bezier handles are shown: (property, frame).
    sel: Option<(Prop, u32)>,
    /// Handle being dragged: (property, frame, out handle).
    handle_drag: Option<(Prop, u32, bool)>,
    /// Copied curve (Motion Editor ▸ Copy curve).
    clip: Option<ac_core::motion::Track>,
}

// ------------------------------------------------------------------ eases

/// Ease dropdown. Returns `Some(choice)` when the user picks something;
/// `None` inside means "simple ease" (the −100…100 slider) / no ease.
pub fn ease_combo(ui: &mut egui::Ui, id: impl std::hash::Hash, cur: Option<&Ease>, none_label: &str, simple: f32, width: f32) -> Option<Option<Ease>> {
    let text = cur.map(|e| e.name()).unwrap_or_else(|| none_label.to_string());
    let mut pick = None;
    egui::ComboBox::from_id_salt(id).width(width).selected_text(text).height(320.0).show_ui(ui, |ui| {
        if ui.selectable_label(cur.is_none(), none_label).clicked() {
            pick = Some(None);
        }
        let is_custom = matches!(cur, Some(Ease::Custom { .. }));
        if ui.selectable_label(is_custom, "Custom curve").on_hover_text("Edit the curve in the graph").clicked() && !is_custom {
            pick = Some(Some(match cur {
                Some(e) => e.to_custom(),
                None if simple != 0.0 => Ease::fit(|t| ac_core::apply_ease(t, simple)),
                None => Ease::linear_custom(),
            }));
        }
        for f in EaseFamily::ALL {
            ui.separator();
            for d in EaseDir::ALL {
                let e = Ease::preset(f, d);
                if ui.selectable_label(cur == Some(&e), e.name()).clicked() {
                    pick = Some(Some(e));
                }
            }
        }
    });
    pick
}

const G_LO: f32 = -0.3;
const G_HI: f32 = 1.3;

/// Ease curve graph. Custom curves are editable: drag anchors and handles,
/// double-click the curve to add an anchor, right-click an anchor to delete.
/// Returns the new points after an edit.
pub fn ease_graph(ui: &mut egui::Ui, id: egui::Id, ease: Option<&Ease>, simple: f32, size: egui::Vec2) -> Option<Vec<CurvePt>> {
    let (rect, bg) = ui.allocate_exact_size(size, Sense::click());
    let inner = rect.shrink(6.0);
    let to_s = |x: f32, y: f32| pos2(inner.left() + x * inner.width(), inner.bottom() - (y - G_LO) / (G_HI - G_LO) * inner.height());
    let from_s = |p: Pos2| ((p.x - inner.left()) / inner.width(), G_LO + (inner.bottom() - p.y) / inner.height() * (G_HI - G_LO));
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 3.0, Color32::from_gray(0x1E));
    for y in [0.0, 1.0] {
        p.line_segment([to_s(0.0, y), to_s(1.0, y)], Stroke::new(1.0_f32, Color32::from_gray(0x44)));
    }
    p.line_segment([to_s(0.0, 0.0), to_s(1.0, 1.0)], Stroke::new(1.0_f32, Color32::from_gray(0x38)));
    let f = |t: f32| match ease {
        Some(e) => e.apply(t),
        None => ac_core::apply_ease(t, simple),
    };
    let curve: Vec<Pos2> = (0..=80).map(|i| i as f32 / 80.0).map(|t| to_s(t, f(t).clamp(G_LO, G_HI))).collect();
    p.add(egui::Shape::line(curve, Stroke::new(2.0_f32, ACCENT)));
    p.text(rect.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, "progress", FontId::proportional(9.0), Color32::from_gray(120));
    p.text(rect.right_bottom() - vec2(4.0, 2.0), Align2::RIGHT_BOTTOM, "time", FontId::proportional(9.0), Color32::from_gray(120));

    let Some(Ease::Custom { points }) = ease else { return None };
    let mut pts = points.clone();
    let mut changed = false;
    let n = pts.len();
    let alt = ui.input(|i| i.modifiers.alt);
    let mut remove = None;
    for i in 0..n {
        let a = pts[i];
        let ap = to_s(a.x, a.y);
        for (out, h) in [(false, a.h_in), (true, a.h_out)] {
            if (!out && i == 0) || (out && i + 1 == n) {
                continue;
            }
            let hp = to_s(a.x + h.x, a.y + h.y);
            p.line_segment([ap, hp], Stroke::new(1.0_f32, Color32::from_gray(150)));
            p.circle_filled(hp, 3.5, Color32::from_gray(220));
            let r = ui.interact(Rect::from_center_size(hp, vec2(11.0, 11.0)), id.with(("h", i, out)), Sense::drag());
            if r.dragged() {
                if let Some(pp) = r.interact_pointer_pos() {
                    let (x, y) = from_s(pp);
                    let rel = pt(x - a.x, y - a.y);
                    let rel = if out { pt(rel.x.max(0.0), rel.y) } else { pt(rel.x.min(0.0), rel.y) };
                    let opp = if out { a.h_in } else { a.h_out };
                    let (len, olen) = ((rel.x * rel.x + rel.y * rel.y).sqrt(), (opp.x * opp.x + opp.y * opp.y).sqrt());
                    if out {
                        pts[i].h_out = rel;
                    } else {
                        pts[i].h_in = rel;
                    }
                    // smooth point: the other handle stays opposite (Alt breaks it)
                    if !alt && len > 1e-4 && ((out && i > 0) || (!out && i + 1 < n)) {
                        let o = pt(-rel.x / len * olen, -rel.y / len * olen);
                        if out {
                            pts[i].h_in = o;
                        } else {
                            pts[i].h_out = o;
                        }
                    }
                    changed = true;
                }
            }
        }
        let sq = Rect::from_center_size(ap, vec2(7.0, 7.0));
        p.rect_filled(sq, 0.0, Color32::WHITE);
        p.rect_stroke(sq, 0.0, Stroke::new(1.0_f32, ACCENT));
        if i == 0 || i + 1 == n {
            continue;
        }
        let r = ui.interact(sq.expand(3.0), id.with(("a", i)), Sense::click_and_drag()).on_hover_text("Drag to move · right-click to delete");
        if r.dragged() {
            if let Some(pp) = r.interact_pointer_pos() {
                let (x, y) = from_s(pp);
                pts[i].x = x.clamp(pts[i - 1].x + 0.01, pts[i + 1].x - 0.01);
                pts[i].y = y.clamp(G_LO, G_HI);
                changed = true;
            }
        }
        if r.secondary_clicked() {
            remove = Some(i);
        }
    }
    if let Some(i) = remove {
        pts.remove(i);
        changed = true;
    }
    if bg.double_clicked() {
        if let Some(pp) = bg.interact_pointer_pos() {
            let (x, _) = from_s(pp);
            if x > 0.01 && x < 0.99 {
                let y = ac_core::motion::eval_curve(&pts, x);
                let h = 1e-3;
                let s = (ac_core::motion::eval_curve(&pts, x + h) - ac_core::motion::eval_curve(&pts, x - h)) / (2.0 * h);
                let i = pts.iter().position(|q| q.x > x).unwrap_or(n - 1);
                let w = (pts[i].x - pts[i - 1].x) / 3.0;
                pts.insert(i, CurvePt { x, y, h_in: pt(-w / 2.0, -w / 2.0 * s), h_out: pt(w / 2.0, w / 2.0 * s) });
                changed = true;
            }
        }
    }
    changed.then_some(pts)
}

/// The ease controls of a tween span (Properties ▸ Frame).
pub fn span_ease_ui(app: &mut App, ui: &mut egui::Ui, curve: Option<&Ease>, simple: f32) {
    ui.horizontal(|ui| {
        ui.label("Ease");
        if let Some(choice) = ease_combo(ui, "span-ease", curve, "Simple (slider)", simple, 150.0) {
            app.ed.set_ease_curve(choice);
        }
    });
    let id = ui.id().with("span-ease-graph");
    let w = ui.available_width().min(260.0);
    if let Some(pts) = ease_graph(ui, id, curve, simple, vec2(w, w * 0.62)) {
        set_curve_live(app, ui, Ease::Custom { points: pts });
    }
    if let Some(e) = curve {
        ui.horizontal(|ui| {
            if !matches!(e, Ease::Custom { .. }) && ui.button("Edit curve").on_hover_text("Turn this preset into an editable custom curve").clicked() {
                app.ed.set_ease_curve(Some(e.to_custom()));
            }
            if ui.button("Reset").on_hover_text("Back to the simple ease").clicked() {
                app.ed.set_ease_curve(None);
            }
        });
        if matches!(e, Ease::Custom { .. }) {
            ui.label(RichText::new("Drag points and handles · double-click to add · right-click to delete · Alt breaks handles").small().color(Color32::from_gray(140)));
        }
    }
}

/// Curve edits during a drag merge into one undo step.
fn set_curve_live(app: &mut App, ui: &egui::Ui, e: Ease) {
    let down = ui.input(|i| i.pointer.any_down());
    if !app.prop_dragging {
        app.ed.begin_edit("Edit Ease Curve");
        app.prop_dragging = down;
    }
    let sel = app.ed.frame_sel;
    // set without another undo step
    let mark = app.ed.undo_depth();
    app.ed.set_ease_curve(Some(e));
    app.ed.squash_undo(mark.saturating_sub(1), "Edit Ease Curve");
    app.ed.frame_sel = sel;
    if !down {
        app.prop_dragging = false;
    }
}

// ------------------------------------------------------------ Motion Editor

enum Act {
    Frame(u32),
    Begin(&'static str),
    Value { prop: Prop, frame: u32, v: f32 },
    Move { prop: Prop, from: u32, to: u32 },
    AddKey(Prop),
    Remove(Prop, u32),
    Smooth(Prop, u32, bool),
    Ease(Prop, Option<Ease>),
    EndDrag,
    KeyAll,
    Fold(Prop),
    /// Freeze this property's graph range for the drag that starts now.
    LockRange(Prop, f32, f32),
    Select(Option<(Prop, u32)>),
    /// Move a Bezier handle of a key: (frames, value) relative to the key; `mirror` keeps the other side opposite.
    Handle { prop: Prop, frame: u32, out: bool, d: (f32, f32), mirror: bool },
    HandleDrag(Option<(Prop, u32, bool)>),
    Copy(Prop),
    Paste(Prop),
    /// OpenToonz keyframe type of the segment starting at a key.
    Interp(Prop, u32, ac_core::motion::Interp),
}

fn key_value(m: &mut Motion, prop: Prop, frame: u32, v: f32) {
    match prop {
        Prop::X | Prop::Y => {
            if let Some(k) = m.path.iter_mut().find(|k| k.frame == frame) {
                if prop == Prop::X {
                    k.at.x = v;
                } else {
                    k.at.y = v;
                }
            }
        }
        _ => {
            if let Some(t) = m.track_mut(prop) {
                t.set(frame, v);
            }
        }
    }
}

fn diamond(p: &egui::Painter, c: Pos2, r: f32, fill: Option<Color32>, stroke: Color32) {
    let pts = vec![c + vec2(0.0, -r), c + vec2(r, 0.0), c + vec2(0.0, r), c + vec2(-r, 0.0)];
    p.add(egui::Shape::convex_polygon(pts, fill.unwrap_or(Color32::TRANSPARENT), Stroke::new(1.2_f32, stroke)));
}

pub fn motion_editor(app: &mut App, ctx: &egui::Context) {
    if !app.motion.open {
        return;
    }
    let mut open = true;
    let mut acts = vec![];
    egui::Window::new("Motion Editor").open(&mut open).default_size([700.0, 470.0]).resizable(true).show(ctx, |ui| {
        acts = body(app, ui);
    });
    app.motion.open = open;
    apply(app, acts);
}

fn body(app: &App, ui: &mut egui::Ui) -> Vec<Act> {
    let mut acts = vec![];
    let Some((st, len, m, kf)) = app.ed.current_motion() else {
        ui.label(RichText::new("Put the playhead inside a motion tween (blue span) to edit its property curves.\nCreate one with Insert › Motion Tween or right-click a frame › Create Motion Tween.").color(Color32::from_gray(160)));
        return acts;
    };
    let (m, el) = (m.clone(), kf.elements.first().cloned());
    let (span_ease, simple) = (kf.ease_curve.clone(), kf.tween.ease());
    let Some(el) = el else { return acts };
    let base = Pose::of(&el, m.pivot);
    let local = app.ed.frame - st;
    let layer_name = app.ed.timeline().layers[app.ed.layer].name.clone();
    ui.horizontal(|ui| {
        ui.label(RichText::new(format!("{layer_name} · frames {}–{} · playhead {}", st + 1, st + len, app.ed.frame + 1)).color(Color32::from_gray(200)));
        if ui.button("Keyframe all at playhead (F6)").clicked() {
            acts.push(Act::KeyAll);
        }
    });
    ui.label(RichText::new("Drag keys up/down to change values, left/right to retime · click a graph to move the playhead · right-click a key for options").small().color(Color32::from_gray(140)));
    ui.separator();
    let poses: Vec<Pose> = (0..len).map(|f| m.pose_at(&base, f as f32, len, simple, span_ease.as_ref())).collect();
    // 3D / tint / filter values come from the evaluated object
    let shown: Vec<Prop> = Prop::ALL.into_iter().chain(m.extra.iter().map(|(p, _)| *p)).collect();
    let els: Vec<ac_core::Element> = if m.extra.is_empty() { vec![] } else { (0..len).filter_map(|f| ac_core::motion_elements(kf, f as f32, len, simple).into_iter().next()).collect() };
    let val = |f: usize, prop: Prop| -> f32 {
        if prop.is_extra() {
            els.get(f).and_then(|e| prop.read(e)).unwrap_or(0.0)
        } else {
            poses.get(f).copied().unwrap_or(base).get(prop)
        }
    };
    let addable: Vec<Prop> = Prop::all_for(&el).into_iter().filter(|p| p.is_extra() && !shown.contains(p)).collect();
    if !addable.is_empty() {
        ui.horizontal(|ui| {
            ui.menu_button("+ Property", |ui| {
                ui.label(RichText::new("Animate 3D, colour and filters").small().color(Color32::from_gray(150)));
                for p in &addable {
                    if ui.button(p.label()).clicked() {
                        acts.push(Act::AddKey(*p));
                        ui.close_menu();
                    }
                }
            });
            ui.label(RichText::new("Tint needs a Tint colour effect; filters come from Properties › Filters").small().color(Color32::from_gray(140)));
        });
    }
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for prop in shown {
            let folded = app.motion.folded.contains(&prop);
            let ds = prop.display_scale();
            let keys = m.keys_of(prop);
            let has_key = keys.iter().any(|(f, _)| *f == local);
            ui.horizontal(|ui| {
                ui.set_width(ui.available_width());
                // left column
                ui.vertical(|ui| {
                    ui.set_width(200.0);
                    ui.horizontal(|ui| {
                        let arrow = if folded { "+" } else { "−" };
                        if ui.small_button(arrow).clicked() {
                            acts.push(Act::Fold(prop));
                        }
                        let name = ui.add(egui::Label::new(RichText::new(prop.label()).strong()).sense(Sense::click()));
                        name.on_hover_text("Right-click: copy / paste the curve").context_menu(|ui| {
                            if ui.button("Copy curve").clicked() {
                                acts.push(Act::Copy(prop));
                                ui.close_menu();
                            }
                            let can = app.motion.clip.is_some() && !matches!(prop, Prop::X | Prop::Y);
                            if ui.add_enabled(can, egui::Button::new("Paste curve")).on_disabled_hover_text("Copy a curve first (position curves can't be pasted onto)").clicked() {
                                acts.push(Act::Paste(prop));
                                ui.close_menu();
                            }
                        });
                        // key toggle (diamond)
                        let (r, resp) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::click());
                        diamond(ui.painter(), r.center(), 5.0, has_key.then_some(Color32::from_rgb(0xF0, 0xC0, 0x40)), Color32::from_gray(200));
                        if resp.on_hover_text(if has_key { "Remove the keyframe at the playhead" } else { "Add a keyframe at the playhead" }).clicked() {
                            acts.push(if has_key { Act::Remove(prop, local) } else { Act::AddKey(prop) });
                        }
                        let mut v = val(local as usize, prop) * ds + 0.0;
                        let r = ui.add(egui::DragValue::new(&mut v).speed(0.5).max_decimals(1).suffix(prop.unit()));
                        if crate::panels::edited(&r) {
                            if r.drag_started() || (!r.dragged()) {
                                acts.push(Act::Begin("Property Value"));
                            }
                            acts.push(Act::Value { prop, frame: local, v: v / ds });
                        }
                        if r.drag_stopped() {
                            acts.push(Act::EndDrag);
                        }
                    });
                    if !folded {
                        let cur = m.ease_of(prop, None);
                        let label = if matches!(prop, Prop::X | Prop::Y) { "Position ease" } else { "Ease" };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(label).small());
                            if let Some(c) = ease_combo(ui, ("me-ease", prop), cur, "Span ease", 0.0, 110.0) {
                                acts.push(Act::Ease(prop, c));
                            }
                        });
                    }
                });
                if folded {
                    return;
                }
                // graph
                let w = ui.available_width().max(120.0);
                let (rect, bg) = ui.allocate_exact_size(vec2(w, 70.0), Sense::click_and_drag());
                let p = ui.painter_at(rect);
                p.rect_filled(rect, 2.0, Color32::from_gray(0x1E));
                let vals: Vec<f32> = (0..len as usize).map(|f| val(f, prop) * ds).collect();
                let (mut lo, mut hi) = vals.iter().chain(keys.iter().map(|(_, v)| v).map(|v| v * ds).collect::<Vec<_>>().iter()).fold((f32::MAX, f32::MIN), |(a, b), &v| (a.min(v), b.max(v)));
                if let Some((p0, l, h)) = app.motion.range_lock {
                    if p0 == prop {
                        (lo, hi) = (l, h);
                    }
                }
                if hi - lo < 1.0 {
                    let c = (hi + lo) / 2.0;
                    (lo, hi) = (c - 10.0, c + 10.0);
                }
                let pad = (hi - lo) * 0.12;
                let (lo, hi) = (lo - pad, hi + pad);
                let fx = |f: f32| rect.left() + 6.0 + f / (len.max(2) - 1) as f32 * (rect.width() - 12.0);
                let fy = |v: f32| rect.bottom() - 4.0 - (v - lo) / (hi - lo) * (rect.height() - 8.0);
                let line: Vec<Pos2> = vals.iter().enumerate().map(|(f, v)| pos2(fx(f as f32), fy(*v))).collect();
                p.add(egui::Shape::line(line, Stroke::new(1.6_f32, ACCENT)));
                p.text(rect.left_top() + vec2(4.0, 2.0), Align2::LEFT_TOP, format!("{:.0}", hi), FontId::monospace(9.0), Color32::from_gray(110));
                p.text(rect.left_bottom() + vec2(4.0, -2.0), Align2::LEFT_BOTTOM, format!("{:.0}", lo), FontId::monospace(9.0), Color32::from_gray(110));
                let px = fx(local as f32);
                p.line_segment([pos2(px, rect.top()), pos2(px, rect.bottom())], Stroke::new(1.0_f32, Color32::from_rgb(0xE5, 0x3B, 0x3B)));
                if bg.clicked() {
                    if let Some(pp) = bg.interact_pointer_pos() {
                        let f = (((pp.x - rect.left() - 6.0) / (rect.width() - 12.0)) * (len.max(2) - 1) as f32).round().clamp(0.0, (len - 1) as f32) as u32;
                        acts.push(Act::Frame(st + f));
                    }
                }
                // Bezier handles of the selected key
                let frame_px = (rect.width() - 12.0) / (len.max(2) - 1) as f32;
                let value_px = (rect.height() - 8.0) / (hi - lo);
                if let (Some((sp, sf)), Some(t)) = (app.motion.sel, m.track(prop)) {
                    if sp == prop {
                        // (handles only shape Auto segments)
                        if let Some(ki) = t.keys.iter().position(|k| k.frame == sf).filter(|&ki| t.keys[ki].interp.is_auto() || ki + 1 == t.keys.len()) {
                            let k = t.keys[ki].clone();
                            let c = pos2(fx(k.frame as f32), fy(k.value * ds));
                            for out in [false, true] {
                                if (!out && ki == 0) || (out && ki + 1 == t.keys.len()) {
                                    continue;
                                }
                                let (df, dv) = t.handle(ki, out);
                                let hp = c + vec2(df * frame_px, -dv * ds * value_px);
                                p.line_segment([c, hp], Stroke::new(1.0_f32, Color32::from_gray(170)));
                                p.circle_filled(hp, 3.5, Color32::WHITE);
                                let r = ui.interact(Rect::from_center_size(hp, vec2(10.0, 10.0)), ui.id().with(("me-h", prop, ki, out)), Sense::drag());
                                if r.drag_started() {
                                    acts.push(Act::Begin("Edit Curve Handle"));
                                    acts.push(Act::LockRange(prop, lo + pad, hi - pad));
                                    acts.push(Act::HandleDrag(Some((prop, k.frame, out))));
                                }
                                if r.dragged() {
                                    if let Some(pp) = r.interact_pointer_pos() {
                                        let df = (pp.x - c.x) / frame_px;
                                        let dv = -(pp.y - c.y) / value_px / ds;
                                        let df = if out { df.max(0.0) } else { df.min(0.0) };
                                        let mirror = !ui.input(|i| i.modifiers.alt);
                                        acts.push(Act::Handle { prop, frame: k.frame, out, d: (df, dv), mirror });
                                    }
                                }
                                if r.drag_stopped() {
                                    acts.push(Act::HandleDrag(None));
                                    acts.push(Act::EndDrag);
                                }
                            }
                        }
                    }
                }
                for (ki, (f, v)) in keys.iter().enumerate() {
                    let c = pos2(fx(*f as f32), fy(v * ds));
                    let dragging = app.motion.drag == Some((prop, *f));
                    diamond(&p, c, 5.0, Some(if dragging { Color32::WHITE } else { Color32::from_rgb(0xF0, 0xC0, 0x40) }), Color32::from_gray(30));
                    let r = ui.interact(Rect::from_center_size(c, vec2(12.0, 12.0)), ui.id().with(("me-key", prop, ki)), Sense::click_and_drag());
                    if r.drag_started() {
                        acts.push(Act::Begin("Edit Property Keyframe"));
                        acts.push(Act::LockRange(prop, lo + pad, hi - pad));
                    }
                    if r.dragged() {
                        if let Some(pp) = r.interact_pointer_pos() {
                            let to = (((pp.x - rect.left() - 6.0) / (rect.width() - 12.0)) * (len.max(2) - 1) as f32).round().clamp(0.0, (len - 1) as f32) as u32;
                            let nv = lo + (rect.bottom() - 4.0 - pp.y) / (rect.height() - 8.0) * (hi - lo);
                            let from = app.motion.drag.filter(|d| d.0 == prop).map(|d| d.1).unwrap_or(*f);
                            acts.push(Act::Move { prop, from, to });
                            acts.push(Act::Value { prop, frame: to, v: nv / ds });
                        }
                    }
                    if r.drag_stopped() {
                        acts.push(Act::EndDrag);
                    }
                    if r.clicked() && !matches!(prop, Prop::X | Prop::Y) {
                        acts.push(Act::Select(if app.motion.sel == Some((prop, *f)) { None } else { Some((prop, *f)) }));
                    }
                    let f = *f;
                    r.on_hover_text(format!("frame {} · {:.1}{}", st + f + 1, v * ds, prop.unit())).context_menu(|ui| {
                        if f != 0 && ui.button("Delete keyframe").clicked() {
                            acts.push(Act::Remove(prop, f));
                            ui.close_menu();
                        }
                        if !matches!(prop, Prop::X | Prop::Y) {
                            let smooth = m.track(prop).and_then(|t| t.keys.iter().find(|k| k.frame == f)).map(|k| k.smooth).unwrap_or(false);
                            if ui.selectable_label(smooth, "Smooth point").clicked() {
                                acts.push(Act::Smooth(prop, f, true));
                                ui.close_menu();
                            }
                            if ui.selectable_label(!smooth, "Corner point").clicked() {
                                acts.push(Act::Smooth(prop, f, false));
                                ui.close_menu();
                            }
                            if ui.button("Bezier handles").on_hover_text("Show this key's handles (or click the key); drag them to shape the curve, Alt breaks them").clicked() {
                                acts.push(Act::Select(Some((prop, f))));
                                ui.close_menu();
                            }
                            let keys_t = m.track(prop).map(|t| t.keys.clone()).unwrap_or_default();
                            if let Some(ki) = keys_t.iter().position(|k| k.frame == f).filter(|ki| ki + 1 < keys_t.len()) {
                                let seg = (keys_t[ki + 1].frame - f) as f32;
                                let cur = keys_t[ki].interp.index();
                                ui.menu_button("Interpolation to next key", |ui| {
                                    for (i, name) in ac_core::motion::Interp::NAMES.iter().enumerate() {
                                        if ui.selectable_label(cur == i, *name).clicked() {
                                            acts.push(Act::Interp(prop, f, ac_core::motion::Interp::of_index(i, seg)));
                                            acts.push(Act::Select(Some((prop, f))));
                                            ui.close_menu();
                                        }
                                    }
                                });
                            }
                        }
                    });
                }
            });
            // the selected key's segment type (OpenToonz keyframe types), under the graph
            if let (Some((sp, sf)), Some(t)) = (app.motion.sel, m.track(prop)) {
                if let Some(ki) = t.keys.iter().position(|k| k.frame == sf).filter(|ki| sp == prop && ki + 1 < t.keys.len()) {
                    segment_ui(ui, &mut acts, prop, &t.keys[ki], (t.keys[ki + 1].frame - sf) as f32);
                }
            }
            ui.separator();
        }
    });
    acts
}

/// Type and parameters of the segment that starts at `key`.
fn segment_ui(ui: &mut egui::Ui, acts: &mut Vec<Act>, prop: Prop, key: &ac_core::motion::PropKey, seg: f32) {
    use ac_core::motion::Interp;
    let f = key.frame;
    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(format!("Key {} to next:", f + 1)).small());
        let cur = key.interp.index();
        let mut pick = cur;
        egui::ComboBox::from_id_salt(("interp", prop, f)).width(150.0).selected_text(Interp::NAMES[cur]).show_ui(ui, |ui| {
            for (i, n) in Interp::NAMES.iter().enumerate() {
                ui.selectable_value(&mut pick, i, *n);
            }
        });
        if pick != cur {
            acts.push(Act::Interp(prop, f, Interp::of_index(pick, seg)));
        }
        match key.interp.clone() {
            Interp::EaseInOut { mut ease_in, mut ease_out } | Interp::EaseInOutPct { mut ease_in, mut ease_out } => {
                let pct = matches!(key.interp, Interp::EaseInOutPct { .. });
                let max = if pct { 100.0 } else { seg };
                let suffix = if pct { " %" } else { " fr" };
                let a = ui.add(egui::DragValue::new(&mut ease_in).range(0.0..=max).speed(0.2).suffix(suffix)).on_hover_text("Ease in");
                let b = ui.add(egui::DragValue::new(&mut ease_out).range(0.0..=max).speed(0.2).suffix(suffix)).on_hover_text("Ease out");
                if a.changed() || b.changed() {
                    acts.push(Act::Interp(prop, f, if pct { Interp::EaseInOutPct { ease_in, ease_out } } else { Interp::EaseInOut { ease_in, ease_out } }));
                }
            }
            Interp::Expression { text } | Interp::SimilarShape { text, .. } => {
                let offset = if let Interp::SimilarShape { offset, .. } = key.interp { Some(offset) } else { None };
                let id = ui.id().with(("expr-buf", prop, f));
                let mut buf: String = ui.data_mut(|d| d.get_temp::<String>(id)).unwrap_or_else(|| text.clone());
                let props = ac_core::motion::expr_props();
                let names: Vec<&str> = props.iter().map(|s| s.as_str()).collect();
                let err = ac_core::expr::Expr::parse(&buf, &names).err();
                let r = ui.add(egui::TextEdit::singleline(&mut buf).desired_width(220.0).font(egui::TextStyle::Monospace).text_color(if err.is_some() { Color32::from_rgb(0xFF, 0x80, 0x80) } else { Color32::from_gray(230) }))
                    .on_hover_text("OpenToonz expression syntax. Variables: t (0–1 across the segment), frame, rframe, v0, v1, and other properties by name (rotation, alpha, x …, also rotation(frame − 3)). Functions: sin cos (degrees), wave, saw, pulse, random, smoothstep, clamp, cycle …");
                ui.data_mut(|d| d.insert_temp(id, buf.clone()));
                let mut off = offset.unwrap_or(0.0);
                let ro = offset.map(|_| ui.add(egui::DragValue::new(&mut off).speed(0.2).prefix("offset ")));
                let commit = r.lost_focus() && err.is_none() && buf != text;
                if commit || ro.is_some_and(|r| r.changed()) {
                    acts.push(Act::Interp(prop, f, match offset {
                        Some(_) => Interp::SimilarShape { text: buf.clone(), offset: off },
                        None => Interp::Expression { text: buf.clone() },
                    }));
                }
                if let Some(e) = err {
                    ui.label(RichText::new(e).small().color(Color32::from_rgb(0xFF, 0x80, 0x80)));
                }
            }
            _ => {}
        }
    });
}

fn apply(app: &mut App, acts: Vec<Act>) {
    for a in acts {
        match a {
            Act::Begin(l) => {
                if app.motion.drag.is_none() && !app.prop_dragging {
                    app.ed.begin_edit(l);
                    app.prop_dragging = true;
                }
            }
            Act::Fold(prop) => {
                if let Some(i) = app.motion.folded.iter().position(|p| *p == prop) {
                    app.motion.folded.remove(i);
                } else {
                    app.motion.folded.push(prop);
                }
            }
            Act::LockRange(prop, lo, hi) => app.motion.range_lock = Some((prop, lo, hi)),
            Act::Frame(f) => {
                app.playing = false;
                app.ed.set_frame(f);
            }
            Act::Value { prop, frame, v } => {
                let (st, _) = match app.ed.motion_span(app.ed.layer, app.ed.frame) {
                    Some(x) => x,
                    None => continue,
                };
                if app.motion.drag.is_some() {
                    app.ed.edit_motion(None, |m, _| key_value(m, prop, frame, v));
                    app.motion.drag = Some((prop, frame));
                } else {
                    app.ed.set_motion_value(prop, st + frame, v, None);
                }
            }
            Act::Move { prop, from, to } => {
                if app.motion.drag.is_none() {
                    app.motion.drag = Some((prop, from));
                }
                let mut now = from;
                app.ed.edit_motion(None, |m, _| now = m.move_key(prop, from, to));
                app.motion.drag = Some((prop, now));
            }
            Act::AddKey(prop) => {
                if let Some((st, len, m, kf)) = app.ed.current_motion() {
                    let base = Pose::of(&kf.elements[0], m.pivot);
                    let local = (app.ed.frame - st) as f32;
                    let v = if prop.is_extra() {
                        ac_core::motion_elements(kf, local, len, kf.tween.ease()).first().and_then(|e| prop.read(e)).unwrap_or(0.0)
                    } else {
                        m.pose_at(&base, local, len, kf.tween.ease(), kf.ease_curve.as_ref()).get(prop)
                    };
                    let f = app.ed.frame;
                    app.ed.set_motion_value(prop, f, v, Some("Add Property Keyframe"));
                }
            }
            Act::Remove(prop, f) => app.ed.edit_motion(Some("Remove Property Keyframe"), |m, _| {
                m.remove_value(prop, f);
            }),
            Act::Smooth(prop, f, s) => app.ed.edit_motion(Some(if s { "Smooth Point" } else { "Corner Point" }), |m, _| m.set_smooth(prop, f, s)),
            Act::Ease(prop, e) => app.ed.edit_motion(Some("Property Ease"), |m, _| m.set_ease(prop, e)),
            Act::EndDrag => {
                app.motion.drag = None;
                app.motion.range_lock = None;
                app.prop_dragging = false;
            }
            Act::KeyAll => {
                let (l, f) = (app.ed.layer, app.ed.frame);
                app.ed.key_all_properties(l, f);
            }
            Act::Select(s) => app.motion.sel = s,
            Act::Interp(prop, f, interp) => {
                let label = format!("Interpolation: {}", ac_core::motion::Interp::NAMES[interp.index()]);
                app.ed.edit_motion(Some(&label), |m, _| {
                    if let Some(k) = m.track_mut(prop).and_then(|t| t.keys.iter_mut().find(|k| k.frame == f)) {
                        k.interp = interp;
                    }
                });
            }
            Act::HandleDrag(h) => app.motion.handle_drag = h,
            Act::Handle { prop, frame, out, d, mirror } => {
                app.ed.edit_motion(None, |m, _| {
                    let Some(t) = m.track_mut(prop) else { return };
                    let Some(ki) = t.keys.iter().position(|k| k.frame == frame) else { return };
                    let (mut hin, mut hout) = (t.handle(ki, false), t.handle(ki, true));
                    if out {
                        hout = d;
                        if mirror {
                            // keep the in-handle opposite, with its own length in frames
                            let len_in = hin.0.abs().max(0.01);
                            let k = len_in / d.0.abs().max(0.01);
                            hin = (-d.0.abs() * k, -d.1 * k);
                        }
                    } else {
                        hin = d;
                        if mirror {
                            let len_out = hout.0.abs().max(0.01);
                            let k = len_out / d.0.abs().max(0.01);
                            hout = (d.0.abs() * k, -d.1 * k);
                        }
                    }
                    t.set_handles(frame, Some([hin.0, hin.1, hout.0, hout.1]));
                });
            }
            Act::Copy(prop) => {
                if let Some((_, _, m, _)) = app.ed.current_motion() {
                    app.motion.clip = Some(m.curve(prop));
                    app.status = format!("Copied the {} curve", prop.label());
                }
            }
            Act::Paste(prop) => {
                if let (Some(t), Some((_, len, ..))) = (app.motion.clip.clone(), app.ed.current_motion()) {
                    app.ed.edit_motion(Some("Paste Curve"), |m, _| {
                        m.paste_curve(prop, t, len);
                    });
                }
            }
        }
    }
    if app.prop_dragging && app.motion.drag.is_none() && !app.ctx.input(|i| i.pointer.any_down()) {
        app.prop_dragging = false;
    }
}

// ------------------------------------------------------------ stage motion path

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathGrab {
    Anchor(usize),
    Bend(usize, f32),
    Handle(usize, bool),
}

/// The motion tween on the current layer at the playhead, with what the stage needs.
fn stage_motion(app: &App) -> Option<(u32, u32, Motion, Pose, f32, Option<Ease>)> {
    let (st, len, m, kf) = app.ed.current_motion()?;
    let base = Pose::of(kf.elements.first()?, m.pivot);
    Some((st, len, m.clone(), base, kf.tween.ease(), kf.ease_curve.clone()))
}

/// What the pointer at `pos` grabs on the motion path (Selection: anchors,
/// path; Subselection: anchors and Bezier handles).
pub fn path_grab(app: &App, rect: Rect, pos: Pos2, handles: bool) -> Option<PathGrab> {
    let (_, _, m, ..) = stage_motion(app)?;
    if m.path.len() < 2 {
        return None;
    }
    if handles {
        for (i, k) in m.path.iter().enumerate() {
            for (out, h) in [(false, k.h_in), (true, k.h_out)] {
                if (!out && i == 0) || (out && i + 1 == m.path.len()) {
                    continue;
                }
                if app.to_screen(rect, pt(k.at.x + h.x, k.at.y + h.y)).distance(pos) < 7.0 {
                    return Some(PathGrab::Handle(i, out));
                }
            }
        }
    }
    if let Some(i) = m.path.iter().position(|k| app.to_screen(rect, k.at).distance(pos) < 7.0) {
        return Some(PathGrab::Anchor(i));
    }
    let p = app.to_doc(rect, pos);
    let (seg, u, d) = m.nearest(p)?;
    (d * app.zoom < 5.0 && u > 0.02 && u < 0.98).then_some(PathGrab::Bend(seg, u))
}

pub fn path_drag(app: &mut App, grab: PathGrab, p: Pt, started: &mut bool, alt: bool) {
    let label = if *started {
        None
    } else {
        Some(match grab {
            PathGrab::Anchor(_) => "Move Path Point",
            PathGrab::Bend(..) => "Bend Motion Path",
            PathGrab::Handle(..) => "Path Handle",
        })
    };
    *started = true;
    app.ed.edit_motion(label, |m, _| match grab {
        PathGrab::Anchor(i) => {
            if let Some(k) = m.path.get_mut(i) {
                k.at = p;
            }
        }
        PathGrab::Bend(seg, u) => m.bend(seg, u, p),
        PathGrab::Handle(i, out) => {
            if let Some(k) = m.path.get_mut(i) {
                let rel = pt(p.x - k.at.x, p.y - k.at.y);
                let opp = if out { k.h_in } else { k.h_out };
                let olen = (opp.x * opp.x + opp.y * opp.y).sqrt();
                let len = (rel.x * rel.x + rel.y * rel.y).sqrt();
                if out {
                    k.h_out = rel;
                } else {
                    k.h_in = rel;
                }
                if !alt && len > 1e-3 {
                    let o = pt(-rel.x / len * olen.max(len), -rel.y / len * olen.max(len));
                    if out {
                        k.h_in = o;
                    } else {
                        k.h_out = o;
                    }
                }
            }
        }
    });
}

/// Draw the motion path: the curve, a dot per frame, squares at position keys
/// (and their handles with the Subselection tool).
pub fn draw_path(app: &App, painter: &egui::Painter, rect: Rect, handles: bool) {
    let Some((st, len, m, base, simple, ease)) = stage_motion(app) else { return };
    let col = Color32::from_rgb(0x3F, 0x9C, 0xF0);
    if m.path.len() >= 2 {
        let line: Vec<Pos2> = m.path_polyline(24).into_iter().map(|q| app.to_screen(rect, q)).collect();
        painter.add(egui::Shape::line(line, Stroke::new(1.0_f32, col)));
    }
    for f in 0..len {
        let q = m.pose_at(&base, f as f32, len, simple, ease.as_ref()).pos;
        let s = app.to_screen(rect, q);
        if st + f == app.ed.frame {
            painter.circle_stroke(s, 4.0, Stroke::new(1.5_f32, Color32::from_rgb(0xE5, 0x3B, 0x3B)));
        } else {
            painter.circle_filled(s, 1.6, col);
        }
    }
    for (i, k) in m.path.iter().enumerate() {
        let a = app.to_screen(rect, k.at);
        if handles {
            for (out, h) in [(false, k.h_in), (true, k.h_out)] {
                if (!out && i == 0) || (out && i + 1 == m.path.len()) {
                    continue;
                }
                let hp = app.to_screen(rect, pt(k.at.x + h.x, k.at.y + h.y));
                painter.line_segment([a, hp], Stroke::new(1.0_f32, col.gamma_multiply(0.7)));
                painter.circle_filled(hp, 3.0, col);
            }
        }
        let r = Rect::from_center_size(a, vec2(6.0, 6.0));
        painter.rect_filled(r, 0.0, Color32::WHITE);
        painter.rect_stroke(r, 0.0, Stroke::new(1.0_f32, col));
    }
}

// ------------------------------------------------------------ shape hints

/// Hints shown on the stage: the shape tween keyframe under the playhead
/// (start or end), its first shape's matrix and outline, and whether it's the start.
/// (keyframe, is start, shape matrix, outline polylines, hint points on stage)
type HintCtx = (u32, bool, ac_core::Matrix, Vec<Vec<Pt>>, Vec<Pt>);

fn hint_ctx(app: &App) -> Option<HintCtx> {
    let (a, b) = app.ed.shape_tween_keys()?;
    let key = if app.ed.frame == a { a } else if app.ed.frame == b { b } else { return None };
    let kf = app.ed.timeline().layers.get(app.ed.layer)?.keyframes.get(&key)?;
    let el = kf.elements.first()?;
    let ac_core::ElementKind::Shape(sh) = &el.kind else { return None };
    let outline = sh.path.transformed(&el.matrix).flatten(16).into_iter().map(|(mut p, closed)| {
        if closed && !p.is_empty() {
            p.push(p[0]);
        }
        p
    });
    let hints = kf.shape_hints.iter().map(|h| el.matrix.apply(*h)).collect();
    Some((key, key == a, el.matrix, outline.collect(), hints))
}

fn nearest_on(outline: &[Vec<Pt>], p: Pt) -> Option<(Pt, f32)> {
    let mut best: Option<(Pt, f32)> = None;
    for poly in outline {
        for w in poly.windows(2) {
            let (a, b) = (w[0], w[1]);
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let l2 = dx * dx + dy * dy;
            let t = if l2 < 1e-9 { 0.0 } else { (((p.x - a.x) * dx + (p.y - a.y) * dy) / l2).clamp(0.0, 1.0) };
            let q = pt(a.x + dx * t, a.y + dy * t);
            let d = q.dist(p);
            if best.map(|x| d < x.1).unwrap_or(true) {
                best = Some((q, d));
            }
        }
    }
    best
}

pub fn draw_hints(app: &App, painter: &egui::Painter, rect: Rect) {
    let Some((_, start, _, outline, hints)) = hint_ctx(app) else { return };
    for (i, h) in hints.iter().enumerate() {
        let on = nearest_on(&outline, *h).map(|(_, d)| d < 1.5).unwrap_or(false);
        let fill = match (on, start) {
            (false, _) => Color32::from_rgb(0xE0, 0x40, 0x40),
            (true, true) => Color32::from_rgb(0xF2, 0xD0, 0x30),
            (true, false) => Color32::from_rgb(0x50, 0xC8, 0x50),
        };
        let c = app.to_screen(rect, *h);
        painter.circle(c, 8.0, fill, Stroke::new(1.0_f32, Color32::from_gray(30)));
        painter.text(c, Align2::CENTER_CENTER, ((b'a' + i as u8) as char).to_string(), FontId::proportional(11.0), Color32::BLACK);
    }
}

/// A hint under the pointer: (keyframe, hint index).
pub fn hint_at(app: &App, rect: Rect, pos: Pos2) -> Option<(u32, usize)> {
    let (key, _, _, _, hints) = hint_ctx(app)?;
    hints.iter().rposition(|h| app.to_screen(rect, *h).distance(pos) < 9.0).map(|i| (key, i))
}

/// Drag a hint; it snaps onto the shape's outline.
pub fn hint_drag(app: &mut App, key: u32, i: usize, p: Pt, started: &mut bool) {
    let Some((_, _, m, outline, _)) = hint_ctx(app) else { return };
    let snap = nearest_on(&outline, p).filter(|(_, d)| *d * app.zoom < 14.0).map(|(q, _)| q).unwrap_or(p);
    let Some(inv) = m.invert() else { return };
    app.ed.move_shape_hint(key, i, inv.apply(snap), if *started { None } else { Some("Move Shape Hint") });
    *started = true;
}
