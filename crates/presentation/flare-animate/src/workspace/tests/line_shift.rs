//! Shift holds the Line tool to 45-degree steps.

use super::*;

/// The box between the two ends of the first drawing, a line. Not the shape's bounds, which grow
/// by the stroke's width.
fn line_ends(ws: &Workspace) -> Rect {
    use flare_document::PathSegment;
    let line = drawn_shape(ws, 1);
    let at = |p: flare_core::DocumentPoint| Pos2::new(p.as_vec().x as f32, p.as_vec().y as f32);
    match line.path() {
        [PathSegment::MoveTo(a), PathSegment::LineTo(b)] => Rect::from_two_pos(at(*a), at(*b)),
        other => panic!("a line expected: {other:?}"),
    }
}

/// The ends of the line a drag from `from` to `to` committed.
fn dragged_line(shift: bool, from: Pos2, to: Pos2) -> Rect {
    let mut ws = workspace();
    ws.set_tool(Tool::Line);
    ws.begin_drag(from);
    ws.uniform_scale = shift;
    ws.update_drag(to);
    ws.end_drag();
    line_ends(&ws)
}

#[test]
fn shift_makes_a_nearly_level_line_level_and_a_nearly_upright_one_upright() {
    let level = dragged_line(true, Pos2::new(10.0, 10.0), Pos2::new(90.0, 22.0));
    assert_eq!(
        (level.min, level.max),
        (Pos2::new(10.0, 10.0), Pos2::new(90.0, 10.0))
    );
    let upright = dragged_line(true, Pos2::new(10.0, 10.0), Pos2::new(3.0, 70.0));
    assert_eq!(
        (upright.min, upright.max),
        (Pos2::new(10.0, 10.0), Pos2::new(10.0, 70.0))
    );
}

#[test]
fn shift_makes_a_line_near_a_diagonal_a_diagonal() {
    let line = dragged_line(true, Pos2::new(10.0, 10.0), Pos2::new(70.0, 60.0));
    assert!((line.width() - line.height()).abs() < 1e-3, "{line:?}");
    assert!((line.width() - 55.0).abs() < 1e-3, "{line:?}");
}

#[test]
fn without_shift_the_line_ends_at_the_pointer() {
    let line = dragged_line(false, Pos2::new(10.0, 10.0), Pos2::new(90.0, 22.0));
    assert_eq!(
        (line.min, line.max),
        (Pos2::new(10.0, 10.0), Pos2::new(90.0, 22.0))
    );
}

#[test]
fn shift_does_not_change_what_the_other_shape_tools_draw() {
    let mut ws = workspace();
    ws.set_tool(Tool::Rectangle);
    ws.begin_drag(Pos2::new(10.0, 10.0));
    ws.uniform_scale = true;
    ws.update_drag(Pos2::new(90.0, 22.0));
    ws.end_drag();
    let b = ops::shape_bounds(&drawn_shape(&ws, 1)).expect("bounds");
    assert_eq!(
        (b.min, b.max),
        (Pos2::new(10.0, 10.0), Pos2::new(90.0, 22.0))
    );
}

/// Drags the Line tool across the stage through the window's own events, a little off level,
/// with `modifiers` held, and returns the committed line's ends.
fn line_dragged_on_the_stage(modifiers: Modifiers) -> Rect {
    line_and_preview(modifiers).0
}

/// The same drag, with the ends of the rubber band on screen just before the pointer is let go.
fn line_and_preview(modifiers: Modifiers) -> (Rect, [Pos2; 2]) {
    let mut ws = workspace();
    ws.set_tool(Tool::Line);
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    // egui keeps the modifiers from this event until told otherwise, as a held key does.
    let held = |time, events| input(time, events);
    pass(
        &mut ws,
        &ctx,
        input(0.15, vec![Event::ModifiersChanged(modifiers)]),
    );
    let button = |at, pressed| Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers,
    };
    let start = Pos2::new(500.0, 330.0);
    pass(&mut ws, &ctx, held(0.2, vec![Event::PointerMoved(start)]));
    pass(&mut ws, &ctx, held(0.3, vec![button(start, true)]));
    let mut time = 0.3;
    let mut at = start;
    for step in 1..=6 {
        time += 0.02;
        at = Pos2::new(500.0 + 30.0 * step as f32, 330.0 + 3.0 * step as f32);
        pass(&mut ws, &ctx, held(time, vec![Event::PointerMoved(at)]));
    }
    ws.paint.stroke_color = Color32::from_rgb(20, 20, 220);
    let ink = ws.paint.stroke_color;
    let mut dragging = ctx.run_ui(held(time + 0.01, vec![]), |ui| ws.show(ui));
    dragging.textures_delta.clear();
    let band = dragging
        .shapes
        .iter()
        .find_map(|s| match &s.shape {
            egui::epaint::Shape::LineSegment { points, stroke } if stroke.color == ink => {
                Some(*points)
            }
            _ => None,
        })
        .expect("the rubber band is drawn while dragging");
    pass(&mut ws, &ctx, held(time + 0.02, vec![button(at, false)]));
    assert_eq!(drawing_count(&ws), 1, "the line was committed");
    (line_ends(&ws), band)
}

#[test]
fn the_stage_reads_shift_from_the_keyboard_while_a_line_is_dragged() {
    let free = line_dragged_on_the_stage(Modifiers::NONE);
    assert!(free.width() > 10.0 && free.height() > 1.0, "{free:?}");
    let held = line_dragged_on_the_stage(Modifiers::SHIFT);
    assert!(held.width() > 10.0, "{held:?}");
    assert_eq!(held.height(), 0.0, "Shift made the line level: {held:?}");
}

#[test]
fn the_rubber_band_shows_the_constrained_line_while_shift_is_held() {
    let (_, [from, to]) = line_and_preview(Modifiers::NONE);
    assert!((to.y - from.y).abs() > 1.0, "free: {from:?} {to:?}");
    let (_, [from, to]) = line_and_preview(Modifiers::SHIFT);
    assert!((to.x - from.x).abs() > 10.0, "{from:?} {to:?}");
    assert_eq!(from.y, to.y, "the preview is level too");
}
