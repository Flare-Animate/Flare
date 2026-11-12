//! Animating to a sound: draw, import the sound, click a frame ahead and press F5 to hold the
//! drawing out to it; the whole sound can be scrolled to however long the scene is.

use super::sound_scrub::sound_layer;
use super::*;

#[test]
fn after_importing_a_sound_f5_ahead_holds_the_drawing_out_to_the_playhead() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let mut ws = blank_workspace();
    rect(&mut ws);
    let drawn = ws.active_layer_id().expect("the layer drawn on");
    ws.import_sound(&path);
    assert_eq!(
        ws.active_layer_id(),
        Some(drawn),
        "the drawing layer is still the one worked on"
    );
    ws.set_frame_in_view(24, 60); // a click on frame 25, past the one-frame scene
    ws.frame_edit(FrameOp::InsertFrame);
    assert_eq!(ws.status(), None);
    assert_eq!(active_spans(&ws), vec![(0, 25)], "held out to the playhead");
    assert_eq!(ws.scene_frames(), 25);
    assert_eq!(ws.frame(), 24, "the playhead stays where it was put");
    assert_eq!(drawing_count(&ws), 1);
    ws.undo();
    assert_eq!(active_spans(&ws), vec![(0, 1)], "one undo step");
}

#[test]
fn f5_on_the_sound_layer_gives_it_frames_to_the_playhead_and_leaves_the_sound_in_place() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav");
    let mut ws = blank_workspace();
    ws.set_frame_in_view(3, 60);
    ws.import_sound(&path); // cued at frame 3
    let sound = sound_layer(&ws);
    ws.select_layer(sound);
    ws.set_frame_in_view(30, 60);
    ws.frame_edit(FrameOp::InsertFrame);
    assert_eq!(ws.status(), None, "no 'cannot insert a frame' any more");
    assert_eq!(active_spans(&ws), vec![(0, 31)]);
    assert_eq!(ws.scene_frames(), 31, "the scene now runs to frame 31");
    let cues: Vec<u32> = ws
        .layer(sound)
        .expect("layer")
        .cues()
        .iter()
        .map(|c| c.start)
        .collect();
    assert_eq!(cues, vec![3]);
}

#[test]
fn the_timeline_reaches_the_end_of_a_sound_longer_than_the_scene() {
    let dir = tempfile::tempdir().expect("dir");
    let path = wav_file(dir.path(), "a.wav"); // one second: 24 frames at 24 fps
    let mut ws = blank_workspace();
    // A panel 100 points wide at 10 points a frame shows 11 cells of the one-frame scene.
    assert_eq!(ws.timeline_shown(100.0, 10.0), 11);
    ws.set_frame_in_view(10, 11);
    ws.import_sound(&path); // cued at frame 10, so it plays to frame 34
    assert_eq!(
        ws.timeline_shown(100.0, 10.0),
        34,
        "cells go on to the sound's end"
    );
    ws.set_frame_in_view(33, ws.timeline_shown(100.0, 10.0));
    assert_eq!(
        ws.frame(),
        33,
        "and the playhead can be put on its last frame"
    );
    assert_eq!(
        ws.timeline_shown(4000.0, 10.0),
        401,
        "a wide panel still fills to its edge"
    );
}
