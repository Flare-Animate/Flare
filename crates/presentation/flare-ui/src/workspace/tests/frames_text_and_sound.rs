use super::*;

#[test]
fn frame_copy_and_paste_refuse_what_cannot_work() {
    let (mut ws, layer) = two_exposures();
    ws.select_layer(layer);
    ws.select_frames(layer, 40, 45); // nothing there
    ws.perform(MenuAction::CopyFrames);
    assert!(ws.status().is_some_and(|m| m.contains("nothing to copy")));
    assert!(
        !ws.is_enabled(MenuAction::PasteFrames),
        "nothing was copied"
    );

    ws.select_frames(layer, 0, 3);
    ws.perform(MenuAction::CopyFrames);
    ws.toggle_layer_locked(layer);
    assert!(
        !ws.is_enabled(MenuAction::PasteFrames),
        "a locked layer is not a target"
    );
    ws.toggle_layer_locked(layer);
    // A copy that would run past the last representable frame is refused by the edit;
    // that case is covered where the paste is computed.
    ws.set_frame(0);
    ws.perform(MenuAction::PasteFrames);
    assert_eq!(layer_spans(&ws, layer), vec![(0, 4), (4, 8), (12, 12)]);
}

#[test]
fn the_frame_selection_goes_away_with_its_layer() {
    let (ctx, mut ws) = (egui::Context::default(), held_then_changing());
    let layer = ws.current_layer.expect("layer");
    ws.select_frames(layer, 2, 5);
    run(&mut ws, &ctx, 0.0);
    assert!(ws.frame_range.is_some());
    ws.select_layer(layer);
    ws.delete_active_layer();
    run(&mut ws, &ctx, 0.1);
    assert!(ws.frame_range.is_none());
}

#[test]
fn placed_text_becomes_a_filled_shape_at_the_clicked_point_in_one_undo_step() {
    let mut ws = text_workspace();
    ws.fill = Color32::from_rgb(10, 20, 30);
    type_text(&mut ws, Pos2::new(100.0, 50.0), "Hi");
    assert_eq!(layer_count(&ws), 2, "a layer for the text");
    let layer = ws.document().scenes()[0].timeline().layers()[1].id();
    ws.selected = Some(SpanRef { layer, start: 0 });
    let shape = selected_shape(&ws);
    assert_eq!(shape.fill(), Rgba8::opaque(10, 20, 30));
    let b = shape_bounds(&ws);
    assert!(
        b.min.x >= 99.0 && b.min.y >= 49.0,
        "starts at the click: {b:?}"
    );
    assert!(
        b.height() > 20.0 && b.height() < 60.0,
        "a 40 px capital: {b:?}"
    );
    ws.undo();
    assert_eq!(layer_count(&ws), 1);
}

#[test]
fn blank_text_and_changing_tools_place_nothing() {
    let mut ws = text_workspace();
    type_text(&mut ws, Pos2::new(10.0, 10.0), "   \n ");
    assert_eq!(layer_count(&ws), 1);
    ws.text_edit = Some(PendingText {
        at: Pos2::new(1.0, 1.0),
        buffer: "abc".into(),
        focused: true,
    });
    ws.set_tool(Tool::Select);
    assert!(
        ws.text_edit.is_none(),
        "switching tools drops the unfinished text"
    );
    assert!(!ws.is_dirty());
}

#[test]
fn a_text_error_is_reported_and_leaves_the_document_alone() {
    let mut ws = text_workspace();
    ws.text_size = 5000.0; // beyond what the engine accepts
    type_text(&mut ws, Pos2::new(10.0, 10.0), "a");
    assert_eq!(layer_count(&ws), 1);
    assert!(
        ws.status().is_some_and(|m| m.contains("Cannot place text")),
        "{:?}",
        ws.status()
    );
}

#[test]
fn with_workers_text_is_placed_when_the_font_work_finishes() {
    let (ctx, mut ws) = (egui::Context::default(), text_workspace());
    ws.start_background_threads(&ctx);
    type_text(&mut ws, Pos2::new(10.0, 10.0), "Hi");
    assert_eq!(
        layer_count(&ws),
        1,
        "not placed until the worker has finished"
    );
    wait_until(&mut ws, &ctx, |w| w.text_pending.is_none());
    assert_eq!(layer_count(&ws), 2, "{:?}", ws.status());
}

#[test]
fn the_text_tool_is_a_menu_action_and_clicking_the_stage_opens_the_editor() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.perform(MenuAction::ToolText);
    assert_eq!(ws.tool(), Tool::Text);
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let center = ws.stage_panel.center();
    let events = vec![
        Event::PointerMoved(center),
        Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        },
        Event::PointerButton {
            pos: center,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        },
    ];
    pass(&mut ws, &ctx, input(0.2, events));
    assert!(
        ws.text_edit.is_some(),
        "a click with the text tool opens the editor"
    );
    run(&mut ws, &ctx, 0.3); // the editor draws without trouble
}

