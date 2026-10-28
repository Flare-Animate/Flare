//! Properties > Filters (docs/animate-ui.md §7.2.4): the selected movie clip's,
//! button's or text field's filters, one card each, under a header holding the
//! add (+) and options (gear) menus.

use std::ops::RangeInclusive;

use eframe::egui::{self, Align2, Color32, Id, Margin, Rect, Response, Sense, Stroke, StrokeKind, Ui, WidgetInfo, WidgetType, pos2, vec2};
use minnamate_core::doc::{Element, Extra, Rgba};
use minnamate_core::filters::{self, Filter, FilterKind, Side};
use minnamate_core::hit::ElementRef;

use crate::app::{Editor, rgba};
use crate::color;
use crate::icons::{self, Icon};
use crate::panels::color32;
use crate::theme;
use crate::widgets::{self, Number, WORD_W};

/// What the cards' controls did this frame.
#[derive(Default)]
struct Edits {
    /// The field scrubbed or typed in (or the colour picked), whose edits share one undo
    /// step until it's let go of. None: a click made the edit, a step of its own.
    field: Option<&'static str>,
    let_go: bool,
}

/// A card header's button that was clicked.
enum Head {
    Eye,
    Trash,
}

/// The Filters section of the element at `r`, a symbol instance or text field.
pub fn section(ui: &mut Ui, ed: &mut Editor, r: ElementRef) {
    let Some(before) = ed.element(r).and_then(extra).map(filters::filters) else { return };
    let mut list = before.clone();
    let mut edits = Edits::default();
    let (top, across) = (ui.cursor().top(), ui.available_rect_before_wrap().x_range());
    // Cards keep their open and linked state per element.
    let state = Id::new(("filter-cards", ed.title.clone(), ed.editing.len(), r));
    theme::section(ui, "Filters", |ui| cards(ui, ed, &mut list, state, &mut edits));
    // theme::section's header: a 2 px divider, then 32 px holding the title.
    header_menus(ui, Rect::from_x_y_ranges(across, top + 2.0..=top + 34.0), &mut list);
    if list != before {
        match edits.field {
            Some(field) => ed.checkpoint_gesture(field),
            None => ed.checkpoint(),
        }
        if let Some(extra) = ed.element_mut(r).and_then(extra_mut) {
            filters::set_filters(extra, &list);
        }
    }
    if edits.let_go {
        ed.end_gesture();
    }
}

/// The XFL kept for the element filters go on.
fn extra(el: &Element) -> Option<&Extra> {
    match el {
        Element::Instance(i) => Some(&i.extra),
        Element::Text(t) => Some(&t.extra),
        _ => None,
    }
}

fn extra_mut(el: &mut Element) -> Option<&mut Extra> {
    match el {
        Element::Instance(i) => Some(&mut i.extra),
        Element::Text(t) => Some(&mut t.extra),
        _ => None,
    }
}

/// The header's add (+) and options (gear) buttons, at its right end, with their menus.
fn header_menus(ui: &mut Ui, header: Rect, list: &mut Vec<Filter>) {
    let gear = Rect::from_center_size(pos2(header.right() - 22.0, header.center().y), vec2(20.0, 20.0));
    let add = glyph_button(ui, gear.translate(vec2(-26.0, 0.0)), Id::new("filters-add"), Icon::Plus, "Add filter");
    egui::Popup::menu(&add).style(theme::menu_style).show(|ui| {
        ui.set_min_width(160.0);
        for kind in FilterKind::ALL {
            if theme::menu_item(ui, true, kind.name(), "").clicked() {
                list.push(Filter::new(kind));
            }
        }
        ui.separator();
        let any = !list.is_empty();
        if theme::menu_item(ui, any, "Remove All", "").clicked() {
            list.clear();
        }
        for (label, on) in [("Enable All", true), ("Disable All", false)] {
            if theme::menu_item(ui, any, label, "").clicked() {
                list.iter_mut().for_each(|f| f.enabled = on);
            }
        }
    });
    let options = glyph_button(ui, gear, Id::new("filters-options"), Icon::Gear, "Filter options");
    let copied_id = Id::new("copied-filters");
    egui::Popup::menu(&options).style(theme::menu_style).show(|ui| {
        ui.set_min_width(180.0);
        // ponytail: cards can't be selected and no presets are kept yet, so Copy Selected Filter
        // and the preset items are greyed.
        theme::menu_item(ui, false, "Copy Selected Filter", "");
        if theme::menu_item(ui, !list.is_empty(), "Copy All Filters", "").clicked() {
            ui.data_mut(|d| d.insert_temp(copied_id, list.clone()));
        }
        let copied: Option<Vec<Filter>> = ui.data(|d| d.get_temp(copied_id));
        if theme::menu_item(ui, copied.is_some(), "Paste Filters", "").clicked() {
            list.extend(copied.unwrap_or_default());
        }
        ui.separator();
        theme::menu_item(ui, false, "Save As Preset...", "");
        theme::menu_item(ui, false, "Edit Presets...", "");
    });
}

