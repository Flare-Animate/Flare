//! The Hand tool pans the view and stays in hand once a pan is let go, so a large stage can be
//! crossed pan after pan without picking it again.

use super::*;

fn button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

fn laid_out() -> (Workspace, egui::Context, f64) {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(700.0, 400.0), Pos2::new(800.0, 500.0));
    let ctx = egui::Context::default();
    for time in [0.0, 0.01, 0.02] {
        run(&mut ws, &ctx, time);
    }
    (ws, ctx, 1.0)
}

/// Drags across the stage by `by`, then one more pass with nothing happening.
fn pan_drag(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, by: Vec2) {
    let from = ws.stage_panel.center() + Vec2::new(-120.0, 60.0);
    let mut events = vec![Event::PointerMoved(from), button(from, true)];
    for step in 1..=10 {
        events.push(Event::PointerMoved(from + by * (step as f32 / 10.0)));
    }
    events.push(button(from + by, false));
    for event in events {
        *time += 0.02;
        pass(ws, ctx, input(*time, vec![event]));
    }
    *time += 0.02;
    pass(ws, ctx, input(*time, vec![]));
}

/// Where the stage's own origin shows on screen: what a pan moves.
fn origin_on_screen(ws: &Workspace) -> Pos2 {
    ws.stage_view
        .to_screen(ws.stage_panel, ws.stage_extent(), Pos2::ZERO)
}

#[test]
fn the_hand_stays_in_hand_after_a_pan() {
    let (mut ws, ctx, mut time) = laid_out();
    ws.set_tool(Tool::Brush);
    ws.set_tool(Tool::Hand);
    let (before, count) = (origin_on_screen(&ws), drawing_count(&ws));
    pan_drag(&mut ws, &ctx, &mut time, Vec2::new(90.0, 40.0));
    let moved = origin_on_screen(&ws) - before;
    assert!(moved.length() > 50.0, "the view panned: {moved:?}");
    assert_eq!(ws.tool(), Tool::Hand, "still the Hand, not the Brush");
    assert_eq!(drawing_count(&ws), count, "a pan draws nothing");
}

#[test]
fn pan_after_pan_keeps_moving_the_view() {
    let (mut ws, ctx, mut time) = laid_out();
    ws.set_tool(Tool::Pen);
    ws.set_tool(Tool::Hand);
    let start = origin_on_screen(&ws);
    let step = Vec2::new(-70.0, 30.0);
    for _ in 0..3 {
        pan_drag(&mut ws, &ctx, &mut time, step);
        assert_eq!(ws.tool(), Tool::Hand);
    }
    let moved = origin_on_screen(&ws) - start;
    assert!(
        (moved - step * 3.0).length() < 30.0,
        "three pans add up: {moved:?}"
    );
}

#[test]
fn a_hand_picked_from_the_rotation_tool_stays_too() {
    let (mut ws, ctx, mut time) = laid_out();
    ws.set_tool(Tool::Pen);
    ws.set_tool(Tool::RotateView);
    ws.set_tool(Tool::Hand);
    pan_drag(&mut ws, &ctx, &mut time, Vec2::new(-80.0, 30.0));
    assert_eq!(ws.tool(), Tool::Hand);
}

#[test]
fn picking_the_hand_lets_go_of_the_selection_and_a_right_click_with_it_picks_nothing() {
    let (mut ws, ctx, mut time) = laid_out();
    ws.set_tool(Tool::Select);
    ws.select_all();
    assert!(ws.has_selection());
    ws.set_tool(Tool::Hand);
    assert!(!ws.has_selection(), "no box left over the drawing");

    // A right click (a two-finger tap, or a pen's side button sent as one) on the rectangle.
    let on = ws
        .stage_view
        .to_screen(ws.stage_panel, ws.stage_extent(), Pos2::new(750.0, 450.0));
    for pressed in [true, false] {
        time += 0.02;
        let event = Event::PointerButton {
            pos: on,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        pass(
            &mut ws,
            &ctx,
            input(time, vec![Event::PointerMoved(on), event]),
        );
    }
    assert!(!ws.has_selection(), "the Hand only looks");

    // The Select tool still picks with a right click, for its menu.
    ws.set_tool(Tool::Select);
    for pressed in [true, false] {
        time += 0.02;
        let event = Event::PointerButton {
            pos: on,
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        pass(
            &mut ws,
            &ctx,
            input(time, vec![Event::PointerMoved(on), event]),
        );
    }
    assert!(ws.has_selection(), "the right click reached the stage");
}
