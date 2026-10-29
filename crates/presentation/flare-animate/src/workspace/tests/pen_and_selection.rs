use super::*;

#[test]
fn test_movie_is_a_request_for_the_shell_and_includes_pending_symbol_edits() {
    let (mut ws, symbol) = with_symbol();
    assert!(!ws.take_test_movie_request());
    ws.perform(MenuAction::TestMovie);
    assert!(ws.take_test_movie_request());
    assert!(!ws.take_test_movie_request(), "delivered once");

    ws.edit_selected_symbol();
    ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 330.0));
    assert!(ws.in_symbol());
    assert_eq!(
        ws.main_document()
            .symbol(symbol)
            .expect("symbol")
            .timeline()
            .layers()
            .len(),
        1
    );
    ws.perform(MenuAction::TestMovie);
    assert!(
        !ws.in_symbol(),
        "the symbol edit was applied before playing"
    );
    assert_eq!(
        ws.main_document()
            .symbol(symbol)
            .expect("symbol")
            .timeline()
            .layers()
            .len(),
        2
    );
    assert!(ws.take_test_movie_request());
    assert_eq!(ws.label(MenuAction::TestMovie), "Test Movie");
}

#[test]
fn a_line_can_be_selected_by_clicking_its_stroke_and_moved() {
    let mut ws = workspace();
    ws.set_tool(Tool::Line);
    ws.begin_drag(Pos2::new(100.0, 100.0));
    ws.update_drag(Pos2::new(300.0, 100.0));
    ws.end_drag();
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(200.0, 101.0)); // within half the 4px stroke
    assert!(
        ws.has_selection(),
        "a line has no fill, so only its stroke can be clicked"
    );
    ws.update_drag(Pos2::new(200.0, 151.0));
    ws.end_drag();
    let bounds = ops::span_bounds(
        ws.document(),
        ws.scene_id().expect("scene"),
        ws.selected.expect("selected"),
    )
    .expect("bounds");
    assert!(
        (bounds.center().y - 150.0).abs() < 1.0,
        "moved 50 down: {bounds:?}"
    );
    ws.begin_drag(Pos2::new(200.0, 400.0));
    assert!(!ws.has_selection(), "empty space still deselects");
}

#[test]
fn finishing_a_pen_path_is_one_undo_step_on_a_new_layer() {
    let mut ws = workspace();
    pen_triangle(&mut ws);
    assert!(ws.pen_is_active());
    assert_eq!(layer_count(&ws), 1, "nothing is committed while drawing");
    ws.pen_finish();
    assert!(!ws.pen_is_active());
    assert_eq!(layer_count(&ws), 2);
    assert_eq!(layer_names(&ws)[1], "Path");
    ws.undo();
    assert_eq!(layer_count(&ws), 1, "one step removes the whole path");
}

#[test]
fn an_open_pen_path_is_stroked_even_with_outline_off() {
    let mut ws = workspace();
    ws.paint.stroke_enabled = false;
    pen_triangle(&mut ws);
    ws.pen_finish();
    let layers = ws.document().scenes()[0].timeline().layers();
    let Some(Content::Shape(shape)) = layers[1].spans().first().map(|s| s.content().clone()) else {
        panic!("a shape expected")
    };
    assert!(
        shape.stroke().is_some(),
        "an open path with no stroke would be invisible"
    );
}

#[test]
fn a_closed_pen_path_strokes_only_when_outline_is_on() {
    for outline in [false, true] {
        let mut ws = workspace();
        ws.paint.stroke_enabled = outline;
        pen_triangle(&mut ws);
        ws.pen_press(Pos2::new(11.0, 11.0));
        assert!(
            !ws.pen_is_active(),
            "clicking the first point closes and commits"
        );
        let layers = ws.document().scenes()[0].timeline().layers();
        let Some(Content::Shape(shape)) = layers[1].spans().first().map(|s| s.content().clone())
        else {
            panic!("a shape expected")
        };
        assert_eq!(shape.stroke().is_some(), outline);
    }
}

#[test]
fn escape_and_switching_tools_cancel_the_pen_path() {
    let mut ws = workspace();
    pen_triangle(&mut ws);
    ws.pen_cancel();
    assert!(!ws.pen_is_active());
    pen_triangle(&mut ws);
    ws.set_tool(Tool::Select);
    assert!(!ws.pen_is_active());
    assert_eq!(layer_count(&ws), 1, "cancelling commits nothing");
}

#[test]
fn finishing_with_one_point_reports_and_keeps_nothing() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pen);
    ws.pen_press(Pos2::new(5.0, 5.0));
    ws.pen_release();
    ws.pen_finish();
    assert_eq!(ws.status(), Some("A path needs at least two points"));
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn the_pen_menu_action_selects_the_tool() {
    let mut ws = workspace();
    ws.perform(MenuAction::ToolPen);
    assert_eq!(ws.tool(), Tool::Pen);
}