/// A 20 px glyph button at `rect`: no fill until hovered or its menu is open.
fn glyph_button(ui: &mut Ui, rect: Rect, id: Id, icon: Icon, tip: &str) -> Response {
    let resp = ui.interact(rect, id, Sense::click());
    let hot = resp.hovered() || egui::Popup::is_id_open(ui.ctx(), egui::Popup::default_response_id(&resp));
    if hot {
        ui.painter().rect_filled(rect, 2.0, theme::BUTTON_HOVER);
    }
    icons::paint(ui.painter(), rect.shrink(2.0), icon, if hot { Color32::WHITE } else { theme::TEXT });
    let label = tip.to_string();
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::Button, true, &label));
    resp.on_hover_text(tip)
}

/// The cards, top to bottom in the order the filters apply. A card's header shows or
/// hides its settings when clicked, and moves the card when dragged.
fn cards(ui: &mut Ui, ed: &Editor, list: &mut Vec<Filter>, state: Id, edits: &mut Edits) {
    let (mut rects, mut dragged, mut dropped, mut clicked) = (Vec::new(), None, None, None);
    for i in 0..list.len() {
        ui.push_id(i, |ui| {
            let frame = egui::Frame::NONE
                .fill(theme::FILTER_CARD)
                .stroke(Stroke::new(1.0, theme::FILTER_CARD_BORDER))
                .corner_radius(4)
                .inner_margin(Margin { left: 8, right: 6, top: 0, bottom: 4 });
            let shown = frame.show(ui, |ui| card(ui, ed, &mut list[i], state.with(i), edits));
            let (head, click) = shown.inner;
            rects.push(shown.response.rect);
            if head.dragged() {
                dragged = Some(i);
            }
            if head.drag_stopped() {
                dropped = Some(i);
            }
            if let Some(click) = click {
                clicked = Some((i, click));
            }
        });
        ui.add_space(6.0);
    }
    match clicked {
        Some((i, Head::Trash)) => drop(list.remove(i)),
        Some((i, Head::Eye)) => {
            // Alt: this filter alone. Ctrl: all of them the way this one goes.
            let mods = ui.input(|input| input.modifiers);
            let on = !list[i].enabled;
            for (j, f) in list.iter_mut().enumerate() {
                if mods.alt {
                    f.enabled = j == i;
                } else if mods.command || j == i {
                    f.enabled = on;
                }
            }
        }
        None => {}
    }
    let (Some(from), Some(pointer)) = (dragged.or(dropped), ui.ctx().pointer_interact_pos()) else { return };
    if rects.len() != list.len() {
        return;
    }
    // Where the dragged card goes: below every other card whose middle is above the pointer.
    let others: Vec<Rect> = rects.iter().enumerate().filter(|&(j, _)| j != from).map(|(_, r)| *r).collect();
    let to = others.iter().filter(|r| r.center().y < pointer.y).count();
    if to == from {
        return;
    }
    if dropped.is_some() {
        let f = list.remove(from);
        list.insert(to, f);
    } else {
        let y = if to == 0 { others[0].top() - 3.0 } else { others[to - 1].bottom() + 3.0 };
        ui.painter().hline(rects[from].x_range(), y, Stroke::new(2.0, theme::ACCENT));
    }
}

