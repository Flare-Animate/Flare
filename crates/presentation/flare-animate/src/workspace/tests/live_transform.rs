//! The selection follows a move, scale, rotate or skew drag while it is held.

use super::*;

/// The bounds of the selected span in `document`.
fn bounds_in(ws: &Workspace, document: &Document) -> Rect {
    let scene = ws.scene_id().expect("scene");
    ops::span_bounds(document, scene, ws.selected.expect("selected")).expect("bounds")
}

#[test]
fn a_held_scale_previews_the_scaled_art_and_the_release_commits_exactly_that() {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    let steps = ws.active().undo_labels().len();
    ws.begin_drag(Pos2::new(50.0, 50.0)); // the bottom-right handle
    ws.update_drag(Pos2::new(90.0, 90.0));

    let (_, shown) = ws.previewed_document().expect("a preview while held");
    let previewed = bounds_in(&ws, &shown);
    assert_eq!(
        (previewed.min, previewed.max),
        (Pos2::new(10.0, 10.0), Pos2::new(90.0, 90.0))
    );
    assert_eq!(
        shape_bounds(&ws).max,
        Pos2::new(50.0, 50.0),
        "not yet changed"
    );
    assert_eq!(
        ws.active().undo_labels().len(),
        steps,
        "holding makes no undo step"
    );

    ws.end_drag();
    assert_eq!(
        shape_bounds(&ws),
        previewed,
        "what is let go is what was shown"
    );
    assert_eq!(ws.active().undo_labels().len(), steps + 1, "one undo step");
    assert!(ws.previewed_document().is_none(), "the preview is gone");
}

#[test]
fn a_held_move_previews_the_snapped_move_with_the_select_tool_too() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.display.snap_to_grid = true;
    ws.display.grid_size = 10.0;
    ws.begin_drag(Pos2::new(30.0, 30.0));
    assert_eq!(ws.drag.map(|d| d.mode), Some(DragMode::Move));
    ws.update_drag(Pos2::new(53.0, 37.0)); // 23 x 7, which the grid makes 20 x 10

    let (_, shown) = ws.previewed_document().expect("a preview while held");
    assert_eq!(bounds_in(&ws, &shown).min, Pos2::new(30.0, 20.0));
    assert_eq!(rect_x(&ws, 2), 100.0, "the other rectangle stays put");
    ws.end_drag();
    assert_eq!(shape_bounds(&ws).min, Pos2::new(30.0, 20.0));
}

#[test]
fn a_held_rotation_previews_the_turn_it_commits() {
    let mut ws = wide_rectangle_selected();
    ws.set_tool(Tool::FreeTransform);
    ws.begin_drag(Pos2::new(120.0, 70.0)); // just outside the bottom-right corner
    ws.update_drag(Pos2::new(25.0, 95.0)); // a quarter turn
    let (_, shown) = ws.previewed_document().expect("a preview while held");
    let previewed = bounds_in(&ws, &shown);
    assert!(
        (previewed.width() - 50.0).abs() < 0.01 && (previewed.height() - 100.0).abs() < 0.01,
        "{previewed:?}"
    );
    ws.end_drag();
    assert_eq!(shape_bounds(&ws), previewed);
}

#[test]
fn a_drag_that_changes_nothing_or_is_cancelled_previews_nothing() {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.begin_drag(Pos2::new(30.0, 30.0)); // a move that has not moved yet
    assert!(ws.previewed_document().is_none());
    ws.end_drag();

    ws.begin_drag(Pos2::new(50.0, 50.0));
    ws.update_drag(Pos2::new(90.0, 90.0));
    assert!(ws.previewed_document().is_some());
    ws.cancel_drag();
    assert!(ws.previewed_document().is_none(), "Escape takes it back");
    ws.end_drag();
    assert_eq!(shape_bounds(&ws).max, Pos2::new(50.0, 50.0));

    // A rectangle drag draws a shape; its rubber band is the preview, not the stage.
    ws.set_tool(Tool::Rectangle);
    ws.begin_drag(Pos2::new(200.0, 200.0));
    ws.update_drag(Pos2::new(240.0, 240.0));
    assert!(ws.previewed_document().is_none());
}

