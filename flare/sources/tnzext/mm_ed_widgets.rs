//! The Properties panel's controls (docs/animate-ui.md §7.1): scrubby numbers,
//! fields, dropdowns, buttons, checkboxes, colour chips, sliders and icon toggles.

use std::ops::RangeInclusive;

use eframe::egui::{
    self, Align, Align2, Color32, DragValue, FontId, Layout, Pos2, Rect, Response, RichText, Sense, Stroke, StrokeKind, TextEdit,
    Ui, Widget, WidgetInfo, WidgetType, pos2, vec2,
};

use crate::app::Editor;
use crate::icons::{self, Icon};
use crate::theme;

/// Properties content rows.
pub const ROW_H: f32 = 28.0;
/// The label column for W, H, X and Y; word labels (Profile, Ease, Name) take `WORD_W`.
pub const LABEL_W: f32 = 24.0;
pub const WORD_W: f32 = 64.0;
/// Fields, dropdowns and push buttons.
pub const FIELD_H: f32 = 22.0;

/// One content row: its widgets left to right, centred on a 28 px line.
pub fn row<R>(ui: &mut Ui, add: impl FnOnce(&mut Ui) -> R) -> R {
    let layout = Layout::left_to_right(Align::Center);
    ui.allocate_ui_with_layout(vec2(ui.available_width(), ROW_H), layout, |ui| {
        ui.set_min_height(ROW_H);
        ui.spacing_mut().item_spacing.x = 6.0;
        add(ui)
    })
    .inner
}

/// A dim label in a column `width` wide.
pub fn label(ui: &mut Ui, text: &str, width: f32) {
    let (rect, _) = ui.allocate_exact_size(vec2(width, FIELD_H), Sense::hover());
    let color = if ui.is_enabled() { theme::TEXT_DIM } else { theme::TEXT_DISABLED };
    ui.painter().text(rect.left_center(), Align2::LEFT_CENTER, text, FontId::proportional(12.0), color);
}

/// A scrubby number: white text over a grey underline, no box. Drag sideways
/// to scrub, click to type; Enter or leaving the field commits, Esc reverts.
/// Values show up to two decimals, units as a suffix.
pub struct Number<'a> {
    label: &'a str,
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    suffix: &'a str,
    speed: f32,
    decimals: Option<usize>,
    width: f32,
}

impl<'a> Number<'a> {
    /// `label` names the field for screen readers (and tests).
    pub fn new(label: &'a str, value: &'a mut f32, range: RangeInclusive<f32>) -> Self {
        Number { label, value, range, suffix: "", speed: 1.0, decimals: None, width: 52.0 }
    }

    pub fn suffix(self, suffix: &'a str) -> Self {
        Number { suffix, ..self }
    }

    /// Units per point dragged; 1 by default.
    pub fn speed(self, speed: f32) -> Self {
        Number { speed, ..self }
    }

    /// Always this many decimals (Stroke shows 1.00).
    pub fn fixed(self, decimals: usize) -> Self {
        Number { decimals: Some(decimals), ..self }
    }

    pub fn width(self, width: f32) -> Self {
        Number { width, ..self }
    }
}

