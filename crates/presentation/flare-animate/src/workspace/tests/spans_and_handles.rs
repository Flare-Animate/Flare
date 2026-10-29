use super::*;

#[test]
fn dragging_a_span_body_moves_it_and_the_edge_resizes_it() {
    let (mut ws, id) = spans_workspace();
    assert!(ws.begin_span_drag(id, 1.5, 0.3));
    ws.update_span_drag(4);
    assert_eq!(
        ws.span_drag_preview(),
        Some((id, 3, 3)),
        "grabbed one frame in"
    );
    ws.end_span_drag();
    assert_eq!(active_spans_of(&ws, id), vec![(3, 3), (6, 2)]);
    ws.undo();
    assert_eq!(
        active_spans_of(&ws, id),
        vec![(0, 3), (6, 2)],
        "one undo step"
    );

    assert!(ws.begin_span_drag(id, 2.9, 0.3), "within the edge");
    ws.update_span_drag(5);
    assert_eq!(ws.span_drag_preview(), Some((id, 0, 6)));
    ws.end_span_drag();
    assert_eq!(active_spans_of(&ws, id), vec![(0, 6), (6, 2)]);
}

#[test]
fn a_span_drag_that_would_overlap_is_refused_with_a_message() {
    let (mut ws, id) = spans_workspace();
    assert!(ws.begin_span_drag(id, 0.5, 0.3));
    ws.update_span_drag(5); // 5..8 meets the span at 6
    ws.end_span_drag();
    assert_eq!(active_spans_of(&ws, id), vec![(0, 3), (6, 2)]);
    assert!(ws.status().is_some_and(|m| m.contains("overlap")));
    assert!(!ws.is_dirty());
}

#[test]
fn gaps_locked_layers_and_escape_do_not_drag_spans() {
    let (mut ws, id) = spans_workspace();
    assert!(!ws.begin_span_drag(id, 4.5, 0.3), "a gap scrubs instead");
    assert!(!ws.begin_span_drag(id, -1.0, 0.3));
    assert!(!ws.begin_span_drag(id, f32::NAN, 0.3));
    assert!(ws.begin_span_drag(id, 0.5, 0.3));
    ws.update_span_drag(2);
    ws.cancel_drag();
    ws.end_span_drag();
    assert_eq!(
        active_spans_of(&ws, id),
        vec![(0, 3), (6, 2)],
        "escape cancels"
    );
    ws.toggle_layer_locked(id);
    assert!(!ws.begin_span_drag(id, 0.5, 0.3), "locked layers refuse");
}

#[test]
fn timeline_zoom_changes_the_view_not_the_document_and_still_draws() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    ws.zoom_timeline(timeline_view::STEP);
    assert!(ws.timeline_zoom > 1.0);
    run(&mut ws, &ctx, 0.1);
    ws.zoom_timeline(1000.0);
    assert_eq!(ws.timeline_zoom, timeline_view::MAX_ZOOM);
    run(&mut ws, &ctx, 0.2);
    ws.zoom_timeline(0.0001);
    assert_eq!(ws.timeline_zoom, timeline_view::MIN_ZOOM);
    assert!(!ws.is_dirty());
}

#[test]
fn gradient_fills_apply_to_new_shapes_across_their_bounds_and_survive_moves() {
    let mut ws = workspace();
    ws.paint.gradient_enabled = true;
    ws.paint.gradient_end = Color32::from_rgb(0, 0, 255);
    ws.paint.fill = Color32::from_rgb(255, 0, 0);
    ws.commit_rectangle(Pos2::new(10.0, 20.0), Pos2::new(110.0, 60.0));
    let layer = ws.current_layer.expect("layer");
    ws.selected = Some(SpanRef { layer, start: 0 });
    let g = selected_shape(&ws).gradient().expect("a gradient");
    assert_eq!((g.from().as_vec().x, g.from().as_vec().y), (10.0, 20.0));
    assert_eq!(
        (g.to().as_vec().x, g.to().as_vec().y),
        (110.0, 20.0),
        "horizontal"
    );
    assert_eq!(g.start(), Rgba8::opaque(255, 0, 0));
    assert_eq!(g.end(), Rgba8::opaque(0, 0, 255));

    ws.move_selection(Vec2::new(5.0, 7.0));
    let g = selected_shape(&ws)
        .gradient()
        .expect("the move keeps the gradient");
    assert_eq!((g.from().as_vec().x, g.from().as_vec().y), (15.0, 27.0));
    assert_eq!(
        (g.to().as_vec().x, g.to().as_vec().y),
        (115.0, 27.0),
        "and moves it too"
    );

    ws.paint.gradient_vertical = true;
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(50.0, 80.0));
    let layer = ws.current_layer.expect("layer");
    ws.selected = Some(SpanRef { layer, start: 0 });
    let g = selected_shape(&ws).gradient().expect("a gradient");
    assert_eq!(
        (g.to().as_vec().x, g.to().as_vec().y),
        (0.0, 80.0),
        "vertical"
    );
}

