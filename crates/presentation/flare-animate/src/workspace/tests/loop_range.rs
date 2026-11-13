//! The loop range: a bar on the ruler, while Loop playback is on, over the frames that play
//! round and round; dragged by its ends, cleared by a double click.

use super::layer_names::{press, settle};
use super::*;
use egui::PointerButton;
use flare_editor::loop_range::LoopRange;

/// A 24-frame scene drawn once, with Loop playback on (the default).
fn drawn() -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws); // 24 frames
    assert!(ws.looping.on, "Loop playback is on by default");
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time)
}

/// Presses at `from`, drags through to `to` and lets go.
fn drag(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, from: Pos2, to: Pos2) {
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        *time += 0.05;
        pass(ws, ctx, input(*time, events));
    };
    step(ws, vec![Event::PointerMoved(from)]);
    step(ws, vec![press(from, PointerButton::Primary, true)]);
    for t in [0.25, 0.5, 1.0] {
        step(ws, vec![Event::PointerMoved(from + (to - from) * t)]);
    }
    step(ws, vec![press(to, PointerButton::Primary, false)]);
}

#[test]
fn dragging_the_end_of_the_bar_sets_the_range_and_leaves_the_playhead() {
    let (ctx, mut ws, mut time) = drawn();
    ws.set_frame(3);
    settle(&mut ws, &ctx, &mut time);
    assert_eq!(ws.loop_range(), None, "the whole scene to begin with");
    let l = ws.timeline_layout.expect("the timeline was drawn");
    let (_, end) = l.loop_bar(LoopRange::new(0, 23));
    let y = l.ruler_top + 3.0;
    // The end handle sits on the right edge of the last frame looped: frame 8's is frame 9's left.
    drag(
        &mut ws,
        &ctx,
        &mut time,
        Pos2::new(end, y),
        Pos2::new(l.frame_x(9.0), y),
    );
    assert_eq!(ws.loop_range(), Some(LoopRange::new(0, 8)));
    assert_eq!(ws.frame(), 3, "the bar moved, not the playhead");
    // Then the start, to frame 2.
    let l = ws.timeline_layout.expect("drawn");
    drag(
        &mut ws,
        &ctx,
        &mut time,
        // Right of the handle: its own x is the edge of the layer names, which drags to widen
        // the names within 3 points of it.
        Pos2::new(l.frame_x(0.0) + 4.5, y),
        Pos2::new(l.frame_x(2.0), y),
    );
    assert_eq!(ws.loop_range(), Some(LoopRange::new(2, 8)));
}

#[test]
fn playing_goes_round_the_range_and_brings_the_playhead_into_it() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.set_loop_range(Some(LoopRange::new(4, 6)));
    ws.set_frame(20);
    ws.playing = true;
    let rate = ws.document().frame_rate().per_second();
    let mut seen = Vec::new();
    ws.advance(0.0);
    // Half a frame in hand, so float rounding never makes a step of none or two.
    for i in 1..=6 {
        ws.advance((f64::from(i) + 0.5) / rate);
        seen.push(ws.frame());
    }
    assert_eq!(seen, vec![4, 5, 6, 4, 5, 6]);
    assert!(ws.playing);
    // With Loop off the range does nothing: playback runs to the end and stops.
    ws.looping.on = false;
    ws.set_frame(22);
    ws.advance(7.5 / rate);
    ws.advance(9.5 / rate);
    assert_eq!(ws.frame(), 23);
    assert!(!ws.playing);
}

#[test]
fn a_double_click_on_the_bar_loops_the_whole_scene_again() {
    let (ctx, mut ws, mut time) = drawn();
    ws.set_loop_range(Some(LoopRange::new(2, 5)));
    settle(&mut ws, &ctx, &mut time);
    let l = ws.timeline_layout.expect("drawn");
    let at = Pos2::new(l.frame_x(4.0), l.ruler_top + 3.0);
    let mut step = |ws: &mut Workspace, events: Vec<Event>| {
        time += 0.02;
        pass(ws, &ctx, input(time, events));
    };
    step(&mut ws, vec![Event::PointerMoved(at)]);
    for _ in 0..2 {
        step(&mut ws, vec![press(at, PointerButton::Primary, true)]);
        step(&mut ws, vec![press(at, PointerButton::Primary, false)]);
    }
    assert_eq!(ws.loop_range(), None);
}

#[test]
fn with_loop_off_the_bar_is_not_there_to_grab() {
    let (ctx, mut ws, mut time) = drawn();
    ws.looping.on = false;
    ws.set_frame(3);
    settle(&mut ws, &ctx, &mut time);
    let l = ws.timeline_layout.expect("drawn");
    let (_, end) = l.loop_bar(LoopRange::new(0, 23));
    let y = l.ruler_top + 3.0;
    drag(
        &mut ws,
        &ctx,
        &mut time,
        Pos2::new(end, y),
        Pos2::new(l.frame_x(9.0), y),
    );
    assert_eq!(ws.loop_range(), None);
}

#[test]
fn a_range_past_the_end_of_a_shorter_scene_is_cut_to_it() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.set_loop_range(Some(LoopRange::new(20, 40)));
    assert_eq!(ws.loop_range(), Some(LoopRange::new(20, 23)));
    ws.set_loop_range(Some(LoopRange::new(30, 40)));
    assert_eq!(ws.loop_range(), None, "nothing of it is left");
}
