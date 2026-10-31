//! The stroke and fill swatches and the three small buttons under them, in the tool strip.

use super::*;
use crate::workspace::ColorTarget;
use crate::workspace::panels::color_wells::{ROW_WIDTH, WellRects, button_side, height, rects_in};

fn rects(width: f32) -> WellRects {
    rects_in(Rect::from_min_size(
        Pos2::ZERO,
        Vec2::new(width, height(width)),
    ))
}

#[test]
fn the_swatches_and_buttons_fit_the_strip_and_none_overlap() {
    // From a strip one tool wide to a wide one.
    for width in [30.0, 34.0, 38.0, 48.0, 72.0, 120.0] {
        let r = rects(width);
        let all = [
            r.stroke_picture,
            r.stroke,
            r.fill_picture,
            r.fill,
            r.defaults,
            r.swap,
        ];
        let room = Rect::from_min_size(Pos2::ZERO, Vec2::new(width.max(ROW_WIDTH), height(width)));
        for rect in all {
            assert!(
                rect.min.x >= room.min.x - 0.01
                    && rect.max.x <= room.max.x + 0.01
                    && rect.max.y <= room.max.y + 0.01,
                "{rect:?} is outside {room:?} at width {width}"
            );
        }
        for (i, a) in all.iter().enumerate() {
            for b in &all[i + 1..] {
                assert!(!a.intersects(*b), "{a:?} overlaps {b:?} at width {width}");
            }
        }
        // The stroke is over the fill, and the buttons are under both.
        assert!(r.stroke.max.y < r.fill.min.y && r.fill.max.y < r.defaults.min.y);
        // Black and white, then swap, side by side under the swatches.
        assert!(r.defaults.max.x <= r.swap.min.x && (r.defaults.min.y - r.swap.min.y).abs() < 0.01);
        assert!(button_side(width) >= 12.0);
    }
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
fn black_and_white_and_swap_are_the_buttons_under_the_swatches() {
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
