//! A transparent layer, to draw over a rough: Shift on the eye, or the layer menu's Transparent
//! and Opacity. Each change is one undo step; exports are not touched.

use super::layer_names::{click, find, settle};
use super::*;
use egui::PointerButton;

fn opacity(ws: &Workspace, index: usize) -> u8 {
    ws.document().scenes()[0].timeline().layers()[index].view_opacity()
}

/// A click at `at` with Shift held.
fn shift_click(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, at: Pos2) {
    let shifted = |time: f64, mut events: Vec<Event>| {
        events.insert(0, Event::ModifiersChanged(Modifiers::SHIFT));
        input(time, events)
    };
    let button = |pressed| Event::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
        modifiers: Modifiers::SHIFT,
    };
    for events in [
        vec![Event::PointerMoved(at)],
        vec![button(true)],
        vec![button(false)],
    ] {
        *time += 0.05;
        pass(ws, ctx, shifted(*time, events));
    }
    *time += 0.05;
    pass(
        ws,
        ctx,
        input(*time, vec![Event::ModifiersChanged(Modifiers::NONE)]),
    );
}

#[test]
fn shift_on_the_eye_makes_the_layer_transparent_and_back() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let l = ws.timeline_layout.expect("drawn");
    let eye = Pos2::new(l.label_edge - 45.0, l.row_top(0) + 10.0);
    shift_click(&mut ws, &ctx, &mut time, eye);
    settle(&mut ws, &ctx, &mut time);
    assert_eq!(opacity(&ws, 0), 50);
    assert!(
        ws.document().scenes()[0].timeline().layers()[0].is_visible(),
        "still shown"
    );
    shift_click(&mut ws, &ctx, &mut time, eye);
    settle(&mut ws, &ctx, &mut time);
    assert_eq!(opacity(&ws, 0), 100);
    ws.undo();
    assert_eq!(opacity(&ws, 0), 50, "one undo step each");
    // A plain click still hides it.
    click(&mut ws, &ctx, &mut time, eye, PointerButton::Primary);
    assert!(!ws.document().scenes()[0].timeline().layers()[0].is_visible());
}

#[test]
fn the_layer_menu_has_transparent_and_opacity_and_setting_it_twice_is_one_step() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let mut time = 0.0;
    let mut at = None;
    for _ in 0..4 {
        time += 0.1;
        at = find(&mut ws, &ctx, time, vec![], "Layer 1").or(at);
    }
    click(
        &mut ws,
        &ctx,
        &mut time,
        at.expect("drawn"),
        PointerButton::Secondary,
    );
    let mut item = None;
    for _ in 0..4 {
        time += 0.1;
        item = find(&mut ws, &ctx, time, vec![], "Transparent").or(item);
    }
    time += 0.1;
    assert!(find(&mut ws, &ctx, time, vec![], "Opacity").is_some());
    click(
        &mut ws,
        &ctx,
        &mut time,
        item.expect("the menu offers Transparent"),
        PointerButton::Primary,
    );
    settle(&mut ws, &ctx, &mut time);
    assert_eq!(opacity(&ws, 0), 50);
    let layer = ws.document().scenes()[0].timeline().layers()[0].id();
    let steps = ws.active().undo_labels().len();
    ws.set_layer_view_opacity(layer, 30);
    ws.set_layer_view_opacity(layer, 30);
    assert_eq!(opacity(&ws, 0), 30);
    assert_eq!(
        ws.active().undo_labels().len(),
        steps + 1,
        "the same value is no step"
    );
    ws.set_layer_view_opacity(layer, 255);
    assert_eq!(opacity(&ws, 0), 100, "held at 100");
}
