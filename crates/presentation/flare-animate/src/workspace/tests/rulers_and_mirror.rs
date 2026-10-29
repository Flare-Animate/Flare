use super::*;
use flare_input::{InputSample, PointerKind};
use flare_tools::symmetry::Symmetry;

fn stroke(points: &[(f64, f64)]) -> Vec<InputSample> {
    points
        .iter()
        .map(|&(x, y)| InputSample::new(x, y, 1.0, PointerKind::Mouse).expect("sample"))
        .collect()
}

/// The x of the first point of the shape on layer `layer`.
fn first_x(ws: &Workspace, layer: usize) -> f64 {
    match drawn_shape(ws, layer).path().first() {
        Some(flare_document::PathSegment::MoveTo(p)) => p.as_vec().x,
        other => panic!("a path starts with a move, not {other:?}"),
    }
}

fn height_of(ws: &Workspace, layer: usize) -> f32 {
    let layers = ws.document().scenes()[0].timeline().layers();
    ops::span_bounds(
        ws.document(),
        ws.scene_id().expect("scene"),
        SpanRef {
            layer: layers[layer].id(),
            start: 0,
        },
    )
    .expect("bounds")
    .height()
}

fn wobble() -> Vec<InputSample> {
    stroke(&[
        (10.0, 50.0),
        (30.0, 80.0),
        (50.0, 20.0),
        (70.0, 90.0),
        (90.0, 50.0),
    ])
}

#[test]
fn a_left_right_symmetric_stroke_is_drawn_with_its_mirror_image_as_one_undo_step() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.paint.symmetry = Symmetry::Vertical;
    ws.paint.symmetry_center = Some(Pos2::new(100.0, 0.0));
    ws.freehand = stroke(&[(10.0, 10.0), (40.0, 30.0), (60.0, 10.0)]);
    ws.finish_freehand(1.0);
    assert_eq!(layer_count(&ws), 3);
    assert_eq!(first_x(&ws, 1), 10.0);
    assert_eq!(first_x(&ws, 2), 190.0);
    ws.undo();
    assert_eq!(layer_count(&ws), 1, "both strokes are one step");
}

#[test]
fn four_way_symmetry_draws_four_strokes_and_off_draws_one() {
    let mut ws = workspace();
    ws.set_tool(Tool::Brush);
    ws.freehand = stroke(&[(10.0, 10.0), (40.0, 30.0), (60.0, 10.0)]);
    ws.finish_freehand(1.0);
    assert_eq!(layer_count(&ws), 2);
    ws.paint.symmetry = Symmetry::Both;
    ws.freehand = stroke(&[(10.0, 10.0), (40.0, 30.0), (60.0, 10.0)]);
    ws.finish_freehand(1.0);
    assert_eq!(layer_count(&ws), 6);
}

#[test]
fn a_straight_edge_draws_a_line_between_the_ends_of_the_drag() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.paint.stroke_width = 4.0;
    ws.freehand = wobble();
    ws.finish_freehand(1.0);
    assert!(
        height_of(&ws, 1) > 20.0,
        "without the edge the wobble stays"
    );
    ws.paint.straight_edge = true;
    ws.freehand = wobble();
    ws.finish_freehand(1.0);
    assert_eq!(
        height_of(&ws, 2),
        4.0,
        "both ends are level, so the line is as tall as its stroke"
    );
}

#[test]
fn holding_shift_straightens_a_stroke_without_the_option() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.paint.stroke_width = 4.0;
    ws.paint.straight_held = true;
    ws.freehand = wobble();
    ws.finish_freehand(1.0);
    assert_eq!(height_of(&ws, 1), 4.0);
}

#[test]
fn a_straight_edge_and_symmetry_work_together() {
    let mut ws = workspace();
    ws.set_tool(Tool::Pencil);
    ws.paint.straight_edge = true;
    ws.paint.symmetry = Symmetry::Horizontal;
    ws.paint.symmetry_center = Some(Pos2::new(0.0, 50.0));
    ws.freehand = stroke(&[(10.0, 10.0), (30.0, 40.0), (60.0, 20.0)]);
    ws.finish_freehand(1.0);
    assert_eq!(layer_count(&ws), 3);
}