/// One filter's card: a header (chevron, name, eye, trash), then its settings unless closed.
fn card(ui: &mut Ui, ed: &Editor, f: &mut Filter, id: Id, edits: &mut Edits) -> (Response, Option<Head>) {
    let open_id = id.with("open");
    let open: bool = ui.data(|d| d.get_temp(open_id)).unwrap_or(true);
    let (row, head) = ui.allocate_exact_size(vec2(ui.available_width(), 28.0), Sense::click_and_drag());
    let name = f.kind.name();
    head.widget_info(|| WidgetInfo::selected(WidgetType::CollapsingHeader, true, open, name));
    let color = if f.enabled { theme::TEXT } else { theme::TEXT_DISABLED };
    let (x, y) = (row.left() + 2.0, row.center().y);
    let chevron = if open {
        vec![pos2(x, y - 2.0), pos2(x + 4.0, y + 2.0), pos2(x + 8.0, y - 2.0)]
    } else {
        vec![pos2(x + 2.0, y - 4.0), pos2(x + 6.0, y), pos2(x + 2.0, y + 4.0)]
    };
    ui.painter().add(egui::Shape::line(chevron, Stroke::new(1.3, color)));
    ui.painter().text(pos2(row.left() + 18.0, y), Align2::LEFT_CENTER, name, theme::semibold(12.0), color);
    let trash = Rect::from_center_size(pos2(row.right() - 10.0, y), vec2(20.0, 20.0));
    let eye = trash.translate(vec2(-24.0, 0.0));
    let mut click = None;
    let eye_icon = if f.enabled { Icon::Eye } else { Icon::TlEyeSlash };
    if glyph_button(ui, eye, id.with("eye"), eye_icon, "Enable or disable filter").clicked() {
        click = Some(Head::Eye);
    }
    if glyph_button(ui, trash, id.with("trash"), Icon::Delete, "Delete filter").clicked() {
        click = Some(Head::Trash);
    }
    if head.clicked() {
        ui.data_mut(|d| d.insert_temp(open_id, !open));
    }
    if open {
        settings(ui, ed, f, id, edits);
    }
    (head, click)
}

/// A filter's settings, as its kind's card lists them.
fn settings(ui: &mut Ui, ed: &Editor, f: &mut Filter, id: Id, edits: &mut Edits) {
    use FilterKind::*;
    let kind = f.kind;
    if kind == AdjustColor {
        for (k, name, range) in [(1, "Contrast", -100.0..=100.0), (0, "Brightness", -100.0..=100.0), (2, "Saturation", -100.0..=100.0), (3, "Hue", -180.0..=180.0)] {
            number(ui, edits, name, &mut f.adjust[k], range, "");
        }
        widgets::row(ui, |ui| {
            if widgets::button(ui, "Reset Filter", 96.0).clicked() {
                f.adjust = [0.0; 4];
            }
        });
        return;
    }
    // Blur X over Blur Y, joined by the bracket that links them.
    let link_id = id.with("linked");
    let mut linked: bool = ui.data(|d| d.get_temp(link_id)).unwrap_or(true);
    let [mut bx, mut by] = f.blur;
    let rx = number(ui, edits, "Blur X", &mut bx, 0.0..=255.0, " px");
    let ry = number(ui, edits, "Blur Y", &mut by, 0.0..=255.0, " px");
    let at = |r: &Response| pos2(r.rect.right() + 4.0, r.rect.center().y);
    if widgets::link_bracket(ui, at(&rx), at(&ry), linked, true).clicked() {
        linked = !linked;
        ui.data_mut(|d| d.insert_temp(link_id, linked));
    }
    if rx.changed() || ry.changed() {
        f.blur = match (linked, rx.changed()) {
            (true, true) => [bx; 2],
            (true, false) => [by; 2],
            _ => [bx, by],
        };
    }
    if kind != Blur {
        let mut strength = f.strength * 100.0;
        if number(ui, edits, "Strength", &mut strength, 0.0..=1000.0, " %").changed() {
            f.strength = strength / 100.0;
        }
    }
    if matches!(kind, DropShadow | Bevel | GradientGlow | GradientBevel) {
        number(ui, edits, "Angle", &mut f.angle, 0.0..=360.0, " °");
        number(ui, edits, "Distance", &mut f.distance, -255.0..=255.0, " px");
    }
    let mut inner = f.side == Side::Inner;
    match kind {
        DropShadow => {
            color_row(ui, ed, edits, "Shadow", &mut f.color);
            checkbox(ui, &mut f.knockout, "Knockout");
            checkbox(ui, &mut inner, "Inner shadow");
            checkbox(ui, &mut f.hide_object, "Hide object");
        }
        Glow => {
            color_row(ui, ed, edits, "Color", &mut f.color);
            checkbox(ui, &mut inner, "Inner glow");
            checkbox(ui, &mut f.knockout, "Knockout");
        }
        Bevel => {
            color_row(ui, ed, edits, "Shadow", &mut f.shadow);
            color_row(ui, ed, edits, "Highlight", &mut f.color);
            checkbox(ui, &mut f.knockout, "Knockout");
        }
        GradientGlow | GradientBevel => {
            gradient(ui, ed, edits, id, &mut f.gradient);
            checkbox(ui, &mut f.knockout, "Knockout");
        }
        Blur | AdjustColor => {}
    }
    if matches!(kind, DropShadow | Glow) {
        f.side = if inner { Side::Inner } else { Side::Outer };
    }
    if matches!(kind, Bevel | GradientGlow | GradientBevel) {
        widgets::row(ui, |ui| {
            widgets::label(ui, "Type", WORD_W);
            let sides = [(Side::Inner, "Inner", true), (Side::Outer, "Outer", true), (Side::Full, "Full", true)];
            if let Some(side) = widgets::pick(ui, "Type", f.side, &sides, 96.0) {
                f.side = side;
            }
        });
    }
    widgets::row(ui, |ui| {
        widgets::label(ui, "Quality", WORD_W);
        let qualities = [(1, "Low", true), (2, "Medium", true), (3, "High", true)];
        if let Some(q) = widgets::pick(ui, "Quality", f.quality, &qualities, 96.0) {
            f.quality = q;
        }
    });
}

