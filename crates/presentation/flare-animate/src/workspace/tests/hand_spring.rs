//! The Hand tool gives back the tool held before it once a pan is let go, and never gives back
//! the Rotation tool.

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
fn letting_go_of_a_pan_gives_back_the_tool_held_before() {
    let (mut ws, ctx, mut time) = laid_out();
    ws.set_tool(Tool::Brush);
    ws.set_tool(Tool::Hand);
    assert_eq!(ws.hand_gives_back(), Some(Tool::Brush));
    let (before, count) = (origin_on_screen(&ws), drawing_count(&ws));
    pan_drag(&mut ws, &ctx, &mut time, Vec2::new(90.0, 40.0));
    let moved = origin_on_screen(&ws) - before;
    assert!(moved.length() > 50.0, "the view panned: {moved:?}");
    assert_eq!(ws.tool(), Tool::Brush, "back to the Brush");
    assert_eq!(
        drawing_count(&ws),
        count,
        "the Brush drew nothing on the way back"
    );
}

#[test]
fn the_rotation_tool_is_skipped_for_the_tool_before_it() {
    let (mut ws, ctx, mut time) = laid_out();
    ws.set_tool(Tool::Pen);
    ws.set_tool(Tool::RotateView);
    ws.set_tool(Tool::Hand);
    assert_eq!(ws.hand_gives_back(), Some(Tool::Pen));
    pan_drag(&mut ws, &ctx, &mut time, Vec2::new(-80.0, 30.0));
    assert_eq!(ws.tool(), Tool::Pen, "not the Rotation tool");
}

#[test]
fn a_click_or_a_hand_with_nothing_before_it_stays_in_hand() {
    let (mut ws, ctx, mut time) = laid_out();
    // Hand, then the Rotation tool, then the Hand again: neither is a tool to give back.
    ws.set_tool(Tool::Hand);
    ws.set_tool(Tool::RotateView);
    ws.set_tool(Tool::Hand);
    assert_eq!(ws.hand_gives_back(), None);
    pan_drag(&mut ws, &ctx, &mut time, Vec2::new(60.0, 0.0));
    assert_eq!(ws.tool(), Tool::Hand);
    // A click without a drag is not a pan: the Hand stays.
    ws.set_tool(Tool::Select);
    ws.set_tool(Tool::Hand);
    let at = ws.stage_panel.center();
    for event in [Event::PointerMoved(at), button(at, true), button(at, false)] {
        time += 0.02;
        pass(&mut ws, &ctx, input(time, vec![event]));
    }
    time += 0.02;
    pass(&mut ws, &ctx, input(time, vec![]));
    assert_eq!(ws.tool(), Tool::Hand);
    // Picking another tool forgets what the Hand would give back.
    ws.set_tool(Tool::Zoom);
    assert_eq!(ws.hand_gives_back(), None);
}
