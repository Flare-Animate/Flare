use super::*;

#[test]
fn typed_transforms_apply_about_the_selection_center_and_colors_swap() {
    let mut ws = workspace();
    rect(&mut ws); // 10..110 x 10..60, center (60, 35)
    ws.perform(MenuAction::SelectAll);
    ws.transform_edit.scale_x = 200.0;
    ws.apply_transform_edit();
    let scene = ws.scene_id().expect("scene");
    let at = ws.selected.expect("selected");
    let bounds = ops::span_bounds(ws.document(), scene, at).expect("bounds");
    assert!((bounds.width() - 200.0).abs() < 1e-2 && (bounds.center().x - 60.0).abs() < 1e-2);
    assert_eq!(
        ws.transform_edit,
        TransformEdit::default(),
        "the panel resets"
    );
    ws.transform_edit.rotate = 90.0;
    ws.apply_transform_edit();
    let turned = ops::span_bounds(ws.document(), scene, at).expect("bounds");
    assert!((turned.height() - 200.0).abs() < 1e-2);
    ws.undo();
    ws.undo();
    assert!(
        (ops::span_bounds(ws.document(), scene, at)
            .expect("bounds")
            .width()
            - 100.0)
            .abs()
            < 1e-2
    );
    ws.fill = Color32::RED;
    ws.stroke_color = Color32::BLUE;
    ws.swap_colors();
    assert_eq!((ws.fill, ws.stroke_color), (Color32::BLUE, Color32::RED));
    ws.default_colors();
    assert_eq!((ws.fill, ws.stroke_color), (Color32::WHITE, Color32::BLACK));
}

#[test]
fn the_library_makes_renames_and_deletes_symbols_but_refuses_to_delete_one_in_use() {
    let mut ws = workspace();
    ws.new_symbol();
    ws.new_symbol();
    let names: Vec<String> = ws
        .document()
        .symbols()
        .iter()
        .map(|s| s.name().to_owned())
        .collect();
    assert_eq!(names, ["Symbol 1", "Symbol 2"]);
    let id = ws.selected_symbol.expect("the new symbol is selected");
    ws.rename_symbol(id, "  Hero ");
    assert_eq!(ws.document().symbol(id).expect("symbol").name(), "Hero");
    ws.rename_symbol(id, "   ");
    assert_eq!(ws.document().symbol(id).expect("symbol").name(), "Hero");
    ws.place_selected_symbol();
    ws.delete_symbol(id);
    assert!(ws.document().symbol(id).is_some(), "in use");
    assert!(ws.status.is_some());
    ws.undo();
    ws.delete_symbol(id);
    assert!(ws.document().symbol(id).is_none());
    assert!(ws.selected_symbol.is_none());
    ws.undo();
    assert!(ws.document().symbol(id).is_some());
}

#[test]
fn the_frame_rate_changes_in_one_undo_step_and_rejects_nonsense() {
    let mut ws = workspace();
    ws.set_frame_rate(30.0);
    let rate = ws.document().frame_rate();
    assert_eq!((rate.numerator(), rate.denominator()), (30, 1));
    ws.set_frame_rate(29.97);
    let rate = ws.document().frame_rate();
    assert_eq!((rate.numerator(), rate.denominator()), (2997, 100));
    ws.set_frame_rate(0.0);
    ws.set_frame_rate(f64::NAN);
    ws.set_frame_rate(1000.0);
    assert_eq!(ws.document().frame_rate().numerator(), 2997);
    ws.undo();
    ws.undo();
    assert_eq!(ws.document().frame_rate().numerator(), 24);
}

#[test]
fn reports_reach_the_output_log_without_repeating_and_the_object_tab_resizes_the_selection() {
    let mut ws = workspace();
    ws.report("first");
    ws.report("first");
    ws.report("second");
    assert_eq!(ws.log, ["first", "second"]);
    for i in 0..(MAX_LOG_LINES + 20) {
        ws.report(format!("line {i}"));
    }
    assert_eq!(ws.log.len(), MAX_LOG_LINES);
    rect(&mut ws); // 10..110 x 10..60
    ws.perform(MenuAction::SelectAll);
    ws.set_selection_bounds(Rect::from_min_size(
        Pos2::new(20.0, 30.0),
        Vec2::new(200.0, 100.0),
    ));
    let b = ws.selection_bounds().expect("bounds");
    assert!((b.min.x - 20.0).abs() < 1e-2 && (b.min.y - 30.0).abs() < 1e-2);
    assert!((b.width() - 200.0).abs() < 1e-2 && (b.height() - 100.0).abs() < 1e-2);
    // Degenerate targets change nothing.
    ws.set_selection_bounds(Rect::from_min_size(Pos2::ZERO, Vec2::new(0.0, 5.0)));
    assert!((ws.selection_bounds().expect("bounds").width() - 200.0).abs() < 1e-2);
    ws.undo();
    assert!((ws.selection_bounds().expect("bounds").width() - 100.0).abs() < 1e-2);
}

#[test]
fn rulers_grid_and_snapping_toggle_from_the_menu_and_snapping_rounds_to_the_grid() {
    let mut ws = workspace();
    assert_eq!(ws.label(MenuAction::ToggleRulers), "Hide Rulers");
    ws.perform(MenuAction::ToggleRulers);
    assert!(!ws.show_rulers);
    assert_eq!(ws.label(MenuAction::ToggleGrid), "Show Grid");
    ws.perform(MenuAction::ToggleGrid);
    assert!(ws.show_grid);
    ws.perform(MenuAction::ToggleSnap);
    assert!(ws.snap_to_grid);
    assert_eq!(
        snap_point(Pos2::new(29.0, 31.0), 20.0),
        Pos2::new(20.0, 40.0)
    );
    assert!(tool_places_points(Tool::Rectangle) && !tool_places_points(Tool::Select));
    // A snapped move lands on grid multiples.
    rect(&mut ws);
    ws.perform(MenuAction::SelectAll);
    ws.set_tool(Tool::Select);
    ws.begin_drag(Pos2::new(50.0, 30.0));
    ws.update_drag(Pos2::new(77.0, 49.0)); // moved by (27, 19)
    ws.end_drag();
    let b = ws.selection_bounds().expect("bounds");
    assert_eq!((b.min.x, b.min.y), (30.0, 30.0), "10 + 20, 10 + 20");
    // A grid with lines too close together to see is skipped, not drawn.
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.show_grid = true;
    ws.grid_size = 1.0;
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
}

#[test]
fn playback_stops_on_the_last_frame_unless_looping_and_mute_silences_cues() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws); // 24 frames
    ws.perform(MenuAction::ToggleLoop);
    assert!(!ws.loop_playback);
    assert_eq!(ws.label(MenuAction::ToggleLoop), "Loop Playback: Off");
    ws.set_frame(22);
    ws.playing = true;
    ws.advance(0.0);
    ws.advance(1.0); // a second at 24 fps runs well past the end
    assert!(!ws.playing);
    assert_eq!(ws.frame, 23);
    ws.perform(MenuAction::ToggleLoop);
    ws.playing = true;
    ws.set_frame(22);
    ws.advance(2.0);
    ws.advance(3.0);
    assert!(ws.playing, "looping keeps playing");
    let _ = ctx;
    ws.perform(MenuAction::ToggleMute);
    assert!(ws.muted);
    assert_eq!(ws.label(MenuAction::ToggleMute), "Unmute Sounds");
    assert!(matches!(
        ws.take_audio_events().last(),
        Some(AudioEvent::Stop)
    ));
}
