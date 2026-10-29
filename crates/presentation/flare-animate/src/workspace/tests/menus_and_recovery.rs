use super::*;

#[test]
fn menu_actions_report_state_and_perform_through_the_workspace() {
    let mut ws = workspace();
    assert!(!ws.is_enabled(MenuAction::Undo));
    assert!(!ws.is_enabled(MenuAction::Delete));
    assert!(!ws.is_enabled(MenuAction::ExitSymbol));
    assert!(
        !ws.is_enabled(MenuAction::DeleteLayer),
        "the last layer stays"
    );
    assert!(ws.is_enabled(MenuAction::Save) && ws.is_enabled(MenuAction::Quit));

    ws.perform(MenuAction::Undo); // disabled: must be a no-op, not an error
    assert!(!ws.is_dirty());

    ws.perform(MenuAction::ToolEllipse);
    assert_eq!(ws.tool(), Tool::Ellipse);
    rect(&mut ws);
    assert!(ws.is_enabled(MenuAction::Undo));
    assert!(ws.is_enabled(MenuAction::DeleteLayer));
    assert!(ws.is_enabled(MenuAction::MoveLayerUp));
    assert!(
        !ws.is_enabled(MenuAction::MoveLayerDown),
        "already the bottom layer"
    );
    ws.perform(MenuAction::Undo);
    assert_eq!(layer_count(&ws), 1);
    ws.perform(MenuAction::Redo);
    assert_eq!(layer_count(&ws), 2);

    ws.perform(MenuAction::PlayPause);
    assert!(ws.is_playing());
    assert_eq!(ws.label(MenuAction::PlayPause), "Pause");
    ws.perform(MenuAction::PlayPause);
    assert_eq!(ws.label(MenuAction::PlayPause), "Play");
}

#[test]
fn toggling_actions_relabel_themselves() {
    let mut ws = workspace();
    rect(&mut ws);
    assert_eq!(ws.label(MenuAction::ToggleLayerVisible), "Hide Layer");
    ws.perform(MenuAction::ToggleLayerVisible);
    assert_eq!(ws.label(MenuAction::ToggleLayerVisible), "Show Layer");
    assert_eq!(ws.label(MenuAction::ToggleLayerLocked), "Lock Layer");
    ws.perform(MenuAction::ToggleLayerLocked);
    assert_eq!(ws.label(MenuAction::ToggleLayerLocked), "Unlock Layer");
    assert!(
        !ws.is_enabled(MenuAction::InsertFrame),
        "a locked layer refuses frame edits"
    );
}

#[test]
fn tween_and_symbol_actions_follow_the_selection() {
    let (mut ws, _) = with_symbol();
    assert!(ws.is_enabled(MenuAction::EditSymbol));
    assert!(
        !ws.is_enabled(MenuAction::ConvertToSymbol),
        "an instance is not a shape"
    );
    assert_eq!(
        ws.label(MenuAction::ToggleMotionTween),
        "Create Motion Tween"
    );
    ws.perform(MenuAction::ToggleMotionTween);
    assert_eq!(
        ws.label(MenuAction::ToggleMotionTween),
        "Remove Motion Tween"
    );
    ws.perform(MenuAction::EditSymbol);
    assert!(ws.in_symbol());
    assert!(ws.is_enabled(MenuAction::ExitSymbol));
    ws.perform(MenuAction::ExitSymbol);
    assert!(!ws.in_symbol());
}

#[test]
fn dialogs_and_quit_are_requests_for_the_shell() {
    let mut ws = workspace();
    ws.perform(MenuAction::Open);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::Open));
    ws.perform(MenuAction::SaveAs);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::SaveAs));
    assert!(!ws.take_quit_request());
    ws.perform(MenuAction::Quit);
    assert!(ws.take_quit_request());
    assert!(!ws.take_quit_request(), "a request is delivered once");
}

