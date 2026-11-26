//! Dragging the edge of merged drawing with the Selection tool bends it: the stage shows the
//! bend while it is held, the release is one undo step, a drawing object is picked instead, and
//! a cancelled drag changes nothing.

use super::*;

fn merged_square(ws: &mut Workspace) {
    ws.paint.object_drawing = false;
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(110.0, 110.0));
    ws.selected = None;
    ws.also_selected.clear();
    ws.set_tool(Tool::Select);
}

fn path_len(ws: &Workspace) -> usize {
    match &drawings(ws)[0] {
        Content::Shape(s) => s.path().len(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn dragging_a_merged_edge_curves_it_with_a_preview_and_one_undo_step() {
    let mut ws = workspace();
    merged_square(&mut ws);
    let steps = ws.active().undo_labels().count();
    let before = path_len(&ws);
    ws.begin_drag(Pos2::new(60.0, 10.0));
    assert!(
        ws.selected.is_none(),
        "the edge was taken, not the shape picked"
    );
    ws.update_drag(Pos2::new(60.0, -30.0));
    let (_, shown) = ws.previewed_document().expect("a preview of the bend");
    assert_ne!(&*shown, ws.document());
    assert_eq!(path_len(&ws), before, "nothing committed yet");
    ws.end_drag();
    assert_eq!(ws.active().undo_labels().count(), steps + 1);
    let bent = drawings(&ws);
    assert!(
        matches!(&bent[0], Content::Shape(s) if s.path().iter().any(|p| matches!(p, flare_document::PathSegment::CubicTo(..))))
    );
    ws.undo();
    assert_eq!(path_len(&ws), before);
}

#[test]
fn a_cancelled_drag_a_drawing_object_and_a_miss_leave_the_drawing_alone() {
    let mut ws = workspace();
    merged_square(&mut ws);
    let before = ws.document().clone();
    ws.begin_drag(Pos2::new(60.0, 10.0));
    ws.update_drag(Pos2::new(60.0, -30.0));
    ws.cancel_drag();
    ws.end_drag();
    assert_eq!(ws.document(), &before, "cancelled");
    // Far from every edge: an ordinary press, which picks what is there.
    ws.begin_drag(Pos2::new(60.0, 60.0));
    ws.end_drag();
    assert_eq!(ws.document(), &before, "a miss bends nothing");

    let mut objects = workspace();
    rect(&mut objects); // a drawing object
    objects.set_tool(Tool::Select);
    let untouched = objects.document().clone();
    objects.begin_drag(Pos2::new(60.0, 10.0));
    objects.update_drag(Pos2::new(60.0, -30.0));
    objects.end_drag();
    assert_ne!(objects.document(), &untouched, "an object moves as a whole");
    assert_eq!(path_len(&objects), 5, "and keeps its outline");
}
