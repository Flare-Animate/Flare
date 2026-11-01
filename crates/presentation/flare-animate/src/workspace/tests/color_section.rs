//! The stroke and fill squares and the two small buttons beside or under them, in the tool strip.

use super::*;
use crate::workspace::ColorTarget;
use crate::workspace::panels::color_wells::{Metrics, WellRects, rects_with};

/// Where the parts are in a strip `width` wide whose left edge is at x = 0: the box is centered
/// across the strip.
fn rects(width: f32) -> WellRects {
    let m = Metrics::for_width(width);
    rects_with(Pos2::new(width / 2.0 - m.width() / 2.0, 0.0), m)
}

#[test]
fn the_squares_and_buttons_fit_the_strip_and_the_buttons_sit_side_by_side_above_them() {
    // From a strip one tool wide to a wide one.
    for width in [30.0, 34.0, 38.0, 47.0, 48.0, 72.0, 120.0] {
        let m = Metrics::for_width(width);
        assert!(
            m.width() <= width.max(25.0),
            "the box fits a strip {width} wide"
        );
        let r = rects_with(Pos2::ZERO, m);
        let room = Rect::from_min_size(Pos2::ZERO, Vec2::new(m.width(), m.height()));
        for rect in [r.fill, r.stroke, r.defaults, r.swap] {
            assert!(
                rect.min.x >= -0.01
                    && rect.max.x <= room.max.x + 0.01
                    && rect.max.y <= room.max.y + 0.01,
                "{rect:?} is outside {room:?} at width {width}"
            );
        }
        // The two squares overlap on purpose, the stroke in front, down and to the right.
        assert!(r.stroke.min.x > r.fill.min.x && r.stroke.min.y > r.fill.min.y);
        assert!(r.stroke.intersects(r.fill));
        // The buttons touch neither square nor each other, and sit side by side above the
        // squares as in Animate's toolbar: black and white on the left, swap on the right.
        for button in [r.defaults, r.swap] {
            assert!(!button.intersects(r.fill) && !button.intersects(r.stroke));
        }
        assert!(!r.defaults.intersects(r.swap));
        assert!(r.defaults.max.y <= r.fill.min.y && r.defaults.max.y <= r.stroke.min.y);
        assert!((r.defaults.min.y - r.swap.min.y).abs() < 0.01, "one row");
        assert!(
            r.defaults.max.x <= r.swap.min.x,
            "black and white is on the left"
        );
    }
    // A narrow strip gets the smaller box, a wide one the full size.
    assert!(Metrics::for_width(34.0).width() < Metrics::for_width(120.0).width());
}

/// The section in a 40 px wide column at the top left of a roomy window, as in a default strip,
/// so its picker has room to open.
fn strip(ws: &mut Workspace, ui: &mut egui::Ui) {
    let column = Rect::from_min_size(Pos2::ZERO, Vec2::new(40.0, 200.0));
    ui.scope_builder(egui::UiBuilder::new().max_rect(column), |ui| {
        ws.color_wells(ui)
    });
}

/// One pass of just the color section in a 40 px wide column (a default strip), with `events`.
fn pass_at(ws: &mut Workspace, ctx: &egui::Context, time: f64, events: Vec<Event>) {
    let raw = RawInput {
        time: Some(time),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
        events,
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(raw, |ui| strip(ws, ui));
    out.textures_delta.clear();
}

fn click(ws: &mut Workspace, ctx: &egui::Context, time: f64, at: Pos2) {
    let press = |pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    };
    pass_at(ws, ctx, time, vec![Event::PointerMoved(at)]);
    pass_at(ws, ctx, time + 0.1, vec![press(true)]);
    pass_at(ws, ctx, time + 0.2, vec![press(false)]);
    pass_at(ws, ctx, time + 0.3, vec![]);
}

fn centers() -> WellRects {
    rects(40.0)
}

#[test]
fn black_and_white_and_swap_are_the_buttons_by_the_swatches() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    pass_at(&mut ws, &ctx, 0.0, vec![]);
    ws.paint.stroke_color = Color32::from_rgb(10, 20, 30);
    ws.paint.fill = Color32::from_rgb(200, 100, 50);
    click(&mut ws, &ctx, 0.1, centers().swap.center());
    assert_eq!(ws.paint.stroke_color, Color32::from_rgb(200, 100, 50));
    assert_eq!(ws.paint.fill, Color32::from_rgb(10, 20, 30), "swapped");
    click(&mut ws, &ctx, 1.0, centers().defaults.center());
    assert_eq!(ws.paint.stroke_color, Color32::BLACK);
    assert_eq!(ws.paint.fill, Color32::WHITE, "black and white");
}

/// Where the text `wanted` was drawn in a pass of the section.
fn text_at(ws: &mut Workspace, ctx: &egui::Context, time: f64, wanted: &str) -> Option<Pos2> {
    let raw = RawInput {
        time: Some(time),
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0))),
        ..RawInput::default()
    };
    let mut out = ctx.run_ui(raw, |ui| strip(ws, ui));
    out.textures_delta.clear();
    out.shapes.iter().rev().find_map(|s| match &s.shape {
        egui::epaint::Shape::Text(t) if t.galley.text() == wanted => {
            Some(t.visual_bounding_rect().center())
        }
        _ => None,
    })
}

#[test]
fn no_color_is_in_each_swatchs_picker_and_there_is_no_button_for_it_in_the_strip() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    let mut tick = |step: f64| {
        time += step;
        time
    };
    pass_at(&mut ws, &ctx, tick(0.1), vec![]);
    assert!(
        text_at(&mut ws, &ctx, tick(0.1), "No color").is_none(),
        "nothing in the strip itself says no color"
    );
    for (swatch, target) in [
        (centers().stroke.center(), ColorTarget::Stroke),
        (centers().fill.center(), ColorTarget::Fill),
    ] {
        egui::Popup::close_all(&ctx);
        pass_at(&mut ws, &ctx, tick(0.1), vec![]);
        click(&mut ws, &ctx, tick(0.1), swatch);
        tick(0.5); // a click takes three more passes
        assert_eq!(ws.color_target, target, "choosing a swatch selects it");
        for _ in 0..3 {
            pass_at(&mut ws, &ctx, tick(0.1), vec![]);
        }
        let none = text_at(&mut ws, &ctx, tick(0.1), "No color").expect("the picker has No color");
        let (stroke_before, fill_before) = (ws.paint.stroke_color, ws.paint.fill);
        click(&mut ws, &ctx, tick(0.1), none);
        tick(0.5);
        match target {
            ColorTarget::Stroke => {
                assert_eq!(ws.paint.stroke_color.a(), 0, "no stroke");
                assert_eq!(ws.paint.fill, fill_before, "the fill is untouched");
            }
            _ => {
                assert_eq!(ws.paint.fill.a(), 0, "no fill");
                assert_eq!(
                    ws.paint.stroke_color, stroke_before,
                    "the stroke is untouched"
                );
            }
        }
    }
}

#[test]
fn a_shape_drawn_with_no_stroke_has_a_stroke_nobody_can_see() {
    let mut ws = workspace();
    ws.paint.stroke_color = Color32::TRANSPARENT;
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
    let shape = drawn_shape(&ws, 1);
    assert!(
        shape.stroke().is_none_or(|s| s.color().a == 0),
        "no stroke shows: {:?}",
        shape.stroke()
    );
}
