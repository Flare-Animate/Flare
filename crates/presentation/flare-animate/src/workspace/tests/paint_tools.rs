//! The Paint Bucket and the Eyedropper act on what is under the click: a fill, the inside of an
//! empty outline, or a line.

use super::*;
use flare_core::DocumentPoint;
use flare_document::{Gradient, PathSegment, Stroke as ShapeStroke};

const BLUE: Color32 = Color32::from_rgb(20, 40, 200);

/// A workspace whose stage shows at 100%, so a screen point is a stage unit.
fn at_full_size() -> Workspace {
    let mut ws = workspace();
    ws.set_stage_size(550, 400);
    ws.stage_panel = Rect::from_min_size(Pos2::ZERO, Vec2::new(550.0, 400.0));
    ws
}

fn point(x: f64, y: f64) -> DocumentPoint {
    DocumentPoint::new(x, y).expect("finite")
}

/// A square outline from (100, 100) to (200, 200) with no fill, drawn as a pencil line is: open,
/// its last side stopping `gap` short of where it started.
fn hand_drawn_loop(ws: &mut Workspace, gap: f64) {
    let path = vec![
        PathSegment::MoveTo(point(100.0, 100.0)),
        PathSegment::LineTo(point(200.0, 100.0)),
        PathSegment::LineTo(point(200.0, 200.0)),
        PathSegment::LineTo(point(100.0, 200.0)),
        PathSegment::LineTo(point(100.0, 100.0 + gap)),
    ];
    let stroke = ShapeStroke::new(2.0, Rgba8::opaque(9, 9, 9)).expect("stroke");
    let shape = Shape::new(ShapeId::new(), path, Rgba8::TRANSPARENT)
        .expect("shape")
        .with_stroke(stroke);
    ws.draw_shape(shape, "loop");
}

fn rgba(c: Color32) -> Rgba8 {
    let [r, g, b, a] = c.to_array();
    Rgba8::new(r, g, b, a)
}

fn click_event(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Everything on the stage made into one group, which is a symbol instance.
fn group_everything(ws: &mut Workspace) {
    ws.set_tool(Tool::Select);
    ws.select_all();
    ws.perform(MenuAction::Group);
    assert!(ws.selection_has_instance(), "grouped");
}

#[test]
fn the_bucket_fills_the_inside_of_an_outline_clicked_on_the_stage() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    // An outline with no fill, dragged out with the Rectangle.
    ws.paint.fill = Color32::TRANSPARENT;
    ws.paint.stroke_enabled = true;
    ws.set_tool(Tool::Rectangle);
    let (from, to) = (Pos2::new(450.0, 300.0), Pos2::new(750.0, 420.0));
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(from)]));
    pass(&mut ws, &ctx, input(0.3, vec![click_event(from, true)]));
    for step in 1..=10 {
        let at = from + (to - from) * (step as f32 / 10.0);
        let time = 0.3 + 0.02 * f64::from(step);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    pass(&mut ws, &ctx, input(0.6, vec![click_event(to, false)]));
    assert_eq!(drawn_shape(&ws, 1).fill(), Rgba8::TRANSPARENT);
    let steps = ws.active().undo_labels().count();

    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    let middle = Pos2::new(600.0, 360.0);
    pass(&mut ws, &ctx, input(1.0, vec![Event::PointerMoved(middle)]));
    pass(&mut ws, &ctx, input(1.1, vec![click_event(middle, true)]));
    pass(&mut ws, &ctx, input(1.15, vec![click_event(middle, false)]));
    let filled = drawn_shape(&ws, 1);
    assert_eq!(filled.fill(), rgba(BLUE), "the empty inside is filled");
    assert!(filled.stroke().is_some(), "the outline stays");
    assert_eq!(ws.active().undo_labels().count(), steps + 1);
    ws.undo();
    assert_eq!(drawn_shape(&ws, 1).fill(), Rgba8::TRANSPARENT);
}

#[test]
fn the_bucket_closes_a_small_gap_in_a_hand_drawn_loop_but_not_a_wide_one() {
    let mut ws = at_full_size();
    hand_drawn_loop(&mut ws, 6.0);
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    assert_eq!(drawn_shape(&ws, 1).fill(), rgba(BLUE));

    let mut ws = at_full_size();
    hand_drawn_loop(&mut ws, 60.0);
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    let before = ws.document().clone();
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    assert_eq!(
        ws.document(),
        &before,
        "a line left wide open has no inside"
    );
    ws.click_with_tool(Pos2::new(200.0, 150.0), false);
    assert_eq!(ws.document(), &before, "clicking the line fills nothing");
}

#[test]
fn the_bucket_pours_the_gradient_when_one_is_on() {
    let mut ws = at_full_size();
    rect(&mut ws); // x 10..110, y 10..60
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.paint.gradient_enabled = true;
    ws.paint.gradient_end = Color32::WHITE;
    ws.paint.gradient_vertical = true;
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    let gradient = drawn_shape(&ws, 1).gradient().expect("a gradient");
    assert_eq!(
        (gradient.start(), gradient.end()),
        (rgba(BLUE), Rgba8::WHITE)
    );
    let (from, to) = (gradient.from().as_vec(), gradient.to().as_vec());
    assert_eq!((from.y, to.y), (10.0, 60.0), "top to bottom of the shape");
}