#[test]
fn a_mirror_copy_reflects_the_selection_across_the_axis_and_keeps_the_original() {
    let mut ws = two_rectangles();
    ws.paint.symmetry_center = Some(Pos2::new(200.0, 0.0));
    ws.perform(MenuAction::SelectAll);
    assert!(ws.is_enabled(MenuAction::MirrorCopyHorizontal));
    ws.perform(MenuAction::MirrorCopyHorizontal);
    assert_eq!(layer_count(&ws), 5);
    assert_eq!(rect_x(&ws, 1), 10.0, "the original stays");
    // The rectangle that spanned x 10..50 now has a copy spanning 350..390.
    let copies: Vec<f32> = (3..5).map(|i| rect_x(&ws, i)).collect();
    assert!(copies.contains(&350.0), "{copies:?}");
    assert_eq!(ws.selection_all().len(), 2, "the copies are selected");
    ws.undo();
    assert_eq!(layer_count(&ws), 3, "one undo step takes the copies away");
}

#[test]
fn a_mirror_copy_needs_a_selection() {
    let mut ws = two_rectangles();
    ws.perform(MenuAction::DeselectAll);
    assert!(!ws.is_enabled(MenuAction::MirrorCopyVertical));
    ws.mirror_copy_selection(false);
    assert_eq!(layer_count(&ws), 3);
}

#[test]
fn the_ruler_measures_the_drag_and_the_mirror_tool_moves_the_axes() {
    let mut ws = workspace();
    assert!(ws.measurement().is_none());
    ws.set_tool(Tool::Ruler);
    ws.assist_drag(Pos2::new(0.0, 0.0), Pos2::new(30.0, -40.0));
    let m = ws.measurement().expect("measured");
    assert_eq!(m.distance, 50.0);
    ws.assist_drag(Pos2::new(0.0, 0.0), Pos2::new(f32::NAN, 1.0));
    assert!(
        ws.measurement().is_none(),
        "a reading that is not a number is not shown"
    );
    ws.set_tool(Tool::Mirror);
    ws.assist_drag(Pos2::new(0.0, 0.0), Pos2::new(120.0, 80.0));
    assert_eq!(ws.symmetry_center(), DVec2::new(120.0, 80.0));
    ws.assist_drag(Pos2::new(0.0, 0.0), Pos2::new(f32::NAN, 80.0));
    assert_eq!(ws.symmetry_center(), DVec2::new(120.0, 80.0));
}

#[test]
fn until_set_the_axes_cross_in_the_middle_of_the_stage() {
    let ws = workspace();
    let stage = ws.document().stage();
    assert_eq!(
        ws.symmetry_center(),
        DVec2::new(
            f64::from(stage.width()) / 2.0,
            f64::from(stage.height()) / 2.0
        )
    );
}

#[test]
fn guides_pull_points_whether_or_not_the_grid_is_on() {
    use super::super::guides::{Guide, snap_for_drawing};
    let guides = [Guide {
        vertical: true,
        at: 100.0,
    }];
    let near = Pos2::new(103.0, 57.0);
    assert_eq!(
        snap_for_drawing(near, None, &guides, 5.0),
        Pos2::new(100.0, 57.0)
    );
    assert_eq!(
        snap_for_drawing(near, Some(20.0), &guides, 5.0),
        Pos2::new(100.0, 60.0)
    );
    assert_eq!(snap_for_drawing(near, None, &[], 5.0), near);
}

#[test]
fn the_new_tools_are_chosen_from_the_menu_and_the_keyboard() {
    let mut ws = workspace();
    ws.perform(MenuAction::ToolRuler);
    assert_eq!(ws.tool(), Tool::Ruler);
    ws.perform(MenuAction::ToolMirror);
    assert_eq!(ws.tool(), Tool::Mirror);
    assert!(TOOL_KEYS.contains(&(Key::M, Tool::Ruler)));
    assert!(TOOL_KEYS.contains(&(Key::X, Tool::Mirror)));
}
