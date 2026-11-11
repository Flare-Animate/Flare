//! The pointer over the Free Transform frame shows what a press there does.

use super::*;
use flare_tools::free_transform::Grab;

/// The rectangle at (10, 10)..(50, 50) selected with the Free Transform tool, and the stage laid
/// out, with handles 2 stage units across.
fn transforming(ctx: &egui::Context) -> Workspace {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    for time in [0.0, 0.01, 0.02] {
        run(&mut ws, ctx, time);
    }
    ws.handle_radius = 2.0;
    ws
}

/// Moves the pointer to `at` on the stage and returns the system pointer the pass asked for.
fn pointer_at(ws: &mut Workspace, ctx: &egui::Context, at: Pos2, time: f64) -> egui::CursorIcon {
    let extent = ws.stage_extent();
    let image = ws.stage_view.image_rect(ws.stage_panel, extent);
    let scale = ws.stage_view.scale(ws.stage_panel, extent);
    let screen = image.min + at.to_vec2() * scale;
    // The pass that moves the pointer learns where it is; the one after hovers there.
    pass(ws, ctx, input(time, vec![Event::PointerMoved(screen)]));
    let mut output = ctx.run_ui(input(time + 0.001, vec![]), |ui| ws.show(ui));
    output.textures_delta.clear();
    output.platform_output.cursor_icon
}

/// A rectangle from (900, 500) to (1000, 600), away from the rulers, selected with the Free
/// Transform tool and the stage laid out.
fn transforming_mid_stage(ctx: &egui::Context) -> Workspace {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(900.0, 500.0), Pos2::new(1000.0, 600.0));
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(950.0, 550.0), false);
    for time in [0.0, 0.01, 0.02] {
        run(&mut ws, ctx, time);
    }
    ws
}

#[test]
fn the_pointer_shows_which_way_each_handle_drags_and_moving_inside() {
    use egui::CursorIcon as C;
    let ctx = egui::Context::default();
    let mut ws = transforming_mid_stage(&ctx);
    let cases = [
        ((900.0, 500.0), C::ResizeNwSe),
        ((1000.0, 600.0), C::ResizeNwSe),
        ((1000.0, 500.0), C::ResizeNeSw),
        ((900.0, 600.0), C::ResizeNeSw),
        ((900.0, 550.0), C::ResizeHorizontal),
        ((1000.0, 550.0), C::ResizeHorizontal),
        ((950.0, 500.0), C::ResizeVertical),
        ((950.0, 600.0), C::ResizeVertical),
        ((950.0, 550.0), C::Move),
        ((1500.0, 900.0), C::Default),
    ];
    for (step, ((x, y), icon)) in cases.into_iter().enumerate() {
        let shown = pointer_at(&mut ws, &ctx, Pos2::new(x, y), 1.0 + step as f64 * 0.01);
        assert_eq!(shown, icon, "at ({x}, {y})");
    }
}

#[test]
fn only_the_free_transform_frame_changes_the_pointer() {
    let ctx = egui::Context::default();
    let mut ws = transforming_mid_stage(&ctx);
    ws.set_tool(Tool::Select);
    assert_eq!(
        pointer_at(&mut ws, &ctx, Pos2::new(1000.0, 600.0), 1.0),
        egui::CursorIcon::Default
    );
    assert_eq!(ws.stage_grab(), None);
}

#[test]
fn alt_shows_a_skew_and_just_outside_a_corner_a_rotation() {
    let ctx = egui::Context::default();
    let mut ws = transforming(&ctx);
    ws.skew_modifier = true;
    assert_eq!(
        ws.transform_grab(Pos2::new(30.0, 10.0)).map(|(g, _)| g),
        Some(Grab::Skew(Handle::Top))
    );
    ws.skew_modifier = false;
    assert_eq!(
        ws.transform_grab(Pos2::new(54.0, 54.0)).map(|(g, _)| g),
        Some(Grab::Rotate)
    );
}

#[test]
fn a_drag_keeps_its_pointer_wherever_the_pointer_goes() {
    let ctx = egui::Context::default();
    let mut ws = transforming(&ctx);
    ws.begin_drag(Pos2::new(50.0, 50.0));
    ws.update_drag(Pos2::new(300.0, 120.0)); // far off every handle
    assert_eq!(ws.stage_grab(), Some(Grab::Scale(Handle::BottomRight)));
    ws.end_drag();
    ws.begin_drag(Pos2::new(30.0, 30.0));
    ws.update_drag(Pos2::new(400.0, 400.0));
    assert_eq!(ws.stage_grab(), Some(Grab::Move));
}

#[test]
fn pressing_inside_the_frame_off_the_ink_moves_the_selection() {
    let ctx = egui::Context::default();
    let mut ws = transforming(&ctx);
    // With both rectangles selected the frame runs 10..140 x 10..50, with empty stage between
    // them: inside the frame, but on no ink, as most of a thin scribble's frame is.
    ws.select_at(Pos2::new(120.0, 30.0), true);
    let point = Pos2::new(75.0, 30.0); // between the two rectangles
    assert!(ws.hit(point).is_none(), "nothing drawn here");
    ws.begin_drag(point);
    assert_eq!(ws.drag.map(|d| d.mode), Some(DragMode::Move));
    ws.update_drag(Pos2::new(85.0, 40.0));
    ws.end_drag();
    assert_eq!(rect_x(&ws, 1), 20.0);
    assert_eq!(rect_x(&ws, 2), 110.0, "the whole selection moved");

    // The Selection tool still needs the ink: the same press there picks nothing.
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(85.0, 40.0));
    assert!(ws.drag.is_none() && !ws.has_selection());
}
