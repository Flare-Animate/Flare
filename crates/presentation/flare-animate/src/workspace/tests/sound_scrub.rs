//! Working to a sound: its waveform is ready to draw once imported, moving the paused playhead
//! plays the stretch under it, and Cmd with + and - zooms the stage, not the window.

use super::*;

/// What the sound card was asked for, with the excerpt of each play.
fn scrub_events(ws: &mut Workspace) -> Vec<String> {
    ws.take_audio_events()
        .into_iter()
        .map(|e| match e {
            AudioEvent::Stop => "stop".to_owned(),
            AudioEvent::Play {
                delay_seconds,
                excerpt,
                ..
            } => format!(
                "play@{delay_seconds:.3} from {:.3} for {:.3}",
                excerpt.offset_seconds,
                excerpt.length_seconds.unwrap_or(f64::INFINITY)
            ),
        })
        .collect()
}

/// The layer an imported sound was put on: the one that cues it.
pub(super) fn sound_layer(ws: &Workspace) -> flare_core::LayerId {
    ws.document().scenes()[0]
        .timeline()
        .cues()
        .next()
        .map(|(layer, _)| layer.id())
        .expect("a layer cues the sound")
}

/// A 24-frame scene with the one-second test sound cued at frame 4, settled and paused.
fn cued(dir: &std::path::Path) -> (egui::Context, Workspace, flare_core::LayerId) {
    let path = wav_file(dir, "a.wav");
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    rect(&mut ws);
    ws.set_frame(4);
    ws.import_sound(&path);
    let layer = sound_layer(&ws);
    ws.set_frame(0);
    run(&mut ws, &ctx, 0.0);
    ws.take_audio_events();
    (ctx, ws, layer)
}

#[test]
fn an_imported_sound_has_its_waveform_ready_for_the_timeline() {
    let dir = tempfile::tempdir().expect("dir");
    let (_, ws, _) = cued(dir.path());
    let id = ws.document().sounds()[0].id();
    let peaks = ws.sound_peaks(id).expect("peaks once decoded");
    // The test sound is a sawtooth of about 0.46 either way, for one second.
    assert_eq!(peaks.len(), flare_audio::PEAKS_PER_SECOND as usize);
    let [low, high] = peaks.between(0.2, 0.3).expect("inside");
    assert!(low < -0.4 && high > 0.4, "{low} {high}");
    assert!(peaks.between(1.5, 2.0).is_none(), "past its end");
}

#[test]
fn moving_the_paused_playhead_plays_the_frame_of_sound_under_it() {
    let dir = tempfile::tempdir().expect("dir");
    let (ctx, mut ws, _) = cued(dir.path());
    ws.set_frame_in_view(2, 40);
    run(&mut ws, &ctx, 0.1);
    assert!(scrub_events(&mut ws).is_empty(), "before the cue: silent");
    ws.set_frame_in_view(10, 40);
    run(&mut ws, &ctx, 0.2);
    assert_eq!(
        scrub_events(&mut ws),
        vec!["stop", "play@0.000 from 0.250 for 0.050"],
        "six frames in at 24 fps, for one frame (but no less than 50 ms)"
    );
    run(&mut ws, &ctx, 0.3);
    assert!(
        scrub_events(&mut ws).is_empty(),
        "holding still adds nothing"
    );
    ws.set_frame_in_view(7, 40);
    run(&mut ws, &ctx, 0.4);
    assert_eq!(
        scrub_events(&mut ws),
        vec!["stop", "play@0.000 from 0.125 for 0.050"],
        "dragging back plays from there, the last slice stopped"
    );
    // Frame 4 + 24 is the sound's end: off it, what sounded stops, then nothing more.
    ws.set_frame_in_view(30, 40);
    run(&mut ws, &ctx, 0.5);
    assert_eq!(scrub_events(&mut ws), vec!["stop"]);
    ws.set_frame_in_view(31, 40);
    run(&mut ws, &ctx, 0.6);
    assert!(scrub_events(&mut ws).is_empty());
}

