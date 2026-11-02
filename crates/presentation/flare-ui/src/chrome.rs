//! Shared look of the interface: icon buttons, section headers and the "not built yet" marking.
//!
//! Anything the interface shows before the feature behind it exists is built from the mock
//! helpers here, so it looks right, says plainly that it does nothing yet, and is easy to find
//! and replace when the feature lands.

use egui::{Response, Sense, Ui, Vec2};

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
    icon_button_for(ui, icon, size, active, enabled, hint, false)
}

/// [`icon_button`], and `unbuilt` says the feature does not exist yet, so the hint says so. A
/// button that is only disabled for now (Undo with nothing to undo) keeps its plain name.
pub fn icon_button_for(
    ui: &mut Ui,
    icon: &str,
    size: f32,
    active: bool,
    enabled: bool,
    hint: &str,
    unbuilt: bool,
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
                theme::accent().gamma_multiply(0.35)
            } else {
                theme::hover()
            };
            ui.painter().rect_filled(rect, 4.0, fill);
        }
        if active {
            ui.painter().rect_stroke(
                rect,
                4.0,
                egui::Stroke::new(1.0, theme::accent()),
                egui::StrokeKind::Inside,
            );
        }
        let tint = if !enabled {
            theme::text_disabled()
        } else if active || hovered {
            theme::text_strong()
        } else {
            theme::text()
        };
        icons::image(icon, size, tint).paint_at(ui, rect.shrink(pad));
    }
    if unbuilt {
        response.on_hover_text(format!("{hint} ({NOT_YET})"))
    } else {
        response.on_hover_text(hint)
    }
}

/// An icon button for something that does not exist yet: drawn normally, dimmed, and says so.
pub fn mock_icon_button(ui: &mut Ui, icon: &str, size: f32, hint: &str) -> Response {
    icon_button_for(ui, icon, size, false, false, hint, true)
}

/// A small heading that opens a group of controls in a panel.
pub fn section(ui: &mut Ui, icon: &str, title: &str) {
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.add(icons::image(icon, 14.0, theme::text_dim()));
        ui.label(egui::RichText::new(title).strong().color(theme::text()));
    });
    ui.add_space(2.0);
    let rect = ui.available_rect_before_wrap();
    ui.painter().hline(
        rect.x_range(),
        rect.top(),
        egui::Stroke::new(1.0, theme::border()),
    );
    ui.add_space(4.0);
}

/// A category in a dialog: a heading with its picture, and under it the controls in a box that
/// sits lower than the dialog around it, so each category reads as its own.
pub fn group<R>(ui: &mut Ui, icon: &str, title: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.add(icons::image(icon, 16.0, theme::text_dim()));
        ui.label(
            egui::RichText::new(title)
                .strong()
                .size(15.0)
                .color(theme::text()),
        );
    });
    ui.add_space(4.0);
    egui::Frame::new()
        .fill(theme::field())
        .stroke(egui::Stroke::new(1.0, theme::border()))
        .corner_radius(6)
        .inner_margin(egui::Margin::symmetric(10, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            add(ui)
        })
        .inner
}

/// A row of "label: control" with the label in a fixed column so controls line up.
pub fn row(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui)) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [64.0, 18.0],
            egui::Label::new(egui::RichText::new(label).color(theme::text_dim())),
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
        ui.add(icons::image(icon, 28.0, theme::text_disabled()));
        ui.add_space(4.0);
        ui.label(egui::RichText::new(message).color(theme::text_dim()));
    });
}

/// A block of `height` across the available width with what `add` draws centered in it, both
/// ways. Returns the block's rectangle and the rectangle the content took.
pub fn centered_block(ui: &mut Ui, height: f32, add: impl Fn(&mut Ui)) -> (egui::Rect, egui::Rect) {
    let width = ui.available_width();
    let (block, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    let layout = egui::Layout::top_down(egui::Align::Center);
    // Draw it once unseen to learn how tall it is, then again with the space above it that
    // puts it in the middle.
    let mut wanted = 0.0;
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(block)
            .layout(layout)
            .sizing_pass()
            .invisible(),
        |ui| {
            add(ui);
            wanted = ui.min_rect().height();
        },
    );
    let mut content = egui::Rect::NOTHING;
    ui.scope_builder(
        egui::UiBuilder::new().max_rect(block).layout(layout),
        |ui| {
            ui.add_space(((height - wanted) / 2.0).max(0.0));
            let top = ui.cursor().min.y;
            add(ui);
            content = egui::Rect::from_min_max(
                egui::pos2(block.left(), top),
                egui::pos2(block.right(), ui.min_rect().max.y),
            );
        },
    );
    (block, content)
}

/// A small icon and a text label on one row, their middles on one line. Returns the icon's
/// rectangle and the label's response.
pub fn icon_and_label(
    ui: &mut Ui,
    icon: &str,
    icon_size: f32,
    text: egui::RichText,
) -> (egui::Rect, egui::Response) {
    ui.horizontal(|ui| {
        let icon = ui
            .add(icons::image(icon, icon_size, theme::text_dim()))
            .rect;
        let label = ui.add(egui::Label::new(text).sense(Sense::click()));
        (icon, label)
    })
    .inner
}

/// A clickable breadcrumb: the text with an underline a little below it, so the line does not
/// crowd the letters.
pub fn crumb(ui: &mut Ui, text: egui::RichText) -> egui::Response {
    let response = ui
        .add(egui::Label::new(text).sense(Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    let y = response.rect.bottom() + CRUMB_UNDERLINE_DROP;
    ui.painter().hline(
        response.rect.x_range(),
        y,
        egui::Stroke::new(1.0, ui.visuals().hyperlink_color),
    );
    response
}

/// How far below the text's box a breadcrumb's underline sits.
pub const CRUMB_UNDERLINE_DROP: f32 = 1.0;

/// A segmented control: the choices side by side in a recessed track, the chosen one raised.
/// It is not a row of tabs: it switches what one panel shows, so it looks unlike the dock's tab
/// bar. Returns the index clicked, if any.
pub fn segmented(ui: &mut Ui, labels: &[&str], selected: usize) -> Option<usize> {
    let height = 24.0;
    let width = ui.available_width();
    let (track, _) = ui.allocate_exact_size(egui::vec2(width, height), Sense::hover());
    let painter = ui.painter();
    painter.rect(
        track,
        6.0,
        theme::field(),
        egui::Stroke::new(1.0, theme::border()),
        egui::StrokeKind::Inside,
    );
    let count = labels.len().max(1) as f32;
    let segment = (track.width() - 4.0) / count;
    let mut clicked = None;
    for (index, label) in labels.iter().enumerate() {
        let rect = egui::Rect::from_min_size(
            track.min + egui::vec2(2.0 + segment * index as f32, 2.0),
            egui::vec2(segment, height - 4.0),
        );
        let response = ui.interact(
            rect,
            ui.make_persistent_id(("segment", index)),
            Sense::click(),
        );
        let chosen = index == selected;
        if chosen {
            painter.rect_filled(rect, 4.0, theme::panel_raised());
        } else if response.hovered() {
            painter.rect_filled(rect, 4.0, theme::hover().gamma_multiply(0.6));
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(13.0),
            if chosen {
                theme::text()
            } else {
                theme::text_dim()
            },
        );
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

#[cfg(test)]
#[path = "chrome_tests.rs"]
mod tests;
