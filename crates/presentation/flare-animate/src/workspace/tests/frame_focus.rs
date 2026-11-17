//! After a click among the timeline's frames, Copy, Paste and Delete act on frames; a selected run
//! of frames drags as one, and Alt copies what is dragged.

use super::layer_names::{click, settle};
use super::*;
use egui::PointerButton;

/// The middle of the cell for frame `frame` (zero based) on row `row`, where it was last drawn.
fn cell(ws: &Workspace, row: usize, frame: u32) -> Pos2 {
    let l = ws.timeline_layout.expect("the timeline was drawn");
    Pos2::new(l.frame_x(frame as f32 + 0.5), l.row_top(row) + 10.0)
}

/// A drawn keyframe on frames 0 and 1, empty frames 2 to 4, and a keyframe on frame 5, drawn
/// once so the timeline has its cells.
fn with_a_gap() -> (egui::Context, Workspace, f64) {
    let (ctx, mut ws) = (egui::Context::default(), blank_workspace());
    flare_ui::icons::install(&ctx);
    rect(&mut ws);
    let (scene, layer) = (
        ws.scene_id().expect("scene"),
        ws.active_layer_id().expect("layer"),
    );
    let held = active_layer(&ws).spans()[0].retimed(0, NonZeroU32::new(2).expect("2"));
    let shape = Shape::rect(ShapeId::new(), 0.0, 0.0, 9.0, 9.0, Rgba8::WHITE).expect("rect");
    let late = FrameSpan::new(5, NonZeroU32::MIN, Content::Shape(shape));
    ws.active_mut()
        .apply([
            Command::RemoveSpan {
                scene,
                layer,
                start: 0,
            },
            Command::InsertSpan {
                scene,
                layer,
                span: held,
            },
            Command::InsertSpan {
                scene,
                layer,
                span: late,
            },
        ])
        .expect("spans");
    let mut time = 0.0;
    settle(&mut ws, &ctx, &mut time);
    (ctx, ws, time)
}

fn press_key(ws: &mut Workspace, ctx: &egui::Context, time: &mut f64, event: Event) {
    *time += 0.1;
    pass(ws, ctx, input(*time, vec![event]));
}

#[test]
fn delete_on_an_empty_frame_between_keyframes_closes_the_gap() {
    let (ctx, mut ws, mut time) = with_a_gap();
    let at = cell(&ws, 0, 3);
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    assert!(ws.frame_focus.on_frames, "the click was among the frames");
    press_key(&mut ws, &ctx, &mut time, key(Key::Delete, Modifiers::NONE));
    assert_eq!(
        active_spans(&ws),
        vec![(0, 2), (4, 1)],
        "one empty frame went"
    );

    // Shift-click selects the two left; Delete takes both.
    ws.set_frame(2);
    ws.select_frames(ws.active_layer_id().expect("layer"), 2, 3);
    press_key(
        &mut ws,
        &ctx,
        &mut time,
        key(Key::Backspace, Modifiers::NONE),
    );
    assert_eq!(active_spans(&ws), vec![(0, 2), (2, 1)], "the gap is closed");
    ws.undo();
    assert_eq!(active_spans(&ws), vec![(0, 2), (4, 1)], "one undo step");

    // On a frame that shows a drawing Delete is the stage's, which has nothing selected.
    ws.set_frame(0);
    ws.frame_range = None;
    press_key(&mut ws, &ctx, &mut time, key(Key::Delete, Modifiers::NONE));
    assert_eq!(active_spans(&ws), vec![(0, 2), (4, 1)], "nothing changed");
}

#[test]
fn copy_and_paste_after_a_click_in_the_timeline_copy_frames_and_a_stage_click_takes_it_back() {
    let (ctx, mut ws, mut time) = with_a_gap();
    let layer = ws.active_layer_id().expect("layer");
    let at = cell(&ws, 0, 1);
    click(&mut ws, &ctx, &mut time, at, PointerButton::Primary);
    assert!(ws.frame_focus.on_frames);
    // As a Shift-click on frame 0 would select them.
    ws.select_frames(layer, 0, 1);
    press_key(&mut ws, &ctx, &mut time, Event::Copy);
    assert_eq!(ws.frame_clipboard.as_ref().map(|c| c.length()), Some(2));
    ws.set_frame(3);
    press_key(&mut ws, &ctx, &mut time, Event::Paste(String::new()));
    assert_eq!(
        active_spans(&ws),
        vec![(0, 2), (3, 2), (5, 1)],
        "pasted at frame 3"
    );

    // A click on the stage points Paste back at what is copied there.
    settle(&mut ws, &ctx, &mut time);
    let stage = ws.stage_panel.center();
    click(&mut ws, &ctx, &mut time, stage, PointerButton::Primary);
    assert!(!ws.frame_focus.on_frames, "the stage has it now");
    let before = active_spans(&ws);
    press_key(&mut ws, &ctx, &mut time, Event::Paste(String::new()));
    assert_eq!(active_spans(&ws), before, "no frames were pasted");
}

#[test]
fn dragging_inside_selected_frames_moves_them_all_and_alt_copies_them() {
    let (_ctx, mut ws, _time) = with_a_gap();
    let layer = ws.active_layer_id().expect("layer");
    ws.select_frames(layer, 0, 5);
    assert!(ws.begin_frames_drag(layer, 3.5), "inside the selection");
    ws.update_span_drag(6, None);
    assert_eq!(ws.span_drag_preview(), Some((layer, 3, 6)));
    ws.end_span_drag();
    assert_eq!(
        active_spans(&ws),
        vec![(3, 2), (8, 1)],
        "both keyframes and the gap moved"
    );
    let range = ws.frame_range.expect("still selected");
    assert_eq!((range.from, range.to), (3, 8), "selected where they landed");

    // Alt held as it is let go leaves the original where it was.
    assert!(ws.begin_frames_drag(layer, 3.5));
    ws.update_span_drag(10, None);
    ws.set_span_drag_copy(true);
    ws.end_span_drag();
    assert_eq!(active_spans(&ws), vec![(3, 2), (8, 1), (10, 2), (15, 1)]);
    assert!(!ws.begin_frames_drag(layer, 30.5), "outside the selection");
}

#[test]
fn alt_dragging_a_keyframe_copies_it() {
    let (_ctx, mut ws, _time) = with_a_gap();
    let layer = ws.active_layer_id().expect("layer");
    assert!(ws.begin_span_drag(layer, 5.5, 0.1));
    ws.update_span_drag(7, None);
    ws.set_span_drag_copy(true);
    ws.end_span_drag();
    assert_eq!(active_spans(&ws), vec![(0, 2), (5, 1), (7, 1)]);
}