#[test]
fn a_scrub_is_silent_on_a_hidden_layer_and_when_muted() {
    let dir = tempfile::tempdir().expect("dir");
    let (ctx, mut ws, layer) = cued(dir.path());
    ws.toggle_layer_visible(layer);
    ws.set_frame_in_view(8, 40);
    run(&mut ws, &ctx, 0.1);
    assert!(scrub_events(&mut ws).is_empty(), "hidden");
    ws.toggle_layer_visible(layer);
    ws.muted = true;
    ws.set_frame_in_view(9, 40);
    run(&mut ws, &ctx, 0.2);
    assert!(scrub_events(&mut ws).is_empty(), "muted");
    ws.muted = false;
    ws.set_frame_in_view(10, 40);
    run(&mut ws, &ctx, 0.3);
    assert_eq!(scrub_events(&mut ws).len(), 2, "heard again");
}

#[test]
fn playing_after_a_scrub_starts_the_sounds_as_before() {
    let dir = tempfile::tempdir().expect("dir");
    let (ctx, mut ws, _) = cued(dir.path());
    ws.set_frame_in_view(5, 40);
    run(&mut ws, &ctx, 0.1);
    assert_eq!(scrub_events(&mut ws).len(), 2);
    ws.set_frame(0);
    ws.playing = true;
    run(&mut ws, &ctx, 0.2);
    let events = scrub_events(&mut ws);
    assert_eq!(events[0], "stop", "the scrub slice is stopped");
    // Playback moved on by the time that passed; the cue is still ahead of it.
    let ahead = f64::from(4 - ws.frame()) / 24.0;
    assert_eq!(
        events.last().cloned(),
        Some(format!("play@{ahead:.3} from 0.000 for inf")),
        "and the whole sound starts at its cue: {events:?}"
    );
}

fn stage_scale(ws: &Workspace) -> f32 {
    ws.stage_view.scale(ws.stage_panel, ws.stage_extent())
}

/// A workspace whose panels have settled: the stage's size moves over the first passes.
fn settled() -> (egui::Context, Workspace) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    for time in [0.0, 0.01, 0.02] {
        run(&mut ws, &ctx, time);
    }
    (ctx, ws)
}

#[test]
fn cmd_plus_and_minus_zoom_the_stage_and_not_the_window() {
    let (ctx, mut ws) = settled();
    let (start, window) = (stage_scale(&ws), ctx.zoom_factor());
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::Equals, Modifiers::COMMAND)]),
    );
    let zoomed_in = stage_scale(&ws);
    assert!(zoomed_in > start, "{zoomed_in} after {start}");
    pass(
        &mut ws,
        &ctx,
        input(
            0.2,
            vec![key(Key::Plus, Modifiers::COMMAND | Modifiers::SHIFT)],
        ),
    );
    assert!(stage_scale(&ws) > zoomed_in, "Cmd and + (with Shift) too");
    for time in [0.3, 0.4, 0.5] {
        pass(
            &mut ws,
            &ctx,
            input(time, vec![key(Key::Minus, Modifiers::COMMAND)]),
        );
    }
    assert!(stage_scale(&ws) < start, "zoomed out past where it began");
    run(&mut ws, &ctx, 0.9);
    assert_eq!(ctx.zoom_factor(), window, "the interface kept its size");
    assert!(!ctx.options(|o| o.zoom_with_keyboard));
}

#[test]
fn the_zoom_keys_are_the_menu_shortcuts_and_follow_a_rebinding() {
    assert_eq!(MenuAction::ZoomIn.shortcut(), Some("CmdOrCtrl+="));
    assert_eq!(MenuAction::ZoomOut.shortcut(), Some("CmdOrCtrl+-"));
    let (ctx, mut ws) = settled();
    ws.set_shortcut(MenuAction::ZoomIn, "CmdOrCtrl+Alt+=")
        .expect("free chord");
    let start = stage_scale(&ws);
    pass(
        &mut ws,
        &ctx,
        input(
            0.1,
            vec![
                key(Key::Equals, Modifiers::COMMAND),
                key(Key::Plus, Modifiers::COMMAND),
            ],
        ),
    );
    assert_eq!(stage_scale(&ws), start, "the old keys stop zooming");
    pass(
        &mut ws,
        &ctx,
        input(
            0.2,
            vec![key(Key::Equals, Modifiers::COMMAND | Modifiers::ALT)],
        ),
    );
    assert!(stage_scale(&ws) > start);
}
