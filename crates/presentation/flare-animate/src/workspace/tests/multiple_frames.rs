//! Edit Multiple Frames: a bracket on the ruler over a range of frames, every keyframe in it on
//! the stage, picked and transformed together in one undo step.

use super::layer_names::{press, settle};
use super::*;
use egui::PointerButton;
use flare_editor::loop_range::LoopRange;
use glamx::{DAffine2, DVec2};

/// Keyframes at frames 0, 8 and 16 of the first layer, a rectangle further right in each.
fn walk() -> Workspace {
    let mut ws = workspace();
    rect(&mut ws);
    for (frame, x) in [(8, 200.0), (16, 400.0)] {
        ws.set_frame(frame);
        ws.perform(MenuAction::InsertBlankKeyframe);
        ws.commit_rectangle(Pos2::new(x, 10.0), Pos2::new(x + 100.0, 60.0));
    }
    ws.set_frame(0);
    ws
}

fn lefts(ws: &Workspace) -> Vec<f32> {
    ws.document().scenes()[0].timeline().layers()[0]
        .spans()
        .iter()
        .filter_map(|s| match s.content() {
            Content::Shape(shape) => ops::shape_bounds(shape).map(|b| b.min.x),
            _ => None,
        })
        .collect()
}

#[test]
fn the_toggle_names_itself_and_turns_the_ghosts_into_the_range_s_keyframes() {
    let mut ws = walk();
    assert_eq!(
        ws.label(MenuAction::ToggleEditMultipleFrames),
        "Edit Multiple Frames"
    );
    assert_eq!(ws.effective_onion().multi, None);
    ws.perform(MenuAction::ToggleEditMultipleFrames);
    assert!(ws.editing_multiple_frames());
    assert_eq!(
        ws.label(MenuAction::ToggleEditMultipleFrames),
        "Stop Editing Multiple Frames"
    );
    let whole = LoopRange::new(0, ws.scene_frames() - 1);
    assert_eq!(ws.multi_frame_range(), Some(whole));
    let onion = ws.effective_onion();
    assert_eq!(onion.multi, Some(whole));
    assert_eq!(onion.tints(), None, "keyframes in their own colors");
    assert_eq!(ws.multi_frames_shown(), vec![0, 8, 16]);
    ws.set_multi_frame_range(Some(LoopRange::new(5, 12)));
    assert_eq!(ws.multi_frames_shown(), vec![5, 8]);
    // Playing shows nothing extra.
    ws.playing = true;
    assert_eq!(ws.effective_onion().multi, None);
}

#[test]
fn select_all_and_a_transform_take_every_keyframe_in_one_undo_step() {
    let mut ws = walk();
    ws.select_all();
    assert_eq!(ws.selection_all().len(), 1, "one frame without it");
    ws.perform(MenuAction::ToggleEditMultipleFrames);
    ws.select_all();
    let mut starts: Vec<u32> = ws.selection_all().iter().map(|s| s.start).collect();
    starts.sort_unstable();
    assert_eq!(starts, vec![0, 8, 16]);
    let before = lefts(&ws);
    ws.transform_selection(DAffine2::from_translation(DVec2::new(50.0, 0.0)));
    let after = lefts(&ws);
    for (a, b) in before.iter().zip(&after) {
        assert!((b - a - 50.0).abs() < 1e-3, "{before:?} -> {after:?}");
    }
    ws.undo();
    assert_eq!(lefts(&ws), before, "one undo step");
}

#[test]
fn a_click_picks_a_shape_of_another_keyframe_in_the_range() {
    let mut ws = walk();
    let on_frame_8 = Pos2::new(250.0, 35.0);
    ws.select_at(on_frame_8, false);
    assert!(ws.selected.is_none(), "not on the playhead's frame");
    ws.set_edit_multiple_frames(true);
    ws.select_at(on_frame_8, false);
    assert_eq!(ws.selected.map(|s| s.start), Some(8));
    // Outside the bracket it is not there to pick.
    ws.set_multi_frame_range(Some(LoopRange::new(0, 4)));
    ws.select_at(on_frame_8, false);
    assert!(ws.selected.is_none());
}

#[test]
fn the_bracket_is_dragged_on_the_ruler_and_a_double_click_takes_the_whole_scene() {
    let (ctx, mut ws) = (egui::Context::default(), walk());
    flare_ui::icons::install(&ctx);
    ws.set_edit_multiple_frames(true);
    ws.set_onion_skin(true);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let l = ws.timeline_layout.expect("drawn");
    let (_, end) = l.loop_bar(LoopRange::new(0, ws.scene_frames() - 1));
    let y = l.ruler_top + 16.0;
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        time += 0.05;
        pass(ws, &ctx, input(time, events));
    };
    let (from, to) = (Pos2::new(end, y), Pos2::new(l.frame_x(10.0), y));
    step(&mut ws, vec![Event::PointerMoved(from)]);
    step(&mut ws, vec![press(from, PointerButton::Primary, true)]);
    for t in [0.3, 0.7, 1.0] {
        step(&mut ws, vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    step(&mut ws, vec![press(to, PointerButton::Primary, false)]);
    assert_eq!(ws.multi_frame_range(), Some(LoopRange::new(0, 9)));
    assert_eq!(
        (ws.onion_skin().before, ws.onion_skin().after),
        (1, 1),
        "onion skin's markers rest meanwhile"
    );
    let at = Pos2::new(l.frame_x(4.0), y);
    step(&mut ws, vec![Event::PointerMoved(at)]);
    for _ in 0..2 {
        step(&mut ws, vec![press(at, PointerButton::Primary, true)]);
        step(&mut ws, vec![press(at, PointerButton::Primary, false)]);
    }
    assert_eq!(
        ws.multi_frame_range(),
        Some(LoopRange::new(0, ws.scene_frames() - 1))
    );
}

#[test]
fn turning_it_on_draws_the_stage_again_with_the_other_keyframes() {
    let (ctx, mut ws) = (egui::Context::default(), walk());
    flare_ui::icons::install(&ctx);
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    let drawn = ws.frames_rendered;
    ws.perform(MenuAction::ToggleEditMultipleFrames);
    settle(&mut ws, &ctx, &mut time);
    assert!(
        ws.frames_rendered > drawn,
        "the other keyframes are drawn in"
    );
    ws.perform(MenuAction::ToggleEditMultipleFrames);
    assert!(!ws.editing_multiple_frames());
}
