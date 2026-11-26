//! A pen's side button, which tablet drivers send as a right click, flips the stage between the
//! Eraser and the tool held before; a mouse's right click still opens the stage's menu.

use super::layer_names::find;
use super::stage_menu::{drawn, right_click};
use super::*;

#[test]
fn the_button_gives_the_eraser_and_then_the_tool_before_it() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.pen_button();
    assert_eq!(ws.tool(), Tool::Eraser);
    assert_eq!(ws.pen_button_gives_back(), Some(Tool::Pencil));
    ws.pen_button();
    assert_eq!(ws.tool(), Tool::Pencil, "back to the Pencil");
    assert_eq!(ws.pen_button_gives_back(), None);
}

#[test]
fn an_eraser_from_the_toolbar_gives_the_brush_and_another_tool_ends_the_flip() {
    let mut ws = workspace();
    ws.set_tool(Tool::Eraser);
    ws.pen_button();
    assert_eq!(ws.tool(), Tool::Brush, "nothing was before it");
    // Picking a tool by hand while the button's Eraser is out forgets what it would give back.
    ws.set_tool(Tool::Pencil);
    ws.pen_button();
    ws.set_tool(Tool::Rectangle);
    assert_eq!(ws.pen_button_gives_back(), None);
    ws.set_tool(Tool::Eraser);
    ws.pen_button();
    assert_eq!(ws.tool(), Tool::Brush, "not the Pencil from before");
}

#[test]
fn a_right_click_from_a_pen_flips_to_the_eraser_and_back_without_a_menu() {
    let (ctx, mut ws, at, mut time) = drawn();
    ws.set_tool(Tool::Brush);
    ws.pen_feed().set(Some(0.0));
    right_click(&mut ws, &ctx, &mut time, at);
    assert_eq!(ws.tool(), Tool::Eraser);
    time += 0.1;
    assert!(
        find(&mut ws, &ctx, time, vec![], "Copy").is_none(),
        "no menu"
    );
    assert!(ws.selection_all().is_empty(), "nothing picked for a menu");
    right_click(&mut ws, &ctx, &mut time, at);
    assert_eq!(ws.tool(), Tool::Brush);
}

#[test]
fn a_right_click_from_a_mouse_still_opens_the_menu() {
    let (ctx, mut ws, at, mut time) = drawn();
    ws.set_tool(Tool::Brush);
    ws.pen_feed().set(None);
    right_click(&mut ws, &ctx, &mut time, at);
    assert_eq!(ws.tool(), Tool::Brush);
    let mut copy = None;
    for _ in 0..4 {
        time += 0.1;
        copy = find(&mut ws, &ctx, time, vec![], "Copy").or(copy);
    }
    assert!(copy.is_some(), "the menu is open");
}

#[test]
fn a_pen_moving_while_its_button_is_held_flips_once() {
    // A hand pressing the side button moves the pen: the press is a drag, not a click.
    let (ctx, mut ws, at, mut time) = drawn();
    ws.set_tool(Tool::Brush);
    ws.pen_feed().set(Some(0.0));
    let secondary = egui::PointerButton::Secondary;
    let mut steps = vec![
        vec![Event::PointerMoved(at)],
        vec![super::layer_names::press(at, secondary, true)],
    ];
    for step in 1..=10 {
        steps.push(vec![Event::PointerMoved(
            at + egui::vec2(3.0 * step as f32, 1.0),
        )]);
    }
    let end = at + egui::vec2(30.0, 1.0);
    steps.push(vec![super::layer_names::press(end, secondary, false)]);
    for events in steps {
        time += 0.02;
        pass(&mut ws, &ctx, input(time, events));
    }
    assert_eq!(ws.tool(), Tool::Eraser, "flipped once, on the press");
    assert_eq!(drawing_count(&ws), 1, "the drag drew nothing");
}
