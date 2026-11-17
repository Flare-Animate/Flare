//! Flare's color picker: a saturation and brightness square, then a track for each of hue,
//! saturation, brightness and alpha with its number beside it, then the red, green and blue as
//! numbers. egui's own picker draws its tracks with no number on them, so a person could see a
//! hue but not read or type it. Every name is passed in, already in the person's language.

mod hsb;
#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;

pub use hsb::Hsb;

use egui::{
    Color32, Id, Mesh, Popup, PopupCloseBehavior, Pos2, Rect, Response, Sense, Shape, Stroke,
    StrokeKind, Ui, Vec2, vec2,
};

use crate::theme;

/// A part of the color as the picker names it: a letter or two on its row, the whole name when
/// the pointer rests on it.
#[derive(Clone, Debug, Default)]
pub struct ChannelText {
    pub short: String,
    pub name: String,
}

/// Every name the picker shows.
#[derive(Clone, Debug, Default)]
pub struct ColorTexts {
    pub hue: ChannelText,
    pub saturation: ChannelText,
    pub brightness: ChannelText,
    pub alpha: ChannelText,
    pub red: ChannelText,
    pub green: ChannelText,
    pub blue: ChannelText,
}

/// The width of a row's letter, and of the track's number.
const LABEL_WIDTH: f32 = 16.0;
const NUMBER_WIDTH: f32 = 52.0;
/// The height of a track.
const TRACK_HEIGHT: f32 = 14.0;
/// Bands a track's gradient is drawn in, and the cells of the square's.
const TRACK_BANDS: usize = 36;
const SQUARE_CELLS: usize = 16;
/// The widest the picker draws itself, in points.
const MAX_WIDTH: f32 = 320.0;

/// The parts of `color` the picker last edited, so a hue survives a trip through grey or black,
/// which have none: kept only while the color is still the one they made.
fn remembered(ui: &Ui, id: Id, color: Color32) -> Hsb {
    ui.data(|d| d.get_temp::<(Color32, Hsb)>(id))
        .filter(|(made, _)| *made == color)
        .map_or_else(|| Hsb::from_color(color), |(_, hsb)| hsb)
}

/// The picker for `color`; true when it was changed.
pub fn color_picker(ui: &mut Ui, color: &mut Color32, texts: &ColorTexts) -> bool {
    let id = ui.make_persistent_id("flare-color-picker");
    let mut hsb = remembered(ui, id, *color);
    let before = hsb;
    let mut rgb_edit = None;
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 4.0;
        let width = ui.available_width().min(MAX_WIDTH);
        ui.set_max_width(width);
        square(ui, &mut hsb, width);
        let base = hsb;
        let hue_at = |t: f32| {
            Hsb {
                hue: t,
                saturation: 1.0,
                brightness: 1.0,
                alpha: 1.0,
            }
            .to_color()
        };
        track_row(ui, &texts.hue, &mut hsb.hue, 360.0, "\u{b0}", hue_at, false);
        let sat_at = |t: f32| {
            Hsb {
                saturation: t,
                ..base
            }
            .opaque()
        };
        track_row(
            ui,
            &texts.saturation,
            &mut hsb.saturation,
            100.0,
            "%",
            sat_at,
            false,
        );
        let bright_at = |t: f32| {
            Hsb {
                brightness: t,
                ..base
            }
            .opaque()
        };
        track_row(
            ui,
            &texts.brightness,
            &mut hsb.brightness,
            100.0,
            "%",
            bright_at,
            false,
        );
        let alpha_at = |t: f32| Hsb { alpha: t, ..base }.to_color();
        track_row(ui, &texts.alpha, &mut hsb.alpha, 100.0, "%", alpha_at, true);
        rgb_edit = rgb_row(ui, hsb, texts);
    });
    let made = match rgb_edit {
        // Typed red, green or blue: the hue and the rest come from the new color.
        Some(typed) => {
            hsb = Hsb::from_color(typed);
            typed
        }
        None if hsb != before => hsb.to_color(),
        None => return false,
    };
    ui.data_mut(|d| d.insert_temp(id, (made, hsb)));
    let changed = made != *color;
    *color = made;
    changed
}