#[test]
fn importing_a_sound_adds_it_and_a_cue_at_the_playhead_in_one_undo_step() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "Clap.wav");
    let mut ws = workspace();
    rect(&mut ws); // a 24-frame scene, so the playhead can be at frame 6
    ws.set_frame(6);
    ws.import_sound(&path);
    assert_eq!(layer_count(&ws), 3);
    let sounds = ws.document().sounds();
    assert_eq!(sounds.len(), 1);
    assert_eq!(
        (
            sounds[0].name(),
            sounds[0].sample_rate(),
            sounds[0].frames()
        ),
        ("Clap", 8000, 8000)
    );
    let layer = &ws.document().scenes()[0].timeline().layers()[2];
    assert_eq!(layer.name(), "Sound: Clap");
    assert_eq!(
        (layer.cues()[0].start, layer.cues()[0].sound),
        (6, sounds[0].id())
    );
    assert!(
        ws.decoded.contains_key(&sounds[0].id()),
        "ready to play at once"
    );
    ws.undo();
    assert!(
        ws.document().sounds().is_empty(),
        "the sound and the layer go together"
    );
    assert_eq!(layer_count(&ws), 2);
}

#[test]
fn sounds_that_cannot_be_imported_are_reported_and_change_nothing() {
    let dir = tempfile::tempdir().expect("dir");
    let text = dir.path().join("notes.wav");
    std::fs::write(&text, b"this is not audio").expect("write");
    let mut ws = workspace();
    ws.import_sound(&text);
    assert!(
        ws.status().is_some_and(|m| m.contains("Cannot import")),
        "{:?}",
        ws.status()
    );
    ws.import_sound(&dir.path().join("missing.wav"));
    assert!(ws.status().is_some_and(|m| m.contains("Cannot import")));
    assert!(ws.document().sounds().is_empty());
    assert!(!ws.is_dirty());
}

#[test]
fn playing_starts_cued_sounds_at_the_right_delay_and_pausing_stops_them() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws); // a 24-frame scene
    ws.set_frame(6);
    ws.import_sound(&path); // cue at frame 6
    ws.set_frame(0);
    run(&mut ws, &ctx, 0.0);
    assert!(drain_audio(&mut ws).is_empty(), "paused: nothing happens");
    ws.playing = true;
    run(&mut ws, &ctx, 0.0);
    assert_eq!(
        drain_audio(&mut ws),
        vec!["stop", "play@0.250"],
        "6 frames at 24 fps"
    );
    run(&mut ws, &ctx, 0.0);
    assert!(
        drain_audio(&mut ws).is_empty(),
        "ordinary playback adds nothing"
    );
    ws.playing = false;
    run(&mut ws, &ctx, 0.0);
    assert_eq!(drain_audio(&mut ws), vec!["stop"]);
}

#[test]
fn looping_restarts_the_sounds_and_hidden_layers_and_past_cues_are_silent() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    ws.set_frame(3);
    ws.import_sound(&path); // cue at 3
    let sound_layer = ws.current_layer.expect("layer");
    ws.set_frame(10); // the playhead is already past the cue
    ws.playing = true;
    run(&mut ws, &ctx, 0.0);
    assert_eq!(
        drain_audio(&mut ws),
        vec!["stop"],
        "a cue behind the playhead does not play"
    );
    ws.frame = 2; // the loop wrapped around
    run(&mut ws, &ctx, 0.0);
    assert_eq!(
        drain_audio(&mut ws),
        vec!["stop", "play@0.042"],
        "and plays again after the wrap"
    );
    ws.toggle_layer_visible(sound_layer);
    ws.frame = 1;
    run(&mut ws, &ctx, 0.0);
    assert_eq!(
        drain_audio(&mut ws),
        vec!["stop"],
        "a hidden layer is silent"
    );
}

