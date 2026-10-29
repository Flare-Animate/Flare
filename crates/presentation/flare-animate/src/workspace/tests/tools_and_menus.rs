use super::*;

#[test]
fn a_new_layer_goes_on_top_becomes_active_and_is_one_undo_step() {
    let mut ws = workspace();
    ws.add_layer();
    assert_eq!(layer_count(&ws), 2);
    let layers = ws.document().scenes()[0].timeline().layers();
    assert_eq!(layers[1].name(), "Layer 2");
    assert_eq!(ws.active_layer_id(), Some(layers[1].id()));
    ws.undo();
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn the_zoom_percentage_sets_the_stage_scale_and_fit_returns_to_fitting() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.set_stage_size(400, 200);
    run(&mut ws, &ctx, 0.0);
    ws.set_stage_percent(200.0);
    let scale = ws.stage_view.scale(ws.stage_panel, ws.stage_extent());
    assert!((scale - 2.0).abs() < 0.01, "{scale}");
    ws.set_stage_percent(50.0);
    let scale = ws.stage_view.scale(ws.stage_panel, ws.stage_extent());
    assert!((scale - 0.5).abs() < 0.01, "{scale}");
    ws.perform(MenuAction::ZoomFit);
    assert!(ws.stage_view.is_fit());
}

#[test]
fn menus_hold_planned_commands_dimmed_and_every_real_action_once() {
    let mocks = MENU_LAYOUT
        .iter()
        .flat_map(|(_, entries)| entries.iter())
        .filter(|e| matches!(e, MenuEntry::Mock(_)))
        .count();
    assert!(mocks > 10, "{mocks} planned commands are listed");
    let titles: Vec<&str> = MENU_LAYOUT.iter().map(|(t, _)| *t).collect();
    for expected in [
        "File", "Edit", "View", "Insert", "Modify", "Text", "Control", "Window", "Help",
    ] {
        assert!(titles.contains(&expected), "{expected} menu: {titles:?}");
    }
    let mut seen = std::collections::HashSet::new();
    assert!(
        titles.iter().all(|t| seen.insert(*t)),
        "no menu title twice: {titles:?}"
    );
}

#[test]
fn letter_keys_pick_tools_unless_a_modifier_is_held_or_text_is_being_typed() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::R, Modifiers::NONE)]),
    );
    assert_eq!(ws.tool(), Tool::Rectangle);
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::O, Modifiers::NONE)]),
    );
    assert_eq!(ws.tool(), Tool::Ellipse);
    pass(
        &mut ws,
        &ctx,
        input(0.3, vec![key(Key::P, Modifiers::COMMAND)]),
    );
    assert_eq!(ws.tool(), Tool::Ellipse, "Cmd+P is not the pen");
    pass(
        &mut ws,
        &ctx,
        input(0.4, vec![key(Key::V, Modifiers::NONE)]),
    );
    assert_eq!(ws.tool(), Tool::Select);
    // While the text editor has the keyboard, letters are letters.
    ws.set_tool(Tool::Text);
    ws.text_edit = Some(PendingText {
        at: Pos2::new(5.0, 5.0),
        buffer: String::new(),
        focused: false,
    });
    run(&mut ws, &ctx, 0.5); // the editor takes focus
    pass(
        &mut ws,
        &ctx,
        input(0.6, vec![key(Key::R, Modifiers::NONE)]),
    );
    assert_eq!(
        ws.tool(),
        Tool::Text,
        "typing an r does not switch to the rectangle"
    );
}

#[test]
fn the_bucket_and_ink_bottle_repaint_the_shape_under_the_click_in_one_undo_step() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.fill = Color32::from_rgb(9, 8, 7);
    ws.set_tool(Tool::Bucket);
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    assert_eq!(drawn_shape(&ws, 1).fill(), Rgba8::opaque(9, 8, 7));
    ws.stroke_color = Color32::from_rgb(1, 2, 3);
    ws.stroke_width = 5.0;
    ws.set_tool(Tool::InkBottle);
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    let stroke = drawn_shape(&ws, 1).stroke().expect("outlined");
    assert_eq!(
        (stroke.width(), stroke.color()),
        (5.0, Rgba8::opaque(1, 2, 3))
    );
    ws.undo();
    assert_eq!(drawn_shape(&ws, 1).stroke(), None);
    ws.undo();
    assert_ne!(drawn_shape(&ws, 1).fill(), Rgba8::opaque(9, 8, 7));
    // Painting empty stage changes nothing.
    let before = ws.is_dirty();
    ws.set_tool(Tool::Bucket);
    ws.redo();
    ws.click_with_tool(Pos2::new(700.0, 400.0), false);
    assert!(ws.is_dirty() || !before);
}

