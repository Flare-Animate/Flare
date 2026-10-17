//! The Align, Transform and Info panels (Window > Align Ctrl+K, Transform
//! Ctrl+T, Info Ctrl+I), which open as flyouts from the collapsed icon strip
//! (panels.rs), and Modify > Transform > Scale and Rotate.

use eframe::egui::{self, Color32, DragValue, Id, Key, Order, Painter, Rect, Response, RichText, Sense, Stroke, Ui, pos2, vec2};
use minnamate_core::arrange::Align;
use minnamate_core::doc::{Element, Matrix, Point};
use minnamate_core::hit;
use minnamate_core::render::render_timeline;

use crate::app::{Action, Editor};
use crate::icons::{self, Icon};
use crate::{free_transform, theme};

/// The Scale and Rotate dialog, while open.
pub fn show(ctx: &egui::Context, ed: &mut Editor) {
    scale_and_rotate(ctx, ed);
}

fn caption(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(theme::TEXT_DIM));
}

/// Align, Distribute, Match size and Space buttons, and Align to stage.
pub fn align(ui: &mut Ui, ed: &mut Editor) {
    use Align::*;
    let enabled = !ed.selection.is_empty();
    let mut picked = None;
    let mut buttons = |ui: &mut Ui, row: &[(Align, &str)]| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for &(how, tip) in row {
                if theme::icon_button(ui, 26.0, false, enabled, tip, |p, r, c| glyph(p, r.shrink(5.0), how, c)).clicked() {
                    picked = Some(how);
                }
            }
        });
    };
    caption(ui, "Align:");
    buttons(ui, &[
        (Edges { y: false, at: 0.0 }, "Align left edge"),
        (Edges { y: false, at: 0.5 }, "Align horizontal center"),
        (Edges { y: false, at: 1.0 }, "Align right edge"),
        (Edges { y: true, at: 0.0 }, "Align top edge"),
        (Edges { y: true, at: 0.5 }, "Align vertical center"),
        (Edges { y: true, at: 1.0 }, "Align bottom edge"),
    ]);
    caption(ui, "Distribute:");
    buttons(ui, &[
        (Distribute { y: true, at: 0.0 }, "Distribute top edge"),
        (Distribute { y: true, at: 0.5 }, "Distribute vertical center"),
        (Distribute { y: true, at: 1.0 }, "Distribute bottom edge"),
        (Distribute { y: false, at: 0.0 }, "Distribute left edge"),
        (Distribute { y: false, at: 0.5 }, "Distribute horizontal center"),
        (Distribute { y: false, at: 1.0 }, "Distribute right edge"),
    ]);
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            caption(ui, "Match size:");
            buttons(ui, &[
                (Match { width: true, height: false }, "Match width"),
                (Match { width: false, height: true }, "Match height"),
                (Match { width: true, height: true }, "Match width and height"),
            ]);
        });
        ui.add_space(24.0);
        ui.vertical(|ui| {
            caption(ui, "Space:");
            buttons(ui, &[(Space { y: true }, "Space evenly vertically"), (Space { y: false }, "Space evenly horizontally")]);
        });
    });
    ui.add_space(2.0);
    ui.checkbox(&mut ed.align_to_stage, "Align to stage");
    if let Some(how) = picked {
        ed.run(Action::Align(how), ui.ctx());
    }
}

