use super::*;

#[test]
fn renders_the_stage_and_refreshes_when_content_or_frame_changes() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.texture.as_ref().map(|t| t.key), Some((0, 0, 0)));
    rect(&mut ws);
    run(&mut ws, &ctx, 0.1);
    let after_edit = ws.texture.as_ref().expect("texture").key;
    assert_eq!(after_edit.2, 0);
    assert!(after_edit.1 > 0);
    ws.set_frame(5);
    run(&mut ws, &ctx, 0.2);
    assert_eq!(ws.texture.as_ref().map(|t| t.key.2), Some(5));
}

#[test]
fn untitled_documents_start_clean_with_one_layer() {
    let ws = Workspace::untitled().expect("untitled");
    assert_eq!(layer_count(&ws), 1);
    assert!(!ws.is_dirty());
    assert_eq!(ws.title(), "Untitled - Flare");
}

#[test]
fn a_rectangle_is_one_undo_step_and_tracks_dirty_state() {
    let mut ws = workspace();
    assert!(!ws.is_dirty());
    rect(&mut ws);
    assert_eq!(layer_count(&ws), 2, "layer and span are added together");
    assert!(ws.is_dirty());
    assert!(ws.title().contains('*'));
    ws.undo();
    assert_eq!(layer_count(&ws), 1);
    assert!(!ws.is_dirty());
    ws.redo();
    assert_eq!(layer_count(&ws), 2);
}

#[test]
fn rectangles_get_a_stroke_only_when_outline_is_enabled() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.stroke_enabled = true;
    ws.stroke_color = Color32::from_rgb(1, 2, 3);
    ws.stroke_width = 6.0;
    ws.commit_rectangle(Pos2::new(200.0, 200.0), Pos2::new(260.0, 240.0));
    let stroke_of = |layer: usize| {
        let timeline = ws.document().scenes()[0].timeline();
        match timeline.layers()[layer].spans()[0].content() {
            Content::Shape(shape) => shape.stroke(),
            _ => panic!("shape expected"),
        }
    };
    assert_eq!(stroke_of(1), None);
    let stroke = stroke_of(2).expect("outlined rectangle");
    assert_eq!(
        (stroke.width(), stroke.color()),
        (6.0, Rgba8::opaque(1, 2, 3))
    );
}

#[test]
fn clicks_and_degenerate_drags_create_nothing() {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(5.0, 5.0), Pos2::new(5.0, 5.0));
    ws.commit_rectangle(Pos2::new(5.0, 5.0), Pos2::new(5.5, 90.0));
    assert_eq!(layer_count(&ws), 1);
    assert!(!ws.is_dirty());
}

#[test]
fn keyboard_shortcuts_undo_and_redo() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    rect(&mut ws);
    let command = Modifiers::COMMAND;
    pass(&mut ws, &ctx, input(0.1, vec![key(Key::Z, command)]));
    assert_eq!(layer_count(&ws), 1);
    let both = Modifiers::COMMAND | Modifiers::SHIFT;
    pass(&mut ws, &ctx, input(0.2, vec![key(Key::Z, both)]));
    assert_eq!(layer_count(&ws), 2);
}

#[test]
fn escape_cancels_a_drag_without_committing() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    ws.drag = Some(Drag {
        bounds: Rect::NOTHING,
        mode: DragMode::Draw,
        start: Pos2::new(1.0, 1.0),
        current: Pos2::new(50.0, 50.0),
    });
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Escape, Modifiers::NONE)]),
    );
    assert!(ws.drag.is_none());
    assert!(ws.drag_cancelled);
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn playback_advances_at_the_document_frame_rate_and_loops() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws); // makes the scene 24 frames long
    run(&mut ws, &ctx, 0.0);
    ws.playing = true;
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.5);
    assert_eq!(ws.frame(), 12);
    run(&mut ws, &ctx, 1.0);
    assert_eq!(ws.frame(), 0, "24 fps over 24 frames wraps around");
    ws.playing = false;
    run(&mut ws, &ctx, 5.0);
    assert_eq!(ws.frame(), 0);
}

#[test]
fn frames_are_clamped_to_the_scene() {
    let mut ws = workspace();
    ws.set_frame(10_000);
    assert_eq!(ws.frame(), 0, "an empty scene has a single frame");
    rect(&mut ws);
    ws.set_frame(10_000);
    assert_eq!(ws.frame(), 23);
}