impl Widget for Number<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        // A value outside the range (a hair-thin selection's width) shows as it is.
        // Typing commits on Enter or leaving the field, not keystroke by keystroke.
        let value = self.value;
        let before = *value;
        let drag = DragValue::new(&mut *value).range(self.range).speed(self.speed).clamp_existing_to_range(false).suffix(self.suffix).update_while_editing(false);
        let drag = match self.decimals {
            Some(d) => drag.fixed_decimals(d),
            // Up to two decimals, as needed: 550, 24, 212.95.
            None => drag.max_decimals(2).custom_formatter(|v, _| {
                let s = format!("{v:.2}");
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }),
        };
        // Values sit at the left of their underline.
        let layout = Layout::left_to_right(Align::Center).with_main_align(Align::Min);
        let resp = ui
            .allocate_ui_with_layout(vec2(self.width, FIELD_H), layout, |ui| {
                let s = ui.style_mut();
                s.spacing.interact_size = vec2(self.width, FIELD_H);
                s.spacing.button_padding = vec2(2.0, 0.0);
                let w = &mut s.visuals.widgets;
                for v in [&mut w.inactive, &mut w.hovered, &mut w.active] {
                    v.weak_bg_fill = Color32::TRANSPARENT;
                    v.bg_stroke = Stroke::NONE;
                    v.fg_stroke.color = theme::TEXT;
                }
                ui.add(drag)
            })
            .inner;
        // Escape reverts what was typed. egui reports the lost focus for two frames, and
        // DragValue commits the typed text on the second, when Escape is no longer down.
        let mut resp = resp;
        let escaped = resp.id.with("escaped");
        if ui.data_mut(|d| d.remove_temp::<bool>(escaped)).is_some() && resp.changed() {
            *value = before;
            resp.flags.remove(egui::response::Flags::CHANGED);
        }
        if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            ui.data_mut(|d| d.insert_temp(escaped, true));
        }
        if !resp.has_focus() {
            let color = if ui.is_enabled() { theme::FIELD_UNDERLINE } else { theme::BORDER };
            ui.painter().hline(resp.rect.x_range(), resp.rect.bottom() - 1.5, Stroke::new(1.0, color));
        }
        let (label, enabled) = (self.label.to_string(), ui.is_enabled());
        resp.widget_info(|| WidgetInfo::labeled(WidgetType::DragValue, enabled, &label));
        resp
    }
}

/// Runs `apply` when `resp` (a field or slider) changed: one drag, or one
/// typed value, is one undo step.
pub fn undoable(ed: &mut Editor, resp: &Response, field: &'static str, apply: impl FnOnce(&mut Editor)) {
    if resp.changed() {
        ed.checkpoint_gesture(field);
        apply(ed);
    }
    settle(ed, resp);
}

/// Ends the undo step of a field whose edits checkpoint as a gesture, once
/// its drag or typing is done.
pub fn settle(ed: &mut Editor, resp: &Response) {
    if resp.drag_stopped() || resp.lost_focus() {
        ed.end_gesture();
    }
}

/// A one-line text field: a dark box with a 1 px outline. Its hint names it
/// for screen readers.
pub fn text_input(ui: &mut Ui, text: &mut String, hint: &str, width: f32) -> Response {
    let shown = RichText::new(hint).italics().color(theme::TEXT_DISABLED);
    let resp = ui.add(TextEdit::singleline(text).hint_text(shown).desired_width(width).min_size(vec2(width, FIELD_H)).vertical_align(Align::Center));
    let (enabled, value) = (ui.is_enabled(), text.clone());
    resp.widget_info(|| WidgetInfo { label: Some(hint.to_string()), ..WidgetInfo::text_edit(enabled, &value, &value, hint) });
    resp
}

/// A dropdown named `label` (for screen readers): a dark field with a light
/// chevron. Its list is a light menu, filled by `list` with `choice`s.
pub fn dropdown<R>(ui: &mut Ui, label: &str, selected: &str, width: f32, list: impl FnOnce(&mut Ui) -> R) -> Option<R> {
    ui.scope(|ui| {
        ui.spacing_mut().interact_size.y = FIELD_H;
        ui.spacing_mut().button_padding.x = 8.0;
        let enabled = ui.is_enabled();
        let w = &mut ui.visuals_mut().widgets.inactive;
        w.weak_bg_fill = if enabled { theme::INSET } else { theme::DROPDOWN_DISABLED };
        if !enabled {
            w.bg_stroke = Stroke::NONE;
        }
        let shown = egui::ComboBox::from_id_salt(label)
            .selected_text(selected)
            .width(width)
            .truncate()
            .icon(|ui, rect, _, _| {
                let color = if ui.is_enabled() { theme::TEXT } else { theme::TEXT_DISABLED };
                icons::paint(ui.painter(), Rect::from_center_size(rect.center(), vec2(12.0, 12.0)), Icon::ChevronDown, color);
            })
            .popup_style(theme::menu_style.into())
            .show_ui(ui, |ui| {
                ui.set_min_width(width);
                list(ui)
            });
        shown.response.widget_info(|| WidgetInfo { current_text_value: Some(selected.to_string()), ..WidgetInfo::labeled(WidgetType::ComboBox, enabled, label) });
        shown.inner
    })
    .inner
}