/// Our own glyphs for the Align panel, on a 16-unit grid: bars against guide
/// lines, drawn across the stage and turned for the `y` variants.
fn glyph(p: &Painter, r: Rect, how: Align, c: Color32) {
    let (turned, bars, guides): (bool, Vec<[f32; 4]>, Vec<f32>) = match how {
        Align::Edges { y, at } => {
            let guide = 1.0 + 14.0 * at;
            let bar = |len: f32, top: f32| [guide - at * len, top, guide + (1.0 - at) * len, top + 4.0];
            (y, vec![bar(10.0, 2.0), bar(6.0, 10.0)], vec![guide])
        }
        Align::Distribute { y, at } => {
            let bars = [[1.0, 5.0, 5.0, 11.0], [9.0, 3.0, 15.0, 13.0]];
            (y, bars.to_vec(), bars.iter().map(|b| b[0] + (b[2] - b[0]) * at).collect())
        }
        Align::Space { y } => (y, vec![[1.0, 4.0, 4.0, 12.0], [6.5, 4.0, 9.5, 12.0], [12.0, 4.0, 15.0, 12.0]], Vec::new()),
        Align::Match { width: true, height: true } => (false, vec![[3.0, 3.0, 13.0, 13.0]], vec![1.0, 15.0]),
        Align::Match { height, .. } => (height, vec![[2.0, 3.0, 14.0, 6.0], [2.0, 10.0, 14.0, 13.0]], vec![1.0, 15.0]),
    };
    let s = r.width() / 16.0;
    let at = |u: f32, v: f32| if turned { pos2(r.left() + v * s, r.top() + u * s) } else { pos2(r.left() + u * s, r.top() + v * s) };
    for [u0, v0, u1, v1] in bars {
        p.rect_filled(Rect::from_two_pos(at(u0, v0), at(u1, v1)), 0.0, c);
    }
    for u in guides {
        p.line_segment([at(u, 0.0), at(u, 16.0)], Stroke::new(1.0, c));
    }
}

/// The selection's scale, rotation and skew as numbers, the flips, and Remove Transform.
pub fn transform(ui: &mut Ui, ed: &mut Editor) {
    let enabled = !ed.selection.is_empty();
    // A lone shape, instance or text shows its own numbers; a wider selection starts at 100% and 0°.
    let lone = match ed.selection[..] {
        [r] => match ed.element(r) {
            // A motion tween's target, as it shows now.
            Some(Element::Instance(_) | Element::Text(_)) if let Some(m) = crate::tweens::shown_matrix(ed, r) => Some(m),
            Some(Element::Shape(s)) => Some(s.matrix),
            Some(Element::Instance(i)) => Some(i.matrix),
            Some(Element::Text(t)) => Some(t.matrix),
            Some(Element::Bitmap(b)) => Some(b.matrix),
            Some(Element::Group(_) | Element::Unsupported { .. }) | None => None,
        },
        _ => None,
    };
    let old = lone.unwrap_or(Matrix::IDENTITY).parts();
    let (mut sx, mut sy, mut kx, mut ky) = (old.0 * 100.0, old.1 * 100.0, old.2.to_degrees(), old.3.to_degrees());
    let id = Id::new("transform-panel");
    let (mut linked, mut skewing) = ui.data(|d| d.get_temp::<(bool, bool)>(id)).unwrap_or_default();
    let mut fields: Vec<Response> = Vec::new();
    let (mut rotated, mut resized) = (None, None);
    ui.add_enabled_ui(enabled, |ui| {
        ui.horizontal(|ui| {
            ui.label(RichText::new("↔").color(theme::TEXT_DIM));
            fields.push(ui.add(DragValue::new(&mut sx).speed(0.5).fixed_decimals(1).suffix(" %")));
            ui.add_space(6.0);
            ui.label(RichText::new("↕").color(theme::TEXT_DIM));
            fields.push(ui.add(DragValue::new(&mut sy).speed(0.5).fixed_decimals(1).suffix(" %")));
            if theme::icon_button(ui, 20.0, linked, true, "Constrain", |p, r, c| chain(p, r, c)).clicked() {
                linked = !linked;
            }
        });
        resized = Some((fields[0].changed(), fields[1].changed()));
        ui.add_space(4.0);
        ui.radio_value(&mut skewing, false, "Rotate");
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            let mut turn = ky;
            let field = ui.add_enabled(!skewing, DragValue::new(&mut turn).speed(0.5).fixed_decimals(1).suffix("°"));
            rotated = field.changed().then_some(turn - ky);
            fields.push(field);
        });
        ui.radio_value(&mut skewing, true, "Skew");
        ui.horizontal(|ui| {
            ui.add_space(20.0);
            fields.push(ui.add_enabled(skewing, DragValue::new(&mut kx).speed(0.5).fixed_decimals(1).suffix("°")));
            ui.add_space(6.0);
            fields.push(ui.add_enabled(skewing, DragValue::new(&mut ky).speed(0.5).fixed_decimals(1).suffix("°")));
        });
    });
    // No 3D here: Animate's 3D rows show, greyed.
    ui.add_enabled_ui(false, |ui| {
        for title in ["3D Rotation", "3D Center point"] {
            caption(ui, title);
            ui.horizontal(|ui| {
                for axis in ["X:", "Y:", "Z:"] {
                    ui.label(axis);
                    ui.add(DragValue::new(&mut 0.0f32).fixed_decimals(1));
                }
            });
        }
    });
    ui.separator();
    ui.horizontal(|ui| {
        let button = |ui: &mut Ui, tip: &str, on: bool, glyph: fn(&Painter, Rect, Color32)| {
            theme::icon_button(ui, 24.0, false, on, tip, |p, r, c| glyph(p, r.shrink(5.0), c)).clicked()
        };
        if button(ui, "Flip Horizontal", enabled, |p, r, c| flip(p, r, c, false)) {
            ed.run(Action::FlipHorizontal, ui.ctx());
        }
        if button(ui, "Flip Vertical", enabled, |p, r, c| flip(p, r, c, true)) {
            ed.run(Action::FlipVertical, ui.ctx());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if button(ui, "Remove Transform", enabled, |p, r, c| icons::paint(p, r, Icon::Reset, c)) {
                ed.run(Action::RemoveTransform, ui.ctx());
            }
            // ponytail: this transforms a copy by the panel's numbers in Animate; here the numbers
            // apply as they're typed, so it stays off.
            button(ui, "Duplicate Selection and Transform", false, |p, r, c| {
                p.rect_stroke(r.shrink(2.0).translate(vec2(-2.0, -2.0)), 0.0, Stroke::new(1.0, c), egui::StrokeKind::Inside);
                p.rect_filled(r.shrink(2.0).translate(vec2(2.0, 2.0)), 0.0, c);
            });
        });
    });
    ui.data_mut(|d| d.insert_temp(id, (linked, skewing)));

    // A change is the turn from the old numbers to the new ones, about the transformation point.
    if let Some((w, h)) = resized
        && linked
        && (w || h)
    {
        if w {
            sy = old.1 * 100.0 * sx / (old.0 * 100.0).max(1e-3);
        } else {
            sx = old.0 * 100.0 * sy / (old.1 * 100.0).max(1e-3);
        }
    }
    if let Some(d) = rotated {
        (kx, ky) = (kx + d, ky + d);
    }
    if fields.iter().any(Response::changed)
        && let Some(undo) = Matrix::from_parts(old).inverse()
    {
        let now = Matrix::from_parts((sx / 100.0, sy / 100.0, kx.to_radians(), ky.to_radians()));
        free_transform::about_pivot(ed, now * undo, Some("transform-panel"));
    }
    if fields.iter().any(|f| f.drag_stopped() || f.lost_focus()) {
        ed.end_gesture();
    }
}