#[test]
fn shift_click_adds_and_removes_spans_from_the_selection() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    assert_eq!(ws.selection_all().len(), 2);
    ws.select_at(Pos2::new(30.0, 30.0), true);
    assert_eq!(ws.selection_all().len(), 1, "shift-click again removes it");
    ws.select_at(Pos2::new(30.0, 30.0), false);
    assert_eq!(
        ws.selection_all().len(),
        1,
        "a plain click replaces the selection"
    );
    ws.select_at(Pos2::new(300.0, 300.0), true);
    assert_eq!(ws.selection_all().len(), 1, "shift on empty stage keeps it");
    ws.select_at(Pos2::new(300.0, 300.0), false);
    assert!(ws.selection_all().is_empty());
}

#[test]
fn dragging_a_selected_span_moves_the_whole_selection_in_one_undo_step() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    ws.begin_drag(Pos2::new(30.0, 30.0));
    ws.update_drag(Pos2::new(40.0, 30.0));
    ws.end_drag();
    assert_eq!((rect_x(&ws, 1), rect_x(&ws, 2)), (20.0, 110.0));
    assert_eq!(ws.selection_all().len(), 2, "the group stays selected");
    ws.undo();
    assert_eq!((rect_x(&ws, 1), rect_x(&ws, 2)), (10.0, 100.0));
}

#[test]
fn delete_removes_every_selected_span_at_once() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    ws.delete_selection();
    assert!(ws.selection_all().is_empty());
    let layers = ws.document().scenes()[0].timeline().layers();
    assert!(layers[1].spans().is_empty() && layers[2].spans().is_empty());
    ws.undo();
    let layers = ws.document().scenes()[0].timeline().layers();
    assert!(!layers[1].spans().is_empty() && !layers[2].spans().is_empty());
}

#[test]
fn locking_a_layer_drops_only_its_span_from_the_selection() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    let locked = ws.document().scenes()[0].timeline().layers()[1].id();
    ws.toggle_layer_locked(locked);
    assert_eq!(ws.selection_all().len(), 1);
    assert_eq!(
        ws.selection_all()[0].layer,
        ws.document().scenes()[0].timeline().layers()[2].id()
    );
}

#[test]
fn zoom_menu_actions_zoom_the_stage_and_fit_resets_it() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0); // lays out the stage panel
    assert!(!ws.is_enabled(MenuAction::ZoomFit), "already fitted");
    ws.perform(MenuAction::ZoomIn);
    assert!(!ws.stage_view.is_fit());
    assert!(ws.is_enabled(MenuAction::ZoomFit));
    let zoomed = ws.stage_view;
    ws.perform(MenuAction::ZoomOut);
    assert_ne!(ws.stage_view, zoomed);
    ws.perform(MenuAction::ZoomIn);
    ws.perform(MenuAction::ZoomIn);
    ws.perform(MenuAction::ZoomFit);
    assert!(ws.stage_view.is_fit());
    assert!(!ws.is_dirty(), "the view is not part of the document");
}

#[test]
fn duplicate_copies_the_selection_independently_on_a_new_layer() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let original = ws.selected.expect("selected");
    ws.perform(MenuAction::Duplicate);
    assert_eq!(layer_count(&ws), 3);
    let copy = ws.selected.expect("the copy is selected");
    assert_ne!(copy.layer, original.layer);
    let scene = ws.scene_id().expect("scene");
    let bounds = |at| ops::span_bounds(ws.document(), scene, at).expect("bounds");
    assert_eq!(bounds(copy).min, bounds(original).min + Vec2::splat(12.0));
    let id_of = |at: SpanRef| match ops::find_span(ws.document(), scene, at)
        .expect("span")
        .content()
    {
        Content::Shape(shape) => shape.id(),
        _ => panic!("a shape expected"),
    };
    assert_ne!(
        id_of(copy),
        id_of(original),
        "the copy has its own identity"
    );
    // Moving the copy leaves the original alone.
    let before = bounds(original);
    let _ = bounds;
    ws.move_selection(Vec2::new(40.0, 0.0));
    let scene = ws.scene_id().expect("scene");
    let after = ops::span_bounds(ws.document(), scene, original).expect("bounds");
    assert_eq!(after, before);
    ws.undo();
    ws.undo();
    assert_eq!(layer_count(&ws), 2, "duplicating is one undo step");
}

#[test]
fn duplicate_without_a_selection_does_nothing() {
    let mut ws = workspace();
    assert!(!ws.is_enabled(MenuAction::Duplicate));
    ws.perform(MenuAction::Duplicate);
    assert_eq!(layer_count(&ws), 1);
    assert!(!ws.is_dirty());
}