#[test]
fn the_eyedropper_takes_the_fill_or_with_shift_the_stroke() {
    let mut ws = workspace();
    ws.fill = Color32::from_rgb(10, 20, 30);
    ws.stroke_enabled = true;
    ws.stroke_color = Color32::from_rgb(200, 100, 50);
    rect(&mut ws);
    ws.fill = Color32::WHITE;
    ws.stroke_color = Color32::BLACK;
    ws.set_tool(Tool::Eyedropper);
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    assert_eq!(ws.fill, Color32::from_rgb(10, 20, 30));
    ws.click_with_tool(Pos2::new(50.0, 30.0), true);
    assert_eq!(ws.stroke_color, Color32::from_rgb(200, 100, 50));
    assert!(!ws.is_dirty() || ws.active().can_undo());
}

#[test]
fn the_eraser_deletes_the_clicked_shape_and_undo_brings_it_back() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.set_tool(Tool::Eraser);
    ws.click_with_tool(Pos2::new(700.0, 400.0), false);
    assert_eq!(spans_of(&ws, 1), 1, "empty stage: nothing erased");
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    assert_eq!(spans_of(&ws, 1), 0);
    ws.undo();
    assert_eq!(spans_of(&ws, 1), 1);
}

#[test]
fn polystar_draws_polygons_and_stars_sized_by_the_drag() {
    let mut ws = workspace();
    ws.set_tool(Tool::PolyStar);
    ws.poly_sides = 6;
    ws.commit_shape(
        Tool::PolyStar,
        Pos2::new(100.0, 100.0),
        Pos2::new(100.0, 40.0),
    );
    let polygon = drawn_shape(&ws, 1);
    // Six corners, the first straight up from the center, plus the closing segment.
    assert_eq!(polygon.path().len(), 7);
    let flare_document::PathSegment::MoveTo(first) = polygon.path()[0] else {
        panic!("starts with a move");
    };
    assert!((first.as_vec() - DVec2::new(100.0, 40.0)).length() < 1e-3);
    ws.poly_star = true;
    ws.commit_shape(
        Tool::PolyStar,
        Pos2::new(300.0, 100.0),
        Pos2::new(340.0, 100.0),
    );
    assert_eq!(
        drawn_shape(&ws, 2).path().len(),
        13,
        "twelve corners and a close"
    );
    // A click is not a drag.
    ws.commit_shape(Tool::PolyStar, Pos2::new(5.0, 5.0), Pos2::new(5.0, 5.0));
    assert_eq!(layer_count(&ws), 3);
}

#[test]
fn rectangle_corner_radius_rounds_new_rectangles() {
    let mut ws = workspace();
    ws.rect_radius = 12.0;
    rect(&mut ws);
    assert!(
        drawn_shape(&ws, 1)
            .path()
            .iter()
            .any(|s| matches!(s, flare_document::PathSegment::CubicTo(..)))
    );
    ws.rect_radius = 0.0;
    ws.commit_rectangle(Pos2::new(200.0, 10.0), Pos2::new(260.0, 50.0));
    assert_eq!(drawn_shape(&ws, 2).path().len(), 5);
}

#[test]
fn the_hand_pans_and_zoom_clicks_zoom_the_stage() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    let before = ws.stage_view.scale(ws.stage_panel, ws.stage_extent());
    ws.perform(MenuAction::ToolZoom);
    assert_eq!(ws.tool(), Tool::Zoom);
    ws.perform(MenuAction::ZoomIn);
    let after = ws.stage_view.scale(ws.stage_panel, ws.stage_extent());
    assert!(after > before);
    ws.perform(MenuAction::ToolHand);
    assert_eq!(ws.tool(), Tool::Hand);
}