/// Two links of a chain: the Constrain toggle.
fn chain(p: &Painter, r: Rect, c: Color32) {
    let c0 = r.center();
    for dx in [-2.5, 2.5] {
        let link = Rect::from_center_size(c0 + vec2(dx, 0.0), vec2(7.0, 4.0));
        p.rect_stroke(link, 2.0, Stroke::new(1.2, c), egui::StrokeKind::Middle);
    }
}

/// Two triangles mirrored across a dashed axis.
fn flip(p: &Painter, r: Rect, c: Color32, vertical: bool) {
    let at = |u: f32, v: f32| if vertical { pos2(r.left() + v * r.width(), r.top() + u * r.height()) } else { pos2(r.left() + u * r.width(), r.top() + v * r.height()) };
    p.add(egui::Shape::convex_polygon(vec![at(0.42, 0.15), at(0.42, 0.85), at(0.0, 0.85)], c, Stroke::NONE));
    p.add(egui::Shape::closed_line(vec![at(0.58, 0.15), at(0.58, 0.85), at(1.0, 0.85)], Stroke::new(1.0, c)));
    p.line_segment([at(0.5, 0.0), at(0.5, 1.0)], Stroke::new(1.0, c));
}

/// The selection's size and place, and the color and place under the pointer.
pub fn info(ui: &mut Ui, ed: &mut Editor) {
    let b = hit::bounds(&ed.doc, ed.timeline(), ed.frame, &ed.selection);
    let id = Id::new("info-center");
    let mut center = ui.data(|d| d.get_temp::<bool>(id)).unwrap_or(false);
    let mut done = false;
    ui.horizontal(|ui| {
        let size = b.map(|b| (b.max.0 - b.min.0, b.max.1 - b.min.1));
        let mut pair = |ui: &mut Ui, names: [&str; 2], values: Option<Point>| {
            ui.vertical(|ui| {
                let [first, second] = [(names[0], values.map(|v| v.0)), (names[1], values.map(|v| v.1))].map(|(name, v)| field(ui, name, v));
                done |= first.1 || second.1;
                (first.0, second.0)
            })
            .inner
        };
        let (w, h) = pair(ui, ["W:", "H:"], size);
        // X and Y are of the box's top-left corner, or with the middle square picked, its middle.
        let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 30.0), Sense::click());
        for row in 0..3 {
            for col in 0..3 {
                let cell = Rect::from_center_size(rect.min + vec2(5.0 + col as f32 * 10.0, 5.0 + row as f32 * 10.0), vec2(6.0, 6.0));
                let on = if center { (row, col) == (1, 1) } else { (row, col) == (0, 0) };
                if on {
                    ui.painter().rect_filled(cell, 0.0, Color32::WHITE);
                } else {
                    ui.painter().rect_stroke(cell, 0.0, Stroke::new(1.0, theme::TEXT_DIM), egui::StrokeKind::Inside);
                }
            }
        }
        if resp.on_hover_text("Registration / transformation point").clicked() {
            center = !center;
        }
        let place = b.map(|b| if center { ((b.min.0 + b.max.0) / 2.0, (b.min.1 + b.max.1) / 2.0) } else { b.min });
        let (x, y) = pair(ui, ["X:", "Y:"], place);
        if let (Some(b), Some(place)) = (b, place) {
            if x.is_some() || y.is_some() {
                ed.nudge(x.unwrap_or(place.0) - place.0, y.unwrap_or(place.1) - place.1, "info-position");
            }
            if w.is_some() || h.is_some() {
                free_transform::resize(ed, b, w.unwrap_or(b.max.0 - b.min.0), h.unwrap_or(b.max.1 - b.min.1));
            }
        }
    });
    if done {
        ed.end_gesture();
    }
    ui.data_mut(|d| d.insert_temp(id, center));
    ui.separator();

    // The pointer, while it's over the stage view and nothing floats above it there.
    let view = Rect::from_center_size(ed.stage_rect.center() - ed.pan, ed.view_size);
    let pointer = ui.ctx().pointer_hover_pos().filter(|&p| view.contains(p) && ui.ctx().layer_id_at(p).is_some_and(|l| l.order == Order::Background));
    let at = pointer.map(|p| ((p.x - ed.stage_rect.left()) / ed.zoom, (p.y - ed.stage_rect.top()) / ed.zoom));
    let rgba = at.and_then(|p| color_at(ed, p));
    ui.horizontal(|ui| {
        egui::Grid::new("info-color").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            for (i, name) in ["R:", "G:", "B:", "A:"].into_iter().enumerate() {
                ui.label(RichText::new(name).color(theme::TEXT_DIM));
                let value = match rgba {
                    Some(c) if i == 3 => format!("{}%", (c[3] as f32 / 2.55).round()),
                    Some(c) => c[i].to_string(),
                    None => String::new(),
                };
                ui.label(value);
                ui.end_row();
            }
        });
        ui.add_space(40.0);
        let (r, _) = ui.allocate_exact_size(vec2(16.0, 16.0), Sense::hover());
        let s = Stroke::new(1.0, theme::TEXT_DIM);
        ui.painter().line_segment([r.center_top(), r.center_bottom()], s);
        ui.painter().line_segment([r.left_center(), r.right_center()], s);
        egui::Grid::new("info-pointer").num_columns(2).spacing([6.0, 3.0]).show(ui, |ui| {
            for (name, v) in [("X:", at.map(|p| p.0)), ("Y:", at.map(|p| p.1))] {
                ui.label(RichText::new(name).color(theme::TEXT_DIM));
                ui.label(v.map_or(String::new(), |v| format!("{v:.1}")));
                ui.end_row();
            }
        });
    });
}