#[test]
fn a_native_menu_owns_its_shortcuts_so_they_are_not_applied_twice() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    ws.set_native_menu(true);
    run(&mut ws, &ctx, 0.0);
    let command = Modifiers::COMMAND;
    pass(&mut ws, &ctx, input(0.1, vec![key(Key::Z, command)]));
    assert_eq!(layer_count(&ws), 2, "undo was left to the native menu");
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::F6, Modifiers::NONE)]),
    );
    assert_eq!(active_spans(&ws), vec![(0, 24)], "so was insert keyframe");
    ws.set_native_menu(false);
    pass(&mut ws, &ctx, input(0.3, vec![key(Key::Z, command)]));
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn autosave_writes_a_recovery_file_only_when_dirty_and_the_interval_has_passed() {
    let dir = tempfile::tempdir().expect("dir");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.enable_recovery(dir.path().to_owned());
    ws.set_autosave_interval(10.0);
    let id = ws.document().id();
    let file = flare_storage::recovery_path(dir.path(), id);

    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 60.0);
    assert!(!file.exists(), "a clean document is never autosaved");

    rect(&mut ws);
    run(&mut ws, &ctx, 61.0);
    assert!(
        !file.exists(),
        "the interval counts from the last clean moment"
    );
    run(&mut ws, &ctx, 71.5);
    assert!(file.exists());
    assert_eq!(flare_storage::load(&file).expect("load"), *ws.document());

    let before = std::fs::metadata(&file)
        .expect("meta")
        .modified()
        .expect("time");
    run(&mut ws, &ctx, 72.0);
    assert_eq!(
        std::fs::metadata(&file)
            .expect("meta")
            .modified()
            .expect("time"),
        before
    );
}

#[test]
fn saving_or_undoing_back_to_clean_removes_the_recovery_file() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let store = dir.path().join("recovery");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.enable_recovery(store.clone());
    ws.set_autosave_interval(1.0);
    let file = flare_storage::recovery_path(&store, ws.document().id());

    run(&mut ws, &ctx, 0.0);
    rect(&mut ws);
    run(&mut ws, &ctx, 5.0);
    assert!(file.exists());
    ws.undo();
    run(&mut ws, &ctx, 6.0);
    assert!(!file.exists(), "undone back to the clean state");

    rect(&mut ws);
    run(&mut ws, &ctx, 20.0);
    assert!(file.exists());
    ws.save_as(&project);
    assert!(
        !file.exists(),
        "saving to the project supersedes the recovery file"
    );
}

#[test]
fn recovered_work_is_unsaved_and_keeps_its_file_until_saved() {
    let dir = tempfile::tempdir().expect("dir");
    let store = dir.path().join("recovery");
    let mut crashed = workspace();
    rect(&mut crashed);
    let saved = flare_storage::save_recovery(crashed.document(), &store).expect("recovery");

    let mut ws = workspace();
    ws.enable_recovery(store.clone());
    ws.recover(&saved);
    assert_eq!(layer_count(&ws), 2);
    assert!(
        ws.is_dirty(),
        "recovered work has never been saved to a project"
    );
    assert!(ws.path().is_none());
    assert!(saved.exists(), "kept in case of another crash");
    ws.discard_recovery();
    assert!(!saved.exists());
}

#[test]
fn recovery_refuses_to_replace_unsaved_work_and_reports_bad_files() {
    let dir = tempfile::tempdir().expect("dir");
    let mut ws = workspace();
    rect(&mut ws);
    ws.recover(&dir.path().join("nope.flare"));
    assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
    assert_eq!(layer_count(&ws), 2);
    ws.undo();
    ws.recover(&dir.path().join("nope.flare"));
    assert!(ws.status().is_some_and(|s| s.contains("Cannot recover")));
}

