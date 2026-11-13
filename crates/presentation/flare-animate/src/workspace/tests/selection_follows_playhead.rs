//! The selection holds only what is on the stage at the playhead, so a transform never turns a
//! keyframe left behind.

use super::*;
use flare_editor::loop_range::LoopRange;

/// A 100 x 50 rectangle at (10, 10) on frame 0, selected, and the keyframe at frame 8 that
/// Insert Keyframe copies it into, so the two lie exactly on top of each other.
fn copied_into_frame_8() -> (Workspace, LayerId) {
    let mut ws = wide_rectangle_selected();
    let layer = ws.current_layer.expect("layer");
    ws.selected = None;
    ws.set_frame_in_view(8, 24);
    ws.perform(MenuAction::InsertKeyframe);
    ws.set_frame(0);
    ws.select_at(Pos2::new(60.0, 35.0), false);
    assert_eq!(ws.selected.map(|s| s.start), Some(0), "frame 0's rectangle");
    (ws, layer)
}

/// The bounds of the rectangle in the keyframe starting at `start`.
fn bounds_at(ws: &Workspace, layer: LayerId, start: u32) -> Rect {
    let scene = ws.scene_id().expect("scene");
    let span = SpanRef {
        layer,
        start,
        element: 0,
    };
    ops::span_bounds(ws.document(), scene, span).expect("bounds")
}

#[test]
fn moving_to_another_keyframe_drops_the_one_left_behind() {
    let (mut ws, _) = copied_into_frame_8();
    ws.set_frame(8);
    assert_eq!(ws.selected, None);
    assert!(ws.selection_rects().is_empty(), "no handles for frame 0");
}

#[test]
fn a_rotation_after_moving_on_turns_nothing_left_behind() {
    let (mut ws, layer) = copied_into_frame_8();
    ws.set_tool(Tool::FreeTransform);
    ws.set_frame(8);
    let behind = bounds_at(&ws, layer, 0);
    // Just outside the bottom-right corner, where the old frame's rotate handle was.
    ws.begin_drag(Pos2::new(120.0, 70.0));
    ws.update_drag(Pos2::new(25.0, 95.0));
    ws.end_drag();
    assert_eq!(bounds_at(&ws, layer, 0), behind, "frame 0 is not turned");
}

#[test]
fn a_rotation_after_reselecting_turns_the_playhead_s_keyframe() {
    let (mut ws, layer) = copied_into_frame_8();
    ws.set_tool(Tool::FreeTransform);
    ws.set_frame(8);
    ws.select_at(Pos2::new(60.0, 35.0), false);
    assert_eq!(ws.selected.map(|s| s.start), Some(8));
    let behind = bounds_at(&ws, layer, 0);
    ws.begin_drag(Pos2::new(120.0, 70.0));
    ws.update_drag(Pos2::new(25.0, 95.0)); // a quarter turn
    ws.end_drag();
    let turned = bounds_at(&ws, layer, 8);
    assert!(
        (turned.width() - 50.0).abs() < 0.01 && (turned.height() - 100.0).abs() < 0.01,
        "{turned:?}"
    );
    assert_eq!(bounds_at(&ws, layer, 0), behind);
}

#[test]
fn moving_within_the_same_keyframe_keeps_the_selection() {
    let (mut ws, _) = copied_into_frame_8();
    ws.set_frame(5);
    assert_eq!(ws.selected.map(|s| s.start), Some(0));
}

#[test]
fn the_ui_pass_drops_it_when_the_playhead_moved_by_itself() {
    let ctx = egui::Context::default();
    let (mut ws, _) = copied_into_frame_8();
    run(&mut ws, &ctx, 0.0);
    // Playback and other paths move the playhead without the setter.
    ws.frame = 8;
    run(&mut ws, &ctx, 0.01);
    assert_eq!(ws.selected, None);
}

#[test]
fn editing_multiple_frames_keeps_the_keyframes_in_the_bracket() {
    let (mut ws, _) = copied_into_frame_8();
    ws.set_edit_multiple_frames(true);
    ws.set_frame(8);
    assert_eq!(
        ws.selected.map(|s| s.start),
        Some(0),
        "frame 0 is still shown"
    );
    // Narrowing the bracket past it takes it off the stage, and out of the selection.
    ws.set_multi_frame_range(Some(LoopRange::new(8, 12)));
    ws.refresh_selection();
    assert_eq!(ws.selected, None);
}