/// A dropdown or menu item; the current one has a check in the gutter.
pub fn choice(ui: &mut Ui, current: bool, enabled: bool, label: &str) -> Response {
    let resp = ui.add_enabled(enabled, egui::Button::new(label));
    if current {
        let c = resp.rect.left_center() + vec2(11.0, 0.0);
        let check = vec![c + vec2(-4.0, 0.0), c + vec2(-1.0, 3.0), c + vec2(4.0, -3.0)];
        ui.painter().add(egui::Shape::line(check, Stroke::new(1.5, theme::ACCENT)));
    }
    resp
}

/// A dropdown over `items`; returns the one picked, if it changed.
pub fn pick<T: Copy + PartialEq>(ui: &mut Ui, label: &str, current: T, items: &[(T, &str, bool)], width: f32) -> Option<T> {
    let shown = items.iter().find(|i| i.0 == current).map_or("", |i| i.1);
    let mut picked = None;
    dropdown(ui, label, shown, width, |ui| {
        for &(value, text, enabled) in items {
            if choice(ui, value == current, enabled, text).clicked() && value != current {
                picked = Some(value);
            }
        }
    });
    picked
}

/// A push button: dark fill, 1 px outline, radius 4, centred text.
pub fn button(ui: &mut Ui, label: &str, width: f32) -> Response {
    ui.add_sized(vec2(width, FIELD_H), egui::Button::new(RichText::new(label).color(theme::TEXT)))
}

/// A 14 px checkbox: a grey ring when off, a light fill with a dark check when on.
pub fn checkbox(ui: &mut Ui, on: &mut bool, label: &str) -> Response {
    let font = FontId::proportional(12.0);
    let galley = ui.painter().layout_no_wrap(label.to_string(), font, theme::TEXT);
    let (rect, mut resp) = ui.allocate_exact_size(vec2(20.0 + galley.size().x, FIELD_H), Sense::click());
    if resp.clicked() {
        *on = !*on;
        resp.mark_changed();
    }
    let enabled = ui.is_enabled();
    let fade = |c: Color32| if enabled { c } else { c.gamma_multiply(0.4) };
    let p = ui.painter();
    let b = Rect::from_min_size(pos2(rect.left(), rect.center().y - 7.0), vec2(14.0, 14.0));
    if *on {
        p.rect_filled(b, 2.0, fade(theme::CHECKBOX_ON));
        let check = vec![b.min + vec2(3.0, 7.0), b.min + vec2(6.0, 10.0), b.min + vec2(11.0, 4.0)];
        p.add(egui::Shape::line(check, Stroke::new(1.6, theme::INSET)));
    } else {
        p.rect_stroke(b, 2.0, Stroke::new(1.5, fade(theme::CHECKBOX_BORDER)), StrokeKind::Inside);
    }
    p.galley(pos2(b.right() + 6.0, rect.center().y - galley.size().y / 2.0), galley, fade(theme::TEXT));
    let label = label.to_string();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Checkbox, enabled, *on, &label));
    resp
}

/// "No colour" is a fully transparent colour.
pub fn is_none(c: Color32) -> bool {
    c.a() == 0
}