#[test]
fn save_and_open_round_trip_and_unsaved_work_opens_beside_not_over_it() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("a.flare");
    let mut ws = workspace();
    ws.save();
    assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
    rect(&mut ws);
    ws.save_as(&path);
    assert!(!ws.is_dirty());
    assert_eq!(ws.path(), Some(path.as_path()));

    rect(&mut ws);
    ws.open(&path);
    assert_eq!(ws.tabs().len(), 2, "the file opened in a tab of its own");
    assert_eq!(layer_count(&ws), 2, "the saved copy is on show");
    ws.switch_to_tab(0);
    assert_eq!(layer_count(&ws), 3, "dirty work was not replaced");

    ws.save();
    let mut other = workspace();
    other.open(&path);
    assert_eq!(layer_count(&other), 3);
    assert_eq!(other.path(), Some(path.as_path()));
}

#[test]
fn failed_open_reports_and_keeps_the_current_document() {
    let dir = tempfile::tempdir().expect("dir");
    let mut ws = workspace();
    ws.open(&dir.path().join("missing.flare"));
    assert!(ws.status().is_some_and(|s| s.contains("Cannot open")));
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn screen_positions_map_to_clamped_stage_coordinates() {
    let rect = Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(400.0, 225.0)); // half scale
    assert_eq!(
        screen_to_stage(Pos2::new(100.0, 50.0), rect, Vec2::new(800.0, 450.0)),
        Pos2::ZERO
    );
    assert_eq!(
        screen_to_stage(Pos2::new(300.0, 162.5), rect, Vec2::new(800.0, 450.0)),
        Pos2::new(400.0, 225.0)
    );
    assert_eq!(
        screen_to_stage(Pos2::new(-999.0, 9999.0), rect, Vec2::new(800.0, 450.0)),
        Pos2::new(0.0, 450.0)
    );
}

#[test]
fn select_tool_selects_the_shape_under_the_pointer_and_clears_on_empty_space() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    assert!(ws.has_selection());
    ws.begin_drag(Pos2::new(700.0, 400.0));
    assert!(!ws.has_selection());
    assert!(ws.drag.is_none(), "nothing to move");
}

#[test]
fn dragging_a_selection_moves_it_as_one_undo_step() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let before = ws.document().clone();
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(80.0, 20.0));
    assert_eq!(
        ws.document(),
        &before,
        "the document is untouched mid-gesture"
    );
    ws.end_drag();
    assert_eq!(shape_bounds(&ws).min, Pos2::new(40.0, 0.0));
    ws.undo();
    assert_eq!(ws.document(), &before);
    assert!(ws.has_selection(), "the span still exists after undo");
}

#[test]
fn cancelled_and_switched_gestures_commit_nothing() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    let before = ws.document().clone();
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(300.0, 300.0));
    ws.cancel_drag();
    ws.end_drag();
    assert_eq!(ws.document(), &before);

    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(300.0, 300.0));
    ws.set_tool(Tool::Rectangle);
    ws.end_drag();
    assert_eq!(ws.document(), &before);
}

#[test]
fn delete_removes_the_selection_and_undo_brings_it_back() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.delete_selection();
    assert_eq!(spans_of(&ws, 1), 0);
    assert!(!ws.has_selection());
    ws.undo();
    assert_eq!(spans_of(&ws, 1), 1);
}

#[test]
fn convert_to_symbol_then_place_instances_from_the_library() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    assert!(ws.can_convert_selection());
    ws.convert_selection_to_symbol();
    assert_eq!(ws.document().symbols().len(), 1);
    assert!(ws.selected_symbol.is_some());
    assert!(
        ws.has_selection(),
        "the instance span takes the shape's place"
    );
    assert!(!ws.can_convert_selection(), "an instance is not a shape");

    let layers = layer_count(&ws);
    ws.place_selected_symbol();
    assert_eq!(layer_count(&ws), layers + 1);
    ws.undo();
    assert_eq!(layer_count(&ws), layers);
    ws.undo();
    assert_eq!(ws.document().symbols().len(), 0);
    assert!(
        ws.selected_symbol.is_none(),
        "stale library selection is dropped"
    );
}

