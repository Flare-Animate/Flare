//! The Eraser's list of erasers, and the vector erasers in it, through the same pointer events
//! the stage gets: Segment takes a line out to where another crosses it, Whole Line takes what
//! the tip touches, Rectangle and Freehand what lies wholly inside the box or loop dragged.

use super::eraser::{button, drag_with_pointer, moves_of_first_drawing, press_and_move};
use super::*;
use crate::workspace::panels::tool_slots::SLOT_GROUPS;
use crate::workspace::tools::vector_eraser::EraserKind;
use flare_tools::eraser::vector::VectorEraserType;

/// The left and right ends of `content`'s points.
fn reach_x(content: &Content) -> (f32, f32) {
    let Content::Shape(shape) = content else {
        return (0.0, 0.0);
    };
    shape
        .path()
        .iter()
        .filter_map(|s| match s {
            flare_document::PathSegment::MoveTo(p) | flare_document::PathSegment::LineTo(p) => {
                Some(p.as_vec().x as f32)
            }
            flare_document::PathSegment::CubicTo(_, _, p) => Some(p.as_vec().x as f32),
            flare_document::PathSegment::Close => None,
        })
        .fold((f32::MAX, f32::MIN), |(lo, hi), x| (lo.min(x), hi.max(x)))
}

/// Two wide bars drawn with the Rectangle as a person would, one above the other, with the
/// Eraser then in hand as `kind`; gives the time the drawing ended at and the lower bar.
fn two_bars(ws: &mut Workspace, ctx: &egui::Context, kind: VectorEraserType) -> (f64, Content) {
    run(ws, ctx, 0.0);
    run(ws, ctx, 0.1);
    ws.set_tool(Tool::Rectangle);
    let time = drag_with_pointer(
        ws,
        ctx,
        0.2,
        Pos2::new(450.0, 330.0),
        Pos2::new(750.0, 370.0),
    );
    let time = drag_with_pointer(
        ws,
        ctx,
        time,
        Pos2::new(450.0, 450.0),
        Pos2::new(750.0, 490.0),
    );
    let bars = drawings(ws);
    assert_eq!(bars.len(), 2, "both bars were drawn");
    ws.set_tool(Tool::Eraser);
    ws.set_eraser_kind(EraserKind::Vector(kind));
    (time, bars[1].clone())
}

#[test]
fn the_vector_erasers_are_in_the_erasers_list_not_cells_of_their_own() {
    let mut ws = workspace();
    let cells: Vec<&str> = SLOT_GROUPS
        .iter()
        .flat_map(|group| group.iter())
        .flat_map(|slot| slot.iter())
        .map(|variant| variant.icon)
        .collect();
    assert!(
        !cells.contains(&"tool-vector-eraser"),
        "one Eraser in the strip"
    );
    assert_eq!(
        EraserKind::ALL.len(),
        5,
        "the Eraser and four vector erasers"
    );
    ws.set_tool(Tool::Eraser);
    assert_eq!(
        ws.vector_eraser(),
        None,
        "the Eraser rubs out to begin with"
    );
    for kind in VectorEraserType::ALL {
        ws.set_eraser_kind(EraserKind::Vector(kind));
        assert_eq!(ws.vector_eraser(), Some(kind));
        assert_eq!(ws.eraser_rubs(), kind.uses_tip(), "{kind:?}");
    }
    ws.set_tool(Tool::Brush);
    assert_eq!(ws.vector_eraser(), None, "only while the Eraser is in hand");
    ws.set_tool(Tool::Eraser);
    assert_eq!(
        ws.vector_eraser(),
        Some(VectorEraserType::Freehand),
        "the Eraser comes back as the eraser last picked"
    );
}

#[test]
fn segment_takes_a_brush_stroke_out_to_the_one_crossing_it_as_it_goes() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    run(&mut ws, &ctx, 0.0);
    run(&mut ws, &ctx, 0.1);
    ws.set_tool(Tool::Brush);
    let time = drag_with_pointer(
        &mut ws,
        &ctx,
        0.2,
        Pos2::new(450.0, 350.0),
        Pos2::new(750.0, 350.0),
    );
    let time = drag_with_pointer(
        &mut ws,
        &ctx,
        time,
        Pos2::new(600.0, 250.0),
        Pos2::new(600.0, 450.0),
    );
    let strokes = drawings(&ws);
    assert_eq!(strokes.len(), 2, "two Brush strokes, one across the other");
    let (across, down) = (reach_x(&strokes[0]), reach_x(&strokes[1]));
    ws.set_tool(Tool::Eraser);
    ws.set_eraser_kind(EraserKind::Vector(VectorEraserType::Segment));
    let before = ws.document().clone();
    let steps = ws.active().undo_labels().count();
    // Down through the stroke across, to the right of where the other crosses it.
    let (top, bottom) = (Pos2::new(700.0, 320.0), Pos2::new(702.0, 380.0));
    let time = press_and_move(&mut ws, &ctx, time, top, bottom);
    pass(
        &mut ws,
        &ctx,
        input(time + 0.02, vec![button(bottom, false)]),
    );
    let after = drawings(&ws);
    assert_eq!(after.len(), 2, "both strokes are still there");
    let left = reach_x(&after[0]);
    assert!(
        (left.0 - across.0).abs() < 0.5,
        "the stroke across keeps its start"
    );
    assert!(
        (left.1 - down.1).abs() < 1.0,
        "and now ends at the far side of the stroke down ({:?} against {down:?})",
        left
    );
    assert_eq!(after[1], strokes[1], "the stroke down is as it was");
    assert_eq!(
        ws.active().undo_labels().count(),
        steps + 1,
        "one undo step"
    );
    ws.undo();
    assert_eq!(ws.document(), &before, "one undo puts it back");
}