/// A colour chip: a 30×16 rounded swatch, opaque whatever the alpha; "no
/// colour" shows white with a red slash. `label` names it for screen readers.
pub fn chip(ui: &mut Ui, color: Color32, label: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(30.0, 16.0), Sense::click());
    paint_swatch(ui.painter(), rect, color, 3.0);
    ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0, theme::BORDER), StrokeKind::Outside);
    let (label, enabled) = (label.to_string(), ui.is_enabled());
    resp.widget_info(|| WidgetInfo::labeled(WidgetType::ColorButton, enabled, &label));
    resp
}

/// `color` filling `rect`, opaque, or the "no colour" glyph.
pub fn paint_swatch(p: &egui::Painter, rect: Rect, color: Color32, radius: f32) {
    if is_none(color) {
        p.rect_filled(rect, radius, Color32::WHITE);
        p.line_segment([rect.left_bottom() + vec2(1.0, -1.0), rect.right_top() + vec2(-1.0, 1.0)], Stroke::new(2.0, theme::NO_COLOR_SLASH));
    } else {
        p.rect_filled(rect, radius, color.to_opaque());
    }
}

/// The 12 px checkerboard that stands for alpha.
pub fn checker(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(12.0, 12.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, 0.0, Color32::WHITE);
    for (x, y) in [(0.0, 0.0), (6.0, 6.0)] {
        p.rect_filled(Rect::from_min_size(rect.min + vec2(x, y), vec2(6.0, 6.0)), 0.0, Color32::from_gray(0xA0));
    }
}

/// A full-width slider: a 2 px track and a 14 px hollow knob.
pub fn slider(ui: &mut Ui, label: &str, value: &mut f32, range: RangeInclusive<f32>) -> Response {
    let (rect, mut resp) = ui.allocate_exact_size(vec2(ui.available_width(), 18.0), Sense::click_and_drag());
    let track = rect.shrink2(vec2(7.0, 0.0));
    let (lo, hi) = (*range.start(), *range.end());
    if let Some(pos) = resp.interact_pointer_pos().filter(|_| resp.dragged() || resp.clicked()) {
        let t = ((pos.x - track.left()) / track.width()).clamp(0.0, 1.0);
        let v = lo + t * (hi - lo);
        if v != *value {
            *value = v;
            resp.mark_changed();
        }
    }
    let enabled = ui.is_enabled();
    let p = ui.painter();
    p.hline(track.x_range(), rect.center().y, Stroke::new(2.0, theme::SLIDER_TRACK));
    let t = ((*value - lo) / (hi - lo)).clamp(0.0, 1.0);
    let knob = pos2(track.left() + t * track.width(), rect.center().y);
    p.circle_filled(knob, 6.0, theme::PANEL_BG);
    let color = if enabled { theme::SLIDER_KNOB } else { theme::TEXT_DISABLED };
    p.circle_stroke(knob, 6.0, Stroke::new(1.5, color));
    let (label, v) = (label.to_string(), *value as f64);
    resp.widget_info(|| WidgetInfo { value: Some(v), ..WidgetInfo::labeled(WidgetType::Slider, enabled, &label) });
    resp
}

/// A Properties icon button, 28×28: a dark fill and outline, lighter while on.
pub fn quick(ui: &mut Ui, icon: Icon, on: bool, enabled: bool, tooltip: &str) -> Response {
    let (rect, resp) = ui.allocate_exact_size(vec2(28.0, 28.0), if enabled { Sense::click() } else { Sense::hover() });
    let p = ui.painter();
    let fill = if enabled && resp.is_pointer_button_down_on() || on {
        theme::BUTTON_ACTIVE
    } else if enabled && resp.hovered() {
        theme::BUTTON_HOVER
    } else {
        theme::BUTTON
    };
    p.rect_filled(rect, 4.0, fill);
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, theme::OUTLINE), StrokeKind::Inside);
    let color = if !enabled { theme::TEXT_DISABLED } else if on { Color32::WHITE } else { theme::TEXT };
    icons::paint(p, Rect::from_center_size(rect.center(), vec2(16.0, 16.0)), icon, color);
    let tip = tooltip.to_string();
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, on, &tip));
    resp.on_hover_text(tooltip)
}

