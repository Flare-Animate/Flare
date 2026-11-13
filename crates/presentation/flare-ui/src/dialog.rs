//! The look every dialog shares: a roomy card in the middle of the window over a dimmed
//! backdrop, a title, labelled rows whose controls line up, and the buttons at the bottom right
//! with the one that does the thing filled in the accent colour. egui's own popup frame is a
//! menu's, too tight for a question someone has to read and answer.

use egui::{Color32, Frame, Id, Margin, Response, RichText, Sense, Stroke, Ui, Vec2, vec2};

use crate::{icons, theme};

/// The room between a dialog's edge and what it holds.
pub const MARGIN: Margin = Margin {
    left: 24,
    right: 24,
    top: 20,
    bottom: 20,
};
/// The width of the label column of [`field`], so the controls of a dialog line up.
pub const LABEL_WIDTH: f32 = 92.0;
/// The smallest a dialog's button is, so short words like OK still make a target.
pub const BUTTON_SIZE: Vec2 = vec2(88.0, 28.0);
/// The height of a [`field`] row.
pub const ROW_HEIGHT: f32 = 28.0;
/// The space above the buttons.
const FOOTER_GAP: f32 = 20.0;

/// The frame of a dialog, in the colours of the palette in use.
pub fn frame(style: &egui::Style) -> Frame {
    Frame::popup(style)
        .inner_margin(MARGIN)
        .corner_radius(10)
        .fill(theme::panel())
        .stroke(Stroke::new(1.0, theme::border()))
}

/// A modal dialog with the house frame. Escape or a click outside asks it to close, as every
/// egui modal does.
pub fn modal(ctx: &egui::Context, id: Id) -> egui::Modal {
    egui::Modal::new(id).frame(frame(&ctx.global_style()))
}

/// The dialog's title, with the room below it the first row needs.
pub fn title(ui: &mut Ui, text: &str) {
    ui.label(
        RichText::new(text)
            .size(15.0)
            .strong()
            .color(theme::text_strong()),
    );
    ui.add_space(12.0);
}

/// A sentence that explains the question, in the quieter text colour.
pub fn message(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).color(theme::text_dim()));
}

/// A row of "label  control", the label in a column [`LABEL_WIDTH`] wide.
pub fn field<R>(ui: &mut Ui, label: &str, add: impl FnOnce(&mut Ui) -> R) -> R {
    ui.horizontal(|ui| {
        ui.set_min_height(ROW_HEIGHT);
        ui.add_sized(
            [LABEL_WIDTH, ROW_HEIGHT],
            egui::Label::new(RichText::new(label).color(theme::text_dim())),
        );
        add(ui)
    })
    .inner
}

/// The buttons at the bottom: `left` at the left edge (a choice that gives something up, as
/// Don't Save), `right` laid out from the right edge inwards, so the first button added there is
/// the rightmost and should be the one that does the thing.
pub fn footer(ui: &mut Ui, left: impl FnOnce(&mut Ui), right: impl FnOnce(&mut Ui)) {
    ui.add_space(FOOTER_GAP);
    egui::Sides::new().spacing(8.0).show(ui, left, |ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        right(ui);
    });
}

/// A dialog button: at least [`BUTTON_SIZE`], with rounded corners.
pub fn button(ui: &mut Ui, text: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(text))
            .min_size(BUTTON_SIZE)
            .corner_radius(6),
    )
}

/// The button that does what the dialog is for, filled in the accent colour. `enabled` false
/// dims it, while what was filled in cannot be used.
pub fn primary_button(ui: &mut Ui, text: &str, enabled: bool) -> Response {
    let button = egui::Button::new(RichText::new(text).color(Color32::WHITE).strong())
        .min_size(BUTTON_SIZE)
        .corner_radius(6)
        .fill(theme::accent());
    ui.add_enabled(enabled, button)
}

/// Choices side by side as cards, each an icon over its name, the chosen one outlined in the
/// accent colour. For a choice whose options are worth seeing, as the type of a new symbol.
/// Returns the index clicked, if any.
pub fn choice_cards(ui: &mut Ui, choices: &[(&str, &str)], chosen: usize) -> Option<usize> {
    const CARD: Vec2 = vec2(92.0, 62.0);
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 8.0;
        for (index, (icon, label)) in choices.iter().enumerate() {
            let (rect, response) = ui.allocate_exact_size(CARD, Sense::click());
            let is_chosen = index == chosen;
            let painter = ui.painter();
            let fill = if is_chosen {
                theme::accent().gamma_multiply(0.18)
            } else if response.hovered() {
                theme::hover()
            } else {
                theme::field()
            };
            let stroke = if is_chosen {
                Stroke::new(1.5, theme::accent())
            } else {
                Stroke::new(1.0, theme::border())
            };
            painter.rect(rect, 8.0, fill, stroke, egui::StrokeKind::Inside);
            let ink = if is_chosen || response.hovered() {
                theme::text_strong()
            } else {
                theme::text_dim()
            };
            icons::image(icon, 22.0, ink).paint_at(
                ui,
                egui::Rect::from_center_size(
                    rect.center_top() + vec2(0.0, 22.0),
                    Vec2::splat(22.0),
                ),
            );
            ui.painter().text(
                rect.center_top() + vec2(0.0, 47.0),
                egui::Align2::CENTER_CENTER,
                *label,
                egui::FontId::proportional(12.0),
                ink,
            );
            let response = response.on_hover_text(*label);
            if response.clicked() {
                clicked = Some(index);
            }
        }
    });
    clicked
}

/// Animate's registration grid: nine squares, read left to right and top to bottom, the chosen
/// one filled. Returns the index clicked, if any.
pub fn point_grid(ui: &mut Ui, chosen: usize) -> Option<usize> {
    const CELL: f32 = 12.0;
    const GAP: f32 = 6.0;
    let side = 3.0 * CELL + 2.0 * GAP;
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(side), Sense::click());
    let painter = ui.painter();
    let cell = |index: usize| {
        let (column, row) = ((index % 3) as f32, (index / 3) as f32);
        egui::Rect::from_min_size(
            rect.min + vec2(column * (CELL + GAP), row * (CELL + GAP)),
            Vec2::splat(CELL),
        )
    };
    let hovered = response
        .hover_pos()
        .and_then(|at| (0..9).find(|&i| cell(i).expand(GAP / 2.0).contains(at)));
    for index in 0..9 {
        let square = cell(index);
        if index == chosen {
            painter.rect_filled(square, 2.0, theme::accent());
        } else {
            let edge = if hovered == Some(index) {
                theme::text_strong()
            } else {
                theme::text_dim()
            };
            painter.rect_stroke(
                square,
                2.0,
                Stroke::new(1.0, edge),
                egui::StrokeKind::Inside,
            );
        }
    }
    if response.clicked() { hovered } else { None }
}

#[cfg(test)]
#[path = "dialog_tests.rs"]
mod tests;