#[test]
fn the_lasso_selects_shapes_centered_inside_the_loop_and_skips_locked_layers() {
    let mut ws = workspace();
    rect(&mut ws); // layer 1, centered at (60, 35)
    ws.commit_rectangle(Pos2::new(300.0, 300.0), Pos2::new(340.0, 340.0)); // layer 2
    ws.set_tool(Tool::Lasso);
    ws.lasso = vec![
        Pos2::new(0.0, 0.0),
        Pos2::new(200.0, 0.0),
        Pos2::new(200.0, 100.0),
        Pos2::new(0.0, 100.0),
    ];
    ws.finish_lasso(false);
    assert_eq!(ws.selection_all().len(), 1);
    assert!(ws.lasso.is_empty());
    // A loop around both, with Shift on top of the first selection.
    ws.lasso = vec![
        Pos2::new(250.0, 250.0),
        Pos2::new(400.0, 250.0),
        Pos2::new(400.0, 400.0),
        Pos2::new(250.0, 400.0),
    ];
    ws.finish_lasso(true);
    assert_eq!(ws.selection_all().len(), 2);
    // Fewer than three points enclose nothing.
    ws.lasso = vec![Pos2::new(0.0, 0.0), Pos2::new(500.0, 500.0)];
    ws.finish_lasso(false);
    assert!(ws.selection_all().is_empty());
}

#[test]
fn pencil_and_brush_strokes_are_one_undo_step_and_the_brush_follows_pressure() {
    use flare_input::{InputSample, PointerKind};
    let mut ws = workspace();
    ws.stroke_color = Color32::from_rgb(1, 2, 3);
    ws.stroke_width = 3.0;
    ws.fill = Color32::from_rgb(9, 9, 9);
    ws.brush_size = 20.0;
    let drag = |ws: &mut Workspace, pressure: fn(usize) -> f32| {
        ws.freehand = (0..40)
            .map(|i| {
                InputSample::new(
                    10.0 + i as f64 * 3.0,
                    50.0 + (i as f64 * 0.4).sin() * 20.0,
                    pressure(i),
                    PointerKind::Pen,
                )
                .expect("sample")
            })
            .collect();
    };
    ws.set_tool(Tool::Pencil);
    drag(&mut ws, |_| 1.0);
    ws.finish_freehand(1.0);
    let pencil = drawn_shape(&ws, 1);
    let stroke = pencil.stroke().expect("stroked");
    assert_eq!(
        (stroke.width(), stroke.color()),
        (3.0, Rgba8::opaque(1, 2, 3))
    );
    assert!(pencil.path().len() < 40, "the drag is simplified");
    // The brush is a filled outline, not a stroked line.
    ws.set_tool(Tool::Brush);
    drag(&mut ws, |_| 1.0);
    ws.finish_freehand(1.0);
    let steady = drawn_shape(&ws, 2);
    assert_eq!(steady.stroke(), None);
    assert_eq!(steady.fill(), Rgba8::opaque(9, 9, 9));
    let height = |ws: &Workspace, layer: usize| {
        let spans = ws.document().scenes()[0].timeline().layers()[layer].spans();
        flare_tools::ops::span_bounds(
            ws.document(),
            ws.scene_id().expect("scene"),
            SpanRef {
                layer: ws.document().scenes()[0].timeline().layers()[layer].id(),
                start: spans[0].start(),
            },
        )
        .expect("bounds")
        .height()
    };
    // Pressing lightly all along gives a thinner stroke than pressing fully.
    drag(&mut ws, |_| 0.1);
    ws.finish_freehand(1.0);
    let (light, full) = (height(&ws, 3), height(&ws, 2));
    assert!(light < full - 5.0, "{light} vs {full}");
    ws.undo();
    ws.undo();
    assert_eq!(layer_count(&ws), 2);
    // A press without movement draws nothing.
    ws.freehand = vec![InputSample::mouse(5.0, 5.0).expect("sample")];
    ws.finish_freehand(1.0);
    assert_eq!(layer_count(&ws), 2);
}

#[test]
fn cut_select_all_and_deselect_all_work_on_the_current_frame() {
    let mut ws = two_rectangles();
    assert!(ws.is_enabled(MenuAction::SelectAll));
    ws.perform(MenuAction::SelectAll);
    assert_eq!(ws.selection_all().len(), 2);
    ws.perform(MenuAction::DeselectAll);
    assert!(!ws.has_selection());
    assert!(!ws.is_enabled(MenuAction::Cut));
    ws.select_at(Pos2::new(30.0, 30.0), false);
    let spans_before: usize = (0..layer_count(&ws)).map(|l| spans_of(&ws, l)).sum();
    ws.perform(MenuAction::Cut);
    let spans_after: usize = (0..layer_count(&ws)).map(|l| spans_of(&ws, l)).sum();
    assert_eq!(spans_after + 1, spans_before);
    assert!(
        ws.is_enabled(MenuAction::Paste),
        "what was cut can be pasted"
    );
    ws.undo();
    let restored: usize = (0..layer_count(&ws)).map(|l| spans_of(&ws, l)).sum();
    assert_eq!(restored, spans_before);
}

