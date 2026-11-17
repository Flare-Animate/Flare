//! The Magnet in the workspace: a drag pulls the lines near where it started, the stage shows
//! the pull while it is held, the release is one undo step, and a pull that reaches nothing says
//! so and changes nothing.

use super::*;

fn top_of(content: &Content) -> f32 {
    match content {
        Content::Shape(shape) => ops::shape_bounds(shape).expect("bounds").min.y,
        other => panic!("a shape expected, got {other:?}"),
    }
}

/// A drag with the tool in hand from `from` to `to`, as the stage gives it.
fn pull(ws: &mut Workspace, from: Pos2, to: Pos2) {
    ws.begin_drag(from);
    ws.update_drag(to);
    ws.end_drag();
}

#[test]
fn a_drag_bends_the_line_near_it_as_one_undo_step() {
    let mut ws = workspace();
    rect(&mut ws); // from (10, 10) to (110, 60)
    let steps = ws.active().undo_labels().count();
    ws.set_tool(Tool::Magnet);
    ws.springs.magnet.radius = 30.0;
    ws.begin_drag(Pos2::new(60.0, 10.0));
    ws.update_drag(Pos2::new(60.0, -30.0));
    // While it is held the stage is drawn from the pulled document; the document is not changed.
    let (_, shown) = ws.previewed_document().expect("a preview");
    let shown_top = shown.scenes()[0].timeline().layers()[0]
        .span_at(0)
        .and_then(|s| s.elements().next().cloned())
        .map(|c| top_of(&c))
        .expect("shown");
    assert!(shown_top < -20.0, "the preview shows the pull: {shown_top}");
    assert_eq!(top_of(&drawings(&ws)[0]), 10.0, "nothing committed yet");
    ws.end_drag();
    let top = top_of(&drawings(&ws)[0]);
    assert!(
        (top - shown_top).abs() < 1e-3,
        "what is let go is what was shown"
    );
    assert_eq!(ws.active().undo_labels().count(), steps + 1);
    ws.undo();
    assert_eq!(top_of(&drawings(&ws)[0]), 10.0);
}

#[test]
fn a_pull_reaching_nothing_says_so_and_a_cancelled_one_does_nothing() {
    let mut ws = workspace();
    rect(&mut ws);
    let before = ws.document().clone();
    ws.set_tool(Tool::Magnet);
    ws.springs.magnet.radius = 10.0;
    pull(&mut ws, Pos2::new(400.0, 400.0), Pos2::new(380.0, 380.0));
    assert_eq!(ws.document(), &before);
    assert_eq!(
        ws.status.as_deref(),
        Some(ws.tr("tools-magnet-nothing").as_str())
    );
    ws.begin_drag(Pos2::new(60.0, 10.0));
    ws.update_drag(Pos2::new(60.0, -30.0));
    ws.cancel_drag();
    ws.end_drag();
    assert_eq!(ws.document(), &before, "Escape drops the pull");
}

#[test]
fn with_a_shape_selected_only_it_is_pulled() {
    let mut ws = workspace();
    rect(&mut ws);
    ws.commit_rectangle(Pos2::new(10.0, 0.0), Pos2::new(110.0, 20.0));
    // Both tops are near (60, 5); select the second alone.
    let second = ws.hit(Pos2::new(60.0, 15.0)).expect("the second");
    ws.selected = Some(second);
    ws.set_tool(Tool::Magnet);
    ws.springs.magnet.radius = 30.0;
    pull(&mut ws, Pos2::new(60.0, 5.0), Pos2::new(60.0, -30.0));
    let all = drawings(&ws);
    assert_eq!(top_of(&all[0]), 10.0, "the first was not selected");
    assert!(top_of(&all[1]) < -10.0, "the selected one bent");
}
