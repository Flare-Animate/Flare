//! Shared look of the interface: icon buttons, section headers and the "not built yet" marking.
//!
//! Anything the interface shows before the feature behind it exists is built from the mock
//! helpers here, so it looks right, says plainly that it does nothing yet, and is easy to find
//! and replace when the feature lands.

use egui::{Color32, Response, Sense, Ui, Vec2};

use crate::icons;
use crate::theme;

/// What an unbuilt feature says when you hover it.
pub const NOT_YET: &str = "Not available yet";

/// A square icon button. `active` marks the current choice; `enabled` greys it out.
pub fn icon_button(
    ui: &mut Ui,
    icon: &str,
    size: f32,
    active: bool,
    enabled: bool,
    hint: &str,
) -> Response {
    // Snug around the picture: a strip of these should look like a toolbar, not a grid of keys.
    let pad = 3.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::splat(size + pad * 2.0),
        if enabled {
            Sense::click()
        } else {
            Sense::hover()
        },
    );
    if ui.is_rect_visible(rect) {
        let hovered = response.hovered() && enabled;
        if active || hovered {
            let fill = if active {
                theme::ACCENT.gamma_multiply(0.35)
            } else {
                theme::HOVER
            };
            ui.painter().rect_filled(rect, 4.0, fill);
        }
        if active {
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, theme::ACCENT),
                egui::StrokeKind::Inside,
            );
        }
        let tint = if !enabled {
            theme::TEXT_DISABLED
        } else if active || hovered {
            Color32::WHITE
        } else {
            theme::TEXT
        };
        icons::image(icon, size, tint).paint_at(ui, rect.shrink(pad));
    }
    if enabled {
        response.on_hover_text(hint)
    } else {
        response.on_hover_text(format!("{hint} ({NOT_YET})"))
    }
}

/// An icon button for something that does not exist yet: drawn normally, dimmed, and says so.
pub fn mock_icon_button(ui: &mut Ui, icon: &str, size: f32, hint: &str) -> Response {
    icon_button(ui, icon, size, false, false, hint)
}

/// A small heading that opens a group of controls in a panel.
pub fn section(ui: &mut Ui, icon: &str, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add(icons::image(icon, 14.0, theme::TEXT_DIM));
        ui.label(egui::RichText::new(title).strong().color(theme::TEXT));
    });
    ui.add_space(2.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        egui::Stroke::new(1.0, theme::BORDER),
    );
    ui.add_space(4.0);
}

/// A row of "label: control" with the label in a fixed column so controls line up.
pub fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [64.0, 18.0],
            egui::Label::new(egui::RichText::new(label).color(theme::TEXT_DIM)),
        );
        add(ui);
    });
}

/// A numeric field that cannot be edited yet.
pub fn mock_number(ui: &mut Ui, value: &str, suffix: &str) {
    let mut text = format!("{value}{suffix}");
    ui.add_enabled(
        false,
        egui::TextEdit::singleline(&mut text).desired_width(58.0),
    )
    .on_disabled_hover_text(NOT_YET);
}

/// Empty-state text for a panel that has nothing to show.
pub fn empty_state(ui: &mut Ui, icon: &str, message: &str) {
    ui.add_space(16.0);
    ui.vertical_centered(|ui| {
        ui.add(icons::image(icon, 28.0, theme::TEXT_DISABLED));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(message).color(theme::TEXT_DIM));
    });
}
