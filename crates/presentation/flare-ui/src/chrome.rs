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
        let icon = ui.add(icons::image(icon, icon_size, theme::TEXT_DIM)).rect;
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
        theme::FIELD,
        egui::Stroke::new(1.0, theme::BORDER),
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
            painter.rect_filled(rect, 4.0, theme::PANEL_RAISED);
        } else if response.hovered() {
            painter.rect_filled(rect, 4.0, theme::HOVER.gamma_multiply(0.6));
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            *label,
            egui::FontId::proportional(13.0),
            if chosen { theme::TEXT } else { theme::TEXT_DIM },
        );
        if response.clicked() {
            clicked = Some(index);
        }
    }
    clicked
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    fn pass(ctx: &egui::Context, time: f64, events: Vec<egui::Event>, chosen: &mut usize) {
        let input = egui::RawInput {
            time: Some(time),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 200.0),
            )),
            events,
            ..egui::RawInput::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            if let Some(index) = segmented(ui, &["One", "Two", "Three", "Four"], *chosen) {
                *chosen = index;
            }
        });
        output.textures_delta.clear();
    }

    #[test]
    fn content_is_centered_in_its_block_whatever_it_is() {
        let ctx = egui::Context::default();
        for lines in [1, 2, 3] {
            let input = egui::RawInput {
                time: Some(0.0),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 300.0),
                )),
                ..egui::RawInput::default()
            };
            let mut measured = None;
            let mut output = ctx.run_ui(input, |ui| {
                measured = Some(centered_block(ui, 120.0, |ui| {
                    for _ in 0..lines {
                        ui.label("No symbol selected");
                    }
                }));
            });
            output.textures_delta.clear();
            let (block, content) = measured.expect("drawn");
            assert_eq!(block.height(), 120.0);
            assert!(
                (block.center().y - content.center().y).abs() < 1.0,
                "{lines} lines: block {block:?}, content {content:?}"
            );
            assert!((block.center().x - content.center().x).abs() < 1.0);
        }
    }

    #[test]
    fn an_icon_and_its_label_share_a_middle() {
        let ctx = egui::Context::default();
        for size in [10.0, 14.0, 20.0] {
            let input = egui::RawInput {
                time: Some(0.0),
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(400.0, 100.0),
                )),
                ..egui::RawInput::default()
            };
            let mut measured = None;
            let mut output = ctx.run_ui(input, |ui| {
                measured = Some(icon_and_label(
                    ui,
                    "document",
                    size,
                    egui::RichText::new("Untitled"),
                ));
            });
            output.textures_delta.clear();
            let (icon, label) = measured.expect("drawn");
            assert!(
                (icon.center().y - label.rect.center().y).abs() < 0.6,
                "icon {icon:?} against label {:?}",
                label.rect
            );
        }
    }

    #[test]
    fn a_breadcrumbs_underline_is_below_its_text() {
        let ctx = egui::Context::default();
        let input = egui::RawInput {
            time: Some(0.0),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(400.0, 100.0),
            )),
            ..egui::RawInput::default()
        };
        let mut rect = egui::Rect::NOTHING;
        let mut output = ctx.run_ui(input, |ui| {
            rect = crumb(ui, egui::RichText::new("Scene 1")).rect;
        });
        let line_at = rect.bottom() + CRUMB_UNDERLINE_DROP;
        assert!(line_at > rect.bottom(), "the line is under the text's box");
        let underline = output
            .shapes
            .iter()
            .filter_map(|s| match &s.shape {
                egui::epaint::Shape::LineSegment { points, .. } => Some(points[0].y),
                _ => None,
            })
            .any(|y| (y - line_at).abs() < 0.01);
        assert!(underline, "the underline is drawn at {line_at}");
        output.textures_delta.clear();
    }

    #[test]
    fn clicking_a_segment_chooses_it_and_only_that_one() {
        let ctx = egui::Context::default();
        let mut chosen = 0;
        pass(&ctx, 0.0, vec![], &mut chosen);
        // The track fills the width; the third of four segments is around 5/8 of it.
        let at = egui::pos2(400.0 * 0.62, 18.0);
        let click = |pressed| egui::Event::PointerButton {
            pos: at,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: egui::Modifiers::NONE,
        };
        pass(
            &ctx,
            0.1,
            vec![egui::Event::PointerMoved(at), click(true)],
            &mut chosen,
        );
        pass(&ctx, 0.2, vec![click(false)], &mut chosen);
        assert_eq!(chosen, 2, "the third segment was chosen");
    }

    #[test]
    fn an_empty_or_out_of_range_selection_still_draws() {
        let ctx = egui::Context::default();
        let mut chosen = 9;
        pass(&ctx, 0.0, vec![], &mut chosen);
        assert_eq!(chosen, 9, "nothing clicked, nothing changed");
    }
}