#[test]
fn deleting_a_sound_layer_removes_sounds_nothing_else_plays_and_undo_brings_them_back() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let mut ws = workspace();
    ws.import_sound(&path);
    let layer = ws.current_layer.expect("layer");
    // A second layer plays the same sound, so deleting the first must keep it.
    let scene = ws.scene_id().expect("scene");
    let sound = ws.document().sounds()[0].id();
    let second = Layer::new(LayerId::new(), "Again", vec![])
        .expect("layer")
        .with_cues(vec![flare_document::SoundCue { start: 9, sound }])
        .expect("cues");
    ws.run_batch(
        vec![Command::AddLayer {
            scene,
            layer: second,
        }],
        "add",
    );
    ws.select_layer(layer);
    ws.delete_active_layer();
    assert_eq!(
        ws.document().sounds().len(),
        1,
        "still played by the other layer"
    );
    let other = ws.document().scenes()[0].timeline().layers()[1].id();
    ws.select_layer(other);
    ws.delete_active_layer();
    assert!(
        ws.document().sounds().is_empty(),
        "nothing plays it any more"
    );
    ws.undo();
    assert_eq!(ws.document().sounds().len(), 1);
    assert_eq!(layer_count(&ws), 2);
}

#[test]
fn opening_a_project_with_sounds_decodes_them_and_stops_what_was_playing() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let project = dir.path().join("p.flare");
    let (ctx, mut source) = (egui::Context::default(), workspace());
    source.import_sound(&path);
    source.save_as(&project);

    let mut ws = workspace();
    ws.open(&project);
    assert!(
        ws.decoded.is_empty(),
        "decoding is requested by the next pass"
    );
    assert!(matches!(
        ws.take_audio_events().last(),
        Some(AudioEvent::Stop)
    ));
    run(&mut ws, &ctx, 0.0);
    let id = ws.document().sounds()[0].id();
    assert!(ws.decoded.contains_key(&id), "decoded for playback");
    assert!(ws.decoding.is_empty());
}

#[test]
fn with_workers_a_sound_is_imported_when_it_has_been_decoded() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    ws.start_background_threads(&ctx);
    ws.import_sound(&path);
    assert!(
        ws.document().sounds().is_empty(),
        "not until the worker has decoded it"
    );
    wait_until(&mut ws, &ctx, |w| w.importing_sound.is_none());
    assert_eq!(ws.document().sounds().len(), 1, "{:?}", ws.status());
}

#[test]
fn sound_cannot_be_imported_while_editing_inside_a_symbol_and_has_a_menu_action() {
    let mut ws = workspace();
    draw_and_select(&mut ws);
    ws.convert_selection_to_symbol();
    ws.edit_selected_symbol();
    assert!(ws.in_symbol());
    assert!(!ws.is_enabled(MenuAction::ImportSound));
    ws.exit_symbol();
    assert!(ws.is_enabled(MenuAction::ImportSound));
    ws.perform(MenuAction::ImportSound);
    assert_eq!(ws.take_dialog(), Some(DialogRequest::ImportSound));
}

#[test]
fn every_panel_draws_with_every_tool_and_in_the_states_that_change_what_they_show() {
    let (ctx, mut ws) = (egui::Context::default(), two_rectangles());
    run(&mut ws, &ctx, 0.0);
    let mut time = 0.1;
    for tool in [
        Tool::Select,
        Tool::Rectangle,
        Tool::Ellipse,
        Tool::Line,
        Tool::Pen,
        Tool::Text,
    ] {
        ws.set_tool(tool);
        for (selected, gradient, playing, onion) in [
            (false, false, false, false),
            (true, true, false, true),
            (true, false, true, false),
        ] {
            if selected {
                ws.select_at(Pos2::new(30.0, 30.0), false);
            } else {
                ws.select_at(Pos2::new(400.0, 400.0), false);
            }
            ws.gradient_enabled = gradient;
            ws.playing = playing;
            ws.set_onion_skin(onion);
            // A pass draws every dock panel, the menu and the status bar; a missing icon
            // name would panic here in a debug build.
            run(&mut ws, &ctx, time);
            time += 0.1;
        }
    }
}

#[test]
fn the_theme_and_icons_are_installed_once_and_the_theme_is_dark() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    assert!(!ws.chrome_ready);
    run(&mut ws, &ctx, 0.0);
    assert!(ws.chrome_ready);
    assert!(ctx.global_style().visuals.dark_mode);
    assert_eq!(ctx.global_style().visuals.panel_fill, crate::theme::PANEL);
}

#[test]
fn the_pointer_position_over_the_stage_is_tracked_in_stage_units() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    let center = ws.stage_panel.center();
    pass(&mut ws, &ctx, input(0.2, vec![Event::PointerMoved(center)]));
    let at = ws.pointer_stage.expect("the pointer is over the stage");
    let extent = ws.stage_extent();
    assert!(
        at.x > 0.0 && at.x < extent.x && at.y > 0.0 && at.y < extent.y,
        "{at:?}"
    );
    pass(
        &mut ws,
        &ctx,
        input(0.3, vec![Event::PointerMoved(Pos2::new(1.0, 1.0))]),
    );
    assert!(
        ws.pointer_stage.is_none(),
        "off the stage panel there is no position"
    );
}
