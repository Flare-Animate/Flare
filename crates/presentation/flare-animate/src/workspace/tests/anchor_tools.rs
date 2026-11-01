//! The Add, Delete and Convert Anchor Point tools on the stage.

use super::*;
use flare_document::PathSegment;

/// A workspace with the 10..110 x 10..60 rectangle selected and `tool` in hand.
fn with(tool: Tool) -> Workspace {
    let mut ws = workspace();
    rect(&mut ws);
    ws.handle_radius = 6.0;
    ws.set_tool(Tool::Select);
    ws.select_at(Pos2::new(50.0, 30.0), false);
    ws.set_tool(tool);
    ws
}

fn path(ws: &Workspace) -> Vec<PathSegment> {
    let scene = ws.document().scenes().first().expect("scene");
    scene
        .timeline()
        .layers()
        .iter()
        .flat_map(|layer| layer.spans())
        .find_map(|span| match span.content() {
            flare_document::Content::Shape(shape) => Some(shape.path().to_vec()),
            _ => None,
        })
        .expect("the shape is still there")
}

fn points(ws: &Workspace) -> usize {
    ws.anchor_points().len()
}

#[test]
fn the_add_tool_puts_a_point_on_the_outline_where_it_is_clicked_as_one_undo_step() {
    let mut ws = with(Tool::AddAnchor);
    assert_eq!(points(&ws), 4);
    // Away from every edge nothing is added.
    ws.anchor_click(Pos2::new(60.0, 30.0));
    assert_eq!(points(&ws), 4);
    // The top edge runs at y = 10.
    ws.anchor_click(Pos2::new(60.0, 11.0));
    assert_eq!(points(&ws), 5);
    ws.undo();
    assert_eq!(points(&ws), 4, "one undo step");
    ws.redo();
    assert_eq!(points(&ws), 5);
}

#[test]
fn the_delete_tool_takes_out_the_clicked_point_and_says_why_it_cannot_leave_too_few() {
    let mut ws = with(Tool::DeleteAnchor);
    ws.anchor_click(Pos2::new(110.0, 60.0));
    assert_eq!(points(&ws), 3, "the bottom-right corner is gone");
    ws.anchor_click(Pos2::new(110.0, 10.0));
    assert_eq!(points(&ws), 3, "a closed shape keeps three");
    assert!(ws.status().is_some_and(|s| s.contains("keeps at least")));
    ws.undo();
    assert_eq!(points(&ws), 4);
    // A click that is near no point deletes nothing.
    ws.anchor_click(Pos2::new(60.0, 30.0));
    assert_eq!(points(&ws), 4);
}

#[test]
fn the_convert_tool_makes_a_corner_smooth_by_a_drag_and_clicking_a_straight_corner_does_nothing() {
    let mut ws = with(Tool::ConvertAnchor);
    let revision = ws.active().revision();
    ws.anchor_click(Pos2::new(110.0, 10.0));
    assert_eq!(
        ws.active().revision(),
        revision,
        "a straight corner has no handles to pull in"
    );
    // Dragging from the top-right corner pulls a handle out of it.
    ws.begin_drag(Pos2::new(110.0, 10.0));
    assert!(ws.anchor_drag.is_some());
    ws.update_drag(Pos2::new(130.0, 10.0));
    ws.end_drag();
    let path = path(&ws);
    assert!(
        matches!(path[1], PathSegment::CubicTo(..)) && matches!(path[2], PathSegment::CubicTo(..)),
        "both edges at the corner are curves now: {path:?}"
    );
    // Clicking it now pulls the handles back in.
    ws.anchor_click(Pos2::new(110.0, 10.0));
    let path = self::path(&ws);
    let (PathSegment::CubicTo(_, c2, p), PathSegment::CubicTo(c1, _, _)) = (path[1], path[2])
    else {
        panic!("curves");
    };
    assert_eq!((c2, c1), (p, p));
    // Escape drops a drag.
    ws.begin_drag(Pos2::new(110.0, 60.0));
    ws.update_drag(Pos2::new(200.0, 200.0));
    ws.cancel_drag();
    ws.end_drag();
    assert!(matches!(self::path(&ws)[3], PathSegment::LineTo(_)));
}

#[test]
fn a_click_on_another_shape_selects_it_and_the_tools_are_in_the_strip_with_their_keys() {
    let mut ws = with(Tool::AddAnchor);
    ws.select_at(Pos2::new(900.0, 900.0), false);
    assert!(ws.selected.is_none());
    ws.anchor_click(Pos2::new(60.0, 30.0));
    assert!(
        ws.selected.is_some(),
        "the shape under the click is selected"
    );
    for (key, tool) in [
        (Key::Equals, Tool::AddAnchor),
        (Key::Minus, Tool::DeleteAnchor),
        (Key::C, Tool::ConvertAnchor),
    ] {
        assert!(TOOL_KEYS.contains(&(key, tool)));
    }
    for tool in [Tool::AddAnchor, Tool::DeleteAnchor, Tool::ConvertAnchor] {
        ws.set_tool(tool);
        assert_eq!(ws.tool, tool);
        assert!(!ws.tool_hint().is_empty());
    }
}