#[test]
fn opening_a_project_keeps_the_native_menu_tool_and_recovery_settings() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let mut source = workspace();
    source.save_as(&project);

    let mut ws = workspace();
    ws.set_native_menu(true);
    ws.set_tool(Tool::Ellipse);
    ws.fill = Color32::from_rgb(1, 2, 3);
    ws.stroke_enabled = true;
    ws.enable_recovery(dir.path().join("recovery"));
    ws.open(&project);
    assert_eq!(ws.path(), Some(project.as_path()));
    assert!(
        ws.native_menu,
        "opening must not turn the in-window menu back on"
    );
    assert_eq!(ws.tool(), Tool::Ellipse);
    assert_eq!(ws.fill, Color32::from_rgb(1, 2, 3));
    assert!(ws.stroke_enabled);
    assert!(ws.recovery_dir.is_some());
}

#[test]
fn the_dock_survives_drawing_and_every_panel_stays_present() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    for t in 0..5 {
        run(&mut ws, &ctx, f64::from(t));
    }
    let json = ws.layout_json().expect("serialize");
    assert!(
        layout::from_json(&json).is_ok(),
        "all four panels are still in the layout"
    );
    assert!(
        ws.texture.is_some(),
        "the stage panel was drawn inside the dock"
    );
}

#[test]
fn a_saved_layout_restores_and_an_unusable_one_is_refused_and_changes_nothing() {
    let mut ws = workspace();
    let saved = ws.layout_json().expect("serialize");
    ws.reset_layout();
    ws.restore_layout(&saved).expect("restore");

    let before = ws.layout_json().expect("serialize");
    for bad in ["not json", "{}", &saved.replace("\"Tools\"", "\"Stage\"")] {
        assert!(ws.restore_layout(bad).is_err(), "{bad}");
    }
    let after = ws.layout_json().expect("serialize");
    let value = |t: &str| serde_json::from_str::<serde_json::Value>(t).expect("json");
    assert_eq!(
        value(&before),
        value(&after),
        "a refused layout leaves the current one alone"
    );
}

#[test]
fn resetting_the_layout_never_touches_the_document() {
    let mut ws = workspace();
    rect(&mut ws);
    let document = ws.document().clone();
    let dirty = ws.is_dirty();
    ws.perform(MenuAction::ResetLayout);
    assert_eq!(ws.document(), &document);
    assert_eq!(ws.is_dirty(), dirty);
    assert!(ws.is_enabled(MenuAction::ResetLayout));
}

#[test]
fn opening_a_project_keeps_the_users_panel_arrangement() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let mut source = workspace();
    source.save_as(&project);
    let mut ws = workspace();
    // A real arrangement: every panel in one tab group, nothing like the default.
    let mut tiles = egui_tiles::Tiles::default();
    let panes: Vec<_> = Pane::ALL
        .into_iter()
        .rev()
        .map(|pane| tiles.insert_pane(pane))
        .collect();
    let root = tiles.insert_tab_tile(panes);
    let custom_tree = egui_tiles::Tree::new(egui::Id::unique("custom"), root, tiles);
    let custom = layout::to_json(&custom_tree).expect("serialize");
    ws.restore_layout(&custom)
        .expect("a tab group of every panel is valid");

    let value = |t: &str| serde_json::from_str::<serde_json::Value>(t).expect("json");
    let default = layout::to_json(&layout::default_tree()).expect("serialize");
    assert_ne!(
        value(&custom),
        value(&default),
        "the test needs a non-default layout"
    );

    ws.open(&project);
    assert_eq!(value(&ws.layout_json().expect("serialize")), value(&custom));
    ws.reset_layout();
    assert_eq!(
        value(&ws.layout_json().expect("serialize")),
        value(&default)
    );
}