#[test]
fn the_stage_shows_the_held_drag_and_its_pictures_stay_out_of_the_cache() {
    let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.01); // the panel is known now, so the view settles
    run(&mut ws, &ctx, 0.02);
    let cached = ws.frame_cache.len();
    let before = ws.frames_rendered;

    ws.begin_drag(Pos2::new(50.0, 50.0));
    for (step, to) in [70.0, 80.0, 90.0].into_iter().enumerate() {
        ws.update_drag(Pos2::new(to, to));
        run(&mut ws, &ctx, 0.1 + step as f64 * 0.01);
        let stamp = ws.previewed_document().map(|(stamp, _)| stamp);
        assert!(stamp.is_some());
        assert_eq!(
            ws.texture.as_ref().and_then(|t| t.preview),
            stamp,
            "the stage is drawn from where the handle is now"
        );
    }
    assert_eq!(ws.frames_rendered, before + 3, "one picture per position");
    assert_eq!(ws.frame_cache.len(), cached, "none of them kept");

    // Escape: the picture from before the drag is still cached and comes straight back.
    ws.cancel_drag();
    ws.end_drag();
    run(&mut ws, &ctx, 0.2);
    assert_eq!(ws.texture.as_ref().map(|t| t.preview), Some(None));
    assert_eq!(ws.frames_rendered, before + 3, "nothing drawn again");
}

#[test]
fn with_workers_a_drag_that_never_rests_still_shows_the_art_moving() {
    let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
    ws.start_background_threads(&ctx);
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    wait_until(&mut ws, &ctx, |w| {
        w.texture.is_some() && w.pending_render.is_none()
    });
    ws.begin_drag(Pos2::new(50.0, 50.0));
    // A new position on every pass, never waiting for a picture.
    let started = std::time::Instant::now();
    let mut to = 60.0;
    while ws.texture.as_ref().and_then(|t| t.preview).is_none() {
        assert!(
            started.elapsed().as_secs() < 15,
            "no picture of the held drag arrived"
        );
        to += 0.5;
        ws.update_drag(Pos2::new(to, to));
        run(&mut ws, &ctx, 1.0 + f64::from(to) * 0.001);
    }
    ws.end_drag();
    wait_until(&mut ws, &ctx, |w| {
        w.pending_render.is_none() && w.texture.as_ref().is_some_and(|t| t.preview.is_none())
    });
    assert!(
        shape_bounds(&ws).max.x > 60.0,
        "the release committed the scale"
    );
}

#[test]
fn a_new_position_waits_for_the_held_drag_picture_in_flight_but_a_release_does_not() {
    let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.01);
    run(&mut ws, &ctx, 0.02);
    ws.begin_drag(Pos2::new(50.0, 50.0));
    ws.update_drag(Pos2::new(70.0, 70.0));
    run(&mut ws, &ctx, 0.1);
    // A picture of the drag still being drawn, as a slow frame on the worker would be. The
    // worker draws only the newest request, so replacing it on every move could starve the
    // stage of pictures for as long as the pointer keeps moving.
    let texture = ws.texture.as_ref().expect("texture");
    let in_flight = PendingRender {
        id: u64::MAX,
        key: texture.key,
        document: ws.active().snapshot(),
        onion: texture.onion,
        view: texture.view,
        signature: 0,
        text_epoch: texture.text_epoch,
        preview: texture.preview,
    };
    ws.pending_render = Some(in_flight);
    let drawn = ws.frames_rendered;
    ws.update_drag(Pos2::new(90.0, 90.0));
    run(&mut ws, &ctx, 0.2);
    assert_eq!(ws.pending_render.as_ref().map(|p| p.id), Some(u64::MAX));
    assert_eq!(
        ws.frames_rendered, drawn,
        "nothing more asked for meanwhile"
    );

    // Letting go is not a preview: the committed picture is asked for at once.
    ws.end_drag();
    run(&mut ws, &ctx, 0.3);
    assert!(ws.pending_render.is_none(), "drawn inline and installed");
    assert_eq!(ws.texture.as_ref().map(|t| t.preview), Some(None));
    assert_eq!(shape_bounds(&ws).max, Pos2::new(90.0, 90.0));
}