/// A labelled scrubby number; returns its response.
fn number(ui: &mut Ui, edits: &mut Edits, name: &'static str, v: &mut f32, range: RangeInclusive<f32>, suffix: &str) -> Response {
    widgets::row(ui, |ui| {
        widgets::label(ui, name, WORD_W);
        let resp = ui.add(Number::new(name, v, range).suffix(suffix).width(64.0));
        if resp.changed() {
            edits.field = Some(name);
        }
        edits.let_go |= resp.drag_stopped() || resp.lost_focus();
        resp
    })
}

fn checkbox(ui: &mut Ui, on: &mut bool, label: &str) {
    widgets::row(ui, |ui| widgets::checkbox(ui, on, label));
}

/// A colour chip and its name; the swatch popup it opens sets the colour, alpha too.
fn color_row(ui: &mut Ui, ed: &Editor, edits: &mut Edits, name: &'static str, c: &mut Rgba) {
    widgets::row(ui, |ui| {
        widgets::label(ui, name, WORD_W);
        let current = color32(*c);
        let chip = widgets::chip(ui, current, name);
        if let Some(picked) = color::swatch_popup(ui, ed, &chip, current, false) {
            *c = rgba(picked);
            // Picks in a row share a step, as the stage colour's do.
            edits.field = Some(name);
        }
    });
}