#[test]
fn ghosts_are_chosen_nearest_last_with_falling_opacity_and_stay_inside_the_scene() {
    // Frame 5 of 10, two before and two after: farthest first so nearer ones draw on top.
    let ghosts = onion_ghosts(5, 10, 2, 2);
    let frames: Vec<u32> = ghosts.iter().map(|(f, _)| *f).collect();
    assert_eq!(frames, vec![3, 7, 4, 6]);
    assert!(ghosts[0].1 < ghosts[2].1, "farther ghosts are fainter");
    assert_eq!(ghosts[2].1, ghosts[3].1, "equal distance, equal opacity");
    // Edges of the scene are skipped, never wrapped or negative.
    assert_eq!(onion_ghosts(0, 10, 3, 0), vec![]);
    assert_eq!(onion_ghosts(9, 10, 0, 3), vec![]);
    assert_eq!(
        onion_ghosts(1, 3, 3, 3)
            .iter()
            .map(|(f, _)| *f)
            .collect::<Vec<_>>(),
        vec![0, 2]
    );
    // The range is capped, and zero means none.
    assert_eq!(
        onion_ghosts(100, 1000, 50, 0).len(),
        MAX_ONION_FRAMES as usize
    );
    assert!(onion_ghosts(5, 10, 0, 0).is_empty());
    assert!(
        onion_ghosts(u32::MAX, u32::MAX, 0, 2).is_empty(),
        "no overflow at the top"
    );
}

#[test]
fn onion_skin_re_renders_when_toggled_and_is_off_while_playing() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    assert_eq!(ws.texture.as_ref().map(|t| t.onion.enabled), Some(false));
    let before = ws.texture.as_ref().expect("texture").key;

    ws.perform(MenuAction::ToggleOnionSkin);
    assert!(ws.onion_skin().enabled);
    assert_eq!(ws.label(MenuAction::ToggleOnionSkin), "Hide Onion Skin");
    run(&mut ws, &ctx, 0.1);
    assert_eq!(ws.texture.as_ref().map(|t| t.onion.enabled), Some(true));
    assert_eq!(
        ws.texture.as_ref().expect("texture").key,
        before,
        "same content, new settings"
    );

    rect(&mut ws);
    ws.playing = true;
    run(&mut ws, &ctx, 0.2);
    assert_eq!(
        ws.texture.as_ref().map(|t| t.onion.enabled),
        Some(false),
        "off while playing"
    );
    ws.playing = false;
    run(&mut ws, &ctx, 0.3);
    assert_eq!(
        ws.texture.as_ref().map(|t| t.onion.enabled),
        Some(true),
        "back when stopped"
    );
}

#[test]
fn onion_range_is_clamped_and_survives_opening_a_project() {
    let dir = tempfile::tempdir().expect("dir");
    let project = dir.path().join("p.flare");
    let mut source = workspace();
    source.save_as(&project);
    let mut ws = workspace();
    ws.set_onion_range(99, 3);
    ws.set_onion_skin(true);
    assert_eq!(
        (ws.onion_skin().before, ws.onion_skin().after),
        (MAX_ONION_FRAMES, 3)
    );
    ws.open(&project);
    assert_eq!(
        ws.onion_skin(),
        OnionSkin {
            enabled: true,
            before: MAX_ONION_FRAMES,
            after: 3
        }
    );
}

#[test]
fn the_history_lists_each_edit_and_clicking_one_returns_to_it() {
    let mut ws = workspace();
    let (entries, current) = ws.history_entries();
    assert_eq!(
        (entries.as_slice(), current),
        (["Original".to_owned()].as_slice(), 0)
    );
    rect(&mut ws);
    ws.perform(MenuAction::SelectAll);
    ws.move_selection(Vec2::new(5.0, 0.0));
    let (entries, current) = ws.history_entries();
    assert_eq!(entries, ["Original", "Draw rectangle", "Move"]);
    assert_eq!(current, 2);
    ws.jump_in_history(1);
    assert_eq!(layer_count(&ws), 2, "the drawing is still there");
    assert_eq!(ws.history_entries().1, 1);
    assert_eq!(
        ws.history_entries().0.len(),
        3,
        "the later step can still be redone"
    );
    ws.jump_in_history(0);
    assert_eq!(layer_count(&ws), 1);
    ws.jump_in_history(2);
    assert_eq!(layer_count(&ws), 2);
    ws.jump_in_history(99); // out of range: nothing happens
    assert_eq!(ws.history_entries().1, 2);
    ws.clear_history();
    assert_eq!(ws.history_entries().0, ["Original"]);
    assert_eq!(layer_count(&ws), 2, "the document stays as it is");
}