/// A swatch of `color` that opens the picker under it; the response is marked changed when the
/// picker changed the color. What egui's `color_edit_button_srgba` was, with Flare's picker.
pub fn color_button(ui: &mut Ui, color: &mut Color32, texts: &ColorTexts) -> Response {
    let size = ui.spacing().interact_size;
    let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());
    let open = Popup::is_id_open(ui.ctx(), response.id.with("popup"));
    egui::color_picker::show_color_at(ui.painter(), *color, rect);
    let edge = if open || response.hovered() {
        theme::accent()
    } else {
        theme::border()
    };
    ui.painter()
        .rect_stroke(rect, 2.0, Stroke::new(1.0, edge), StrokeKind::Inside);
    let mut changed = false;
    Popup::menu(&response)
        .id(response.id.with("popup"))
        .close_behavior(PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_width(240.0);
            changed = color_picker(ui, color, texts);
        });
    if changed {
        response.mark_changed();
    }
    response
}

/// Where a press or drag on `rect` lands, from 0 to 1 each way, while it is held there.
fn held_at(response: &Response, rect: Rect) -> Option<Vec2> {
    if !response.is_pointer_button_down_on() {
        return None;
    }
    let p = response.interact_pointer_pos()?;
    let x = ((p.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
    let y = ((p.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
    Some(vec2(x, y))
}

/// The saturation (across) and brightness (up) of the hue in hand.
fn square(ui: &mut Ui, hsb: &mut Hsb, width: f32) {
    let height = (width * 0.55).clamp(60.0, 160.0);
    let (rect, response) = ui.allocate_exact_size(vec2(width, height), Sense::click_and_drag());
    if let Some(at) = held_at(&response, rect) {
        hsb.saturation = at.x;
        hsb.brightness = 1.0 - at.y;
    }
    let mut mesh = Mesh::default();
    let n = SQUARE_CELLS;
    for row in 0..=n {
        for col in 0..=n {
            let (x, y) = (col as f32 / n as f32, row as f32 / n as f32);
            let color = Hsb {
                saturation: x,
                brightness: 1.0 - y,
                ..*hsb
            }
            .opaque();
            let at = Pos2::new(rect.left() + x * width, rect.top() + y * height);
            mesh.colored_vertex(at, color);
        }
    }
    let stride = (n + 1) as u32;
    for row in 0..n as u32 {
        for col in 0..n as u32 {
            let i = row * stride + col;
            mesh.add_triangle(i, i + 1, i + stride);
            mesh.add_triangle(i + 1, i + stride + 1, i + stride);
        }
    }
    let painter = ui.painter_at(rect);
    painter.add(Shape::mesh(mesh));
    let at = Pos2::new(
        rect.left() + hsb.saturation * width,
        rect.top() + (1.0 - hsb.brightness) * height,
    );
    painter.circle_stroke(at, 5.0, Stroke::new(2.0, Color32::BLACK));
    painter.circle_stroke(at, 4.0, Stroke::new(1.5, Color32::WHITE));
}

/// One part of the color: its letter, a track showing what each value of it gives, and its
/// number, from 0 to `scale`. The track and the number edit the same value.
fn track_row(
    ui: &mut Ui,
    text: &ChannelText,
    value: &mut f32,
    scale: f32,
    suffix: &str,
    color_at: impl Fn(f32) -> Color32,
    checkered: bool,
) {
    ui.horizontal(|ui| {
        letter(ui, text);
        let width = (ui.available_width() - NUMBER_WIDTH - ui.spacing().item_spacing.x).max(40.0);
        let (rect, response) =
            ui.allocate_exact_size(vec2(width, TRACK_HEIGHT), Sense::click_and_drag());
        if let Some(at) = held_at(&response, rect) {
            *value = at.x;
        }
        let painter = ui.painter_at(rect);
        if checkered {
            checker(&painter, rect);
        }
        painter.add(Shape::mesh(gradient(rect, color_at)));
        painter.rect_stroke(
            rect,
            0.0,
            Stroke::new(1.0, theme::border()),
            StrokeKind::Inside,
        );
        let x = rect.left() + *value * rect.width();
        let mark = Rect::from_center_size(Pos2::new(x, rect.center().y), vec2(4.0, TRACK_HEIGHT));
        painter.rect(
            mark,
            1.0,
            Color32::WHITE,
            Stroke::new(1.0, Color32::BLACK),
            StrokeKind::Outside,
        );
        response.on_hover_text(&text.name);
        let mut shown = (*value * scale).round();
        let number = ui.add_sized(
            [NUMBER_WIDTH, TRACK_HEIGHT + 4.0],
            egui::DragValue::new(&mut shown)
                .range(0.0..=scale)
                .max_decimals(0)
                .suffix(suffix),
        );
        if number.changed() {
            *value = shown / scale;
        }
        number.on_hover_text(&text.name);
    });
}

/// The red, green and blue as numbers from 0 to 255; the color typed, if one was.
fn rgb_row(ui: &mut Ui, hsb: Hsb, texts: &ColorTexts) -> Option<Color32> {
    let color = hsb.to_color();
    let [mut r, mut g, mut b, a] = color.to_srgba_unmultiplied();
    let mut typed = false;
    ui.horizontal(|ui| {
        for (text, channel) in [
            (&texts.red, &mut r),
            (&texts.green, &mut g),
            (&texts.blue, &mut b),
        ] {
            letter(ui, text);
            typed |= ui
                .add(egui::DragValue::new(channel).range(0..=255))
                .on_hover_text(&text.name)
                .changed();
        }
    });
    typed.then(|| Color32::from_rgba_unmultiplied(r, g, b, a))
}

fn letter(ui: &mut Ui, text: &ChannelText) {
    ui.add_sized(
        [LABEL_WIDTH, TRACK_HEIGHT],
        egui::Label::new(egui::RichText::new(&text.short).color(theme::text_dim())),
    )
    .on_hover_text(&text.name);
}

/// A band across `rect` from `color_at(0)` on the left to `color_at(1)` on the right.
fn gradient(rect: Rect, color_at: impl Fn(f32) -> Color32) -> Mesh {
    let mut mesh = Mesh::default();
    for i in 0..=TRACK_BANDS {
        let t = i as f32 / TRACK_BANDS as f32;
        let x = rect.left() + t * rect.width();
        let color = color_at(t);
        mesh.colored_vertex(Pos2::new(x, rect.top()), color);
        mesh.colored_vertex(Pos2::new(x, rect.bottom()), color);
    }
    for i in 0..TRACK_BANDS as u32 {
        let k = i * 2;
        mesh.add_triangle(k, k + 1, k + 2);
        mesh.add_triangle(k + 1, k + 3, k + 2);
    }
    mesh
}

/// Light and dark squares, for an alpha track to show its see-through end against.
fn checker(painter: &egui::Painter, rect: Rect) {
    let side = rect.height() / 2.0;
    let (light, dark) = (Color32::from_gray(170), Color32::from_gray(90));
    let columns = (rect.width() / side).ceil() as usize;
    for col in 0..columns {
        for row in 0..2 {
            let min = rect.min + vec2(col as f32 * side, row as f32 * side);
            let cell = Rect::from_min_size(min, Vec2::splat(side)).intersect(rect);
            let shade = if (col + row) % 2 == 0 { light } else { dark };
            painter.rect_filled(cell, 0.0, shade);
        }
    }
}
