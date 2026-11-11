//! The Eraser rubs out what it is dragged over, through the same pointer events the stage gets.

use super::*;

fn button(at: Pos2, pressed: bool) -> Event {
    Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: Modifiers::NONE,
    }
}

/// Presses the tool in hand at `from` and moves it to `to` in ten steps, in window points,
/// through pointer events, starting at `time` seconds; gives the time it ended at, still pressed.
fn press_and_move(ws: &mut Workspace, ctx: &egui::Context, time: f64, from: Pos2, to: Pos2) -> f64 {
    let mut time = time;
    pass(ws, ctx, input(time, vec![Event::PointerMoved(from)]));
    time += 0.1;
    pass(ws, ctx, input(time, vec![button(from, true)]));
    for step in 1..=10 {
        time += 0.02;
        let at = from + (to - from) * (step as f32 / 10.0);
        pass(ws, ctx, input(time, vec![Event::PointerMoved(at)]));
    }
    time
}

/// Drags the tool in hand from `from` to `to`, as [`press_and_move`], and lets go.
fn drag_with_pointer(
    ws: &mut Workspace,
    ctx: &egui::Context,
    time: f64,
    from: Pos2,
    to: Pos2,
) -> f64 {
    let time = press_and_move(ws, ctx, time, from, to) + 0.02;
    pass(ws, ctx, input(time, vec![button(to, false)]));
    time + 0.1
}

/// A wide bar across the middle of the stage, drawn with the Rectangle as a person would, with
/// the Eraser then in hand; gives the time the drawing ended at.
fn bar_then_eraser(ws: &mut Workspace, ctx: &egui::Context) -> f64 {
    run(ws, ctx, 0.0);
    run(ws, ctx, 0.1);
    ws.set_tool(Tool::Rectangle);
    let time = drag_with_pointer(
        ws,
        ctx,
        0.2,
        Pos2::new(450.0, 330.0),
        Pos2::new(750.0, 370.0),
    );
    assert_eq!(drawings(ws).len(), 1, "the bar was drawn");
    ws.set_tool(Tool::Eraser);
    time
}

fn moves_of_first_drawing(ws: &Workspace) -> usize {
    match drawings(ws).first() {
        Some(Content::Shape(shape)) => shape
            .path()
            .iter()
            .filter(|s| matches!(s, flare_document::PathSegment::MoveTo(_)))
            .count(),
        _ => 0,
    }
}

#[test]
fn dragging_the_eraser_rubs_out_as_it_goes_and_undoes_in_one_step() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let time = bar_then_eraser(&mut ws, &ctx);
    let before = ws.document().clone();
    let steps = ws.active().undo_labels().count();
    let (top, bottom) = (Pos2::new(600.0, 300.0), Pos2::new(602.0, 400.0));
    let time = press_and_move(&mut ws, &ctx, time, top, bottom);
    assert_eq!(drawings(&ws).len(), 1);
    assert_eq!(
        moves_of_first_drawing(&ws),
        2,
        "the bar is cut in two before the eraser is let go"
    );
    assert!(
        ws.freehand_preview(1.0).is_empty(),
        "nothing is drawn over what is already rubbed out"
    );
    pass(
        &mut ws,
        &ctx,
        input(time + 0.02, vec![button(bottom, false)]),
    );
    assert_eq!(moves_of_first_drawing(&ws), 2);
    assert_eq!(
        ws.active().undo_labels().count(),
        steps + 1,
        "the whole drag is one undo step"
    );
    ws.undo();
    assert_eq!(ws.document(), &before, "one undo puts it back");
}

#[test]
fn cancelling_an_eraser_drag_takes_back_what_it_rubbed_out() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let time = bar_then_eraser(&mut ws, &ctx);
    let before = ws.document().clone();
    let (top, bottom) = (Pos2::new(600.0, 300.0), Pos2::new(602.0, 400.0));
    press_and_move(&mut ws, &ctx, time, top, bottom);
    assert_eq!(moves_of_first_drawing(&ws), 2, "rubbed out while dragging");
    ws.cancel_drag();
    assert_eq!(ws.document(), &before, "the cancelled drag leaves nothing");
}

#[test]
fn a_straightened_eraser_drag_shows_its_line_and_rubs_it_out_on_release() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let time = bar_then_eraser(&mut ws, &ctx);
    ws.paint.straight_edge = true;
    let (top, bottom) = (Pos2::new(600.0, 300.0), Pos2::new(602.0, 400.0));
    let time = press_and_move(&mut ws, &ctx, time, top, bottom);
    assert_eq!(moves_of_first_drawing(&ws), 1, "nothing is rubbed out yet");
    assert!(!ws.freehand_preview(1.0).is_empty(), "the line is shown");
    pass(
        &mut ws,
        &ctx,
        input(time + 0.02, vec![button(bottom, false)]),
    );
    assert_eq!(moves_of_first_drawing(&ws), 2, "and rubbed out on release");
}

#[test]
fn a_click_rubs_out_a_dot_and_faucet_deletes_the_whole_shape() {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(110.0, 60.0));
    ws.set_tool(Tool::Eraser);
    ws.eraser_size = 10.0;
    ws.click_with_tool(Pos2::new(60.0, 35.0), false);
    assert_eq!(drawings(&ws).len(), 1, "the shape is still there");
    assert_eq!(moves_of_first_drawing(&ws), 2, "with a hole in it");
    ws.click_with_tool(Pos2::new(700.0, 400.0), false);
    assert!(ws.active().can_undo());
    let steps = ws.active().undo_labels().count();
    ws.click_with_tool(Pos2::new(700.0, 400.0), false);
    assert_eq!(
        ws.active().undo_labels().count(),
        steps,
        "a miss adds no undo step"
    );
    ws.eraser_faucet = true;
    ws.click_with_tool(Pos2::new(20.0, 20.0), false);
    assert!(drawings(&ws).is_empty());
}

#[test]
fn the_eraser_shows_its_size_at_the_pointer_and_the_area_it_will_rub_while_dragging() {
    let mut ws = workspace();
    ws.set_tool(Tool::Eraser);
    ws.eraser_size = 30.0;
    ws.pointer_stage = Some(Pos2::new(100.0, 100.0));
    let cursor = ws.brush_cursor().expect("cursor");
    assert!(
        cursor
            .iter()
            .all(|p| ((*p - Pos2::new(100.0, 100.0)).length() - 15.0).abs() < 1e-3)
    );
    let samples: Vec<flare_input::InputSample> = (0..=10)
        .map(|i| {
            flare_input::InputSample::new(
                f64::from(i) * 10.0,
                50.0,
                0.1,
                flare_input::PointerKind::Pen,
            )
            .expect("s")
        })
        .collect();
    let preview = ws.previews_of(std::slice::from_ref(&samples), 1.0);
    let sides = preview[0].sides.as_ref().expect("the covered area");
    let middle = sides.left.len() / 2;
    let half = (sides.left[middle] - sides.right[middle]).length() / 2.0;
    assert!(
        (half - 15.0).abs() < 1e-3,
        "a light touch rubs as wide as a firm one"
    );
    ws.eraser_faucet = true;
    assert!(
        ws.brush_cursor().is_none(),
        "Faucet deletes by clicking, with no rubbing size"
    );
}