/// The gradient filters' bar, with a chip under each stop that recolours it.
// ponytail: stops are recoloured only; dragging them along, adding and removing (up to 15)
// wait on a gradient editor, which the Color panel needs too.
fn gradient(ui: &mut Ui, ed: &Editor, edits: &mut Edits, id: Id, stops: &mut [(f32, Rgba)]) {
    let bar = widgets::row(ui, |ui| {
        widgets::label(ui, "Gradient", WORD_W);
        ui.allocate_exact_size(vec2(ui.available_width() - 8.0, 14.0), Sense::hover()).0
    });
    let mut sorted = stops.to_vec();
    sorted.sort_by(|a, b| a.0.total_cmp(&b.0));
    if let (Some(&(_, first)), Some(&(_, last))) = (sorted.first(), sorted.last()) {
        let mut mesh = egui::Mesh::default();
        let ends = std::iter::once((0.0, first)).chain(sorted.iter().copied()).chain([(1.0, last)]);
        for (t, c) in ends {
            let x = bar.left() + t.clamp(0.0, 1.0) * bar.width();
            mesh.colored_vertex(pos2(x, bar.top()), color32(c));
            mesh.colored_vertex(pos2(x, bar.bottom()), color32(c));
        }
        for k in 0..mesh.vertices.len() as u32 / 2 - 1 {
            mesh.add_triangle(2 * k, 2 * k + 1, 2 * k + 2);
            mesh.add_triangle(2 * k + 1, 2 * k + 3, 2 * k + 2);
        }
        ui.painter().add(egui::Shape::mesh(mesh));
    }
    ui.painter().rect_stroke(bar, 0.0, Stroke::new(1.0, theme::OUTLINE), StrokeKind::Outside);
    let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 16.0), Sense::hover());
    for (k, (t, c)) in stops.iter_mut().enumerate() {
        let chip = Rect::from_center_size(pos2(bar.left() + t.clamp(0.0, 1.0) * bar.width(), strip.center().y), vec2(10.0, 12.0));
        let resp = ui.interact(chip, id.with(("stop", k)), Sense::click());
        let current = color32(*c);
        widgets::paint_swatch(ui.painter(), chip, current, 2.0);
        ui.painter().rect_stroke(chip, 2.0, Stroke::new(1.0, theme::TEXT_DIM), StrokeKind::Outside);
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::ColorButton, true, "Gradient stop"));
        if let Some(picked) = color::swatch_popup(ui, ed, &resp, current, false) {
            *c = rgba(picked);
            edits.field = Some("Gradient");
        }
    }
}

#[cfg(test)]
mod tests {
    use egui_kittest::kittest::Queryable;
    use minnamate_core::doc::{Element, Extra, Matrix, Paint, TextAlign, TextKind, TextRun};
    use minnamate_core::filters::{Effects, Filter, FilterKind};
    use minnamate_core::{doc, draw};

    use super::*;
    use crate::app::testkit::*;
    use crate::app::{Action, PropsTab};