#[test]
fn placing_without_a_library_selection_explains_itself() {
    let mut ws = workspace();
    ws.place_selected_symbol();
    assert!(ws.status().is_some_and(|s| s.contains("library")));
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn delete_and_f8_shortcuts_act_on_the_selection() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    draw_and_select(&mut ws);
    run(&mut ws, &ctx, 0.0);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::F8, Modifiers::NONE)]),
    );
    assert_eq!(ws.document().symbols().len(), 1);
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::Delete, Modifiers::NONE)]),
    );
    assert_eq!(spans_of(&ws, 1), 0);
}

#[test]
fn stage_edits_are_undoable_and_invalid_sizes_are_refused() {
    let mut ws = workspace();
    let original = ws.document().stage();
    ws.set_stage_size(400, 200);
    assert_eq!(
        (
            ws.document().stage().width(),
            ws.document().stage().height()
        ),
        (400, 200)
    );
    assert!(ws.is_dirty());
    ws.set_stage_background(Rgba8::opaque(1, 2, 3));
    assert_eq!(ws.document().stage().background(), Rgba8::opaque(1, 2, 3));
    assert_eq!(ws.document().stage().width(), 400, "size is kept");
    ws.undo();
    ws.undo();
    assert_eq!(ws.document().stage(), original);

    ws.set_stage_size(0, 100);
    assert!(ws.status().is_some_and(|s| s.contains("stage sides")));
    assert_eq!(ws.document().stage(), original);
    let before = ws.document().clone();
    ws.set_stage_background(original.background());
    assert_eq!(
        ws.document(),
        &before,
        "an unchanged background is not an edit"
    );
}

#[test]
fn placement_and_rendering_follow_the_document_stage() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_stage_size(400, 200);
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    ws.place_selected_symbol();
    let scene = ws.scene_id().expect("scene");
    let layer = ws.document().scenes()[0]
        .timeline()
        .layers()
        .last()
        .expect("layer")
        .id();
    let placed =
        ops::span_bounds(ws.document(), scene, SpanRef { layer, start: 0 }).expect("bounds");
    assert_eq!(placed.center(), Pos2::new(200.0, 100.0));
    run(&mut ws, &ctx, 0.0);
    assert!(ws.texture.is_some());
    assert_eq!(ws.stage_extent(), Vec2::new(400.0, 200.0));
}

#[test]
fn drawing_makes_the_new_layer_active_and_frame_ops_act_on_it() {
    let mut ws = workspace();
    rect(&mut ws);
    assert_eq!(
        ws.active_layer_id(),
        Some(ws.document().scenes()[0].timeline().layers()[1].id())
    );
    assert_eq!(active_spans(&ws), vec![(0, 24)]);
    ws.set_frame(10);
    ws.frame_edit(FrameOp::InsertKeyframe);
    assert_eq!(active_spans(&ws), vec![(0, 10), (10, 14)]);
    ws.frame_edit(FrameOp::InsertBlankKeyframe); // already a keyframe at 10
    assert!(
        ws.status()
            .is_some_and(|s| s.contains("Cannot insert a blank keyframe"))
    );
    ws.set_frame(3);
    ws.frame_edit(FrameOp::InsertFrame);
    assert_eq!(active_spans(&ws), vec![(0, 11), (11, 14)]);
    ws.frame_edit(FrameOp::RemoveFrame);
    assert_eq!(active_spans(&ws), vec![(0, 10), (10, 14)]);
    ws.set_frame(10);
    ws.frame_edit(FrameOp::ClearKeyframe);
    assert_eq!(active_spans(&ws), vec![(0, 24)]);
    ws.undo();
    assert_eq!(
        active_spans(&ws),
        vec![(0, 10), (10, 14)],
        "each operation is one undo step"
    );
}

#[test]
fn selecting_a_layer_redirects_frame_operations() {
    let mut ws = workspace();
    rect(&mut ws);
    let first = ws.document().scenes()[0].timeline().layers()[0].id();
    ws.select_layer(first);
    ws.frame_edit(FrameOp::InsertFrame);
    assert!(
        ws.status().is_some(),
        "the first layer is empty, so there is nothing to extend"
    );
    assert_eq!(layer_count(&ws), 2);
}
