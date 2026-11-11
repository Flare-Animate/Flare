//! Scrubbing the timeline: the playhead may rest in the empty cells past the scene's end,
//! where Animate lets you place a first keyframe; playback still stops at the scene itself.

use super::*;

#[test]
fn scrubbing_rests_in_empty_cells_while_set_frame_clamps_to_the_scene() {
    // A new document: a layer with no spans, so the scene itself is one frame.
    let mut ws = blank_workspace();
    assert_eq!(ws.frame(), 0);
    ws.set_frame_in_view(5, 20);
    assert_eq!(ws.frame(), 5, "the playhead rests in the shown empty cells");
    ws.set_frame_in_view(50, 20);
    assert_eq!(ws.frame(), 19, "a scrub is clamped to the cells the panel draws");
    ws.set_frame(9);
    assert_eq!(ws.frame(), 0, "set_frame still clamps to the scene itself");
}

#[test]
fn a_keyframe_at_a_scrubbed_playhead_holds_the_layer_out_to_it() {
    let mut ws = workspace(); // one 24-frame Empty span
    // The panel shows more cells than the scene holds; scrubbing rests at 30.
    ws.set_frame_in_view(30, 40);
    assert_eq!(ws.frame(), 30);
    ws.frame_edit(FrameOp::InsertKeyframe);
    assert_eq!(
        active_spans(&ws),
        vec![(0, 30), (30, 31)],
        "inserting at the playhead extends the layer to the playhead"
    );
    assert_eq!(ws.scene_frames(), 31, "the scene grew to hold the keyframe");
}