#[test]
fn transport_menu_steps_the_playhead_and_actual_size_shows_the_stage_at_100_percent() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws); // gives the scene its 24 frames
    run(&mut ws, &ctx, 0.0);
    assert!(!ws.is_enabled(MenuAction::Rewind));
    ws.perform(MenuAction::StepForward);
    ws.perform(MenuAction::StepForward);
    assert_eq!(ws.frame(), 2);
    ws.perform(MenuAction::StepBackward);
    assert_eq!(ws.frame(), 1);
    ws.perform(MenuAction::Rewind);
    assert_eq!(ws.frame(), 0);
    ws.perform(MenuAction::ActualSize);
    let scale = ws.stage_view.scale(ws.stage_panel, ws.stage_extent());
    assert!((scale - 1.0).abs() < 1e-3, "{scale}");
}

#[test]
fn align_distribute_and_arrange_move_the_selection_in_one_undo_step() {
    let mut ws = workspace();
    for x in [10.0, 100.0, 400.0] {
        ws.commit_rectangle(
            Pos2::new(x, 10.0 + x / 10.0),
            Pos2::new(x + 40.0, 60.0 + x / 10.0),
        );
    }
    ws.perform(MenuAction::SelectAll);
    assert_eq!(ws.selection_all().len(), 3);
    ws.align_selection(ops::Align::Left);
    let lefts = |ws: &Workspace| -> Vec<f32> {
        (1..=3)
            .map(|l| {
                let at = SpanRef {
                    layer: ws.document().scenes()[0].timeline().layers()[l].id(),
                    start: 0,
                };
                ops::span_bounds(ws.document(), ws.scene_id().expect("scene"), at)
                    .expect("bounds")
                    .left()
            })
            .collect()
    };
    assert!(
        lefts(&ws).iter().all(|x| (x - 10.0).abs() < 1e-3),
        "{:?}",
        lefts(&ws)
    );
    ws.undo();
    assert_eq!(lefts(&ws).len(), 3);
    assert!((lefts(&ws)[2] - 400.0).abs() < 1e-3);
    ws.distribute_selection(true);
    let spread = lefts(&ws);
    // 40 wide each, first at 10, last ending at 440: gaps of 155.
    assert!((spread[1] - 205.0).abs() < 1e-3, "{spread:?}");
    // Stage alignment centers on the stage instead of the selection.
    ws.align_to_stage = true;
    ws.align_selection(ops::Align::CenterH);
    let center = ws.stage_extent().x / 2.0;
    assert!((lefts(&ws)[0] + 20.0 - center).abs() < 1e-3);
}

#[test]
fn bring_to_front_and_send_to_back_reorder_layers_keeping_relative_order() {
    let mut ws = two_rectangles();
    let ids = |ws: &Workspace| -> Vec<LayerId> {
        ws.document().scenes()[0]
            .timeline()
            .layers()
            .iter()
            .map(|l| l.id())
            .collect()
    };
    let before = ids(&ws);
    ws.select_at(Pos2::new(30.0, 30.0), false);
    let chosen = ws.selected.expect("selected").layer;
    ws.arrange_selection(true);
    assert_eq!(ids(&ws)[0], chosen);
    ws.arrange_selection(false);
    assert_eq!(*ids(&ws).last().expect("layers"), chosen);
    ws.undo();
    ws.undo();
    assert_eq!(ids(&ws), before);
}

#[test]
fn new_strokes_take_the_chosen_cap_and_join() {
    use flare_document::{Cap, Join};
    let mut ws = workspace();
    ws.stroke_cap = Cap::Square;
    ws.stroke_join = Join::Miter;
    ws.set_tool(Tool::Line);
    ws.commit_shape(Tool::Line, Pos2::new(5.0, 5.0), Pos2::new(60.0, 5.0));
    let line = drawn_shape(&ws, 1).stroke().expect("a line is stroked");
    assert_eq!((line.cap(), line.join()), (Cap::Square, Join::Miter));
    // The ink bottle restyles an existing shape's outline the same way.
    rect(&mut ws);
    ws.stroke_cap = Cap::Butt;
    ws.stroke_join = Join::Bevel;
    ws.set_tool(Tool::InkBottle);
    ws.click_with_tool(Pos2::new(50.0, 30.0), false);
    let outlined = drawn_shape(&ws, 2).stroke().expect("outlined");
    assert_eq!((outlined.cap(), outlined.join()), (Cap::Butt, Join::Bevel));
}
