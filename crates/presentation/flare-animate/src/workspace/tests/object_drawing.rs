//! The Object Drawing switch, and Merge Drawing when it is off.

use super::*;

fn square(ws: &mut Workspace, x: f32) {
    ws.commit_rectangle(Pos2::new(x, 10.0), Pos2::new(x + 100.0, 110.0));
}

fn merged(ws: &Workspace) -> Vec<bool> {
    drawings(ws)
        .iter()
        .map(|d| matches!(d, Content::Shape(s) if s.is_merged()))
        .collect()
}

#[test]
fn object_drawing_is_on_until_it_is_switched_off_and_then_shapes_merge() {
    let mut ws = workspace();
    square(&mut ws, 10.0);
    square(&mut ws, 60.0);
    assert_eq!(merged(&ws), [false, false], "two drawing objects");

    let mut ws = workspace();
    ws.set_tool(Tool::Rectangle);
    ws.perform(MenuAction::ToggleObjectDrawing);
    assert!(!ws.paint.object_drawing);
    square(&mut ws, 10.0);
    let first = drawings(&ws);
    square(&mut ws, 60.0);
    assert_eq!(merged(&ws), [true], "the same colour joined into one fill");
    assert_ne!(drawings(&ws), first, "the fill grew");
    ws.undo();
    assert_eq!(drawings(&ws), first, "one undo step takes the join back");
}

#[test]
fn a_merged_shape_with_an_outline_is_a_fill_and_a_line() {
    let mut ws = workspace();
    ws.paint.object_drawing = false;
    ws.paint.stroke_enabled = true;
    square(&mut ws, 10.0);
    let pieces = drawings(&ws);
    assert_eq!(pieces.len(), 2);
    let Content::Shape(fill) = &pieces[0] else {
        panic!("a shape")
    };
    let Content::Shape(line) = &pieces[1] else {
        panic!("a shape")
    };
    assert!(fill.stroke().is_none() && !fill.fill_is_invisible());
    assert!(line.stroke().is_some() && line.fill_is_invisible());
}

#[test]
fn j_switches_it_only_on_the_tools_that_draw_shapes() {
    let (ctx, mut ws) = (egui::Context::default(), workspace());
    run(&mut ws, &ctx, 0.0);
    ws.set_tool(Tool::Select);
    assert!(!ws.is_enabled(MenuAction::ToggleObjectDrawing));
    pass(
        &mut ws,
        &ctx,
        input(0.1, vec![key(Key::J, Modifiers::NONE)]),
    );
    assert!(ws.paint.object_drawing, "nothing to switch with Select");
    ws.set_tool(Tool::Brush);
    pass(
        &mut ws,
        &ctx,
        input(0.2, vec![key(Key::J, Modifiers::NONE)]),
    );
    assert!(!ws.paint.object_drawing, "J turned it off");
    assert_eq!(ws.tool(), Tool::Brush, "and picked no tool");
    pass(
        &mut ws,
        &ctx,
        input(0.3, vec![key(Key::J, Modifiers::NONE)]),
    );
    assert!(ws.paint.object_drawing, "and on again");
}

#[test]
fn text_is_a_drawing_object_whatever_the_switch_says() {
    let mut ws = workspace();
    ws.paint.object_drawing = false;
    let source = flare_document::TextSource::new(
        "Hi",
        "sans-serif",
        12.0,
        false,
        false,
        flare_core::DocumentPoint::new(0.0, 0.0).expect("point"),
    )
    .expect("text");
    let text = Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, Rgba8::opaque(0, 0, 0))
        .expect("rect")
        .with_text(source);
    let plain =
        Shape::rect(ShapeId::new(), 0.0, 0.0, 10.0, 10.0, Rgba8::opaque(0, 0, 0)).expect("rect");
    let drawn = ws.in_drawing_mode(vec![text, plain]);
    assert!(!drawn[0].is_merged());
    assert!(drawn[1].is_merged());
}

#[test]
fn the_switch_is_kept_with_the_preferences_and_older_ones_read_as_on() {
    let mut ws = workspace();
    ws.paint.object_drawing = false;
    let saved = ws.preferences_json().expect("json");
    let mut fresh = workspace();
    fresh.restore_preferences(&saved).expect("restored");
    assert!(!fresh.paint.object_drawing);
    fresh.restore_preferences("{}").expect("an old file");
    assert!(fresh.paint.object_drawing);
}

#[test]
fn the_switch_carries_to_a_new_tab() {
    let mut ws = workspace();
    ws.paint.object_drawing = false;
    ws.new_tab();
    assert!(!ws.paint.object_drawing);
}
