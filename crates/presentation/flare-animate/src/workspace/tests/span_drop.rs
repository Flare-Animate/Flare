//! A span dragged in the timeline and dropped over other frames, on its row or another.

use super::*;

#[test]
fn a_span_dropped_over_another_replaces_it_as_one_undo_step() {
    let (mut ws, id) = spans_workspace();
    assert!(ws.begin_span_drag(id, 0.5, 0.3));
    ws.update_span_drag(5, None); // 5..8 covers the span at 6
    ws.end_span_drag();
    assert_eq!(active_spans_of(&ws, id), vec![(5, 3)]);
    ws.undo();
    assert_eq!(active_spans_of(&ws, id), vec![(0, 3), (6, 2)]);
}

#[test]
fn a_span_dragged_onto_another_row_moves_to_that_layer() {
    let (mut ws, id) = spans_workspace();
    ws.perform(MenuAction::NewLayer);
    let other = ws.active_layer_id().expect("the new layer");
    assert_ne!(other, id);
    assert!(ws.begin_span_drag(id, 0.5, 0.3));
    ws.update_span_drag(1, Some(other));
    assert_eq!(
        ws.span_drag_preview(),
        Some((other, 1, 3)),
        "shown on its row"
    );
    ws.end_span_drag();
    assert_eq!(active_spans_of(&ws, id), vec![(6, 2)]);
    assert_eq!(active_spans_of(&ws, other), vec![(1, 3)]);
    assert_eq!(
        ws.active_layer_id(),
        Some(other),
        "the layer it went to is active"
    );

    ws.toggle_layer_locked(id);
    assert!(ws.begin_span_drag(other, 1.5, 0.3));
    ws.update_span_drag(6, Some(id));
    ws.end_span_drag();
    assert_eq!(
        active_spans_of(&ws, other),
        vec![(1, 3)],
        "a locked layer takes none"
    );
    assert!(ws.status().is_some());
}
