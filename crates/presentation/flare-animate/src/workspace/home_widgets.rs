//! What the Home page is drawn with: its sidebar's items and buttons, and the cards the presets
//! and recent projects are shown on.

use egui::{Align2, FontId, Rect, Sense, Stroke, StrokeKind, Ui, Vec2, pos2, vec2};
use flare_ui::{icons, theme};

const TILE_WIDTH: f32 = 168.0;
const PREVIEW_HEIGHT: f32 = 96.0;
/// Room for the title, and for each line of detail under it.
const LABEL_BASE: f32 = 32.0;
const DETAIL_HEIGHT: f32 = 14.0;

pub(super) fn nav_item(ui: &mut Ui, icon: &str, label: &str, selected: bool) {
    let (rect, response) = ui.allocate_exact_size(vec2(ui.available_width(), 30.0), Sense::click());
    if selected || response.hovered() {
        let fill = if selected {
            theme::panel_raised()
        } else {
            theme::hover()
        };
        ui.painter().rect_filled(rect, 5.0, fill);
    }
    let ink = if ui.is_enabled() {
        theme::text()
    } else {
        theme::text_disabled()
    };
    let image = icons::image(icon, 16.0, ink);
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), Vec2::splat(16.0));
    image.paint_at(ui, icon_rect);
    ui.painter().text(
        pos2(rect.left() + 34.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
}

/// A full-width button with an icon; the primary one is filled with the accent.
pub(super) fn wide_button(
    ui: &mut Ui,
    width: f32,
    icon: &str,
    label: &str,
    primary: bool,
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(vec2(width, 32.0), Sense::click());
    let fill = match (primary, response.hovered()) {
        (true, true) => theme::accent().gamma_multiply(0.9),
        (true, false) => theme::accent().gamma_multiply(0.75),
        (false, true) => theme::hover(),
        (false, false) => theme::panel_raised(),
    };
    ui.painter().rect(
        rect,
        5.0,
        fill,
        Stroke::new(
            1.0,
            if primary {
                theme::accent()
            } else {
                theme::border()
            },
        ),
        StrokeKind::Inside,
    );
    let icon_rect =
        Rect::from_center_size(pos2(rect.left() + 18.0, rect.center().y), Vec2::splat(16.0));
    // On the accent fill, white or black as the accent needs (black on High contrast's yellow);
    // on the plain fill, the palette's own text, which is dark on the light palettes.
    let ink = if primary {
        theme::ink_on(fill)
    } else {
        theme::text()
    };
    icons::image(icon, 16.0, ink).paint_at(ui, icon_rect);
    ui.painter().text(
        pos2(rect.left() + 38.0, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(14.0),
        ink,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A card: a picture drawn by `preview` over a title and a line of detail.
pub(super) fn tile(
    ui: &mut Ui,
    title: &str,
    details: &[&str],
    preview: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    chosen_tile(ui, title, details, false, preview)
}

/// A card as [`tile`] draws it, outlined in the accent color when `chosen`, as the New Document
/// dialog marks the size it will make.
pub(super) fn chosen_tile(
    ui: &mut Ui,
    title: &str,
    details: &[&str],
    chosen: bool,
    preview: impl FnOnce(&egui::Painter, Rect),
) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(
            TILE_WIDTH,
            PREVIEW_HEIGHT + LABEL_BASE + DETAIL_HEIGHT * details.len() as f32,
        ),
        Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter();
        let border = if chosen {
            Stroke::new(2.0, theme::accent())
        } else if response.hovered() {
            Stroke::new(1.0, theme::accent())
        } else {
            Stroke::new(1.0, theme::border())
        };
        painter.rect(rect, 6.0, theme::panel(), border, StrokeKind::Inside);
        let top = Rect::from_min_size(
            rect.min + Vec2::splat(1.0),
            vec2(rect.width() - 2.0, PREVIEW_HEIGHT),
        );
        painter.rect_filled(top, 5.0, theme::stage_surround());
        preview(&painter.with_clip_rect(top), top);
        let text = painter.with_clip_rect(Rect::from_min_max(
            pos2(rect.left() + 10.0, top.bottom()),
            pos2(rect.right() - 10.0, rect.bottom()),
        ));
        text.text(
            pos2(rect.left() + 10.0, top.bottom() + 8.0),
            Align2::LEFT_TOP,
            title,
            FontId::proportional(13.0),
            theme::text(),
        );
        for (row, detail) in details.iter().rev().enumerate() {
            text.text(
                pos2(
                    rect.left() + 10.0,
                    rect.bottom() - 8.0 - row as f32 * DETAIL_HEIGHT,
                ),
                Align2::LEFT_BOTTOM,
                *detail,
                FontId::proportional(11.0),
                theme::text_dim(),
            );
        }
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A frame with the proportions of a stage, centered in `rect`: drawn in the interface's own
/// greys, not as a white sheet, so a row of them reads as shapes to pick between and not as glare.
pub(super) fn stage_glyph(painter: &egui::Painter, rect: Rect, width: u32, height: u32) {
    let room = rect.shrink2(vec2(36.0, 20.0));
    let scale = (room.width() / width as f32).min(room.height() / height as f32);
    let size = vec2(width as f32 * scale, height as f32 * scale);
    let glyph = Rect::from_center_size(rect.center(), size);
    painter.rect(
        glyph,
        3.0,
        theme::panel_raised(),
        Stroke::new(1.5, theme::text_dim()),
        StrokeKind::Inside,
    );
}

/// A plus, for the card that makes a project of a size of the person's choosing.
pub(super) fn plus_glyph(painter: &egui::Painter, rect: Rect) {
    let c = rect.center();
    let stroke = Stroke::new(2.5, theme::text_dim());
    painter.hline((c.x - 14.0)..=(c.x + 14.0), c.y, stroke);
    painter.vline(c.x, (c.y - 14.0)..=(c.y + 14.0), stroke);
}