/// A labelled number field, empty and off without a value. Returns the value
/// if edited, and whether an edit ended (one drag or typing run is one undo step).
fn field(ui: &mut Ui, label: &str, value: Option<f32>) -> (Option<f32>, bool) {
    ui.horizontal(|ui| {
        ui.add_sized(vec2(16.0, 20.0), egui::Label::new(RichText::new(label).color(theme::TEXT_DIM)));
        match value {
            Some(mut v) => {
                let resp = ui.add_sized(vec2(64.0, 20.0), DragValue::new(&mut v).speed(1.0).fixed_decimals(1));
                (resp.changed().then_some(v), resp.drag_stopped() || resp.lost_focus())
            }
            None => {
                ui.add_enabled(false, egui::TextEdit::singleline(&mut String::new()).desired_width(64.0));
                (None, false)
            }
        }
    })
    .inner
}

/// The color under stage point `p`: the artwork over the stage color there,
/// or over nothing off the stage. Premultiplied off the stage.
// ponytail: the scene's, without a symbol being edited in place on top of it.
fn color_at(ed: &Editor, (x, y): Point) -> Option<[u8; 4]> {
    let frame = ed.editing.first().map_or(ed.frame, |t| t.frame);
    let probe = Matrix::translate(0.5 - x, 0.5 - y);
    let (pixmap, _) = render_timeline(&ed.doc, &ed.doc.scenes[ed.scene], frame, &probe, (1.0, 1.0), 1.0).ok()?;
    let px = pixmap.pixel(0, 0)?;
    let on_stage = (0.0..ed.doc.width).contains(&x) && (0.0..ed.doc.height).contains(&y);
    let under = if on_stage { ed.doc.background.map(|c| c * 255.0) } else { [0.0; 4] };
    let a = px.alpha() as f32 / 255.0;
    let over = |c: u8, u: f32| (c as f32 + u * (1.0 - a)).round() as u8;
    Some([over(px.red(), under[0]), over(px.green(), under[1]), over(px.blue(), under[2]), over(px.alpha(), under[3])])
}