#[test]
fn whole_line_takes_the_shape_it_touches_whole_as_it_goes() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let (time, lower) = two_bars(&mut ws, &ctx, VectorEraserType::Whole);
    let (top, bottom) = (Pos2::new(600.0, 300.0), Pos2::new(602.0, 400.0));
    let time = press_and_move(&mut ws, &ctx, time, top, bottom);
    assert_eq!(
        drawings(&ws),
        vec![lower.clone()],
        "the upper bar went before the eraser was let go, not cut in two"
    );
    pass(
        &mut ws,
        &ctx,
        input(time + 0.02, vec![button(bottom, false)]),
    );
    assert_eq!(drawings(&ws), vec![lower]);
}

#[test]
fn rectangle_takes_only_what_lies_wholly_inside_the_box() {
    let mut ws = workspace();
    let ctx = egui::Context::default();
    let (time, lower) = two_bars(&mut ws, &ctx, VectorEraserType::Rectangular);
    // Round the whole upper bar, and down into the lower one without holding it.
    let time = press_and_move(
        &mut ws,
        &ctx,
        time,
        Pos2::new(420.0, 300.0),
        Pos2::new(780.0, 470.0),
    );
    assert_eq!(
        drawings(&ws).len(),
        2,
        "nothing goes until the box is let go"
    );
    pass(
        &mut ws,
        &ctx,
        input(time + 0.02, vec![button(Pos2::new(780.0, 470.0), false)]),
    );
    assert_eq!(
        drawings(&ws),
        vec![lower.clone()],
        "the lower bar stays whole"
    );
    assert_eq!(moves_of_first_drawing(&ws), 1);
    // A box that holds nothing whole takes nothing and makes no undo step.
    let steps = ws.active().undo_labels().count();
    drag_with_pointer(
        &mut ws,
        &ctx,
        time + 0.2,
        Pos2::new(600.0, 430.0),
        Pos2::new(800.0, 470.0),
    );
    assert_eq!(drawings(&ws), vec![lower]);
    assert_eq!(ws.active().undo_labels().count(), steps);
}

#[test]
fn freehand_takes_what_its_loop_holds_and_a_click_with_it_is_no_faucet() {
    let mut ws = workspace();
    ws.commit_rectangle(Pos2::new(10.0, 10.0), Pos2::new(110.0, 60.0));
    ws.commit_rectangle(Pos2::new(200.0, 10.0), Pos2::new(300.0, 60.0));
    ws.set_tool(Tool::Eraser);
    ws.eraser_faucet = true;
    ws.set_eraser_kind(EraserKind::Vector(VectorEraserType::Freehand));
    assert!(ws.vector_encloses() && !ws.eraser_rubs());
    ws.click_with_tool(Pos2::new(250.0, 35.0), false);
    assert_eq!(
        drawings(&ws).len(),
        2,
        "Faucet belongs to the Eraser that rubs out"
    );
    let at = |x: f64, y: f64| flare_input::InputSample::mouse(x, y).expect("sample");
    ws.freehand = vec![at(0.0, 0.0), at(120.0, 0.0), at(120.0, 70.0), at(0.0, 70.0)];
    ws.finish_freehand(1.0);
    assert_eq!(drawings(&ws).len(), 1, "the loop held the first rectangle");
    ws.set_eraser_kind(EraserKind::Vector(VectorEraserType::Whole));
    ws.click_with_tool(Pos2::new(250.0, 35.0), false);
    assert!(
        drawings(&ws).is_empty(),
        "a Whole Line click took the second"
    );
}

#[test]
fn a_vector_eraser_on_a_raster_layer_does_not_rub_out_pixels() {
    let mut ws = super::blend::empty_workspace();
    ws.perform(flare_menubar::MenuAction::NewRasterLayer);
    ws.set_tool(Tool::Eraser);
    assert_eq!(ws.blend_kind(), Some(flare_tools::blend::BlendKind::Erase));
    ws.set_eraser_kind(EraserKind::Vector(VectorEraserType::Segment));
    assert_eq!(
        ws.blend_kind(),
        None,
        "the picked vector eraser, not the pixel one"
    );
}
