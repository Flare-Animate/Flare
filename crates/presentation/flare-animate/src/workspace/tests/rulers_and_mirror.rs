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

mod stroke_preview {
    use super::*;
    use crate::workspace::stroke_preview::MAX_FREEHAND_SAMPLES;

    fn drag(ws: &mut Workspace) {
        ws.set_tool(Tool::Brush);
        ws.freehand = stroke(&[(10.0, 10.0), (40.0, 30.0), (80.0, 10.0)]);
        ws.finish_freehand(1.0);
    }

    #[test]
    fn samples_closer_than_half_a_pixel_are_dropped_and_a_stroke_has_a_limit() {
        let mut ws = workspace();
        let sample = |x: f64| InputSample::new(x, 0.0, 1.0, PointerKind::Mouse).expect("sample");
        ws.push_freehand(sample(0.0), 1.0);
        ws.push_freehand(sample(0.2), 1.0);
        assert_eq!(ws.freehand.len(), 1, "a sub-pixel move adds nothing");
        ws.push_freehand(sample(0.2), 4.0);
        assert_eq!(
            ws.freehand.len(),
            2,
            "zoomed in, the same move is a visible step"
        );
        for i in 0..MAX_FREEHAND_SAMPLES + 50 {
            ws.push_freehand(sample(10.0 + i as f64), 1.0);
        }
        assert_eq!(ws.freehand.len(), MAX_FREEHAND_SAMPLES);
    }

    #[test]
    fn a_committed_stroke_stays_on_screen_until_the_stage_picture_has_it() {
        let mut ws = workspace();
        let ctx = egui::Context::default();
        run(&mut ws, &ctx, 0.0);
        drag(&mut ws);
        assert!(
            ws.paint.settling.is_some(),
            "the stroke is held after release"
        );
        // The picture on the stage is of the document before the stroke.
        ws.retire_settled();
        assert!(ws.paint.settling.is_some());
        // Once the picture of the new document state is up, the held stroke goes.
        ws.ensure_stage_texture(&ctx);
        ws.retire_settled();
        assert!(ws.paint.settling.is_none());
    }

    #[test]
    fn undo_or_another_edit_drops_a_held_stroke_that_is_no_longer_true() {
        let mut ws = workspace();
        drag(&mut ws);
        assert!(ws.paint.settling.is_some());
        ws.undo();
        ws.retire_settled();
        assert!(ws.paint.settling.is_none());
    }

    #[test]
    fn nothing_is_held_for_a_click_or_a_failed_commit() {
        let mut ws = workspace();
        ws.set_tool(Tool::Brush);
        ws.freehand = stroke(&[(5.0, 5.0)]);
        ws.finish_freehand(1.0);
        assert!(ws.paint.settling.is_none());
    }

    #[test]
    fn the_preview_is_as_wide_as_the_committed_stroke_and_follows_pressure() {
        let mut ws = workspace();
        ws.set_tool(Tool::Brush);
        ws.brush_size = 20.0;
        ws.brush_light = 0.0;
        let light = InputSample::new(0.0, 0.0, 0.0, PointerKind::Pen).expect("sample");
        let full = InputSample::new(30.0, 0.0, 1.0, PointerKind::Pen).expect("sample");
        ws.freehand = vec![light, full];
        let preview = ws.freehand_preview();
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0].halves, [0.0, 10.0]);
        ws.set_tool(Tool::Pencil);
        ws.freehand = vec![light, full];
        ws.paint.stroke_width = 6.0;
        assert_eq!(ws.freehand_preview()[0].halves, [3.0, 3.0]);
    }

    #[test]
    fn the_preview_has_the_mirror_images_the_commit_will_have() {
        let mut ws = workspace();
        ws.set_tool(Tool::Pencil);
        ws.paint.symmetry = Symmetry::Both;
        ws.freehand = stroke(&[(1.0, 1.0), (5.0, 9.0)]);
        assert_eq!(ws.freehand_preview().len(), 4);
    }
}