/// Modify > Transform > Scale and Rotate: scales and turns the selection about
/// its transformation point by the numbers typed.
fn scale_and_rotate(ctx: &egui::Context, ed: &mut Editor) {
    let Some((mut scale, mut turn)) = ed.scale_rotate else { return };
    let mut ok = ctx.input(|i| i.key_pressed(Key::Enter));
    let mut cancel = false;
    let modal = egui::Modal::new(Id::new("scale-and-rotate")).show(ctx, |ui| {
        ui.set_width(280.0);
        ui.label(RichText::new("Scale and Rotate").heading().color(theme::TEXT));
        ui.add_space(8.0);
        egui::Grid::new("scale-rotate").num_columns(3).spacing([10.0, 8.0]).show(ui, |ui| {
            ui.label(RichText::new("Scale:").color(theme::TEXT_DIM));
            let field = ui.add(DragValue::new(&mut scale).range(1.0..=10_000.0).fixed_decimals(0));
            if ui.memory(|m| m.focused().is_none()) {
                field.request_focus();
            }
            ui.label("%");
            ui.end_row();
            ui.label(RichText::new("Rotate:").color(theme::TEXT_DIM));
            ui.add(DragValue::new(&mut turn).range(-360.0..=360.0).fixed_decimals(0));
            ui.label("degrees");
            ui.end_row();
        });
        ui.add_space(12.0);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
            cancel |= ui.button("  Cancel  ").clicked();
            ok |= theme::cta_button(ui, "    OK    ").clicked();
        });
    });
    ed.scale_rotate = Some((scale, turn));
    if modal.should_close() || cancel {
        ed.scale_rotate = None;
    } else if ok {
        ed.scale_rotate = None;
        let m = Matrix::rotate(turn.to_radians()) * Matrix::scale(scale / 100.0, scale / 100.0);
        free_transform::about_pivot(ed, m, None);
    }
}