#[test]
fn the_bucket_leaves_locked_layers_and_says_how_to_paint_inside_a_group() {
    let mut ws = at_full_size();
    rect(&mut ws);
    let before = ws.document().clone();
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    ws.toggle_layer_locked(layer);
    let locked = ws.document().clone();
    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    assert_eq!(ws.document(), &locked, "a locked layer is not painted");
    ws.toggle_layer_locked(layer);
    assert_eq!(ws.document(), &before);

    ws.commit_rectangle(Pos2::new(200.0, 10.0), Pos2::new(250.0, 60.0));
    group_everything(&mut ws);
    let grouped = ws.document().clone();
    ws.set_tool(Tool::Bucket);
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    assert_eq!(ws.document(), &grouped, "what is inside the group is kept");
    assert!(
        ws.status().is_some_and(|s| s.contains("double-click")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn the_eyedropper_takes_the_whole_stroke_from_a_line() {
    let mut ws = at_full_size();
    hand_drawn_loop(&mut ws, 60.0);
    ws.paint.fill = BLUE;
    ws.paint.stroke_enabled = false;
    ws.paint.stroke_width = 1.0;
    let steps = ws.active().undo_labels().count();
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(200.0, 150.0), false);
    assert_eq!(ws.paint.stroke_color, Color32::from_rgb(9, 9, 9));
    assert_eq!(ws.paint.stroke_width, 2.0);
    assert!(ws.paint.stroke_enabled, "new shapes get the stroke taken");
    assert_eq!(ws.paint.fill, BLUE, "the fill is left as it was");
    assert_eq!(ws.color_target, ColorTarget::Stroke);
    assert_eq!(
        ws.active().undo_labels().count(),
        steps,
        "taking a color changes nothing"
    );
}

#[test]
fn the_eyedropper_looks_through_an_empty_inside_to_the_fill_under_it() {
    let mut ws = at_full_size();
    hand_drawn_loop(&mut ws, 0.0);
    ws.paint.fill = BLUE;
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    assert_eq!(
        ws.paint.fill,
        Color32::WHITE,
        "the stage shows through, never the outline's invisible fill"
    );

    // A red square under the outline shows through it.
    let red = Shape::rect(
        ShapeId::new(),
        0.0,
        0.0,
        300.0,
        300.0,
        Rgba8::opaque(200, 0, 0),
    )
    .expect("rect");
    let mut ws = at_full_size();
    ws.draw_shape(red, "under");
    hand_drawn_loop(&mut ws, 0.0);
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(150.0, 150.0), false);
    assert_eq!(ws.paint.fill, Color32::from_rgb(200, 0, 0));
    assert_eq!(ws.color_target, ColorTarget::Fill);
}

#[test]
fn the_eyedropper_takes_a_gradient_and_reaches_inside_groups() {
    let mut ws = at_full_size();
    let gradient = Gradient::linear(
        point(0.0, 0.0),
        point(100.0, 0.0),
        Rgba8::opaque(1, 2, 3),
        Rgba8::opaque(250, 251, 252),
    )
    .expect("gradient");
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 100.0, 100.0, Rgba8::WHITE)
        .expect("rect")
        .with_gradient(gradient);
    ws.draw_shape(shape, "shaded");
    ws.commit_rectangle(Pos2::new(200.0, 10.0), Pos2::new(250.0, 60.0));
    group_everything(&mut ws);
    ws.paint.gradient_vertical = true;
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(50.0, 50.0), false);
    assert!(ws.paint.gradient_enabled);
    assert_eq!(ws.paint.fill, Color32::from_rgb(1, 2, 3));
    assert_eq!(ws.paint.gradient_end, Color32::from_rgb(250, 251, 252));
    assert!(!ws.paint.gradient_vertical, "it runs across");

    // Taking a solid fill afterwards turns the gradient off.
    let mut ws2 = at_full_size();
    rect(&mut ws2);
    ws2.paint.gradient_enabled = true;
    ws2.set_tool(Tool::Eyedropper);
    ws2.click_with_tool(Pos2::new(50.0, 30.0), false);
    assert!(!ws2.paint.gradient_enabled);
}

/// Presses at `at`, holds it longer than a click lasts while the pointer wanders 12 points, and
/// lets go, starting at `time`: what a hand on a trackpad or a pen often does.
fn slow_wobbly_press(ws: &mut Workspace, ctx: &egui::Context, time: f64, at: Pos2) {
    pass(ws, ctx, input(time, vec![Event::PointerMoved(at)]));
    pass(ws, ctx, input(time + 0.1, vec![click_event(at, true)]));
    let moved = at + Vec2::new(12.0, 0.0);
    pass(ws, ctx, input(time + 0.6, vec![Event::PointerMoved(moved)]));
    pass(ws, ctx, input(time + 1.5, vec![click_event(moved, false)]));
}

#[test]
fn the_bucket_and_eyedropper_act_on_a_press_held_long_or_moved_a_little() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    ws.set_tool(Tool::Rectangle);
    let (from, to) = (Pos2::new(450.0, 300.0), Pos2::new(750.0, 420.0));
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(from)]));
    pass(&mut ws, &ctx, input(0.3, vec![click_event(from, true)]));
    for step in 1..=10 {
        let at = from + (to - from) * (step as f32 / 10.0);
        let time = 0.3 + 0.02 * f64::from(step);
        pass(&mut ws, &ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    pass(&mut ws, &ctx, input(0.6, vec![click_event(to, false)]));
    let middle = Pos2::new(600.0, 360.0);

    ws.set_tool(Tool::Bucket);
    ws.paint.fill = BLUE;
    let steps = ws.active().undo_labels().count();
    slow_wobbly_press(&mut ws, &ctx, 1.0, middle);
    assert_eq!(
        drawn_shape(&ws, 1).fill(),
        rgba(BLUE),
        "filled on the press"
    );
    assert_eq!(ws.active().undo_labels().count(), steps + 1, "once");

    ws.paint.fill = Color32::WHITE;
    ws.set_tool(Tool::Eyedropper);
    slow_wobbly_press(&mut ws, &ctx, 3.0, middle);
    assert_eq!(ws.paint.fill, BLUE, "the color is taken on the press");
}