    /// The tween fixture with its movie clip selected, on the Object tab.
    fn clip_selected() -> Harness<'static, Editor> {
        let mut h = harness(false);
        h.run();
        let ed = h.state_mut();
        ed.selection = vec![ElementRef { layer: 0, keyframe: 0, element: 0 }];
        ed.props_tab = PropsTab::Object;
        h.run();
        h
    }

    fn filters_at(h: &Harness<'_, Editor>, r: ElementRef) -> Vec<Filter> {
        h.state().element(r).and_then(extra).map(filters::filters).unwrap_or_default()
    }

    #[test]
    fn a_drop_shadow_added_and_edited_undoes_step_by_step() {
        let mut h = clip_selected();
        let r = h.state().selection[0];
        h.get_by_label("Add filter").click();
        h.run();
        h.get_by_label("Drop Shadow").click();
        h.run();
        assert_eq!(filters_at(&h, r), [Filter::new(FilterKind::DropShadow)]);
        h.get_by_label("Distance").click();
        h.run();
        h.get_by_label("Distance").type_text("12");
        h.key_press(egui::Key::Enter);
        h.run();
        assert_eq!(filters_at(&h, r)[0].distance, 12.0);
        h.get_by_label("Knockout").click();
        h.run();
        assert!(filters_at(&h, r)[0].knockout);

        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert!(!filters_at(&h, r)[0].knockout, "the checkbox is a step");
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(filters_at(&h, r)[0].distance, 4.0, "typing 12 is one step");
        h.state_mut().run(Action::Undo, &ctx);
        assert!(filters_at(&h, r).is_empty(), "and adding it another");
    }

    #[test]
    fn eyes_and_trash() {
        let mut h = clip_selected();
        let r = h.state().selection[0];
        // Blurs, whose short cards all fit in the window.
        let three = [1.0, 2.0, 3.0].map(|b| Filter { blur: [b; 2], ..Filter::new(FilterKind::Blur) });
        h.state_mut().checkpoint();
        if let Some(extra) = h.state_mut().element_mut(r).and_then(extra_mut) {
            filters::set_filters(extra, &three);
        }
        h.run();
        let eyes: Vec<_> = h.get_all_by_label("Enable or disable filter").map(|n| n.rect().center()).collect();
        assert_eq!(eyes.len(), 3);
        click_at(&mut h, eyes[1]);
        h.run();
        assert_eq!(filters_at(&h, r).iter().map(|f| f.enabled).collect::<Vec<_>>(), [true, false, true]);
        assert_eq!(Effects::of(h.state().element(r).and_then(extra).unwrap()).filters.len(), 2, "a disabled filter doesn't draw");
        // Alt held through the click (screen_click lets go of it in the release's frame).
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::ALT));
        click_at(&mut h, eyes[2]);
        h.event(egui::Event::ModifiersChanged(egui::Modifiers::NONE));
        h.run();
        assert_eq!(filters_at(&h, r).iter().map(|f| f.enabled).collect::<Vec<_>>(), [false, false, true], "Alt: that one alone");
        let trash = h.get_all_by_label("Delete filter").map(|n| n.rect().center()).next().unwrap();
        click_at(&mut h, trash);
        h.run();
        assert_eq!(filters_at(&h, r).iter().map(|f| f.blur[0]).collect::<Vec<_>>(), [2.0, 3.0], "the first card's trash");
        // Dragging a card's header above another moves its filter before it.
        let heads: Vec<_> = h.get_all_by_label("Blur").map(|n| n.rect().center()).collect();
        screen_drag(&mut h, heads[1], heads[0] - vec2(0.0, 20.0), egui::Modifiers::NONE);
        h.run();
        assert_eq!(filters_at(&h, r).iter().map(|f| f.blur[0]).collect::<Vec<_>>(), [3.0, 2.0]);
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(filters_at(&h, r).iter().map(|f| f.blur[0]).collect::<Vec<_>>(), [2.0, 3.0], "one step");
    }

    #[test]
    fn display_options_set_their_attributes() {
        let mut h = clip_selected();
        let r = h.state().selection[0];
        let attr = |h: &Harness<'_, Editor>, name: &str| h.state().element(r).and_then(extra).and_then(|e| e.get(name).map(String::from));
        for (dropdown, choice) in [("Render", "Export as Bitmap"), ("Blending", "Multiply")] {
            h.get_by_label(dropdown).click();
            h.run();
            h.get_by_label(choice).click();
            h.run();
        }
        assert_eq!(attr(&h, filters::EXPORT_AS_BITMAP).as_deref(), Some("true"));
        assert_eq!(attr(&h, "blendMode").as_deref(), Some("multiply"));
        h.get_by_label("Render").click();
        h.run();
        h.get_by_label("Cache as Bitmap").click();
        h.run();
        assert_eq!((attr(&h, filters::EXPORT_AS_BITMAP), attr(&h, filters::CACHE_AS_BITMAP).as_deref()), (None, Some("true")), "one at a time");
        let ctx = h.ctx.clone();
        h.state_mut().run(Action::Undo, &ctx);
        assert_eq!(attr(&h, filters::EXPORT_AS_BITMAP).as_deref(), Some("true"), "a step each");
    }

    #[test]
    fn text_fields_have_filters_too() {
        let mut h = harness(false);
        h.run();
        let ed = h.state_mut();
        ed.load(showcase(), "Filters".into(), None);
        ed.selection = vec![ElementRef { layer: 0, keyframe: 0, element: 9 }];
        ed.props_tab = PropsTab::Object;
        h.run();
        assert!(h.query_by_label("Add filter").is_some() && h.query_by_label("Glow").is_some(), "the glow's card");
        assert!(h.query_by_label("Blending").is_none(), "text has no Display section");
    }

    /// A stage of filtered and blended instances of one symbol, and a glowing text field.
    fn showcase() -> doc::Document {
        use FilterKind::*;
        let mut doc = doc::Document::new(550.0, 400.0, 24.0);
        let mut badge = doc::Timeline::new("Badge");
        let frame = &mut badge.layers[0].frames[0];
        draw::add(frame, draw::rectangle([(-36.0, -36.0), (36.0, 36.0)], 10.0, None, Some(Paint::Solid([1.0, 0.6, 0.0, 1.0]))));
        draw::add(frame, draw::oval([(-18.0, -18.0), (18.0, 18.0)], None, Some(Paint::Solid([0.1, 0.4, 0.9, 1.0]))));
        doc.library.insert("Badge".into(), doc::Symbol { kind: doc::SymbolKind::MovieClip, timeline: badge, extra: Extra::default() });
        let stage = &mut doc.scenes[0].layers[0].frames[0];
        draw::add(stage, draw::rectangle([(0.0, 220.0), (550.0, 400.0)], 0.0, None, Some(Paint::Solid([0.55, 0.75, 0.6, 1.0]))));
        let looks: [(&[Filter], Option<&str>); 8] = [
            (&[Filter { blur: [8.0; 2], distance: 8.0, strength: 0.8, ..Filter::new(DropShadow) }], None),
            (&[Filter { blur: [10.0; 2], quality: 3, ..Filter::new(Blur) }], None),
            (&[Filter { blur: [14.0; 2], strength: 2.0, color: [1.0, 0.85, 0.0, 1.0], ..Filter::new(Glow) }], None),
            (&[Filter { blur: [6.0; 2], distance: 5.0, strength: 1.5, ..Filter::new(Bevel) }], None),
            (&[Filter { blur: [16.0; 2], strength: 1.5, ..Filter::new(GradientGlow) }], None),
            (&[Filter { adjust: [0.0, 20.0, -40.0, 150.0], ..Filter::new(AdjustColor) }], Some("multiply")),
            (&[Filter { blur: [6.0; 2], strength: 2.0, side: Side::Inner, ..Filter::new(Glow) }, Filter::new(DropShadow)], Some("screen")),
            (&[], Some("difference")),
        ];
        for (k, (list, blend)) in looks.into_iter().enumerate() {
            let mut inst = doc::Instance {
                symbol: "Badge".into(),
                kind: doc::SymbolKind::MovieClip,
                matrix: Matrix { tx: 75.0 + 133.0 * (k % 4) as f32, ty: 110.0 + 180.0 * (k / 4) as f32, ..Matrix::IDENTITY },
                pivot: (0.0, 0.0),
                color: doc::ColorTransform::IDENTITY,
                looping: doc::Loop::Loop,
                first_frame: 0,
                extra: Extra::default(),
            };
            filters::set_filters(&mut inst.extra, list);
            inst.extra.set("blendMode", blend);
            stage.elements.push(Element::Instance(inst));
        }
        let run = TextRun {
            text: "Filters".into(),
            face: "Arial".into(),
            size: 28.0,
            color: [1.0; 4],
            bold: true,
            italic: false,
            letter_spacing: 0.0,
            extra: Extra::default(),
        };
        let mut text = doc::Text {
            kind: TextKind::Static,
            matrix: Matrix { tx: 225.0, ty: 8.0, ..Matrix::IDENTITY },
            width: None,
            align: TextAlign::Left,
            line_spacing: 2.0,
            runs: vec![run],
            extra: Extra::default(),
        };
        filters::set_filters(&mut text.extra, &[Filter { blur: [6.0; 2], strength: 3.0, color: [0.1, 0.3, 0.8, 1.0], ..Filter::new(Glow) }]);
        stage.elements.push(Element::Text(text));
        doc
    }

    /// Writes target/screenshots/filters.png (the showcase on stage) and filters_panel.png
    /// (one instance's cards in Properties).
    #[test]
    #[ignore = "needs a GPU adapter; run with --ignored"]
    fn filters_screenshot() {
        let mut h = harness(true);
        h.run();
        h.state_mut().load(showcase(), "Filters".into(), None);
        h.run();
        screenshot(&mut h, "filters");
        let ed = h.state_mut();
        ed.selection = vec![ElementRef { layer: 0, keyframe: 0, element: 7 }];
        ed.props_tab = PropsTab::Object;
        h.run();
        // Scrolled to the Filters section, as one would to edit them.
        let panel = h.get_by_label("Color effect").rect().left_center() - vec2(8.0, 0.0);
        h.hover_at(panel);
        h.event(egui::Event::MouseWheel { unit: egui::MouseWheelUnit::Point, delta: vec2(0.0, -400.0), phase: egui::TouchPhase::Move, modifiers: egui::Modifiers::NONE });
        h.run_steps(30);
        h.remove_cursor();
        h.run_steps(5);
        screenshot(&mut h, "filters_panel");
    }
}
