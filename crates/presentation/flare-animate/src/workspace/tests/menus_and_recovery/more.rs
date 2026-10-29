use super::*;

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
            after: 3,
            outlines: false
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
fn revert_reloads_the_saved_copy_and_closing_the_last_document_leaves_an_empty_one() {
    let dir = tempfile::tempdir().expect("dir");
    let path = dir.path().join("p.flare");
    let mut ws = workspace();
    rect(&mut ws);
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
    assert_eq!(ws.tabs().len(), 1, "reverting stays in the same tab");
    ws.perform(MenuAction::Close);
    assert_eq!(layer_count(&ws), 1, "an empty project");
    assert_eq!(ws.path(), None);
    assert_eq!(ws.title(), "Untitled - Flare");
    assert_eq!(ws.tabs().len(), 1);
}

#[test]
fn tabs_hold_several_documents_switch_between_them_and_close_without_losing_work() {
    let mut ws = workspace();
    rect(&mut ws); // tab 0: a drawing, unsaved
    ws.perform(MenuAction::New);
    assert_eq!(ws.tabs().len(), 2);
    assert!(ws.tabs()[1].current && !ws.tabs()[0].current);
    assert_eq!(layer_count(&ws), 1, "the new document is empty");
    assert!(ws.any_dirty(), "the first tab's unsaved work counts");
    assert!(!ws.is_dirty(), "the one on show is untouched");
    ws.set_tool(Tool::Ellipse);
    ws.commit_shape(Tool::Ellipse, Pos2::new(5.0, 5.0), Pos2::new(60.0, 40.0));
    ws.commit_shape(Tool::Ellipse, Pos2::new(70.0, 5.0), Pos2::new(90.0, 40.0));
    assert_eq!(layer_count(&ws), 3);

    // Switching brings the other document back whole and keeps the person's tool.
    ws.switch_to_tab(0);
    assert_eq!(layer_count(&ws), 2);
    assert_eq!(ws.tool(), Tool::Ellipse, "the tool in hand stays in hand");
    assert!(ws.tabs()[0].current);
    ws.switch_to_tab(1);
    assert_eq!(layer_count(&ws), 3);
    ws.switch_to_tab(7); // no such tab: nothing happens
    assert_eq!(layer_count(&ws), 3);

    // Dirty tabs are not closed from under their work.
    ws.perform(MenuAction::Close);
    assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
    assert_eq!(ws.tabs().len(), 2);
    ws.close_tab(0);
    assert!(ws.status().is_some_and(|s| s.contains("Save or undo")));
    // Undoing back to clean allows it; the neighbour takes over.
    ws.undo();
    ws.undo();
    assert!(!ws.is_dirty());
    ws.perform(MenuAction::Close);
    assert_eq!(ws.tabs().len(), 1);
    assert_eq!(layer_count(&ws), 2, "the first document is the one left");
    // A parked, clean tab closes without being shown.
    ws.perform(MenuAction::New);
    ws.switch_to_tab(0);
    ws.undo();
    ws.undo();
    ws.close_tab(1);
    assert_eq!(ws.tabs().len(), 1);
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
fn importing_an_xfl_folder_makes_a_new_unsaved_project_in_place_of_a_blank_one_and_beside_other_work()
 {
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
    // A second import opens beside the first rather than over its unsaved work.
    ws.import_project(&folder);
    assert_eq!(ws.tabs().len(), 2);
    assert!(ws.any_dirty());
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

#[test]
fn a_fla_held_in_memory_imports_like_one_read_from_disk() {
    let mut source = workspace();
    rect(&mut source);
    let output = flare_export::export_xfl(source.document()).expect("export");
    let bytes = flare_export::fla_bytes(&output).expect("bytes");
    let mut ws = workspace();
    ws.import_project_bytes("art.fla", bytes);
    assert_eq!(layer_count(&ws), 2);
    assert_eq!(ws.path(), None);
    assert!(ws.is_dirty());
    let mut other = workspace();
    other.import_project_bytes("junk.fla", b"not a zip".to_vec());
    assert!(other.status().is_some_and(|s| s.contains("Cannot import")));
    assert_eq!(layer_count(&other), 1);
}

#[test]
fn export_movie_asks_the_shell_for_a_file() {
    let mut ws = workspace();
    assert_eq!(ws.label(MenuAction::ExportMovie), "Export Movie (GIF)...");
    assert!(ws.is_enabled(MenuAction::ExportMovie));
    ws.perform(MenuAction::ExportMovie);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ExportMovie));
}

#[test]
fn recent_projects_are_remembered_newest_first_capped_and_kept_with_the_preferences() {
    let dir = tempfile::tempdir().expect("dir");
    let (a, b) = (dir.path().join("a.flare"), dir.path().join("b.flare"));
    let mut ws = workspace();
    assert!(
        !ws.is_enabled(MenuAction::OpenRecent),
        "nothing to open yet"
    );
    ws.save_as(&a);
    ws.save_as(&b);
    ws.save_as(&a);
    assert_eq!(
        ws.recent_files(),
        [a.clone(), b.clone()],
        "newest first, no repeats"
    );
    assert!(ws.is_enabled(MenuAction::OpenRecent));
    // Opening a project makes it the newest too.
    let mut other = workspace();
    other.open(&b);
    assert_eq!(other.recent_files().first(), Some(&b));
    // The list is capped and survives through the preferences.
    for i in 0..30 {
        ws.note_recent(&dir.path().join(format!("p{i}.flare")));
    }
    assert_eq!(ws.recent_files().len(), 10);
    let json = ws.preferences_json().expect("json");
    let mut restored = workspace();
    restored.restore_preferences(&json).expect("restore");
    assert_eq!(restored.recent_files(), ws.recent_files());
    // A file with no list, or blank entries, is fine.
    restored
        .restore_preferences(r#"{"recent": ["", "/x/y.flare"]}"#)
        .expect("restore");
    assert_eq!(
        restored.recent_files(),
        [std::path::PathBuf::from("/x/y.flare")]
    );
    restored.perform(MenuAction::OpenRecent);
    assert!(restored.recent_open);
    let ctx = egui::Context::default();
    run(&mut restored, &ctx, 0.0);
}

#[test]
fn save_all_saves_what_changed_and_publish_settings_are_remembered() {
    let dir = tempfile::tempdir().expect("dir");
    let mut ws = workspace();
    assert!(!ws.is_enabled(MenuAction::SaveAll), "nothing changed");
    rect(&mut ws);
    assert!(ws.is_enabled(MenuAction::SaveAll));
    ws.perform(MenuAction::SaveAll);
    assert_eq!(
        ws.take_dialog(),
        Some(DialogRequest::SaveAs),
        "no file yet, so it asks for one"
    );
    ws.save_as(&dir.path().join("p.flare"));
    rect(&mut ws);
    ws.perform(MenuAction::SaveAll);
    assert!(!ws.is_dirty());
    assert!(ws.publish_compress());
    ws.perform(MenuAction::PublishSettings);
    assert!(ws.publish_settings_open);
    let mut prefs = ws.preferences();
    prefs.publish_compress = false;
    ws.apply_preferences(&prefs);
    assert!(!ws.publish_compress());
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
}