#[test]
fn gradients_survive_duplicating_and_the_setting_survives_opening_a_project() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let mut ws = workspace();
    ws.paint.gradient_enabled = true;
    ws.commit_rectangle(Pos2::new(0.0, 0.0), Pos2::new(40.0, 40.0));
    let layer = ws.current_layer.expect("layer");
    ws.selected = Some(SpanRef { layer, start: 0 });
    ws.save_as(&project);
    ws.open(&project);
    assert!(ws.paint.gradient_enabled, "opening keeps the tool settings");
    let layer = ws.document().scenes()[0].timeline().layers()[1].id();
    ws.selected = Some(SpanRef { layer, start: 0 });
    assert!(
        selected_shape(&ws).gradient().is_some(),
        "saved and reloaded"
    );
}

#[test]
fn copy_then_paste_places_independent_copies_in_place_at_the_current_frame() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    assert!(!ws.is_enabled(MenuAction::Paste), "nothing copied yet");
    ws.perform(MenuAction::Copy);
    assert!(ws.is_enabled(MenuAction::Paste));
    ws.set_frame(5);
    ws.perform(MenuAction::PasteInPlace);
    assert_eq!(layer_count(&ws), 5);
    let pasted = ws.selection_all();
    assert_eq!(pasted.len(), 2);
    assert!(pasted.iter().all(|s| s.start == 5), "at the current frame");
    let scene = ws.scene_id().expect("scene");
    let ids: Vec<ShapeId> = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .flat_map(|l| l.spans().iter())
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => Some(shape.id()),
            _ => None,
        })
        .collect();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "every shape has its own identity");
    let bounds = ops::span_bounds(ws.document(), scene, pasted[0]).expect("bounds");
    assert_eq!(bounds.min.x, 10.0, "pasted in place, not offset");
    // The clipboard survives the original being deleted, and one undo removes the paste.
    ws.undo();
    assert_eq!(layer_count(&ws), 3);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.delete_selection();
    ws.perform(MenuAction::PasteInPlace);
    assert_eq!(layer_count(&ws), 5);
}

#[test]
fn plain_paste_centers_the_copies_on_the_stage_together_in_place_does_not() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.perform(MenuAction::Copy);
    ws.perform(MenuAction::Paste);
    let scene = ws.scene_id().expect("scene");
    let pasted = ws.selection_all()[0];
    let bounds = ops::span_bounds(ws.document(), scene, pasted).expect("bounds");
    let stage = ws.document().stage();
    assert!(
        (bounds.center().x - stage.width() as f32 / 2.0).abs() < 0.01
            && (bounds.center().y - stage.height() as f32 / 2.0).abs() < 0.01,
        "centered on the stage: {bounds:?}"
    );
    ws.undo();
    ws.perform(MenuAction::PasteInPlace);
    let pasted = ws.selection_all()[0];
    let bounds = ops::span_bounds(ws.document(), scene, pasted).expect("bounds");
    assert_eq!(bounds.min.x, 10.0, "where it was copied from");
}

#[test]
fn copy_and_paste_events_act_on_the_selection_unless_a_field_wants_the_keys() {
    let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
    ws.select_at(Pos2::new(30.0, 30.0), false);
    pass(&mut ws, &ctx, input(0.0, vec![Event::Copy]));
    assert!(ws.is_enabled(MenuAction::Paste));
    pass(&mut ws, &ctx, input(0.1, vec![Event::Paste(String::new())]));
    assert_eq!(layer_count(&ws), 4);
}

#[test]
fn pasting_an_instance_into_a_document_without_its_symbol_is_refused() {
    let mut source = workspace();
    draw_and_select(&mut source);
    source.convert_selection_to_symbol();
    source.perform(MenuAction::Copy);
    assert!(source.is_enabled(MenuAction::Paste));
    let items = source.clipboard.clone();
    let mut other = workspace();
    other.clipboard = items;
    other.perform(MenuAction::Paste);
    assert_eq!(layer_count(&other), 1);
    assert!(other.status().is_some_and(|m| m.contains("symbol")));
    assert!(!other.is_dirty());
    // Back in the document that has the symbol it works.
    source.perform(MenuAction::Paste);
    assert_eq!(layer_count(&source), 3);
}

