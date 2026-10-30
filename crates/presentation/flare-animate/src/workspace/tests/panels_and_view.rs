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
    ws.paint.fill = Color32::RED;
    ws.paint.stroke_color = Color32::BLUE;
    ws.swap_colors();
    assert_eq!(
        (ws.paint.fill, ws.paint.stroke_color),
        (Color32::BLUE, Color32::RED)
    );
    ws.default_colors();
    assert_eq!(
        (ws.paint.fill, ws.paint.stroke_color),
        (Color32::WHITE, Color32::BLACK)
    );
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
    assert!(!ws.display.show_rulers);
    assert_eq!(ws.label(MenuAction::ToggleGrid), "Show Grid");
    ws.perform(MenuAction::ToggleGrid);
    assert!(ws.display.show_grid);
    ws.perform(MenuAction::ToggleSnap);
    assert!(ws.display.snap_to_grid);
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
    ws.display.show_grid = true;
    ws.display.grid_size = 1.0;
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

#[test]
fn group_makes_one_instance_and_ungroup_puts_the_parts_back_in_one_undo_step_each() {
    let mut ws = two_rectangles();
    assert!(!ws.is_enabled(MenuAction::Group), "nothing selected");
    ws.perform(MenuAction::SelectAll);
    assert!(ws.is_enabled(MenuAction::Group));
    assert!(
        !ws.is_enabled(MenuAction::Ungroup),
        "shapes are not instances"
    );
    ws.perform(MenuAction::Group);
    assert_eq!(ws.document().symbols().len(), 1);
    assert!(ws.selection_has_instance());
    assert!(ws.is_enabled(MenuAction::BreakApart));
    ws.perform(MenuAction::Ungroup);
    let shapes = ws.document().scenes()[0]
        .timeline()
        .layers()
        .iter()
        .filter(|l| {
            matches!(
                l.spans().first().map(|s| s.content()),
                Some(Content::Shape(_))
            )
        })
        .count();
    assert_eq!(shapes, 2, "both rectangles are back");
    assert_eq!(ws.selection_all().len(), 2, "the parts are selected");
    ws.undo();
    assert!(ws.selection_has_instance() || ws.document().symbols().len() == 1);
    ws.undo();
    assert!(
        ws.document().symbols().is_empty(),
        "undoing the group removes its symbol"
    );
}

#[test]
fn the_subselection_tool_drags_a_point_of_the_selected_shape() {
    let mut ws = workspace();
    rect(&mut ws); // 10..110 x 10..60
    ws.set_tool(Tool::Subselection);
    ws.select_at(Pos2::new(50.0, 30.0), false);
    assert_eq!(ws.anchor_points().len(), 4);
    ws.handle_radius = 6.0;
    // Pressing away from every point grabs nothing.
    ws.begin_drag(Pos2::new(60.0, 30.0));
    assert!(ws.anchor_drag.is_none());
    ws.end_drag();
    // The bottom-right corner is at (110, 60).
    ws.begin_drag(Pos2::new(111.0, 61.0));
    assert!(ws.anchor_drag.is_some());
    ws.update_drag(Pos2::new(150.0, 90.0));
    ws.end_drag();
    let b = ws.selection_bounds().expect("bounds");
    assert_eq!((b.max.x, b.max.y), (150.0, 90.0));
    ws.undo();
    assert_eq!(ws.selection_bounds().expect("bounds").max.x, 110.0);
    // Escape cancels a drag in progress.
    ws.begin_drag(Pos2::new(110.0, 60.0));
    ws.update_drag(Pos2::new(300.0, 300.0));
    ws.cancel_drag();
    ws.end_drag();
    assert_eq!(ws.selection_bounds().expect("bounds").max.x, 110.0);
}

#[test]
fn document_settings_opens_the_doc_tab_and_publishing_asks_the_shell_for_a_file() {
    let mut ws = workspace();
    ws.perform(MenuAction::DocumentSettings);
    assert_eq!(ws.props_tab, PropsTab::Doc);
    ws.perform(MenuAction::PublishSwf);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::PublishSwf));
    assert_eq!(ws.label(MenuAction::PublishSwf), "Publish SWF...");
}

#[test]
fn onion_outlines_are_a_setting_that_changes_what_is_drawn_not_just_a_flag() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    ws.set_frame(3);
    ws.set_onion_skin(true);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let faded = ws.texture.as_ref().map(|t| t.onion);
    assert_eq!(faded.map(|o| o.outlines), Some(false));
    ws.set_onion_outlines(true);
    run(&mut ws, &ctx, 0.2);
    run(&mut ws, &ctx, 0.3);
    assert_eq!(ws.texture.as_ref().map(|t| t.onion.outlines), Some(true));
    assert!(ws.onion_skin().outlines);
    ws.set_onion_outlines(false);
    assert!(!ws.onion_skin().outlines);
}

#[test]
fn the_library_draws_a_preview_of_the_selected_symbol_once_per_change() {
    let ctx = egui::Context::default();
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.perform(MenuAction::ConvertToSymbol);
    let symbol = ws.selected_symbol.expect("the new symbol is selected");
    let first = ws.symbol_preview(&ctx, symbol).expect("a preview");
    assert!(first.size()[0] > 0 && first.size()[1] > 0);
    // Same project, same texture; an edit makes a new one.
    let again = ws.symbol_preview(&ctx, symbol).expect("a preview");
    assert_eq!(first.id(), again.id());
    ws.new_symbol();
    let empty = ws.selected_symbol.expect("selected");
    assert!(
        ws.symbol_preview(&ctx, empty).is_none(),
        "an empty symbol has nothing to show"
    );
    // The panel itself draws with a symbol selected.
    ws.selected_symbol = Some(symbol);
    run(&mut ws, &ctx, 0.0);
}

#[test]
fn symbols_can_be_made_and_changed_into_each_kind_with_one_undo_step() {
    use flare_document::SymbolKind;
    let mut ws = workspace();
    for kind in SymbolKind::ALL {
        ws.new_symbol_kind = kind;
        ws.new_symbol();
        let id = ws.selected_symbol.expect("the new symbol is selected");
        assert_eq!(ws.document().symbol(id).expect("symbol").kind(), kind);
    }
    let id = ws.selected_symbol.expect("selected");
    ws.set_symbol_kind(id, SymbolKind::Graphic);
    assert_eq!(
        ws.document().symbol(id).expect("symbol").kind(),
        SymbolKind::Graphic
    );
    ws.undo();
    assert_eq!(
        ws.document().symbol(id).expect("symbol").kind(),
        SymbolKind::Button
    );
    // The same kind again is not an edit.
    let revision = ws.active().revision();
    ws.set_symbol_kind(id, SymbolKind::Button);
    assert_eq!(ws.active().revision(), revision);
}

#[test]
fn converting_a_selection_makes_the_chosen_kind_and_each_kind_has_its_own_icon() {
    use crate::workspace::panels::library::kind_icon;
    use flare_document::SymbolKind;
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.new_symbol_kind = SymbolKind::MovieClip;
    ws.perform(MenuAction::ConvertToSymbol);
    let id = ws.selected_symbol.expect("the symbol is selected");
    assert_eq!(
        ws.document().symbol(id).expect("symbol").kind(),
        SymbolKind::MovieClip
    );
    let icons: std::collections::HashSet<_> = SymbolKind::ALL.into_iter().map(kind_icon).collect();
    assert_eq!(icons.len(), 3);
    for icon in icons {
        assert!(flare_ui::icons::exists(icon), "{icon}");
    }
}