#[test]
fn new_and_close_start_an_empty_project_but_never_over_unsaved_work_and_revert_reloads() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let mut ws = workspace();
    rect(&mut ws);
    ws.perform(MenuAction::New);
    assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
    assert_eq!(layer_count(&ws), 2, "unsaved work was kept");
    assert!(
        !ws.is_enabled(MenuAction::Revert),
        "nothing was saved to go back to"
    );
    ws.save_as(&path);
    rect(&mut ws);
    assert!(ws.is_enabled(MenuAction::Revert));
    ws.perform(MenuAction::Revert);
    ws.settle_inline();
    assert_eq!(layer_count(&ws), 2, "back to the saved copy");
    assert!(!ws.is_dirty());
    ws.perform(MenuAction::Close);
    assert_eq!(layer_count(&ws), 1, "an empty project");
    assert_eq!(ws.path(), None);
    assert_eq!(ws.title(), "Untitled - Flare");
}

#[test]
fn preferences_round_trip_stay_in_range_and_refuse_junk() {
    let mut ws = workspace();
    ws.perform(MenuAction::Preferences);
    assert!(ws.preferences_open);
    let mut wanted = ws.preferences();
    wanted.autosave_seconds = 90.0;
    wanted.show_grid = true;
    wanted.grid_size = 8.0;
    wanted.loop_playback = false;
    ws.apply_preferences(&wanted);
    let json = ws.preferences_json().expect("json");
    let mut other = workspace();
    other.restore_preferences(&json).expect("restore");
    assert_eq!(other.preferences(), wanted);
    // Out-of-range and missing values are brought into range or defaulted.
    other
        .restore_preferences(r#"{"autosave_seconds": 0.0, "grid_size": 1e9}"#)
        .expect("partial");
    let p = other.preferences();
    assert_eq!((p.autosave_seconds, p.grid_size), (5.0, 500.0));
    assert!(p.show_rulers, "a missing field takes its default");
    // Junk changes nothing.
    assert!(other.restore_preferences("{ not json").is_err());
    assert!(
        other
            .restore_preferences(&" ".repeat(MAX_PREFERENCES_BYTES + 1))
            .is_err()
    );
    assert_eq!(other.preferences(), p);
    // Drawing the window does not panic and closing it works.
    let ctx = egui::Context::default();
    run(&mut other, &ctx, 0.0);
    other.preferences_open = false;
    run(&mut other, &ctx, 0.1);
}

#[test]
fn importing_an_xfl_folder_makes_a_new_unsaved_project_and_never_replaces_unsaved_work() {
    let mut source = workspace();
    rect(&mut source);
    let output = flare_export::export_xfl(source.document()).expect("export");
    let dir = tempfile::tempdir().expect("dir");
    let folder = dir.path().join("art");
    flare_export::write_directory(&output, &folder).expect("write");

    let mut ws = workspace();
    ws.import_project(&folder);
    assert_eq!(layer_count(&ws), 2, "the drawing came across");
    assert_eq!(
        ws.path(),
        None,
        "an import is not tied to the file it came from"
    );
    assert!(ws.is_dirty(), "it is unsaved work until saved as a project");
    assert!(ws.status().is_some_and(|s| s.contains("Imported")));
    // Unsaved work is protected.
    ws.import_project(&folder);
    assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
    // Something that is not an XFL says why.
    let mut other = workspace();
    other.import_project(&dir.path().join("missing"));
    assert!(other.status().is_some_and(|s| s.contains("Cannot import")));
    assert_eq!(layer_count(&other), 1);
    assert_eq!(
        other.label(MenuAction::ImportProject),
        "Import XFL or FLA..."
    );
    other.perform(MenuAction::ImportProject);
    assert_eq!(other.take_dialog(), Some(DialogRequest::ImportProject));
}