/// Icon buttons joined in one outlined row (Cap, Join, Looping), each
/// (glyph, tooltip, enabled); returns the one clicked. The current one is
/// lighter, and with `accent` also outlined in blue.
pub fn segmented(ui: &mut Ui, items: &[(Icon, &str, bool)], current: usize, cell: f32, accent: bool) -> Option<usize> {
    let size = vec2(cell, 24.0);
    let (rect, row) = ui.allocate_exact_size(vec2(cell * items.len() as f32, size.y), Sense::hover());
    let mut clicked = None;
    ui.painter().rect_filled(rect, 4.0, theme::BUTTON);
    for (i, &(icon, tip, enabled)) in items.iter().enumerate() {
        let r = Rect::from_min_size(rect.min + vec2(cell * i as f32, 0.0), size);
        let resp = ui.interact(r, row.id.with(i), if enabled { Sense::click() } else { Sense::hover() });
        let p = ui.painter();
        if i == current {
            p.rect_filled(r, 4.0, theme::BUTTON_ACTIVE);
        } else if enabled && resp.hovered() {
            p.rect_filled(r, 4.0, theme::BUTTON_HOVER);
        }
        if i > 0 {
            p.vline(r.left(), r.y_range(), Stroke::new(1.0, theme::OUTLINE));
        }
        let color = if !enabled { theme::TEXT_DISABLED } else if i == current { Color32::WHITE } else { theme::TEXT };
        icons::paint(p, Rect::from_center_size(r.center(), vec2(16.0, 16.0)), icon, color);
        let tip_text = tip.to_string();
        resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, i == current, &tip_text));
        if resp.on_hover_text(tip).clicked() {
            clicked = Some(i);
        }
    }
    let p = ui.painter();
    p.rect_stroke(rect, 4.0, Stroke::new(1.0, theme::OUTLINE), StrokeKind::Inside);
    if accent && current < items.len() {
        let r = Rect::from_min_size(rect.min + vec2(cell * current as f32, 0.0), size);
        p.rect_stroke(r, 4.0, Stroke::new(2.0, theme::ACCENT), StrokeKind::Inside);
    }
    clicked
}

/// The 1×24 line between groups of quick actions.
pub fn group_divider(ui: &mut Ui) {
    let (rect, _) = ui.allocate_exact_size(vec2(9.0, 24.0), Sense::hover());
    ui.painter().vline(rect.center().x, rect.y_range(), Stroke::new(1.0, theme::GROUP_DIVIDER));
}

/// The bracket joining W and H (or Blur X and Y) at the right of their fields,
/// with the link toggle in its middle; returns its response.
pub fn link_bracket(ui: &mut Ui, top: Pos2, bottom: Pos2, linked: bool, enabled: bool) -> Response {
    let p = ui.painter();
    let stroke = Stroke::new(1.0, theme::GROUP_DIVIDER);
    let x = top.x + 4.0;
    let mid = (top.y + bottom.y) / 2.0;
    p.line(vec![top, pos2(x, top.y), pos2(x, mid - 9.0)], stroke);
    p.line(vec![pos2(x, mid + 9.0), pos2(x, bottom.y), bottom], stroke);
    let rect = Rect::from_center_size(pos2(x, mid), vec2(16.0, 16.0));
    let resp = ui.interact(rect, ui.id().with(("link", top.y as i32)), if enabled { Sense::click() } else { Sense::hover() });
    let color = if !enabled { theme::TEXT_DISABLED } else if linked || resp.hovered() { Color32::WHITE } else { theme::TEXT_DIM };
    icons::paint(ui.painter(), rect, if linked { Icon::Link } else { Icon::Unlink }, color);
    resp.widget_info(|| WidgetInfo::selected(WidgetType::Button, enabled, linked, "Link"));
    resp.on_hover_text(if linked { "Unlink width and height" } else { "Link width and height" })
}
