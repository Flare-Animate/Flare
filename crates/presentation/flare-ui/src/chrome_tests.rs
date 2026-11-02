//! Tests of the shared widgets in `chrome.rs`.
#![allow(clippy::expect_used)]

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

/// The tooltip a button shows when the pointer rests on it, drawn by `add`.
fn tooltip_of(add: impl Fn(&mut Ui)) -> String {
    let ctx = egui::Context::default();
    let at = egui::pos2(14.0, 14.0);
    let mut text = String::new();
    for step in 0..12 {
        let events = if step == 0 {
            vec![egui::Event::PointerMoved(at)]
        } else {
            vec![]
        };
        let input = egui::RawInput {
            time: Some(f64::from(step) * 0.2),
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(200.0, 100.0),
            )),
            events,
            ..egui::RawInput::default()
        };
        let mut out = ctx.run_ui(input, |ui| add(ui));
        out.textures_delta.clear();
        for shape in &out.shapes {
            if let egui::epaint::Shape::Text(t) = &shape.shape {
                text.push_str(t.galley.text());
            }
        }
    }
    text
}

#[test]
fn a_group_has_its_heading_above_a_box_that_holds_its_controls() {
    let ctx = egui::Context::default();
    icons::install(&ctx);
    let input = egui::RawInput {
        time: Some(0.0),
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(400.0, 300.0),
        )),
        ..egui::RawInput::default()
    };
    let mut out = ctx.run_ui(input, |ui| {
        group(ui, "gear", "General", |ui| {
            ui.label("Autosave every");
        });
    });
    out.textures_delta.clear();
    let mut boxed = None;
    let (mut heading, mut control) = (None, None);
    for shape in &out.shapes {
        match &shape.shape {
            egui::epaint::Shape::Rect(r) if r.fill == theme::field() => boxed = Some(r.rect),
            egui::epaint::Shape::Text(t) if t.galley.text() == "General" => {
                heading = Some(t.visual_bounding_rect());
            }
            egui::epaint::Shape::Text(t) if t.galley.text() == "Autosave every" => {
                control = Some(t.visual_bounding_rect());
            }
            _ => {}
        }
    }
    let (boxed, heading, control) = (
        boxed.expect("the box is drawn"),
        heading.expect("heading"),
        control.expect("control"),
    );
    assert!(
        boxed.contains_rect(control),
        "the control is inset in the box"
    );
    assert!(control.min.x - boxed.min.x >= 8.0, "with room around it");
    assert!(heading.max.y <= boxed.min.y, "the heading is above the box");
}

#[test]
fn a_button_that_is_only_off_for_now_keeps_its_plain_name_and_only_unbuilt_ones_say_so() {
    let off = tooltip_of(|ui| {
        icon_button(ui, "undo", 16.0, false, false, "Undo");
    });
    assert!(off.contains("Undo"), "{off}");
    assert!(!off.contains("Not available"), "{off}");
    let unbuilt = tooltip_of(|ui| {
        mock_icon_button(ui, "undo", 16.0, "Undo");
    });
    assert!(unbuilt.contains("Not available yet"), "{unbuilt}");
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