#[test]
fn duplicate_copies_every_selected_span_and_selects_the_copies() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    let originals = ws.selection_all();
    ws.perform(MenuAction::Duplicate);
    assert_eq!(layer_count(&ws), 5);
    let copies = ws.selection_all();
    assert_eq!(copies.len(), 2);
    assert!(copies.iter().all(|c| !originals.contains(c)));
    ws.undo();
    assert_eq!(layer_count(&ws), 3, "one undo step for the whole group");
}

#[test]
fn flips_mirror_about_the_center_of_the_selection() {
    let mut ws = line_selected();
    ws.perform(MenuAction::FlipHorizontal);
    assert_eq!(
        endpoints(&selected_shape(&ws)),
        vec![(50.0, 10.0), (10.0, 30.0)]
    );
    ws.perform(MenuAction::FlipVertical);
    assert_eq!(
        endpoints(&selected_shape(&ws)),
        vec![(50.0, 30.0), (10.0, 10.0)]
    );
    assert!(
        selected_shape(&ws).stroke().is_some(),
        "the stroke survives"
    );
}

#[test]
fn rotation_turns_clockwise_on_screen_and_four_turns_come_home() {
    let mut ws = line_selected();
    ws.perform(MenuAction::RotateClockwise);
    let turned = endpoints(&selected_shape(&ws));
    // (10,10) is up-left of the center (30,20); clockwise 90 degrees puts it at (40,0).
    assert!(
        (turned[0].0 - 40.0).abs() < 1e-9 && turned[0].1.abs() < 1e-9,
        "{turned:?}"
    );
    ws.perform(MenuAction::RotateCounterclockwise);
    let back = endpoints(&selected_shape(&ws));
    assert!(
        (back[0].0 - 10.0).abs() < 1e-9 && (back[0].1 - 10.0).abs() < 1e-9,
        "{back:?}"
    );
    for _ in 0..4 {
        ws.perform(MenuAction::RotateClockwise);
    }
    let home = endpoints(&selected_shape(&ws));
    assert!(
        (home[1].0 - 50.0).abs() < 1e-9 && (home[1].1 - 30.0).abs() < 1e-9,
        "{home:?}"
    );
}

#[test]
fn scaling_grows_and_shrinks_about_the_center_and_each_is_one_undo_step() {
    let mut ws = line_selected();
    // The path itself scales; the stroke width does not, so measure the path.
    let length = |ws: &Workspace| {
        let p = endpoints(&selected_shape(ws));
        ((p[1].0 - p[0].0).powi(2) + (p[1].1 - p[0].1).powi(2)).sqrt()
    };
    let middle = |ws: &Workspace| {
        let p = endpoints(&selected_shape(ws));
        ((p[0].0 + p[1].0) / 2.0, (p[0].1 + p[1].1) / 2.0)
    };
    let before = length(&ws);
    ws.perform(MenuAction::ScaleUp);
    let grown = length(&ws);
    assert!((grown - before * 1.25).abs() < 1e-9);
    assert_eq!(middle(&ws), (30.0, 20.0), "scaling is about the center");
    ws.perform(MenuAction::ScaleDown);
    assert!((length(&ws) - before).abs() < 1e-9);
    ws.undo();
    assert!((length(&ws) - grown).abs() < 1e-9);
}

#[test]
fn transforms_need_a_selection_and_an_instance_keeps_its_symbol() {
    let mut ws = workspace();
    assert!(!ws.is_enabled(MenuAction::FlipHorizontal));
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    let scene = ws.scene_id().expect("scene");
    let at = ws.selected.expect("instance selected");
    let symbol = match ops::find_span(ws.document(), scene, at)
        .expect("span")
        .content()
    {
        Content::Instance(i) => i.symbol(),
        _ => panic!("an instance expected"),
    };
    let before = shape_bounds(&ws);
    ws.perform(MenuAction::RotateClockwise);
    let after = shape_bounds(&ws);
    assert!((after.width() - before.height()).abs() < 1e-3);
    assert!((after.center() - before.center()).length() < 1e-3);
    match ops::find_span(ws.document(), scene, at)
        .expect("span")
        .content()
    {
        Content::Instance(i) => assert_eq!(i.symbol(), symbol),
        _ => panic!("still an instance"),
    }
}

#[test]
fn degenerate_matrices_are_refused() {
    let mut ws = line_selected();
    let before = ws.document().clone();
    ws.transform_selection(DAffine2::from_scale(DVec2::ZERO));
    ws.transform_selection(DAffine2::from_scale(DVec2::new(f64::NAN, 1.0)));
    assert_eq!(ws.document(), &before);
}

#[test]
fn transforming_a_group_pivots_on_the_center_of_the_whole_selection() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    // The rectangles span x 10..140; flipping the group swaps them end for end.
    ws.perform(MenuAction::FlipHorizontal);
    assert_eq!((rect_x(&ws, 1), rect_x(&ws, 2)), (100.0, 10.0));
    ws.undo();
    assert_eq!(
        (rect_x(&ws, 1), rect_x(&ws, 2)),
        (10.0, 100.0),
        "one undo step"
    );
}
