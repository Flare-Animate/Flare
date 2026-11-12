//! Artwork off the stage lies on the pasteboard around it, as in Animate: drawn there, shown
//! there and picked from there. Only the published picture is cut to the stage.

use super::*;

/// The stage the editor draws, after the passes that lay it out.
fn texture_region(ws: &mut Workspace, ctx: &egui::Context, time: f64) -> Rect {
    run(ws, ctx, time);
    run(ws, ctx, time + 0.01);
    ws.texture.as_ref().expect("texture").view.region
}

#[test]
fn the_view_draws_the_pasteboard_around_the_stage_until_it_is_hidden() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    let stage = Rect::from_min_size(Pos2::ZERO, ws.stage_extent());
    let shown = texture_region(&mut ws, &ctx, 0.0);
    assert!(shown.contains_rect(stage), "{shown:?}");
    // The panel is not the stage's shape: the room beside or above and below it is drawn too.
    let beyond = |r: Rect| r.width() > stage.width() || r.height() > stage.height();
    assert!(
        beyond(shown),
        "the panel has room around the fitted stage, and it is drawn: {shown:?}"
    );
    assert_eq!(ws.label(MenuAction::TogglePasteboard), "Hide Pasteboard");

    ws.perform(MenuAction::TogglePasteboard);
    assert_eq!(
        texture_region(&mut ws, &ctx, 1.0),
        stage,
        "cut to the stage"
    );
    assert_eq!(ws.label(MenuAction::TogglePasteboard), "Show Pasteboard");

    ws.perform(MenuAction::TogglePasteboard);
    let again = texture_region(&mut ws, &ctx, 2.0);
    assert!(again.contains_rect(stage) && beyond(again), "{again:?}");
}

#[test]
fn looking_through_the_camera_shows_only_what_it_frames() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_tool(Tool::Camera);
    ws.set_tool(Tool::Select);
    ws.set_camera_preview(true);
    let stage = Rect::from_min_size(Pos2::ZERO, ws.stage_extent());
    assert_eq!(texture_region(&mut ws, &ctx, 0.0), stage);
    ws.set_camera_preview(false);
    assert!(texture_region(&mut ws, &ctx, 1.0).contains_rect(stage));
}

#[test]
fn artwork_drawn_on_the_pasteboard_stays_there_and_its_color_can_be_taken() {
    let mut ws = workspace();
    ws.paint.fill = Color32::from_rgb(10, 200, 30);
    ws.commit_rectangle(Pos2::new(-300.0, -200.0), Pos2::new(-100.0, -50.0));
    let drawn = drawings(&ws);
    let Some(Content::Shape(shape)) = drawn.last() else {
        panic!("the rectangle is drawn: {drawn:?}");
    };
    let bounds = ops::shape_bounds(shape).expect("bounds");
    assert!(bounds.max.x <= -99.0 && bounds.max.y <= -49.0, "{bounds:?}");

    ws.paint.fill = Color32::from_rgb(0, 0, 255);
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(-200.0, -100.0), false);
    assert_eq!(ws.paint.fill, Color32::from_rgb(10, 200, 30));
    // Bare pasteboard has no color to take: the stage's color is only on the stage.
    ws.click_with_tool(Pos2::new(-50.0, 300.0), false);
    assert_eq!(ws.paint.fill, Color32::from_rgb(10, 200, 30));
}