#[test]
fn copying_nothing_or_an_empty_span_keeps_the_clipboard() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.perform(MenuAction::Copy);
    let kept = ws.clipboard.clone();
    ws.select_at(Pos2::new(300.0, 300.0), false);
    ws.copy_selection();
    assert_eq!(ws.clipboard, kept);
}

#[test]
fn dragging_a_corner_handle_scales_the_selection_about_the_opposite_corner() {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.begin_drag(Pos2::new(50.0, 50.0)); // the bottom-right handle
    assert!(matches!(
        ws.drag.map(|d| d.mode),
        Some(DragMode::Scale(Handle::BottomRight))
    ));
    ws.update_drag(Pos2::new(90.0, 90.0));
    ws.end_drag();
    let b = shape_bounds(&ws);
    assert_eq!(
        (b.min, b.max),
        (Pos2::new(10.0, 10.0), Pos2::new(90.0, 90.0))
    );
    assert_eq!(rect_x(&ws, 2), 100.0, "the other rectangle is untouched");
    ws.undo();
    assert_eq!(
        shape_bounds(&ws).max,
        Pos2::new(50.0, 50.0),
        "one undo step"
    );
}

#[test]
fn handles_of_a_group_scale_every_member_about_the_group_anchor() {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.select_at(Pos2::new(120.0, 30.0), true);
    // The group spans x 10..140; the right handle sits at (140, 30).
    ws.begin_drag(Pos2::new(140.0, 30.0));
    ws.update_drag(Pos2::new(270.0, 30.0));
    ws.end_drag();
    // Doubling the width about x = 10: the second rectangle (100..140) lands at 190..270.
    assert_eq!(rect_x(&ws, 1), 10.0);
    assert_eq!(rect_x(&ws, 2), 190.0);
}

#[test]
fn shift_keeps_proportions_and_pressing_off_a_handle_still_moves() {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.uniform_scale = true;
    ws.begin_drag(Pos2::new(50.0, 50.0));
    ws.update_drag(Pos2::new(90.0, 60.0)); // x doubles, y grows less
    ws.end_drag();
    let b = shape_bounds(&ws);
    assert_eq!((b.width(), b.height()), (80.0, 80.0));

    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.begin_drag(Pos2::new(30.0, 30.0)); // inside, away from every handle
    assert_eq!(ws.drag.map(|d| d.mode), Some(DragMode::Move));
}

#[test]
fn a_cancelled_scale_changes_nothing_and_shift_press_never_grabs_a_handle() {
    let mut ws = two_rectangles();
    ws.set_tool(Tool::FreeTransform);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    ws.begin_drag(Pos2::new(50.0, 50.0));
    ws.update_drag(Pos2::new(90.0, 90.0));
    ws.cancel_drag();
    ws.end_drag();
    assert_eq!(shape_bounds(&ws).max, Pos2::new(50.0, 50.0));
    ws.begin_drag_with(Pos2::new(50.0, 50.0), true);
    assert!(ws.drag.is_none(), "shift only edits the selection");
}

#[test]
fn dragging_to_the_very_last_frame_does_not_overflow() {
    let (mut ws, id) = spans_workspace();
    assert!(ws.begin_span_drag(id, 2.9, 0.3), "the right edge");
    ws.update_span_drag(u32::MAX);
    let (_, start, duration) = ws.span_drag_preview().expect("preview");
    assert_eq!((start, u64::from(duration)), (0, u64::from(u32::MAX)));
    ws.end_span_drag();
    assert_eq!(
        active_spans_of(&ws, id),
        vec![(0, 3), (6, 2)],
        "it would overlap, so refused"
    );
}

#[test]
fn only_rows_in_view_are_painted() {
    let clip = |a: f32, b: f32| Rect::from_min_max(Pos2::new(0.0, a), Pos2::new(100.0, b));
    // Header row 0..22; layer 0 spans 22..44, layer 1 44..66, and so on.
    assert_eq!(
        visible_rows(0.0, 22.0, clip(0.0, 44.0)),
        (0, 1),
        "only layer 0 is below the header"
    );
    assert_eq!(visible_rows(0.0, 22.0, clip(0.0, 45.0)), (0, 2));
    assert_eq!(visible_rows(0.0, 22.0, clip(100.0, 140.0)), (3, 3));
    // The track starts below the top of the scroll content.
    assert_eq!(visible_rows(50.0, 22.0, clip(0.0, 80.0)), (0, 1));
    for bad in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
        assert_eq!(visible_rows(0.0, bad, clip(0.0, 80.0)).1, 0, "row {bad}");
    }
    assert_eq!(visible_rows(f32::NAN, 22.0, clip(0.0, 80.0)), (0, 0));
    assert_eq!(visible_rows(0.0, 22.0, clip(f32::NAN, f32::NAN)), (0, 0));
}

#[test]
fn the_texture_covers_what_is_on_screen_at_screen_resolution() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_stage_size(2000, 1000);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1); // the second pass knows the panel
    let fitted = ws.texture.as_ref().expect("texture").view;
    assert_eq!(
        fitted.region.size(),
        Vec2::new(2000.0, 1000.0),
        "all of it is visible"
    );
    assert!(
        fitted.size[0] < 1400 && fitted.size[1] < 800,
        "the texture is about the size of the window, not of the 2000x1000 stage: {:?}",
        fitted.size
    );

    // Zoomed in 8x about the center, only an eighth of the stage is visible, and the texture
    // is no bigger than before: the pixels go into what can be seen.
    for _ in 0..3 {
        ws.perform(MenuAction::ZoomIn);
        ws.perform(MenuAction::ZoomIn);
        ws.perform(MenuAction::ZoomIn);
    }
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    let zoomed = ws.texture.as_ref().expect("texture").view;
    assert!(
        zoomed.region.width() < fitted.region.width() / 4.0,
        "{zoomed:?}"
    );
    assert!(
        zoomed.size[0] <= 1400 && zoomed.size[1] <= 800,
        "{:?}",
        zoomed.size
    );
    let density = |v: &TextureView| v.size[0] as f32 / v.region.width();
    assert!(
        density(&zoomed) > density(&fitted) * 4.0,
        "and it is sharper"
    );
}

#[test]
fn a_stage_too_big_to_render_whole_still_shows_the_visible_part() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_stage_size(16_000, 16_000);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let texture = ws.texture.as_ref().expect("a texture, not a render error");
    assert!(
        texture.view.size[0] <= texture_side_limit(&ctx)
            && texture.view.size[1] <= texture_side_limit(&ctx)
    );
    assert!(
        ws.status().is_none_or(|m| !m.contains("Cannot render")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn panning_changes_the_texture_view_but_not_the_document() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_stage_size(2000, 1000);
    run(&mut ws, &ctx, 0.0); // lays out the panel, which zooming aims at
    for _ in 0..4 {
        ws.perform(MenuAction::ZoomIn);
    }
    run(&mut ws, &ctx, 0.1);
    run(&mut ws, &ctx, 0.15);
    let before = ws.texture.as_ref().expect("texture").view;
    ws.stage_view.pan_by(Vec2::new(-150.0, 0.0));
    run(&mut ws, &ctx, 0.2);
    let after = ws.texture.as_ref().expect("texture").view;
    assert!(
        after.region.min.x > before.region.min.x,
        "{before:?} then {after:?}"
    );
    assert_eq!(after.size, before.size);
}

#[test]
fn only_the_free_transform_tool_grabs_handles_and_both_select_by_clicking() {
    let mut ws = two_rectangles();
    ws.select_at(Pos2::new(30.0, 30.0), false);
    // With the selection tool the corner is just part of the shape: pressing near it moves it.
    ws.begin_drag(Pos2::new(46.0, 46.0));
    assert!(matches!(ws.drag.map(|d| d.mode), Some(DragMode::Move)));
    ws.end_drag();
    ws.set_tool(Tool::FreeTransform);
    ws.begin_drag(Pos2::new(50.0, 50.0));
    assert!(matches!(
        ws.drag.map(|d| d.mode),
        Some(DragMode::Scale(Handle::BottomRight))
    ));
    ws.cancel_drag();
    // It still picks what is under the pointer.
    ws.begin_drag(Pos2::new(120.0, 30.0));
    ws.end_drag();
    assert_eq!(ws.selection_all().len(), 1);
    assert_eq!(ws.label(MenuAction::ToolFreeTransform), "Free Transform");
    ws.perform(MenuAction::ToolSelect);
    ws.perform(MenuAction::ToolFreeTransform);
    assert_eq!(ws.tool(), Tool::FreeTransform);
}
